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

            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
                use tauri_nspanel::WebviewWindowExt;
                if let Some(win) = app.get_webview_window("panel") {
                    match win.to_panel() {
                        Ok(panel) => {
                            tracing::info!("panel window converted to NSPanel");

                            // NSWindowStyleMaskNonactivatingPanel (1 << 7)
                            panel.set_style_mask(1 << 7);
                            // statusBar level — full-screen 앱 위
                            panel.set_level(25);
                            // canJoinAllSpaces | fullScreenAuxiliary | stationary
                            panel.set_collection_behaviour(
                                NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary,
                            );

                            tracing::info!("panel: style/level/collectionBehaviour set");
                        }
                        Err(e) => {
                            tracing::error!("to_panel failed: {e:?}");
                        }
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
