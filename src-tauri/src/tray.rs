use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, LogicalPosition, Manager,
};
use tracing::info;

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
            info!("tray event: {:?}", event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("panel") {
                    let is_vis = win.is_visible().unwrap_or(false);
                    info!("panel visible={}, tray pos={:?}", is_vis, position);
                    if is_vis {
                        let _ = win.hide();
                    } else {
                        // full-screen 앱 위에도 올라오도록 모든 Space + fullScreenAuxiliary
                        let _ = win.set_visible_on_all_workspaces(true);
                        let _ = win.set_always_on_top(true);

                        // 트레이 아이콘 아래쪽으로 패널 위치
                        let scale = win.scale_factor().unwrap_or(1.0);
                        let x = position.x / scale - 180.0; // 360 wide / 2
                        let y = position.y / scale + 6.0;
                        let _ = win.set_position(LogicalPosition::new(x, y));
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                } else {
                    info!("panel window NOT FOUND");
                }
            }
        })
        .build(app)?;

    Ok(())
}
