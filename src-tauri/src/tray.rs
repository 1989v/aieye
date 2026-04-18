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
                position,
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
                                    let scale = win.scale_factor().unwrap_or(1.0);
                                    let x = position.x / scale - 180.0;
                                    let y = position.y / scale + 6.0;
                                    let _ = win.set_position(LogicalPosition::new(x, y));
                                }
                                // 매 show 마다 level/collection 재적용
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
