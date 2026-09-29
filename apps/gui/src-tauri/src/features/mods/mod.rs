//! Installed workshop mods: list them, check them against the Workshop,
//! delete them, link and unlink them, and open their folders.

use dz_game::mods::{self, InstalledMod};
use futures_util::StreamExt;
use rustc_hash::FxHashMap;
use serde::Serialize;
use std::sync::OnceLock;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{ResultExt, spawn_blocking_mapped};
use crate::state::SharedState;

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct InstalledModDto {
    pub name: String,
    pub id: u64,
    /// When steamcmd last wrote the mod (Unix seconds).
    pub local_updated: i64,
    pub size: u64,
    pub size_human: String,
    /// Installed by this app (it may delete or unlink it).
    pub managed: bool,
    /// `time_updated` on the Workshop; null until checked.
    pub remote_updated: Option<i64>,
    /// True when `remote_updated > local_updated`.
    pub update_available: bool,
}

fn installed_mod_to_dto(m: &InstalledMod, update_cache: &FxHashMap<u64, i64>) -> InstalledModDto {
    let remote_updated = update_cache.get(&m.id).copied();
    InstalledModDto {
        name: m.name.clone(),
        id: m.id,
        local_updated: m.local_updated,
        size: m.size,
        size_human: mods::format_size(m.size),
        managed: m.managed,
        remote_updated,
        update_available: remote_updated.is_some_and(|r| r > m.local_updated),
    }
}

/// The client for the Steam Workshop API, built once.
fn workshop_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("Mozilla/5.0")
            .build()
            .expect("Failed to build Workshop HTTP client")
    })
}

/// Get installed mods. Uses spawn_blocking for filesystem scan.
/// Enriches each mod with `remote_updated` / `update_available` from the in-memory cache.
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_installed_mods(
    state: State<'_, SharedState>,
) -> Result<Vec<InstalledModDto>, String> {
    let (ctl_clone, update_cache) = {
        let s = state.read().await;
        (s.ctl.clone_for_task(), s.mod_update_cache.clone())
    };

    let mods = spawn_blocking_mapped(move || ctl_clone.get_installed_mods()).await?;

    Ok(mods
        .iter()
        .map(|m| installed_mod_to_dto(m, &update_cache))
        .collect())
}

/// Fetch `time_updated` for all installed mods from the Steam Workshop API and
/// cache the results. Returns the enriched mod list (same as `get_installed_mods`).
#[tauri::command]
#[specta::specta]
pub(crate) async fn check_mod_updates(
    state: State<'_, SharedState>,
) -> Result<Vec<InstalledModDto>, String> {
    let (api_key, ctl_clone) = {
        let s = state.read().await;
        (
            s.ctl.profile().steam_api_key.clone(),
            s.ctl.clone_for_task(),
        )
    };

    let installed_mods = spawn_blocking_mapped(move || ctl_clone.get_installed_mods())
        .await
        .unwrap_or_default();

    if installed_mods.is_empty() {
        return Ok(vec![]);
    }

    let mod_ids: Vec<u64> = installed_mods.iter().map(|m| m.id).collect();

    let client = workshop_client();

    let api_key = api_key.filter(|k| !k.is_empty());

    // Query the Workshop API for each 100-id chunk concurrently — the chunks are
    // independent and the shared client pools connections, so for users with
    // several hundred mods this turns N serial round-trips into a few parallel
    // waves instead.
    let chunks: Vec<Vec<u64>> = mod_ids.chunks(100).map(|c| c.to_vec()).collect();
    let chunk_results: Vec<Result<Vec<(u64, i64)>, String>> = futures_util::stream::iter(chunks)
        .map(|chunk| {
            let api_key = api_key.clone();
            async move {
                let mut params: Vec<(String, String)> = Vec::new();
                if let Some(key) = &api_key {
                    params.push(("key".to_string(), key.clone()));
                }
                params.push(("includetags".to_string(), "0".to_string()));
                params.push(("includeadditionalpreviews".to_string(), "0".to_string()));
                params.push(("includechildren".to_string(), "0".to_string()));
                params.push(("includesummary".to_string(), "0".to_string()));
                params.push(("includevotes".to_string(), "0".to_string()));
                for (i, id) in chunk.iter().enumerate() {
                    params.push((format!("publishedfileids[{}]", i), id.to_string()));
                }

                let resp: serde_json::Value = client
                    .get("https://api.steampowered.com/IPublishedFileService/GetDetails/v1/")
                    .query(&params)
                    .send()
                    .await
                    .cmd_err()?
                    .json::<serde_json::Value>()
                    .await
                    .cmd_err()?;

                let mut out = Vec::new();
                if let Some(files) = resp["response"]["publishedfiledetails"].as_array() {
                    for file in files {
                        let id = file["publishedfileid"]
                            .as_str()
                            .and_then(|s| s.parse::<u64>().ok());
                        let ts = file["time_updated"].as_i64();
                        if let (Some(id), Some(ts)) = (id, ts) {
                            out.push((id, ts));
                        }
                    }
                }
                Ok(out)
            }
        })
        .buffer_unordered(6)
        .collect()
        .await;

    let mut remote_map: FxHashMap<u64, i64> = FxHashMap::default();
    for chunk in chunk_results {
        for (id, ts) in chunk? {
            remote_map.insert(id, ts);
        }
    }

    let dtos = installed_mods
        .iter()
        .map(|m| installed_mod_to_dto(m, &remote_map))
        .collect();
    state.write().await.mod_update_cache = remote_map;
    Ok(dtos)
}

