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
                use tauri_nspanel::{panel_delegate, WebviewWindowExt};
                if let Some(win) = app.get_webview_window("panel") {
                    // 네이티브 popover 스타일: 블러 vibrancy + 둥근 모서리 10pt
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                    if let Err(e) = apply_vibrancy(
                        &win,
                        NSVisualEffectMaterial::HudWindow,
                        Some(NSVisualEffectState::Active),
                        Some(10.0),
                    ) {
                        tracing::warn!("apply_vibrancy failed: {e:?}");
                    }

                    match win.to_panel() {
                        Ok(panel) => {
                            tracing::info!("panel window converted to NSPanel");

                            panel.set_style_mask(1 << 7); // NSNonactivatingPanel
                            panel.set_level(25);          // statusBar level
                            panel.set_collection_behaviour(
                                NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary,
                            );

                            // 외부 클릭 시 자동 닫힘 (포커스 잃으면 order_out)
                            let panel_for_delegate = panel.clone();
                            let delegate = panel_delegate!(AieyePanelDelegate {
                                window_did_resign_key
                            });
                            delegate.set_listener(Box::new(move |name: String| {
                                if name == "window_did_resign_key" {
                                    panel_for_delegate.order_out(None);
                                }
                            }));
                            panel.set_delegate(delegate);

                            tracing::info!("panel: style/level/behaviour/delegate set");
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
