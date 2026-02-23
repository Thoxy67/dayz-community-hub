use dayz_community_hub_core::{
    a2s_query,
    api::{self, Server},
    config::{self, Profile},
    ctl::{DayzCtl, ModOperation},
    mods::{self, InstalledMod},
    news,
    offline::OfflineMode,
    steamcmd::ModProgress,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{ipc::Channel, AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

// ─── Shared state ─────────────────────────────────────────────────────────────

pub struct AppState {
    pub ctl: DayzCtl,
    pub servers: Vec<Server>,
    pub ping_cache: std::collections::HashMap<String, u32>,
    /// Cached Steam avatar as a data: URI. Populated by `fetch_steam_avatar`.
    pub cached_avatar: Option<String>,
    /// mod_id → remote time_updated from Steam Workshop API.
    pub mod_update_cache: std::collections::HashMap<u64, i64>,
}

type SharedState = Arc<Mutex<AppState>>;

// ─── Serializable DTOs ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerDto {
    pub game_port: i64,
    pub ip: String,
    pub query_port: i64,
    pub name: String,
    pub map: String,
    pub players: i64,
    pub max_players: i64,
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub first_person_only: bool,
    pub time: String,
    pub mods_count: usize,
    pub mods: Vec<ModDto>,
    pub vac: bool,
    pub battl_eye: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerSlimDto {
    pub game_port: i64,
    pub ip: String,
    pub query_port: i64,
    pub name: String,
    pub map: String,
    pub players: i64,
    pub max_players: i64,
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub first_person_only: bool,
    pub time: String,
    pub mods_count: usize,
    pub vac: bool,
    pub battl_eye: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModDto {
    pub name: String,
    pub steam_workshop_id: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstalledModDto {
    pub name: String,
    pub id: u64,
    pub local_updated: i64,
    pub size: u64,
    pub size_human: String,
    pub managed: bool,
    /// Remote `time_updated` from Steam Workshop API. None if not yet checked.
    pub remote_updated: Option<i64>,
    /// True when `remote_updated > local_updated`.
    pub update_available: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FavoriteDto {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryDto {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub ts: i64,
    pub relative_time: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileDto {
    pub steam_login: Option<String>,
    pub steam_password: Option<String>,
    pub steam_root: Option<String>,
    pub steamcmd_enabled: bool,
    /// Explicit path to steamcmd binary (overrides auto-detection).
    pub steamcmd_path: Option<String>,
    pub player: Option<String>,
    pub steam_api_key: Option<String>,
    pub steam_id: Option<String>,
    pub favorites: Vec<FavoriteDto>,
    pub history: Vec<HistoryDto>,
    pub options: Vec<LaunchOptionDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LaunchOptionDto {
    pub key: String,
    pub enabled: bool,
    pub value: Option<String>,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ArticleDto {
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_text: String,
    pub content_html: String,
    pub date: String,
    pub url: String,
    pub image_url: Option<String>,
    pub category: Option<String>,
    pub author: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct A2sPlayerDto {
    pub name: String,
    pub score: i32,
    pub duration: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct A2sDetailsDto {
    pub server_name: String,
    pub game: String,
    pub players: u8,
    pub max_players: u8,
    pub map: String,
    pub version: String,
    pub players_list: Vec<A2sPlayerDto>,
    /// Mods from the server list (if server was found); empty if querying unknown server
    pub mods: Vec<ModDto>,
    /// The query port actually used (so frontend can display/cache correctly)
    pub query_port: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppStatsDto {
    pub server_count: usize,
    pub total_players: i64,
    pub player_name: Option<String>,
    pub steam_login: Option<String>,
    pub has_steamcmd: bool,
    /// Steam avatar as a data: URI, or None if not configured / failed.
    pub avatar_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModProgressEvent {
    pub kind: String,
    pub current: usize,
    pub total: usize,
    pub mod_id: u64,
    pub name: String,
    pub ok: usize,
    pub failed: usize,
    pub hint: Option<String>,
}

/// Response from the initialize command.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InitResult {
    pub server_count: usize,
    pub from_cache: bool,
}

/// Ping result sent via Channel.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PingResultDto {
    pub ip: String,
    pub port: i64,
    pub ms: u32,
}

// ─── Conversions ──────────────────────────────────────────────────────────────

fn server_to_dto(s: &Server) -> ServerDto {
    ServerDto {
        game_port: s.game_port,
        ip: s.endpoint.ip.clone(),
        query_port: s.endpoint.port,
        name: s.name.clone(),
        map: s.map.clone(),
        players: s.players,
        max_players: s.max_players,
        environment: s.environment.clone(),
        password: s.password,
        version: s.version.clone(),
        first_person_only: s.first_person_only,
        time: s.time.clone(),
        mods_count: s.mods.len(),
        mods: s
            .mods
            .iter()
            .map(|m| ModDto {
                name: m.name.clone(),
                steam_workshop_id: m.steam_workshop_id,
            })
            .collect(),
        vac: s.vac,
        battl_eye: s.battl_eye,
    }
}

fn server_to_slim_dto(s: &Server) -> ServerSlimDto {
    ServerSlimDto {
        game_port: s.game_port,
        ip: s.endpoint.ip.clone(),
        query_port: s.endpoint.port,
        name: s.name.clone(),
        map: s.map.clone(),
        players: s.players,
        max_players: s.max_players,
        environment: s.environment.clone(),
        password: s.password,
        version: s.version.clone(),
        first_person_only: s.first_person_only,
        time: s.time.clone(),
        mods_count: s.mods.len(),
        vac: s.vac,
        battl_eye: s.battl_eye,
    }
}

fn profile_to_dto(profile: &Profile) -> ProfileDto {
    ProfileDto {
        steam_login: profile.steam_login.clone(),
        steam_password: profile.steam_password.clone(),
        steam_root: profile.steam_root.clone(),
        steamcmd_enabled: profile.steamcmd_enabled,
        steamcmd_path: profile.steamcmd_path.clone(),
        player: profile.player.clone(),
        steam_api_key: profile.steam_api_key.clone(),
        steam_id: profile.steam_id.clone(),
        favorites: profile
            .favorites
            .iter()
            .map(|f| FavoriteDto {
                name: f.name.clone(),
                ip: f.ip.clone(),
                port: f.port,
            })
            .collect(),
        history: profile
            .history
            .iter()
            .map(|h| HistoryDto {
                name: h.name.clone(),
                ip: h.ip.clone(),
                port: h.port,
                ts: h.ts,
                relative_time: h.relative_time(),
            })
            .collect(),
        options: profile
            .options
            .all_options()
            .iter()
            .map(|(key, opt)| LaunchOptionDto {
                key: key.to_string(),
                enabled: opt.enabled,
                value: opt.value.clone(),
                description: opt.description.clone(),
            })
            .collect(),
    }
}

fn installed_mod_to_dto(
    m: &InstalledMod,
    update_cache: &std::collections::HashMap<u64, i64>,
) -> InstalledModDto {
    let remote_updated = update_cache.get(&m.id).copied();
    let update_available = remote_updated
        .map(|r| r > m.local_updated)
        .unwrap_or(false);
    InstalledModDto {
        name: m.name.clone(),
        id: m.id,
        local_updated: m.local_updated,
        size: m.size,
        size_human: mods::format_size(m.size),
        managed: m.managed,
        remote_updated,
        update_available,
    }
}

fn mod_progress_to_event(msg: &ModProgress) -> ModProgressEvent {
    match msg {
        ModProgress::ShuttingDownSteam => ModProgressEvent {
            kind: "shutting_down_steam".into(),
            current: 0,
            total: 0,
            mod_id: 0,
            name: "Closing Steam...".into(),
            ok: 0,
            failed: 0,
            hint: None,
        },
        ModProgress::Starting {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent {
            kind: "starting".into(),
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: name.clone(),
            ok: 0,
            failed: 0,
            hint: None,
        },
        ModProgress::Done {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent {
            kind: "done".into(),
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: name.clone(),
            ok: 0,
            failed: 0,
            hint: None,
        },
        ModProgress::Failed {
            current,
            total,
            mod_id,
            name,
            error,
        } => ModProgressEvent {
            kind: "failed".into(),
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: format!("{} ({})", name, error),
            ok: 0,
            failed: 0,
            hint: None,
        },
        ModProgress::Finished {
            ok,
            failed,
            total: _,
            hint,
        } => ModProgressEvent {
            kind: "finished".into(),
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: *ok,
            failed: *failed,
            hint: hint.clone(),
        },
    }
}

// ─── Tauri Commands ───────────────────────────────────────────────────────────

/// Initialize the application: create DayzCtl, load cached servers.
/// Called once by the frontend on mount. The window is already visible.
#[tauri::command]
async fn initialize(app: AppHandle) -> Result<InitResult, String> {
    let profile_path = config::default_profile_path();
    let ctl = DayzCtl::new(&profile_path)
        .await
        .map_err(|e| format!("Failed to init controller: {}", e))?;

    // Try to load cached server list (sync file read — fast)
    let cache_path = config::default_data_dir().join("server_list_cache.json");
    let (servers, from_cache) = if let Some(cache) = api::load_server_list_cache(&cache_path) {
        (cache.list.result, true)
    } else {
        // No cache — fetch from network
        let client = ctl.http_client().clone();
        match api::fetch_servers(&client).await {
            Ok(list) => {
                api::save_server_list_cache(&cache_path, &list);
                (list.result, false)
            }
            Err(_) => (Vec::new(), false),
        }
    };

    let server_count = servers.len();
    let state: SharedState = Arc::new(Mutex::new(AppState {
        ctl,
        servers,
        ping_cache: std::collections::HashMap::new(),
        cached_avatar: None,
        mod_update_cache: std::collections::HashMap::new(),
    }));

    app.manage(state);

    Ok(InitResult {
        server_count,
        from_cache,
    })
}

/// Get the slim server list (no mod details — for the table).
#[tauri::command]
async fn get_servers(state: State<'_, SharedState>) -> Result<Vec<ServerSlimDto>, String> {
    let state = state.lock().await;
    Ok(state.servers.iter().map(server_to_slim_dto).collect())
}

/// Get full server details including mods for a specific server.
#[tauri::command]
async fn get_server_details(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<ServerDto, String> {
    let state = state.lock().await;
    let server = state
        .servers
        .iter()
        .find(|s| s.endpoint.ip == ip && s.endpoint.port == port)
        .ok_or_else(|| "Server not found".to_string())?;
    Ok(server_to_dto(server))
}

/// Refresh the server list from the API.
#[tauri::command]
async fn refresh_servers(state: State<'_, SharedState>) -> Result<Vec<ServerSlimDto>, String> {
    let client = {
        let state = state.lock().await;
        state.ctl.http_client().clone()
    };

    let cache_path = config::default_data_dir().join("server_list_cache.json");
    let list = api::fetch_servers(&client)
        .await
        .map_err(|e| e.to_string())?;

    api::save_server_list_cache(&cache_path, &list);

    let mut state = state.lock().await;
    state.servers = list.result;
    Ok(state.servers.iter().map(server_to_slim_dto).collect())
}

/// Get ping for a server (from cache).
#[tauri::command]
async fn get_ping(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<Option<u32>, String> {
    let state = state.lock().await;
    let key = format!("{}:{}", ip, port);
    Ok(state.ping_cache.get(&key).copied())
}

/// Get the current profile.
#[tauri::command]
async fn get_profile(state: State<'_, SharedState>) -> Result<ProfileDto, String> {
    let state = state.lock().await;
    Ok(profile_to_dto(state.ctl.profile()))
}

/// Save profile settings.
#[tauri::command]
async fn save_profile_settings(
    player: Option<String>,
    steam_login: Option<String>,
    steam_password: Option<String>,
    steam_root: Option<String>,
    steamcmd_enabled: bool,
    steamcmd_path: Option<String>,
    steam_api_key: Option<String>,
    steam_id: Option<String>,
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
async fn add_favorite(
    name: String,
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.ctl.add_favorite(name, ip, port);
    state.ctl.save_profile().map_err(|e| e.to_string())
}

/// Remove a favorite.
#[tauri::command]
async fn remove_favorite(
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
async fn remove_history_entry(
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
async fn clear_history(state: State<'_, SharedState>) -> Result<(), String> {
    let mut state = state.lock().await;
    state.ctl.profile_mut().history.clear();
    state.ctl.save_profile().map_err(|e| e.to_string())
}

/// Get installed mods. Uses spawn_blocking for filesystem scan.
/// Enriches each mod with `remote_updated` / `update_available` from the in-memory cache.
#[tauri::command]
async fn get_installed_mods(state: State<'_, SharedState>) -> Result<Vec<InstalledModDto>, String> {
    let (ctl_clone, update_cache) = {
        let s = state.lock().await;
        (s.ctl.clone_for_launch(), s.mod_update_cache.clone())
    };

    let mods = tokio::task::spawn_blocking(move || ctl_clone.get_installed_mods())
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())?;

    Ok(mods.iter().map(|m| installed_mod_to_dto(m, &update_cache)).collect())
}

/// Fetch `time_updated` for all installed mods from the Steam Workshop API and
/// cache the results. Returns the enriched mod list (same as `get_installed_mods`).
///
/// Uses `IPublishedFileService/GetDetails/v1` which is public — no API key is
/// required for public workshop items, but the stored key is used when available
/// to benefit from higher rate limits.
#[tauri::command]
async fn check_mod_updates(state: State<'_, SharedState>) -> Result<Vec<InstalledModDto>, String> {
    // Collect mod IDs and api key without holding the lock during network I/O
    let (mod_ids, api_key, ctl_clone) = {
        let s = state.lock().await;
        let ids: Vec<u64> = s
            .ctl
            .get_installed_mods()
            .unwrap_or_default()
            .iter()
            .map(|m| m.id)
            .collect();
        let key = s.ctl.profile().steam_api_key.clone();
        let ctl = s.ctl.clone_for_launch();
        (ids, key, ctl)
    };

    if mod_ids.is_empty() {
        return Ok(vec![]);
    }

    // Build query — IPublishedFileService/GetDetails accepts up to 100 IDs per
    // request via numbered `publishedfileids[N]` params (POST form or GET query).
    // We chunk to 100 to be safe and issue requests sequentially.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| e.to_string())?;

    let mut remote_map: std::collections::HashMap<u64, i64> = std::collections::HashMap::new();

    for chunk in mod_ids.chunks(100) {
        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(ref key) = api_key {
            if !key.is_empty() {
                params.push(("key".to_string(), key.clone()));
            }
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
            .map_err(|e| e.to_string())?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| e.to_string())?;

        if let Some(files) = resp["response"]["publishedfiledetails"].as_array() {
            for file in files {
                let id = file["publishedfileid"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok());
                let ts = file["time_updated"].as_i64();
                if let (Some(id), Some(ts)) = (id, ts) {
                    remote_map.insert(id, ts);
                }
            }
        }
    }

    // Write back to cache
    {
        let mut s = state.lock().await;
        s.mod_update_cache = remote_map.clone();
    }

    // Re-scan filesystem and return enriched list
    let mods = tokio::task::spawn_blocking(move || ctl_clone.get_installed_mods())
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())?;

    Ok(mods.iter().map(|m| installed_mod_to_dto(m, &remote_map)).collect())
}

/// Delete a mod by ID.
#[tauri::command]
async fn delete_mod(mod_id: u64, state: State<'_, SharedState>) -> Result<(), String> {
    let ctl_clone = {
        let state = state.lock().await;
        state.ctl.clone_for_launch()
    };
    tokio::task::spawn_blocking(move || ctl_clone.delete_mod(mod_id, false))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())
}

/// Toggle managed status of a mod.
#[tauri::command]
async fn toggle_mod_managed(mod_id: u64, state: State<'_, SharedState>) -> Result<bool, String> {
    let ctl_clone = {
        let state = state.lock().await;
        state.ctl.clone_for_launch()
    };
    tokio::task::spawn_blocking(move || ctl_clone.toggle_mod_managed(mod_id))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())
}

/// Cleanup all managed mods and symlinks.
#[tauri::command]
async fn cleanup_mods(state: State<'_, SharedState>) -> Result<String, String> {
    let ctl_clone = {
        let state = state.lock().await;
        state.ctl.clone_for_launch()
    };
    let stats = tokio::task::spawn_blocking(move || ctl_clone.cleanup_mods())
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "Removed {} mods ({}) and {} symlinks",
        stats.removed_count,
        mods::format_size(stats.removed_size),
        stats.symlinks_removed
    ))
}

/// Get missing mod IDs for a server.
#[tauri::command]
async fn get_missing_mods(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<Vec<u64>, String> {
    let (server, ctl_clone) = {
        let state = state.lock().await;
        let server = state
            .servers
            .iter()
            .find(|s| s.endpoint.ip == ip && s.endpoint.port == port)
            .cloned()
            .ok_or_else(|| "Server not found".to_string())?;
        let ctl_clone = state.ctl.clone_for_launch();
        (server, ctl_clone)
    };
    tokio::task::spawn_blocking(move || ctl_clone.get_missing_mod_ids(&server))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())
}

/// Toggle a launch option.
#[tauri::command]
async fn toggle_launch_option(key: String, state: State<'_, SharedState>) -> Result<bool, String> {
    let mut state = state.lock().await;
    let options = &mut state.ctl.profile_mut().options;
    let mut all = options.all_options_mut();
    if let Some((_, opt)) = all.iter_mut().find(|(k, _)| *k == key) {
        opt.enabled = !opt.enabled;
        let new_state = opt.enabled;
        drop(all);
        state.ctl.save_profile().map_err(|e| e.to_string())?;
        Ok(new_state)
    } else {
        Err(format!("Unknown option: {}", key))
    }
}

/// Set a launch option value.
#[tauri::command]
async fn set_launch_option_value(
    key: String,
    value: Option<String>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    let options = &mut state.ctl.profile_mut().options;
    let mut all = options.all_options_mut();
    if let Some((_, opt)) = all.iter_mut().find(|(k, _)| *k == key) {
        opt.value = value.clone();
        if value.is_some() {
            opt.enabled = true;
        }
        drop(all);
        state.ctl.save_profile().map_err(|e| e.to_string())
    } else {
        Err(format!("Unknown option: {}", key))
    }
}

/// Launch a game server. Emits "launch-done" or "launch-error" event.
#[tauri::command]
async fn launch_server(
    ip: String,
    port: i64,
    password: Option<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, mut ctl_clone) = {
        let state = state.lock().await;
        let server = state
            .servers
            .iter()
            .find(|s| s.endpoint.ip == ip && s.endpoint.port == port)
            .cloned()
            .ok_or_else(|| "Server not found".to_string())?;
        let ctl_clone = state.ctl.clone_for_launch();
        (server, ctl_clone)
    };

    tokio::spawn(async move {
        match ctl_clone.launch_game(&server, password.as_deref()).await {
            Ok(_) => {
                let _ = app.emit("launch-done", server.name.clone());
            }
            Err(e) => {
                let _ = app.emit("launch-error", e.to_string());
            }
        }
    });

    Ok(())
}

/// Launch with a direct ip:port.
#[tauri::command]
async fn launch_direct(
    ip: String,
    game_port: u16,
    password: Option<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, mut ctl_clone) = {
        let st = state.lock().await;
        let server = st
            .servers
            .iter()
            .find(|s| {
                s.endpoint.ip == ip
                    && (s.game_port as u16 == game_port || s.endpoint.port as u16 == game_port)
            })
            .cloned()
            .unwrap_or_else(|| Server {
                endpoint: dayz_community_hub_core::Endpoint {
                    ip: ip.clone(),
                    port: game_port as i64,
                },
                name: format!("{}:{}", ip, game_port),
                game_port: game_port as i64,
                mods: vec![],
                ..Default::default()
            });

        if !server.mods.is_empty() {
            let _ = st.ctl.setup_mod_symlinks(&server);
        }

        let ctl_clone = st.ctl.clone_for_launch();
        (server, ctl_clone)
    };

    tokio::spawn(async move {
        match ctl_clone.launch_game(&server, password.as_deref()).await {
            Ok(_) => {
                let _ = app.emit("launch-done", server.name.clone());
            }
            Err(e) => {
                let _ = app.emit("launch-error", e.to_string());
            }
        }
    });

    Ok(())
}

/// Start a background mod operation. Streams progress via Channel.
#[tauri::command]
async fn start_mod_operation(
    op_type: String,
    ip: Option<String>,
    port: Option<i64>,
    mod_id: Option<u64>,
    mod_name: Option<String>,
    on_progress: Channel<ModProgressEvent>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut rx = {
        let state = state.lock().await;
        let op = match op_type.as_str() {
            "install_server" | "update_server" => {
                let ip = ip.ok_or("ip required")?;
                let port = port.ok_or("port required")?;
                let server = state
                    .servers
                    .iter()
                    .find(|s| s.endpoint.ip == ip && s.endpoint.port == port)
                    .cloned()
                    .ok_or_else(|| "Server not found".to_string())?;
                if op_type == "install_server" {
                    ModOperation::InstallOnly { server }
                } else {
                    ModOperation::UpdateThenLaunch {
                        server,
                        password: None,
                    }
                }
            }
            "update_all" => ModOperation::UpdateAll,
            "update_stale" => {
                // `state` is already the held MutexGuard here — use it directly
                let cache = &state.mod_update_cache;
                let stale = state
                    .ctl
                    .get_installed_mods()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|m| cache.get(&m.id).map(|&r| r > m.local_updated).unwrap_or(false))
                    .map(|m| (m.id, m.name.clone()))
                    .collect::<Vec<_>>();
                ModOperation::UpdateStale { stale_mods: stale }
            }
            "update_one" => ModOperation::UpdateOne {
                mod_id: mod_id.ok_or("mod_id required")?,
                name: mod_name.unwrap_or_default(),
            },
            other => return Err(format!("Unknown operation: {}", other)),
        };
        let (rx, _handle) = state
            .ctl
            .start_mod_operation(op)
            .map_err(|e| e.to_string())?;
        rx
    };

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(msg) => {
                    let evt = mod_progress_to_event(&msg);
                    let is_finished = evt.kind == "finished";
                    let _ = on_progress.send(evt);
                    if is_finished {
                        break;
                    }
                }
                None => break,
            }
        }
    });

    Ok(())
}

/// Query A2S live info for a server.
///
/// `port` is whatever port the caller knows about (game port or query port).
/// We search the server list matching by IP and either port field so we always
/// use the correct query port (`endpoint.port`).  If the server is not in the
/// list we treat `port` as the query port directly (best-effort fallback).
#[tauri::command]
async fn query_a2s(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<A2sDetailsDto, String> {
    let (query_addr, mods_dto) = {
        let state = state.lock().await;
        // Match by query port OR game port so favorites stored with the game
        // port still resolve to the correct query port.
        let found = state
            .servers
            .iter()
            .find(|s| s.endpoint.ip == ip && (s.endpoint.port == port || s.game_port == port));
        match found {
            Some(s) => {
                let addr = format!("{}:{}", s.endpoint.ip, s.endpoint.port);
                let mods = s
                    .mods
                    .iter()
                    .map(|m| ModDto {
                        name: m.name.clone(),
                        steam_workshop_id: m.steam_workshop_id,
                    })
                    .collect::<Vec<_>>();
                eprintln!("[query_a2s] found in list → query addr={addr}");
                (addr, mods)
            }
            None => {
                let addr = format!("{}:{}", ip, port);
                eprintln!("[query_a2s] not in list → query addr={addr} (using supplied port as query port)");
                (addr, vec![])
            }
        }
    }; // Mutex released here — before any network I/O

    eprintln!("[query_a2s] creating A2SClient …");
    let client = a2s::A2SClient::new()
        .await
        .map_err(|e| format!("A2SClient::new failed: {e}"))?;

    eprintln!("[query_a2s] querying info …");
    let info = client
        .info(&query_addr)
        .await
        .map_err(|e| format!("A2S info failed for {query_addr}: {e}"))?;

    eprintln!(
        "[query_a2s] info ok: name={:?} players={}/{}",
        info.name, info.players, info.max_players
    );

    // Players query with a fresh client (socket is stateful — sharing causes InvalidResponse)
    let client2 = a2s::A2SClient::new()
        .await
        .map_err(|e| format!("A2SClient::new (players) failed: {e}"))?;
    let players_list = match client2.players(&query_addr).await {
        Ok(pl) => {
            eprintln!("[query_a2s] players ok: {} entries", pl.len());
            pl.into_iter()
                .filter(|p| !p.name.is_empty())
                .map(|p| A2sPlayerDto {
                    name: p.name.clone(),
                    score: p.score,
                    duration: p.duration,
                })
                .collect()
        }
        Err(e) => {
            eprintln!("[query_a2s] players failed (ok for DayZ): {e}");
            vec![]
        }
    };

    let query_port = query_addr
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(port);

    Ok(A2sDetailsDto {
        server_name: info.name,
        game: info.game,
        players: info.players,
        max_players: info.max_players,
        map: info.map,
        version: info.version,
        players_list,
        mods: mods_dto,
        query_port,
    })
}

/// Fetch an image URL through Rust (bypasses WebKit TLS cert validation) and
/// return it as a `data:<mime>;base64,<bytes>` URI ready for use in <img src>.
#[tauri::command]
async fn fetch_image(url: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .split(';')
        .next()
        .unwrap_or("image/jpeg")
        .trim()
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", content_type, b64))
}

/// Fetch the latest news articles.
#[tauri::command]
async fn fetch_news(state: State<'_, SharedState>) -> Result<Vec<ArticleDto>, String> {
    let client = {
        let state = state.lock().await;
        state.ctl.http_client().clone()
    };
    let articles = news::fetch_news(&client).await.map_err(|e| e.to_string())?;
    Ok(articles
        .iter()
        .map(|a| ArticleDto {
            title: a.title.clone(),
            slug: a.slug.clone(),
            excerpt: a.excerpt.clone(),
            content_text: a.html_to_text(),
            content_html: a.content_html(),
            date: a.date().to_string(),
            url: a.url(),
            image_url: a.image.as_ref().map(|img| {
                format!(
                    "https://dayz.com/app-static/uploads/article/{}/{}",
                    a.id, img
                )
            }),
            category: a.category.as_ref().map(|c| c.name.clone()),
            author: a.author.as_ref().map(|au| au.name.clone()),
        })
        .collect())
}

/// Get app stats.
#[tauri::command]
async fn get_app_stats(state: State<'_, SharedState>) -> Result<AppStatsDto, String> {
    let state = state.lock().await;
    let total_players: i64 = state.servers.iter().map(|s| s.players).sum();
    Ok(AppStatsDto {
        server_count: state.servers.len(),
        total_players,
        player_name: state.ctl.profile().player.clone(),
        steam_login: state.ctl.steamcmd_login().map(|s| s.to_string()),
        has_steamcmd: state.ctl.has_steamcmd(),
        avatar_url: state.cached_avatar.clone(),
    })
}

/// Fetch the Steam avatar for the configured account and cache it as a data: URI.
/// Returns the data URI on success, or an error string.
#[tauri::command]
async fn fetch_steam_avatar(state: State<'_, SharedState>) -> Result<Option<String>, String> {
    // Read credentials without holding the lock during network I/O
    let (api_key, steam_id) = {
        let s = state.lock().await;
        (
            s.ctl.profile().steam_api_key.clone(),
            s.ctl.profile().steam_id.clone(),
        )
    };

    let (api_key, steam_id) = match (api_key, steam_id) {
        (Some(k), Some(id)) if !k.is_empty() && !id.is_empty() => (k, id),
        _ => {
            // Clear any stale cached avatar when credentials are missing
            state.lock().await.cached_avatar = None;
            return Ok(None);
        }
    };

    let url = format!(
        "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/?key={}&steamids={}",
        api_key, steam_id
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| e.to_string())?;

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    let avatar_img_url = resp["response"]["players"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p["avatarmedium"].as_str())
        .map(|s: &str| s.to_string());

    let data_uri = match avatar_img_url {
        None => None,
        Some(img_url) => {
            // Download and convert to data URI so WebKit can display it
            let img_resp = client.get(&img_url).send().await.map_err(|e| e.to_string())?;
            let content_type = img_resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("image/jpeg")
                .split(';')
                .next()
                .unwrap_or("image/jpeg")
                .trim()
                .to_string();
            let bytes = img_resp.bytes().await.map_err(|e| e.to_string())?;
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Some(format!("data:{};base64,{}", content_type, b64))
        }
    };

    state.lock().await.cached_avatar = data_uri.clone();
    Ok(data_uri)
}

/// Fetch Steam player count for DayZ.
#[tauri::command]
async fn fetch_steam_player_count(state: State<'_, SharedState>) -> Result<u32, String> {
    let client = {
        let state = state.lock().await;
        state.ctl.http_client().clone()
    };
    api::fetch_steam_player_count(&client)
        .await
        .map_err(|e| e.to_string())
}

/// Get available offline missions.
#[tauri::command]
async fn get_offline_missions(state: State<'_, SharedState>) -> Result<Vec<String>, String> {
    let (dayz_path, client) = {
        let state = state.lock().await;
        let path = state.ctl.dayz_path().map_err(|e| e.to_string())?;
        let client = state.ctl.http_client().clone();
        (path, client)
    };
    let om = OfflineMode::new(dayz_path, client);
    tokio::task::spawn_blocking(move || om.get_available_missions())
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())
}

/// Download/update DayZCommunityOfflineMode.
#[tauri::command]
async fn update_offline_mode(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    let (dayz_path, client) = {
        let state = state.lock().await;
        let path = state.ctl.dayz_path().map_err(|e| e.to_string())?;
        let client = state.ctl.http_client().clone();
        (path, client)
    };
    let om = OfflineMode::new(dayz_path, client);
    tokio::spawn(async move {
        match om.update().await {
            Ok(()) => {
                let _ = app.emit("offline-mode-updated", ());
            }
            Err(e) => {
                let _ = app.emit("offline-mode-error", e.to_string());
            }
        }
    });
    Ok(())
}

/// Launch an offline mission.
#[tauri::command]
async fn launch_offline_mission(
    mission: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (dayz_path, client, player) = {
        let state = state.lock().await;
        let path = state.ctl.dayz_path().map_err(|e| e.to_string())?;
        let client = state.ctl.http_client().clone();
        let player = state.ctl.profile().player.clone();
        (path, client, player)
    };

    let om = OfflineMode::new(dayz_path, client);
    let dayz_args = om.build_launch_args(&mission, &[], false);
    let steam_args = dayz_community_hub_core::launch::build_steam_applaunch_args(
        dayz_community_hub_core::steamcmd::DAYZ_GAME_ID,
        &dayz_args,
        player.as_deref(),
    );

    if let Err(e) = dayz_community_hub_core::steamcmd::SteamClient::start() {
        return Err(format!("Could not start Steam: {}", e));
    }

    let steam_bin = dayz_community_hub_core::steamcmd::SteamClient::steam_exe();
    std::process::Command::new(steam_bin)
        .args(&steam_args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Start background pinging. Streams results via Channel.
/// Pings specific endpoints (favorites, history, and visible servers).
#[tauri::command]
async fn start_pinging(
    targets: Vec<String>, // "ip:port" strings to ping (sent by frontend)
    on_result: Channel<PingResultDto>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    // Deduplicate and collect endpoints
    let mut endpoints: Vec<(String, i64)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Add explicitly requested targets first
    for target in &targets {
        if seen.insert(target.clone()) {
            if let Some((ip, port_str)) = target.rsplit_once(':') {
                if let Ok(port) = port_str.parse::<i64>() {
                    endpoints.push((ip.to_string(), port));
                }
            }
        }
    }

    // Add first 300 servers from state (if not already in targets)
    {
        let state = state.lock().await;
        for s in state.servers.iter().take(300) {
            let key = format!("{}:{}", s.endpoint.ip, s.endpoint.port);
            if seen.insert(key) {
                endpoints.push((s.endpoint.ip.clone(), s.endpoint.port));
            }
        }
    }

    let state_arc = state.inner().clone();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(50)); // max 50 concurrent pings

    tokio::spawn(async move {
        let mut handles = Vec::new();
        for (ip, port) in endpoints {
            let ip_clone = ip.clone();
            let on_result_clone = on_result.clone();
            let state_clone = state_arc.clone();
            let permit = semaphore.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit.acquire().await;
                // Build a minimal Server so we can call a2s_query::ping_server,
                // exactly like the TUI does — ICMP echo, no TCP, no UDP A2S.
                let server = api::Server {
                    endpoint: dayz_community_hub_core::Endpoint {
                        ip: ip_clone.clone(),
                        port,
                    },
                    ..Default::default()
                };
                if let Ok(ms) = a2s_query::ping_server(&server).await {
                    let key = format!("{}:{}", ip_clone, port);
                    {
                        let mut state = state_clone.lock().await;
                        state.ping_cache.insert(key, ms);
                    }
                    let _ = on_result_clone.send(PingResultDto {
                        ip: ip_clone,
                        port,
                        ms,
                    });
                }
            });
            handles.push(handle);
        }
        // Wait for all pings to complete
        for h in handles {
            let _ = h.await;
        }
    });

    Ok(())
}

/// Check if a server is in favorites.
#[tauri::command]
async fn is_favorite(ip: String, port: u16, state: State<'_, SharedState>) -> Result<bool, String> {
    let state = state.lock().await;
    Ok(state.ctl.profile().is_favorite(&ip, port))
}

/// Setup mod symlinks for a server.
#[tauri::command]
async fn setup_mod_symlinks(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let (server, ctl_clone) = {
        let state = state.lock().await;
        let server = state
            .servers
            .iter()
            .find(|s| s.endpoint.ip == ip && s.endpoint.port == port)
            .cloned()
            .ok_or_else(|| "Server not found".to_string())?;
        let ctl_clone = state.ctl.clone_for_launch();
        (server, ctl_clone)
    };
    tokio::task::spawn_blocking(move || ctl_clone.setup_mod_symlinks(&server).map(|_| ()))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| e.to_string())
}

/// Find a server in the list by ip. Returns the full DTO if found.
/// Used by favorites/history to check if a server is online.
#[tauri::command]
async fn find_server(
    ip: String,
    port: u16,
    state: State<'_, SharedState>,
) -> Result<Option<ServerSlimDto>, String> {
    let state = state.lock().await;
    let server = state
        .servers
        .iter()
        .find(|s| {
            s.endpoint.ip == ip && (s.endpoint.port as u16 == port || s.game_port as u16 == port)
        })
        .map(server_to_slim_dto);
    Ok(server)
}

// ─── Profile export / import / reset ─────────────────────────────────────────

/// Bundle format written inside the zstd stream.
#[derive(serde::Serialize, serde::Deserialize)]
struct ProfileBundle {
    /// Format version — bump if the bundle layout changes.
    version: u8,
    /// Contents of profile.json
    profile: serde_json::Value,
    /// Contents of mods.json (optional — absent if file doesn't exist)
    #[serde(skip_serializing_if = "Option::is_none")]
    mods: Option<serde_json::Value>,
}

/// Export profile + mods as a zstd-compressed bundle written to `path`.
#[tauri::command]
async fn export_profile(path: String) -> Result<(), String> {
    let data_dir = config::default_data_dir();

    // Read profile.json
    let profile_path = data_dir.join("profile.json");
    let profile_json: serde_json::Value = {
        let raw = std::fs::read_to_string(&profile_path)
            .map_err(|e| format!("Cannot read profile.json: {e}"))?;
        serde_json::from_str(&raw).map_err(|e| format!("profile.json parse error: {e}"))?
    };

    // Read mods.json (optional)
    let mods_path = data_dir.join("mods.json");
    let mods_json: Option<serde_json::Value> = if mods_path.exists() {
        let raw = std::fs::read_to_string(&mods_path)
            .map_err(|e| format!("Cannot read mods.json: {e}"))?;
        Some(serde_json::from_str(&raw).map_err(|e| format!("mods.json parse error: {e}"))?)
    } else {
        None
    };

    let bundle = ProfileBundle { version: 1, profile: profile_json, mods: mods_json };
    let json_bytes = serde_json::to_vec(&bundle).map_err(|e| e.to_string())?;

    // Compress with zstd (level 9)
    let compressed = zstd::encode_all(std::io::Cursor::new(&json_bytes), 9)
        .map_err(|e| format!("zstd compression failed: {e}"))?;

    std::fs::write(&path, &compressed)
        .map_err(|e| format!("Cannot write export file: {e}"))?;

    Ok(())
}

/// Import a profile bundle from `path` (zstd-compressed JSON), overwriting current profile + mods.
/// Returns the loaded profile DTO so the frontend can refresh immediately.
#[tauri::command]
async fn import_profile(
    path: String,
    state: State<'_, SharedState>,
) -> Result<ProfileDto, String> {
    let compressed = std::fs::read(&path)
        .map_err(|e| format!("Cannot read import file: {e}"))?;

    let json_bytes = zstd::decode_all(std::io::Cursor::new(&compressed))
        .map_err(|e| format!("zstd decompression failed: {e}"))?;

    let bundle: ProfileBundle = serde_json::from_slice(&json_bytes)
        .map_err(|e| format!("Bundle parse error: {e}"))?;

    if bundle.version != 1 {
        return Err(format!("Unsupported bundle version {}", bundle.version));
    }

    let data_dir = config::default_data_dir();
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    // Write profile.json
    let profile_str = serde_json::to_string_pretty(&bundle.profile)
        .map_err(|e| e.to_string())?;
    std::fs::write(data_dir.join("profile.json"), &profile_str)
        .map_err(|e| format!("Cannot write profile.json: {e}"))?;

    // Write mods.json if present
    if let Some(mods) = &bundle.mods {
        let mods_str = serde_json::to_string_pretty(mods).map_err(|e| e.to_string())?;
        std::fs::write(data_dir.join("mods.json"), &mods_str)
            .map_err(|e| format!("Cannot write mods.json: {e}"))?;
    }

    // Reload profile in state
    let profile_path = config::default_profile_path();
    let mut state = state.lock().await;
    state.ctl.reload_profile(&profile_path).map_err(|e| e.to_string())?;
    Ok(profile_to_dto(state.ctl.profile()))
}

/// Reset the profile to factory defaults (keeps mods.json untouched).
#[tauri::command]
async fn reset_profile(state: State<'_, SharedState>) -> Result<ProfileDto, String> {
    let profile_path = config::default_profile_path();
    // Build a fresh profile and save it
    let mut fresh = config::Profile::default_with_version("0.1.0");
    fresh.path = profile_path.clone();
    fresh.save().map_err(|e| e.to_string())?;
    // Reload into state
    let mut state = state.lock().await;
    state.ctl.reload_profile(&profile_path).map_err(|e| e.to_string())?;
    Ok(profile_to_dto(state.ctl.profile()))
}

// ─── Application entry point ──────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            initialize,
            get_servers,
            get_server_details,
            refresh_servers,
            get_ping,
            get_profile,
            save_profile_settings,
            add_favorite,
            remove_favorite,
            remove_history_entry,
            clear_history,
            get_installed_mods,
            delete_mod,
            toggle_mod_managed,
            cleanup_mods,
            get_missing_mods,
            toggle_launch_option,
            set_launch_option_value,
            launch_server,
            launch_direct,
            start_mod_operation,
            query_a2s,
            fetch_image,
            fetch_steam_avatar,
            check_mod_updates,
            fetch_news,
            get_app_stats,
            fetch_steam_player_count,
            get_offline_missions,
            update_offline_mode,
            launch_offline_mission,
            start_pinging,
            is_favorite,
            setup_mod_symlinks,
            find_server,
            export_profile,
            import_profile,
            reset_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