/// Delete a mod by ID.
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_mod(mod_id: u64, state: State<'_, SharedState>) -> Result<(), String> {
    let ctl_clone = { state.read().await.ctl.clone_for_task() };
    spawn_blocking_mapped(move || ctl_clone.delete_mod(mod_id, false)).await
}

/// Delete multiple mods by ID in one call.
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_mods_bulk(
    mod_ids: Vec<u64>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let ctl_clone = { state.read().await.ctl.clone_for_task() };
    spawn_blocking_mapped(move || -> std::result::Result<(), String> {
        for id in mod_ids {
            ctl_clone.delete_mod(id, false).cmd_err()?;
        }
        Ok(())
    })
    .await
}

/// Toggle managed status of a mod.
#[tauri::command]
#[specta::specta]
pub(crate) async fn toggle_mod_managed(
    mod_id: u64,
    state: State<'_, SharedState>,
) -> Result<bool, String> {
    let ctl_clone = { state.read().await.ctl.clone_for_task() };
    spawn_blocking_mapped(move || ctl_clone.toggle_mod_managed(mod_id)).await
}

/// Cleanup all managed mods and symlinks.
#[tauri::command]
#[specta::specta]
pub(crate) async fn cleanup_mods(state: State<'_, SharedState>) -> Result<String, String> {
    let ctl_clone = { state.read().await.ctl.clone_for_task() };
    let stats = spawn_blocking_mapped(move || ctl_clone.cleanup_mods()).await?;
    Ok(format!(
        "Removed {} mods ({}) and {} symlinks",
        stats.removed_count,
        mods::format_size(stats.removed_size),
        stats.symlinks_removed
    ))
}

/// Open the Steam Workshop directory (all mods) in the system file manager.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_workshop_dir(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let path = {
        let state = state.read().await;
        state.ctl.workshop_path().cmd_err()?
    };
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .cmd_err()
}

/// Open a specific mod's directory in the system file manager.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_mod_dir(
    app: AppHandle,
    mod_id: u64,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let path = {
        let state = state.read().await;
        state
            .ctl
            .workshop_path()
            .cmd_err()?
            .join(mod_id.to_string())
    };
    app.opener()
        .open_path(path.to_string_lossy().as_ref(), None::<&str>)
        .cmd_err()
}

/// Create the `@<id>` links a listed server's mods need.
#[tauri::command]
#[specta::specta]
pub(crate) async fn setup_mod_symlinks(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, ctl) = {
        let state = state.read().await;
        let server = state
            .find_by_query_port(&ip, port)
            .cloned()
            .ok_or_else(|| "Server not found".to_string())?;
        (server, state.ctl.clone_for_task())
    };
    spawn_blocking_mapped(move || ctl.setup_mod_symlinks(&server).map(|_| ())).await
}
