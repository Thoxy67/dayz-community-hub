mod cli;
mod commands;
mod convert;
mod dto;
mod helpers;
mod state;
mod utils;

#[cfg(windows)]
mod updater;

pub use cli::CliArgs;

use clap::Parser;
use tauri::Manager;

use cli::CLI_ARGS;

// ─── Application entry point ──────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(args: CliArgs) {
    // Store args globally so `get_cli_args` can serve them later.
    let _ = CLI_ARGS.set(args);

    tauri::Builder::default()
        .setup(|app| {
            // Register Windows-only plugins via setup so we can use cfg guards.
            #[cfg(windows)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
                app.manage(updater::PendingUpdate(std::sync::Mutex::new(None)));
            }

            // Register the image cache directory into the asset protocol scope at
            // runtime. This is more reliable than the static `tauri.conf.json` scope
            // because it uses Tauri's own path resolver — the exact same source that
            // the asset protocol uses when checking requests.
            if let Ok(cache_dir) = app.path().app_data_dir() {
                let images_dir = cache_dir.join("cache").join("images");
                let _ = app
                    .asset_protocol_scope()
                    .allow_directory(&images_dir, false);
            }

            // Deep-link: register dzch:// scheme at runtime on Linux (and
            // Windows debug builds) so it works outside of an installed package.
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }

            // Forward dzch:// deep-link opens to the frontend as a CliArgs event
            // so it follows the same code path as --connect / --open.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    if let Some(url) = event.urls().first() {
                        let url_str = url.to_string();
                        let args = CliArgs {
                            connect: None,
                            reconnect: false,
                            open: Some(url_str),
                        };
                        use tauri::Emitter;
                        let _ = handle.emit("cli-args", args);
                    }
                });
            }

            Ok(())
        })
        // Single-instance guard: if another instance is already running,
        // that instance's frontend receives a "cli-args" event carrying the
        // new invocation's arguments, then this process exits.
        .plugin(
            tauri_plugin_single_instance::Builder::new()
                .callback(|app, argv, _cwd| {
                    let new_args = CliArgs::try_parse_from(&argv).unwrap_or(CliArgs {
                        connect: None,
                        reconnect: false,
                        open: None,
                    });
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                    use tauri::Emitter;
                    let _ = app.emit("cli-args", new_args);
                })
                .build(),
        )
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::servers::initialize,
            commands::servers::get_servers,
            commands::servers::get_server_details,
            commands::servers::refresh_servers,
            commands::servers::get_ping,
            commands::profile::get_profile,
            commands::profile::save_profile_settings,
            commands::profile::add_favorite,
            commands::profile::remove_favorite,
            commands::profile::remove_history_entry,
            commands::profile::clear_history,
            commands::profile::add_excluded_ip,
            commands::profile::remove_excluded_ip,
            commands::mods::get_installed_mods,
            commands::mods::delete_mod,
            commands::mods::delete_mods_bulk,
            commands::mods::toggle_mod_managed,
            commands::mods::cleanup_mods,
            commands::mods::open_workshop_dir,
            commands::mods::open_mod_dir,
            commands::mods::open_mission_dir,
            commands::mods::get_missing_mods,
            commands::launch::toggle_launch_option,
            commands::launch::set_launch_option_value,
            commands::launch::launch_server,
            commands::launch::launch_direct,
            commands::steamcmd_ops::start_mod_operation,
            commands::steamcmd_ops::send_steamcmd_input,
            commands::steamcmd_ops::cancel_mod_operation,
            commands::a2s::query_a2s,
            commands::images::fetch_image,
            commands::images::resolve_cached_images,
            commands::steam::fetch_steam_avatar,
            commands::mods::check_mod_updates,
            commands::news_stats::fetch_news,
            commands::news_stats::get_app_stats,
            commands::steam::fetch_steam_player_count,
            commands::offline::get_offline_missions,
            commands::offline::update_offline_mode,
            commands::offline::remove_offline_mode,
            commands::offline::remove_mission,
            commands::offline::open_missions_dir,
            commands::offline::clear_offline_saves,
            commands::offline::launch_offline_mission,
            commands::ping::start_pinging,
            commands::ping::ping_single,
            commands::misc::is_favorite,
            commands::misc::setup_mod_symlinks,
            commands::misc::find_server,
            commands::profile_io::export_profile,
            commands::profile_io::import_profile,
            commands::profile_io::reset_profile,
            commands::profile_io::restart_app,
            commands::steamcmd_detect::detect_steamcmd,
            commands::steamcmd_detect::watch_steamcmd,
            commands::steamcmd_detect::download_steamcmd_windows,
            commands::cli_cmd::get_cli_args,
            commands::cli_cmd::read_dzch_file,
            commands::cli_cmd::write_dzch_file,
            commands::cli_cmd::parse_dzch_url,
            commands::battlemetrics::fetch_battlemetrics_server,
            // Windows-only auto-updater commands
            #[cfg(windows)]
            updater::check_for_update,
            #[cfg(windows)]
            updater::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
