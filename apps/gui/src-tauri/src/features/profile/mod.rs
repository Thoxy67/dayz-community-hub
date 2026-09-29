//! The profile: account settings, favorites, history and excluded IPs.
//! Every change is written to disk once the state lock is released.

mod dto;
pub(crate) mod io;

pub use dto::ProfileDto;
pub(crate) use dto::profile_to_dto;

use tauri::State;

use crate::state::{SharedState, mutate_profile};

/// The current profile.
#[tauri::command]
pub(crate) async fn get_profile(state: State<'_, SharedState>) -> Result<ProfileDto, String> {
    Ok(profile_to_dto(state.read().await.ctl.profile()))
}

/// Save the account and ping settings.
// Tauri receives a command's arguments as one flat object; one parameter per
// setting keeps each addressable from the window.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn save_profile_settings(
    player: Option<String>,
    steam_login: Option<String>,
    steam_password: Option<String>,
    steam_root: Option<String>,
    steamcmd_enabled: bool,
    steamcmd_path: Option<String>,
    steam_api_key: Option<String>,
    steam_id: Option<String>,
    battlemetrics_api_key: Option<String>,
    user_location: Option<(f64, f64)>,
    ping_concurrency: u32,
    ping_timeout_auto: u32,
    ping_timeout_manual: u32,
    ping_max_retries: u32,
    ping_scan_favorites: bool,
    ping_scan_history: bool,
    ping_scan_servers: bool,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    mutate_profile(&state, |s| {
        let credentials_changed = {
            let p = s.ctl.profile();
            p.steam_api_key != steam_api_key || p.steam_id != steam_id
        };
        let profile = s.ctl.profile_mut();
        profile.player = player;
        profile.steam_login = steam_login;
        profile.steam_password = steam_password;
        profile.steam_root = steam_root;
        profile.steamcmd_enabled = steamcmd_enabled;
        profile.steamcmd_path = steamcmd_path;
        profile.steam_api_key = steam_api_key;
        profile.steam_id = steam_id;
        profile.battlemetrics_api_key = battlemetrics_api_key;
        profile.user_location = user_location;
        profile.ping_concurrency = ping_concurrency.clamp(5, 100);
        profile.ping_timeout_auto = ping_timeout_auto.clamp(1000, 5000);
        profile.ping_timeout_manual = ping_timeout_manual.clamp(1000, 30000);
        profile.ping_max_retries = ping_max_retries.min(5);
        profile.ping_scan_favorites = ping_scan_favorites;
        profile.ping_scan_history = ping_scan_history;
        profile.ping_scan_servers = ping_scan_servers;
        // New avatar credentials: fetch the avatar again.
        if credentials_changed {
            s.cached_avatar = None;
        }
        s.ctl.rebuild_steamcmd();
        Ok(())
    })
    .await
}

/// Add a favorite (or update its name and password).
#[tauri::command]
pub(crate) async fn add_favorite(
    name: String,
    ip: String,
    port: u16,
    password: Option<String>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    mutate_profile(&state, |s| {
        s.ctl.profile_mut().add_favorite(name, ip, port, password);
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_favorite(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    mutate_profile(&state, |s| {
        s.ctl.profile_mut().remove_favorite(&ip, port);
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_history_entry(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    mutate_profile(&state, |s| {
        s.ctl.profile_mut().remove_history(&ip, port);
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn clear_history(state: State<'_, SharedState>) -> Result<(), String> {
    mutate_profile(&state, |s| {
        s.ctl.profile_mut().history.clear();
        Ok(())
    })
    .await
}

/// Hide an IP's servers from the browser.
#[tauri::command]
pub(crate) async fn add_excluded_ip(ip: String, state: State<'_, SharedState>) -> Result<(), String> {
    mutate_profile(&state, |s| {
        s.ctl.profile_mut().add_excluded_ip(ip);
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_excluded_ip(
    ip: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    mutate_profile(&state, |s| {
        s.ctl.profile_mut().remove_excluded_ip(&ip);
        Ok(())
    })
    .await
}
