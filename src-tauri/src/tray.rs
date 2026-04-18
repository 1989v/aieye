use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, LogicalPosition, Manager,
};
use tracing::info;

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

    let _tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
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
