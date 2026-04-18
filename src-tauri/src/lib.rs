mod commands;
mod parser;
mod sessions;
mod tray;

#[cfg(target_os = "macos")]
mod macos_panel;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aieye=info".into()),
        )
        .init();

    let builder = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::list_sessions]);

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .setup(|app| {
            tray::build_tray(app)?;

            // macOS: panel window 를 NSPanel 로 변환해 full-screen 앱 위에도 뜨게.
            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                use tauri_nspanel::WebviewWindowExt;
                if let Some(win) = app.get_webview_window("panel") {
                    match win.to_panel() {
                        Ok(_panel) => {
                            tracing::info!("panel window converted to NSPanel");
                        }
                        Err(e) => {
                            tracing::error!("to_panel failed: {e:?}");
                        }
                    }
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
