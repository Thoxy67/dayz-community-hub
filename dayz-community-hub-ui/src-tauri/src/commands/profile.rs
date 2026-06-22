use tauri::State;

use crate::convert::profile_to_dto;
use crate::dto::ProfileDto;
use crate::state::SharedState;
use crate::utils::error::ResultExt;

/// Get the current profile.
#[tauri::command]
pub(crate) async fn get_profile(state: State<'_, SharedState>) -> Result<ProfileDto, String> {
    let state = state.read().await;
    Ok(profile_to_dto(state.ctl.profile()))
}

/// Save profile settings.
// Tauri IPC commands receive args as a flat object — the only way to keep
// each setting addressable from the JS side is one parameter per field.
// Wrapping these in a single struct just trades the warning for boilerplate.
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
    dayzavr_enabled: bool,
    dayzavr_dayz_path: Option<String>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.write().await;
    let credentials_changed = {
        let p = state.ctl.profile();
        p.steam_api_key != steam_api_key || p.steam_id != steam_id
    };
    {
        let profile = state.ctl.profile_mut();
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
        profile.ping_max_retries = ping_max_retries.clamp(0, 5);
        profile.ping_scan_favorites = ping_scan_favorites;
        profile.ping_scan_history = ping_scan_history;
        profile.ping_scan_servers = ping_scan_servers;
        profile.dayzavr_enabled = dayzavr_enabled;
        profile.dayzavr_dayz_path = dayzavr_dayz_path;
    }
    // If avatar credentials changed, invalidate the cache so it gets re-fetched
    if credentials_changed {
        state.cached_avatar = None;
    }
    state.ctl.rebuild_steamcmd();
    state.ctl.save_profile_async().await.cmd_err()
}

/// Add a server to favorites.
#[tauri::command]
pub(crate) async fn add_favorite(
    name: String,
    ip: String,
    port: u16,
    password: Option<String>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.ctl.add_favorite(name, ip, port, password);
    state.ctl.save_profile_async().await.cmd_err()
}

/// Remove a favorite.
#[tauri::command]
pub(crate) async fn remove_favorite(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.ctl.remove_favorite(&ip, port);
    state.ctl.save_profile_async().await.cmd_err()
}

/// Remove a history entry.
#[tauri::command]
pub(crate) async fn remove_history_entry(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state
        .ctl
        .profile_mut()
        .history
        .retain(|h| h.ip != ip || h.port != port);
    state.ctl.save_profile_async().await.cmd_err()
}

/// Clear all history.
#[tauri::command]
pub(crate) async fn clear_history(state: State<'_, SharedState>) -> Result<(), String> {
    let mut state = state.write().await;
    state.ctl.profile_mut().history.clear();
    state.ctl.save_profile_async().await.cmd_err()
}

/// Add an IP to the excluded list (persisted to profile).
#[tauri::command]
pub(crate) async fn add_excluded_ip(
    ip: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.ctl.profile_mut().add_excluded_ip(ip);
    state.ctl.save_profile_async().await.cmd_err()
}

/// Remove an IP from the excluded list (persisted to profile).
#[tauri::command]
pub(crate) async fn remove_excluded_ip(
    ip: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.ctl.profile_mut().remove_excluded_ip(&ip);
    state.ctl.save_profile_async().await.cmd_err()
}
