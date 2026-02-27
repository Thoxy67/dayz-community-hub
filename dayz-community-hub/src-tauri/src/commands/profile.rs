use tauri::State;

use crate::convert::profile_to_dto;
use crate::dto::ProfileDto;
use crate::state::SharedState;

/// Get the current profile.
#[tauri::command]
pub(crate) async fn get_profile(state: State<'_, SharedState>) -> Result<ProfileDto, String> {
    let state = state.lock().await;
    Ok(profile_to_dto(state.ctl.profile()))
}

/// Save profile settings.
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
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.lock().await;
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
    }
    // If avatar credentials changed, invalidate the cache so it gets re-fetched
    if credentials_changed {
        state.cached_avatar = None;
    }
    state.ctl.rebuild_steamcmd();
    state.ctl.save_profile().map_err(|e| e.to_string())
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
    let mut state = state.lock().await;
    state.ctl.add_favorite(name, ip, port, password);
    state.ctl.save_profile().map_err(|e| e.to_string())
}

/// Remove a favorite.
#[tauri::command]
pub(crate) async fn remove_favorite(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.ctl.remove_favorite(&ip, port);
    state.ctl.save_profile().map_err(|e| e.to_string())
}

/// Remove a history entry.
#[tauri::command]
pub(crate) async fn remove_history_entry(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state
        .ctl
        .profile_mut()
        .history
        .retain(|h| h.ip != ip || h.port != port);
    state.ctl.save_profile().map_err(|e| e.to_string())
}

/// Clear all history.
#[tauri::command]
pub(crate) async fn clear_history(state: State<'_, SharedState>) -> Result<(), String> {
    let mut state = state.lock().await;
    state.ctl.profile_mut().history.clear();
    state.ctl.save_profile().map_err(|e| e.to_string())
}

/// Add an IP to the excluded list (persisted to profile).
#[tauri::command]
pub(crate) async fn add_excluded_ip(
    ip: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.ctl.profile_mut().add_excluded_ip(ip);
    state.ctl.save_profile().map_err(|e| e.to_string())
}

/// Remove an IP from the excluded list (persisted to profile).
#[tauri::command]
pub(crate) async fn remove_excluded_ip(
    ip: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.ctl.profile_mut().remove_excluded_ip(&ip);
    state.ctl.save_profile().map_err(|e| e.to_string())
}
