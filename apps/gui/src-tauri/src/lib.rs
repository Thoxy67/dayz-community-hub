//! The DayZ Community Hub shell: plugins, windows and the commands the window
//! calls. The work itself lives in `features` and in the `dz-*` crates.

mod error;
mod features;
mod state;

pub use features::dzch_cli::CliArgs;

use clap::Parser;
use std::sync::Arc;
use tauri::{Emitter, Manager};

use features::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(args: CliArgs) {
    args.remember();

    tauri::Builder::default()
        .setup(|app| {
            // The one-shot slot the news WebView fallback returns its JSON through.
            app.manage(news::webview::NewsWebviewState::new());
            app.manage(Arc::new(ping::PingState::default()));

            #[cfg(windows)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
                app.manage(updater::PendingUpdate(std::sync::Mutex::new(None)));
            }

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
                        let _ = handle.emit("cli-args", args);
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
                    let _ = app.emit("cli-args", args);
                })
                .build(),
        )
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            servers::check_first_launch,
            servers::initialize,
            servers::get_servers,
            servers::get_server_details,
            servers::refresh_servers,
            servers::get_app_stats,
            ping::ping_all_background,
            ping::ping_servers,
            ping::get_pings,
            ping::ping_single,
            ping::cancel_ping,
            ping::toggle_ping_pause,
            a2s::query_a2s,
            battlemetrics::fetch_battlemetrics_server,
            profile::get_profile,
            profile::save_profile_settings,
            profile::add_favorite,
            profile::remove_favorite,
            profile::remove_history_entry,
            profile::clear_history,
            profile::add_excluded_ip,
            profile::remove_excluded_ip,
            profile::io::export_profile,
            profile::io::import_profile,
            profile::io::reset_profile,
            profile::io::restart_app,
            mods::get_installed_mods,
            mods::check_mod_updates,
            mods::delete_mod,
            mods::delete_mods_bulk,
            mods::toggle_mod_managed,
            mods::cleanup_mods,
            mods::open_workshop_dir,
            mods::open_mod_dir,
            mods::setup_mod_symlinks,
            launch::toggle_launch_option,
            launch::set_launch_option_value,
            launch::launch_server,
            launch::launch_direct,
            steamcmd::start_mod_operation,
            steamcmd::send_steamcmd_input,
            steamcmd::cancel_mod_operation,
            steamcmd::detect::detect_steamcmd,
            steamcmd::detect::watch_steamcmd,
            steamcmd::detect::download_steamcmd_windows,
            offline::get_offline_missions,
            offline::update_offline_mode,
            offline::remove_offline_mode,
            offline::remove_mission,
            offline::clear_offline_saves,
            offline::open_missions_dir,
            offline::open_mission_dir,
            offline::launch_offline_mission,
            news::fetch_news,
            news::webview::news_webview_result,
            news::images::fetch_image,
            news::images::resolve_cached_images,
            steam::fetch_steam_avatar,
            steam::fetch_steam_player_count,
            dzch_cli::get_cli_args,
            dzch_cli::read_dzch_file,
            dzch_cli::write_dzch_file,
            dzch_cli::parse_dzch_url,
            system::get_system_specs,
            updater::check_for_update,
            updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
