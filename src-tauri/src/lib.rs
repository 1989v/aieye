mod commands;
mod macos_panel;
mod parser;
mod sessions;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aieye=info".into()),
        )
        .init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::list_sessions])
        .setup(|app| {
            tray::build_tray(app)?;
            // panel window 를 NSWindow popup 레벨로 승격 → full-screen 앱 위에도 출현
            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                if let Some(win) = app.get_webview_window("panel") {
                    if let Ok(ns) = win.ns_window() {
                        macos_panel::elevate_to_panel(ns);
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
