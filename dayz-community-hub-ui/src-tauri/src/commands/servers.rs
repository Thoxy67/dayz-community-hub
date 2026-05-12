use dayz_community_hub_core::{api, config};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::RwLock;

use crate::convert::{server_to_dto, server_to_slim_dto};
use crate::dto::*;
use crate::helpers::{dedup_servers, find_server_in};
use crate::state::{AppState, PingCache, SharedState};
use crate::utils::error::ResultExt;

/// Quick check if this is a first launch (no profile exists).
/// Called before initialize to show wizard immediately.
#[tauri::command]
pub(crate) fn check_first_launch() -> bool {
    let profile_path = config::default_profile_path();
    !profile_path.exists()
}

/// Initialize the application: create DayzCtl, load cached servers.
/// Called once by the frontend on mount. The window is already visible.
#[tauri::command]
pub(crate) async fn initialize(app: AppHandle) -> Result<InitResult, String> {
    let profile_path = config::default_profile_path();
    let is_first_launch = !profile_path.exists();
    let ctl = dayz_community_hub_core::ctl::DayzCtl::new(&profile_path)
        .await
        .map_err(|e| format!("Failed to init controller: {}", e))?;

    // Try to load cached server list (sync file read — fast)
    let cache_path = config::default_data_dir().join("server_list_cache.json");
    let (servers, fetched_at, from_cache) =
        if let Some(cache) = api::load_server_list_cache(&cache_path).await {
            (dedup_servers(cache.list.result), cache.fetched_at, true)
        } else {
            // No cache — fetch from network
            let client = ctl.http_client().clone();
            match api::fetch_servers(&client).await {
                Ok(list) => {
                    api::save_server_list_cache(&cache_path, &list).await;
                    (dedup_servers(list.result), now_secs(), false)
                }
                Err(_) => (Vec::new(), 0, false),
            }
        };

    let server_count = servers.len();

    // Create AppState using the new() constructor
    let mut app_state = AppState::new(ctl);
    app_state.set_servers(servers);
    app_state.servers_fetched_at = fetched_at;

    let state: SharedState = Arc::new(RwLock::new(app_state));
    let ping_cache: PingCache = Arc::new(RwLock::new(rustc_hash::FxHashMap::default()));

    app.manage(state);
    app.manage(ping_cache);

    Ok(InitResult {
        server_count,
        from_cache,
        is_first_launch,
    })
}

/// Get the slim server list (no mod details — for the table).
#[tauri::command]
pub(crate) async fn get_servers(
    state: State<'_, SharedState>,
) -> Result<Vec<ServerSlimDto>, String> {
    let state = state.read().await;
    Ok(state.servers.iter().map(server_to_slim_dto).collect())
}

/// Get full server details including mods for a specific server.
#[tauri::command]
pub(crate) async fn get_server_details(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<ServerDto, String> {
    let state = state.read().await;
    let server =
        find_server_in(&state.servers, &ip, port).ok_or_else(|| "Server not found".to_string())?;
    Ok(server_to_dto(server))
}

/// Refresh the server list from the API.
#[tauri::command]
pub(crate) async fn refresh_servers(
    state: State<'_, SharedState>,
) -> Result<Vec<ServerSlimDto>, String> {
    let client = {
        let state = state.read().await;
        state.ctl.http_client().clone()
    };

    let cache_path = config::default_data_dir().join("server_list_cache.json");
    let list = api::fetch_servers(&client).await.cmd_err()?;

    api::save_server_list_cache(&cache_path, &list).await;

    let mut state = state.write().await;
    state.set_servers(dedup_servers(list.result));
    state.servers_fetched_at = now_secs();
    Ok(state.servers.iter().map(server_to_slim_dto).collect())
}

/// Age (in seconds) of the in-memory server list since it was last fetched
/// from the API or loaded from on-disk cache. Returns `None` if the list is
/// empty/never populated. Frontend uses this to decide when to auto-refresh.
#[tauri::command]
pub(crate) async fn get_servers_cache_age_secs(
    state: State<'_, SharedState>,
) -> Result<Option<u64>, String> {
    let state = state.read().await;
    if state.servers_fetched_at == 0 {
        Ok(None)
    } else {
        Ok(Some(now_secs().saturating_sub(state.servers_fetched_at)))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Get ping for a server (from cache).
/// Returns just the ms value for backward compatibility.
#[tauri::command]
pub(crate) async fn get_ping(
    ip: String,
    port: i64,
    ping_cache: State<'_, PingCache>,
) -> Result<Option<u32>, String> {
    let cache = ping_cache.read().await;
    let key = format!("{}:{}", ip, port);
    Ok(cache.get(&key).map(|r| r.ms))
}
