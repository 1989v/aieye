use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, LogicalPosition, Manager,
};
use tracing::info;

use crate::sessions::SessionCoordinator;
use crate::tray_icons::TrayIcons;
use crate::tray_state::{is_mtime_fresh, SessionObservation, SharedTrayState, TraySummary};

/// 현재 애니메이션 프레임 인덱스 (generating 중일 때만 의미). 폴링과 애니메이션
/// 스레드 둘 다 읽음.
static FRAME_IDX: AtomicU32 = AtomicU32::new(0);
/// 마지막 poll 결과를 캐싱해 애니메이션 tick 이 같은 상태를 유지할 수 있게 함.
/// (generating_count, finished_count) 만 필요.
static LAST_GEN_COUNT: AtomicU32 = AtomicU32::new(0);
static LAST_FINISHED_COUNT: AtomicU32 = AtomicU32::new(0);

#[cfg(target_os = "macos")]
use crate::macos_panel;

#[cfg(target_os = "macos")]
use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
#[cfg(target_os = "macos")]
use tauri_nspanel::ManagerExt;

/// Tauri 의 Rect (position: Position enum, size: Size enum) 에서
/// physical pixel 좌표 4개를 추출.
fn extract_rect(rect: &tauri::Rect) -> (f64, f64, f64, f64) {
    use tauri::{Position, Size};
    let (x, y) = match &rect.position {
        Position::Physical(p) => (p.x as f64, p.y as f64),
        Position::Logical(p) => (p.x, p.y),
    };
    let (w, h) = match &rect.size {
        Size::Physical(s) => (s.width as f64, s.height as f64),
        Size::Logical(s) => (s.width, s.height),
    };
    (x, y, w, h)
}

