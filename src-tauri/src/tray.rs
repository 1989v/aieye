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
                                    // tray icon 의 rect → 메뉴바 하단 경계 기준으로 panel 위치
                                    let (tx, ty, tw, th) = extract_rect(&rect);
                                    let center_x = (tx + tw / 2.0) / scale;
                                    let bottom_y = (ty + th) / scale;
                                    const PANEL_W: f64 = 360.0;
                                    const GAP: f64 = 4.0;
                                    let x = center_x - PANEL_W / 2.0;
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
