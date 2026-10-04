//! Every command the window may invoke and every event it may hear, described
//! once through tauri-specta. [`builder`] is what `run` hands Tauri to
//! dispatch an invoke, and what [`export`] renders into the window's
//! `bindings.ts`.

use crate::features::*;

/// Where the window's generated bindings live.
const BINDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc/bindings.ts");

const HEADER: &str = "// @generated from apps/gui/src-tauri by `make bindings`. Do not edit.\n";

/// The commands and events, with how their types are written out.
pub(crate) fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            servers::check_first_launch,
            servers::initialize,
            servers::get_server_details,
            servers::refresh_servers,
            servers::get_app_stats,
            browser::servers_query,
            browser::servers_lookup,
            browser::server_maps,
            browser::start_scan,
            ping::ping_servers,
            ping::get_pings,
            ping::ping_single,
            ping::cancel_ping,
            ping::toggle_ping_pause,
            a2s::query_a2s,
            metrics::fetch_server_metrics,
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
            steamcmd::steamcmd_dirs,
            steamcmd::open_steamcmd_dir,
            steamcmd::detect::detect_steamcmd,
            steamcmd::detect::watch_steamcmd,
            steamcmd::detect::download_steamcmd_windows,
            steamworks::set_mod_downloader,
            steamworks::steamworks_status,
            steamworks::steamworks_check,
            steamworks::steam_subscriptions,
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
            updater::update_support,
            native::open_url,
            native::copy_text,
            native::pick_file,
            native::save_file,
            native::geolocate_ip,
            gamepad::gamepad_status,
        ])
        .events(crate::events::collect())
        // Ports, timestamps, sizes and workshop ids are numbers in the window,
        // and every one of them is below 2^53.
        .dangerously_cast_bigints_to_number()
        // A command rejects with its error string, as a plain invoke does.
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
}

/// Write `bindings.ts`, only when its contents change (a rewrite of an
/// identical file would still set off vite's reload).
pub(crate) fn export(builder: &tauri_specta::Builder<tauri::Wry>) -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!("dzch-bindings-{}.ts", std::process::id()));
    builder
        .export(
            specta_typescript::Typescript::default().header(HEADER),
            &tmp,
        )
        .map_err(|e| format!("exporting the bindings: {e}"))?;
    let fresh =
        std::fs::read_to_string(&tmp).map_err(|e| format!("reading {}: {e}", tmp.display()));
    let _ = std::fs::remove_file(&tmp);
    let fresh = fresh?;
    if std::fs::read_to_string(BINDINGS).ok().as_deref() != Some(fresh.as_str()) {
        if let Some(dir) = std::path::Path::new(BINDINGS).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(BINDINGS, fresh).map_err(|e| format!("writing {BINDINGS}: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// `make bindings`: `cargo test -p dayz-community-hub export_bindings`.
    #[test]
    fn export_bindings() {
        super::export(&super::builder()).unwrap();
    }
}
