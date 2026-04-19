mod commands;
mod parser;
mod resume;
mod sessions;
mod settings;
mod tray;
mod tray_icons;
mod tray_state;

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
        .invoke_handler(tauri::generate_handler![
            commands::list_sessions,
            commands::resume_session,
            commands::resume_session_force_new,
            commands::reveal_in_finder,
            commands::list_installed_terminals,
            commands::get_settings,
            commands::set_settings,
            commands::acknowledge_finished,
            commands::get_session_preview,
            commands::archive_session_file
        ]);

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .manage(tray_state::SharedTrayState::new())
        .manage(std::sync::Arc::new(tray_icons::generate_all()))
        .setup(|app| {
            tray::build_tray(app)?;
            tray::spawn_poll_task(app.handle().clone());
            tray::spawn_animation_task(app.handle().clone());

            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
                use tauri_nspanel::{panel_delegate, WebviewWindowExt};
                if let Some(win) = app.get_webview_window("panel") {
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

                            // NSWindow 를 완전 투명하게 (webview 가 올라가기 전에)
                            if let Ok(ns) = win.ns_window() {
                                macos_panel::make_window_transparent(ns);
                            }

                            // to_panel() 이후에 vibrancy 적용 — NSPanel 전환으로
                            // contentView 가 교체된 뒤에 NSVisualEffectView 삽입.
                            use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                            if let Err(e) = apply_vibrancy(
                                &win,
                                NSVisualEffectMaterial::HudWindow,
                                Some(NSVisualEffectState::Active),
                                Some(10.0),
                            ) {
                                tracing::warn!("apply_vibrancy failed: {e:?}");
                            } else {
                                tracing::info!("vibrancy applied: HudWindow");
                            }

                            // WKWebView 자체를 투명 (기본 불투명 흰 배경 제거 → vibrancy 노출)
                            #[cfg(target_os = "macos")]
                            {
                                use objc2::msg_send;
                                use objc2::runtime::AnyObject;
                                use objc2_foundation::NSString;
                                let _ = win.with_webview(|webview| unsafe {
                                    let wkwebview = webview.inner() as *mut AnyObject;
                                    let key = NSString::from_str("drawsBackground");
                                    let value: *mut AnyObject = std::ptr::null_mut();
                                    // value = [NSNumber numberWithBool:NO] equivalent via kCFBooleanFalse
                                    // simpler: use NSControlStateValueOff / pass nil for NO effectively?
                                    // 가장 호환성 좋은 방법: setValue:@(NO) forKey:
                                    let ns_number_class = objc2::runtime::AnyClass::get(c"NSNumber").unwrap();
                                    let no_number: *mut AnyObject = msg_send![ns_number_class, numberWithBool: false];
                                    let _ = value; // unused
                                    let _: () = msg_send![wkwebview, setValue: no_number, forKey: &*key];
                                    tracing::info!("WKWebView drawsBackground=NO set");
                                });
                            }

                            // 외부 클릭 시 자동 닫힘 (포커스 잃으면 order_out)
                            let panel_for_delegate = panel.clone();
                            let delegate = panel_delegate!(AieyePanelDelegate {
                                window_did_resign_key
                            });
                            delegate.set_listener(Box::new(move |name: String| {
                                tracing::info!("panel delegate event: {}", name);
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
