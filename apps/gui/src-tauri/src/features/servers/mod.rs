//! The server list: loading it (from cache or the API), refreshing it, and
//! the counters the title bar shows.

mod dto;

pub use dto::{AppStatsDto, InitResult, ModDto, ServerDto, ServerSlimList};
pub(crate) use dto::{mods_to_dto, server_to_dto};

use dz_api::ServerList;
use dz_common::paths;
use dz_game::DayzCtl;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::RwLock;

use crate::error::ResultExt;
use crate::state::{AppState, SharedState};

/// Whether this is the first launch (no profile yet), asked before
/// `initialize` so the wizard can show at once.
#[tauri::command]
pub(crate) fn check_first_launch() -> bool {
    !paths::default_profile_path().exists()
}

/// Create the controller and load the server list: from the on-disk cache
/// when there is one (the window refreshes it in the background), else from
/// the API. Called once, when the window mounts.
#[tauri::command]
pub(crate) async fn initialize(app: AppHandle) -> Result<InitResult, String> {
    let profile_path = paths::default_profile_path();
    let is_first_launch = !profile_path.exists();
    let ctl = DayzCtl::new(&profile_path)
        .await
        .map_err(|e| format!("Failed to init controller: {e}"))?;

    let cache_path = paths::server_list_cache_path();
    let (list, from_cache) = if let Some(cache) = dz_api::load_server_list_cache(&cache_path).await
    {
        (dedup(cache.list), true)
    } else {
        match dz_api::fetch_servers(ctl.http_client()).await {
            Ok(list) => {
                let list = dedup(list);
                save_cache_in_background(Arc::clone(&list));
                (list, false)
            }
            Err(_) => (Arc::default(), false),
        }
    };

    let server_count = list.result.len();
    let mut app_state = AppState::new(ctl);
    app_state.set_servers(list);

    let state: SharedState = Arc::new(RwLock::new(app_state));
    app.manage(state);

    Ok(InitResult {
        server_count,
        from_cache,
        is_first_launch,
    })
}

/// The server list, without mod details.
#[tauri::command]
pub(crate) async fn get_servers(state: State<'_, SharedState>) -> Result<ServerSlimList, String> {
    Ok(ServerSlimList(Arc::clone(&state.read().await.servers)))
}

/// One server with its mods, by query port.
#[tauri::command]
pub(crate) async fn get_server_details(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<ServerDto, String> {
    let state = state.read().await;
    let server = state
        .find_by_query_port(&ip, port)
        .ok_or_else(|| "Server not found".to_string())?;
    Ok(server_to_dto(server))
}

/// Fetch the list from the API again.
#[tauri::command]
pub(crate) async fn refresh_servers(
    state: State<'_, SharedState>,
) -> Result<ServerSlimList, String> {
    let client = state.read().await.ctl.http_client().clone();
    let list = dedup(dz_api::fetch_servers(&client).await.cmd_err()?);
    save_cache_in_background(Arc::clone(&list));
    state.write().await.set_servers(Arc::clone(&list));
    Ok(ServerSlimList(list))
}

/// The title bar's counters.
#[tauri::command]
pub(crate) async fn get_app_stats(state: State<'_, SharedState>) -> Result<AppStatsDto, String> {
    let state = state.read().await;
    Ok(AppStatsDto {
        server_count: state.servers.result.len(),
        total_players: state.servers.result.iter().map(|s| s.players).sum(),
        player_name: state.ctl.profile().player.clone(),
        steam_login: state.ctl.steamcmd_login().map(str::to_string),
        has_steamcmd: state.ctl.has_steamcmd(),
    })
}

fn dedup(mut list: ServerList) -> Arc<ServerList> {
    list.result = dz_api::dedup_servers(std::mem::take(&mut list.result));
    Arc::new(list)
}

/// Write the cache without making the caller wait for a multi-MB write.
fn save_cache_in_background(list: Arc<ServerList>) {
    tauri::async_runtime::spawn(async move {
        dz_api::save_server_list_cache(&paths::server_list_cache_path(), list).await;
    });
}
