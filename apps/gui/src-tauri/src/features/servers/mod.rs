//! The server list: loading it (from cache or the API), refreshing it, and
//! the counters the title bar shows.

mod dto;

pub use dto::{AppStatsDto, InitResult, ModDto, ServerDto};
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
#[specta::specta]
pub(crate) async fn check_first_launch() -> bool {
    !tokio::fs::try_exists(paths::default_profile_path())
        .await
        .unwrap_or(false)
}

/// Create the controller and load the server list: from the on-disk cache
/// when there is one (the window refreshes it in the background), else from
/// the API. The profile and the cache are read at the same time.
///
/// Called when the window mounts, and again when it reloads (the "Retry"
/// after a failed start): a second call replaces the state in place.
#[tauri::command]
#[specta::specta]
pub(crate) async fn initialize(app: AppHandle) -> Result<InitResult, String> {
    let profile_path = paths::default_profile_path();
    let is_first_launch = !tokio::fs::try_exists(&profile_path).await.unwrap_or(false);
    let cache_path = paths::server_list_cache_path();

    let (ctl, cache) = tokio::join!(
        DayzCtl::new(&profile_path),
        dz_api::load_server_list_cache(&cache_path)
    );
    let ctl = ctl.map_err(|e| format!("Could not load the profile: {e}"))?;
    crate::features::stats::open(&ctl.profile().history);

    // The cache holds the official servers merged last time; a cold start
    // fetches both lists at once.
    let (list, from_cache, list_error) = match cache {
        Some(cache) if !cache.list.result.is_empty() => (dedup(cache.list), true, None),
        _ => {
            let key = ctl.profile().steam_api_key.clone();
            let (community, official) = tokio::join!(
                dz_api::fetch_servers(ctl.http_client()),
                fetch_official(ctl.http_client(), key.as_deref())
            );
            match community {
                Ok(list) => {
                    let list = dedup(with_official(list, official));
                    save_cache_in_background(Arc::clone(&list));
                    (list, false, None)
                }
                Err(e) => (Arc::default(), false, Some(e.to_string())),
            }
        }
    };

    let server_count = list.result.len();
    let mut app_state = AppState::new(ctl);
    app_state.set_servers(Arc::clone(&list));

    match app.try_state::<SharedState>() {
        Some(existing) => *existing.write().await = app_state,
        None => {
            app.manage::<SharedState>(Arc::new(RwLock::new(app_state)));
        }
    }
    crate::features::browser::list_replaced(list);

    Ok(InitResult {
        server_count,
        from_cache,
        is_first_launch,
        list_error,
    })
}

/// One server with its mods, by query port.
#[tauri::command]
#[specta::specta]
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

/// Fetch the list from the API again. Returns how many servers it holds;
/// the browser re-queries on the `servers-changed` that follows.
#[tauri::command]
#[specta::specta]
pub(crate) async fn refresh_servers(state: State<'_, SharedState>) -> Result<u32, String> {
    let (client, key) = {
        let s = state.read().await;
        (
            s.ctl.http_client().clone(),
            s.ctl.profile().steam_api_key.clone(),
        )
    };
    let (community, official) = tokio::join!(
        dz_api::fetch_servers(&client),
        fetch_official(&client, key.as_deref())
    );
    let list = dedup(with_official(community.cmd_err()?, official));
    save_cache_in_background(Arc::clone(&list));
    state.write().await.set_servers(Arc::clone(&list));
    let count = list.result.len() as u32;
    crate::features::browser::list_replaced(list);
    Ok(count)
}

/// The title bar's counters.
#[tauri::command]
#[specta::specta]
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

/// The official servers, when a Steam API key lets us ask Steam's master
/// server for them. Failing to get them never fails the list: the community
/// servers still load, and the next refresh tries again.
async fn fetch_official(client: &reqwest::Client, key: Option<&str>) -> Vec<dz_api::Server> {
    let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) else {
        return Vec::new();
    };
    dz_api::fetch_official_servers(client, key)
        .await
        .unwrap_or_else(|e| {
            eprintln!("official servers: {e}");
            Vec::new()
        })
}

fn with_official(mut list: ServerList, official: Vec<dz_api::Server>) -> ServerList {
    dz_api::merge_official(&mut list.result, official);
    list
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
