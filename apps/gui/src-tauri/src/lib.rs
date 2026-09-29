//! The DayZ Community Hub shell: plugins, windows and the commands the window
//! calls. The work itself lives in `features` and in the `dz-*` crates.

mod error;
mod events;
mod features;
mod ipc;
mod net;
mod state;

pub use features::dzch_cli::CliArgs;

use clap::Parser;
use std::sync::Arc;
use tauri::Manager;
use tauri_specta::Event;

use features::{news, ping, updater};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(args: CliArgs) {
    args.remember();

    let builder = ipc::builder();
    let invoke_handler = builder.invoke_handler();
    // In a debug build, keep the window's bindings in step with the Rust. A
    // debug binary run away from the sources cannot write them: say so, run.
    #[cfg(debug_assertions)]
    if let Err(e) = ipc::export(&builder) {
        eprintln!("bindings not refreshed: {e}");
    }

    tauri::Builder::default()
        .setup(move |app| {
            builder.mount_events(app);
            features::browser::spawn_change_notifier(app.handle().clone());
            features::gamepad::spawn(app.handle().clone());
            // The one-shot slot the news WebView fallback returns its JSON through.
            app.manage(news::webview::NewsWebviewState::new());
            app.manage(Arc::new(ping::PingState::default()));

            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            app.manage(updater::Pending::default());
            #[cfg(windows)]
            app.handle().plugin(tauri_plugin_process::init())?;

            // Allow the image cache in the asset protocol scope, resolved by
            // Tauri's own path resolver (the same one the protocol checks).
            if let Ok(data_dir) = app.path().app_data_dir() {
                let images_dir = data_dir.join("cache").join("images");
                let _ = app
                    .asset_protocol_scope()
                    .allow_directory(&images_dir, false);
            }

            // Register dzch:// at runtime on Linux (and Windows debug builds),
            // so it works outside an installed package.
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }

            // A dzch:// link follows the same path as --open.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    if let Some(url) = event.urls().first() {
                        let args = CliArgs {
                            open: Some(url.to_string()),
                            ..CliArgs::none()
                        };
                        let _ = args.emit(&handle);
                    }
                });
            }

            Ok(())
        })
        // Single instance: a second launch hands its arguments to this one
        // (as a "cli-args" event) and exits.
        .plugin(
            tauri_plugin_single_instance::Builder::new()
                .callback(|app, argv, _cwd| {
                    let args = CliArgs::try_parse_from(&argv).unwrap_or_else(|_| CliArgs::none());
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                    let _ = args.emit(app);
                })
                .build(),
        )
        // The pads speak only to a focused window.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(f) = event
                && window.label() == "main"
            {
                features::gamepad::set_focused(*f);
            }
        })
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(invoke_handler)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