pub fn build_tray(app: &App) -> tauri::Result<()> {
    let quit_item = MenuItem::with_id(app, "quit", "Quit aieye", true, Some("cmd+q"))?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    let icons = app.state::<Arc<TrayIcons>>();
    let idle_icon = tauri::image::Image::from_bytes(&icons.idle)?;
    let _tray = TrayIconBuilder::with_id("main")
        .icon(idle_icon)
        .icon_as_template(true)
        .menu(&menu)
        .menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                // 트레이 클릭 = 모든 finished 확인 처리
                if let Some(state) = app.try_state::<SharedTrayState>() {
                    if let Ok(mut s) = state.0.lock() {
                        s.acknowledge_all();
                    }
                }
                apply_tray_visual(app, 0, 0, &FRAME_IDX);

                #[cfg(target_os = "macos")]
                {
                    match app.get_webview_panel("panel") {
                        Ok(panel) => {
                            if panel.is_visible() {
                                panel.order_out(None);
                            } else {
                                if let Some(win) = app.get_webview_window("panel") {
                                    let scale = win.scale_factor().unwrap_or(2.0);
                                    let (tx, ty, tw, th) = extract_rect(&rect);
                                    let left_edge = tx / scale;
                                    let right_edge = (tx + tw) / scale;
                                    let bottom_y = (ty + th) / scale;
                                    const PANEL_W: f64 = 360.0;
                                    // macOS popover 는 메뉴바와 거의 붙음 (SwiftUI MenuBarExtra 관례)
                                    const GAP: f64 = 0.0;
                                    const EDGE_MARGIN: f64 = 8.0;

                                    // 기본: 아이콘 좌측 끝 기준으로 우측 하단 펼침
                                    // 화면 우측 경계를 넘으면 우측 끝 기준으로 좌측 하단 펼침
                                    let (screen_left, screen_right) = match win.current_monitor() {
                                        Ok(Some(m)) => {
                                            let pos = m.position();
                                            let size = m.size();
                                            (
                                                pos.x as f64 / scale,
                                                (pos.x as f64 + size.width as f64) / scale,
                                            )
                                        }
                                        _ => (0.0, f64::MAX),
                                    };

                                    let right_aligned = left_edge + PANEL_W > screen_right - EDGE_MARGIN;
                                    let x = if right_aligned {
                                        // 우측 끝 기준 좌측으로 펼침
                                        (right_edge - PANEL_W).max(screen_left + EDGE_MARGIN)
                                    } else {
                                        left_edge
                                    };

                                    let y = bottom_y + GAP;
                                    let _ = win.set_position(LogicalPosition::new(x, y));
                                }
                                panel.set_level(25);
                                panel.set_collection_behaviour(
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                                        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                                        | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary,
                                );
                                panel.show();
                                info!("panel.show() called");
                            }
                        }
                        Err(e) => {
                            info!("get_webview_panel failed: {:?}, falling back to window.show", e);
                            if let Some(win) = app.get_webview_window("panel") {
                                if win.is_visible().unwrap_or(false) {
                                    let _ = win.hide();
                                } else {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                if let Some(win) = app.get_webview_window("panel") {
                    if win.is_visible().unwrap_or(false) {
                        let _ = win.hide();
                    } else {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// 주기적으로 세션 스캔 → TrayState 갱신 → 트레이 아이콘/title 반영.
pub fn spawn_poll_task(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        interval.tick().await; // 즉시 1회
        loop {
            poll_once(&app).await;
            interval.tick().await;
        }
    });
}

async fn poll_once(app: &AppHandle) {
    use crate::parser::{claude_activity, codex_activity};
    use crate::resume::{match_running, snapshot_running};
    use crate::sessions::CliKind;

    let coord = SessionCoordinator::with_defaults();
    let sessions = coord.scan_all().await;
    let claude_snap = snapshot_running("claude");
    let codex_snap = snapshot_running("codex");

    let mut obs: Vec<SessionObservation> = Vec::new();
    let mut tagged_pids: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for s in &sessions {
        let Some(cwd) = s.project_path.as_deref() else { continue };
        let snap = match s.cli {
            CliKind::Claude => &claude_snap,
            CliKind::Codex => &codex_snap,
        };
        let Some(r) = match_running(snap, cwd, &s.id) else { continue };
        if tagged_pids.contains(&r.pid) {
            continue;
        }
        tagged_pids.insert(r.pid);
        let activity = match s.cli {
            CliKind::Claude => claude_activity(&s.jsonl_path),
            CliKind::Codex => codex_activity(&s.jsonl_path),
        };
        obs.push(SessionObservation {
            id: s.id.clone(),
            activity: Some(activity),
            mtime_fresh: is_mtime_fresh(&s.jsonl_path),
        });
    }

    let summary = {
        let Some(state) = app.try_state::<SharedTrayState>() else { return };
        let Ok(mut s) = state.0.lock() else { return };
        s.update(&obs)
    };

    LAST_GEN_COUNT.store(summary.generating_count as u32, Ordering::Relaxed);
    LAST_FINISHED_COUNT.store(summary.finished_count as u32, Ordering::Relaxed);

    apply_tray_visual(
        app,
        summary.generating_count as u32,
        summary.finished_count as u32,
        &FRAME_IDX,
    );

    tracing::info!(
        "poll: gen={} fin={} gen_ids={:?} fin_ids={:?}",
        summary.generating_count,
        summary.finished_count,
        summary.generating_ids,
        summary.finished_ids
    );
}

/// generating 세션이 있을 때만 200ms 마다 프레임 교체.
pub fn spawn_animation_task(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            interval.tick().await;
            let gen = LAST_GEN_COUNT.load(Ordering::Relaxed);
            if gen == 0 {
                continue;
            }
            FRAME_IDX.fetch_add(1, Ordering::Relaxed);
            let finished = LAST_FINISHED_COUNT.load(Ordering::Relaxed);
            apply_tray_visual(&app, gen, finished, &FRAME_IDX);
        }
    });
}

fn apply_tray_visual(
    app: &AppHandle,
    generating: u32,
    finished: u32,
    frame_idx: &AtomicU32,
) {
    let Some(icons) = app.try_state::<Arc<TrayIcons>>() else { return };

    let frame = (frame_idx.load(Ordering::Relaxed) as usize) % icons.generating.len();
    let (icon_bytes, title) = if generating > 0 {
        (icons.generating[frame].clone(), format!(" {}", generating))
    } else if finished > 0 {
        (icons.finished.clone(), format!(" {}", finished))
    } else {
        (icons.idle.clone(), String::new())
    };

    // macOS tray UI 는 main thread 에서만 실제 반영됨. background tokio 태스크
    // 에서 직접 set_icon 호출하면 API 는 Ok 반환해도 시각적으로 변화 없음.
    let app_main = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(tray) = app_main.tray_by_id("main") else { return };
        match tauri::image::Image::from_bytes(&icon_bytes) {
            Ok(img) => {
                if let Err(e) = tray.set_icon(Some(img)) {
                    tracing::error!("set_icon failed: {e:?}");
                }
                if let Err(e) = tray.set_icon_as_template(true) {
                    tracing::error!("set_icon_as_template failed: {e:?}");
                }
            }
            Err(e) => tracing::error!("Image::from_bytes failed: {e:?}"),
        }
        if let Err(e) = tray.set_title(Some(&title)) {
            tracing::error!("set_title failed: {e:?}");
        }
    });
    tracing::debug!(
        "tray visual request: gen={} fin={} frame={}",
        generating,
        finished,
        frame
    );
}

#[allow(dead_code)]
fn tray_handle(app: &AppHandle) -> Option<TrayIcon> {
    app.tray_by_id("main")
}
