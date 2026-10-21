//! Installed workshop mods: list them, check them against the Workshop,
//! delete them, link and unlink them, and open their folders.

use crate::Hub;
use dz_game::mods::{self, DeleteOutcome, InstalledMod, ModSource};
use futures_util::StreamExt;
use rustc_hash::FxHashMap;
use serde::Serialize;

use crate::error::{HttpResultExt, send_ok};
use crate::error::{ResultExt, spawn_blocking_mapped};

#[derive(Serialize, Clone, Debug)]
pub struct InstalledModDto {
    pub name: String,
    pub id: u64,
    /// When steamcmd last wrote the mod (Unix seconds).
    pub local_updated: i64,
    pub size: u64,
    pub size_human: String,
    /// An `@<id>` link in the DayZ directory loads it.
    pub managed: bool,
    /// Whose folder the copy in use is in: the launcher's, which it updates
    /// and deletes, or a Steam library's, which it only reads.
    pub source: ModSourceDto,
    /// The copy's directory.
    pub path: String,
    /// Another, older copy is in the other kind of folder.
    pub other_copy: bool,
    /// `time_updated` on the Workshop; null until checked.
    pub remote_updated: Option<i64>,
    /// True when `remote_updated > local_updated`.
    pub update_available: bool,
}

/// Whose folder a mod is in.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModSourceDto {
    Launcher,
    Steam,
}

impl From<ModSource> for ModSourceDto {
    fn from(s: ModSource) -> Self {
        match s {
            ModSource::Launcher => Self::Launcher,
            ModSource::Steam => Self::Steam,
        }
    }
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
        source: m.source.into(),
        path: m.path.to_string_lossy().into_owned(),
        other_copy: m.other_copy,
        remote_updated,
        update_available: remote_updated.is_some_and(|r| r > m.local_updated),
    }
}

/// Which saved servers run a mod: the favourites, and the servers played
/// recently (history), by name.
#[derive(Serialize, Clone, Debug)]
pub struct ModUsageDto {
    pub id: u64,
    pub favorites: Vec<String>,
    pub history: Vec<String>,
}

/// Each mod's id and `local_updated`, for [`crate::state::AppState::mods_on_disk`].
fn on_disk(mods: &[InstalledMod]) -> FxHashMap<u64, i64> {
    mods.iter().map(|m| (m.id, m.local_updated)).collect()
}

/// `time_updated` of each of `ids`, from the Workshop's Web API.
async fn updates_from_web(ids: &[u64], key: &str) -> Result<FxHashMap<u64, i64>, String> {
    let client = crate::net::api();

    // Query the Workshop API for each 100-id chunk concurrently — the chunks are
    // independent and the shared client pools connections, so for users with
    // several hundred mods this turns N serial round-trips into a few parallel
    // waves instead.
    let chunks: Vec<Vec<u64>> = ids.chunks(100).map(<[u64]>::to_vec).collect();
    let chunk_results: Vec<Result<Vec<(u64, i64)>, String>> = futures_util::stream::iter(chunks)
        .map(|chunk| async move {
            let mut params: Vec<(String, String)> = vec![("key".to_string(), key.to_owned())];
            for flag in [
                "includetags",
                "includeadditionalpreviews",
                "includechildren",
                "includesummary",
                "includevotes",
            ] {
                params.push((flag.to_string(), "0".to_string()));
            }
            for (i, id) in chunk.iter().enumerate() {
                params.push((format!("publishedfileids[{i}]"), id.to_string()));
            }

            let resp: serde_json::Value = send_ok(
                client
                    .get("https://api.steampowered.com/IPublishedFileService/GetDetails/v1/")
                    .query(&params),
            )
            .await
            .http_err("Steam Workshop")?
            .json::<serde_json::Value>()
            .await
            .http_err("Steam Workshop")?;

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
    Ok(remote_map)
}

/// The same, from the Steam client.
async fn updates_from_steam(ids: Vec<u64>) -> Result<FxHashMap<u64, i64>, String> {
    let details = tokio::task::spawn_blocking(move || dz_steamworks::workshop_details(&ids))
        .await
        .map_err(|e| e.to_string())??;
    Ok(details
        .into_iter()
        .filter(|d| d.updated > 0)
        .map(|d| (d.id, i64::from(d.updated)))
        .collect())
}

impl Hub {
    /// For each mod run by a favourite or a server in the history, which ones,
    /// from the server list's mod lists. A server not in the list counts for
    /// nothing: its mods are not known.
    pub async fn mods_usage(&self) -> Result<Vec<ModUsageDto>, String> {
        mocked!(self, w => w.mods_usage_now(self).await);
        let state = self.state()?;
        let s = state.read().await;
        let p = s.ctl.profile();
        let mut by: FxHashMap<u64, ModUsageDto> = FxHashMap::default();
        let mut add = |ip: &str, port: u16, name: &str, favorite: bool| {
            let Some(srv) = s.find_flexible(ip, i64::from(port)) else {
                return;
            };
            for m in &srv.mods {
                let id = m.steam_workshop_id as u64;
                let u = by.entry(id).or_insert_with(|| ModUsageDto {
                    id,
                    favorites: Vec::new(),
                    history: Vec::new(),
                });
                let list = if favorite {
                    &mut u.favorites
                } else {
                    &mut u.history
                };
                if !list.iter().any(|n| n == name) {
                    list.push(name.to_string());
                }
            }
        };
        for f in &p.favorites {
            add(&f.ip, f.port, &f.name, true);
        }
        for h in &p.history {
            add(&h.ip, h.port, &h.name, false);
        }
        Ok(by.into_values().collect())
    }

    /// Get installed mods. Uses spawn_blocking for filesystem scan.
    /// Enriches each mod with `remote_updated` / `update_available` from the in-memory cache.
    pub async fn get_installed_mods(&self) -> Result<Vec<InstalledModDto>, String> {
        mocked!(self, w => Ok(w.installed_mods()));
        let state = self.state()?;
        let (ctl_clone, update_cache) = {
            let s = state.read().await;
            (s.ctl.clone_for_task(), s.mod_update_cache.clone())
        };

        let mods = spawn_blocking_mapped(move || ctl_clone.get_installed_mods()).await?;
        {
            let mut s = state.write().await;
            s.mods_on_disk = Some(on_disk(&mods));
            s.profile_changed();
        }

        Ok(mods
            .iter()
            .map(|m| installed_mod_to_dto(m, &update_cache))
            .collect())
    }

    /// Fetch `time_updated` for all installed mods (from the Workshop's Web API
    /// with a key, else or failing that from the Steam client) and cache the
    /// results. Returns the enriched mod list (same as `get_installed_mods`).
    pub async fn check_mod_updates(&self) -> Result<Vec<InstalledModDto>, String> {
        mocked!(self, w => Ok(w.installed_mods()));
        let state = self.state()?;
        let ctl_clone = state.read().await.ctl.clone_for_task();
        let api_key = crate::features::steam::access::web_key(&state).await;

        let installed_mods = spawn_blocking_mapped(move || ctl_clone.get_installed_mods())
            .await
            .unwrap_or_default();

        if installed_mods.is_empty() {
            let mut s = state.write().await;
            s.mods_on_disk = Some(FxHashMap::default());
            s.profile_changed();
            return Ok(vec![]);
        }

        let mod_ids: Vec<u64> = installed_mods.iter().map(|m| m.id).collect();
        // The Web API needs a key (401 without); the Steam client answers the
        // same, at the cost of a moment of Steam showing DayZ running.
        let remote_map = match api_key {
            Some(key) => match updates_from_web(&mod_ids, &key).await {
                Ok(map) => map,
                Err(web) => updates_from_steam(mod_ids)
                    .await
                    .map_err(|steam| format!("{web} {steam}"))?,
            },
            None => updates_from_steam(mod_ids).await?,
        };

        let dtos = installed_mods
            .iter()
            .map(|m| installed_mod_to_dto(m, &remote_map))
            .collect();
        {
            let mut s = state.write().await;
            s.mod_update_cache = remote_map;
            s.mods_on_disk = Some(on_disk(&installed_mods));
            s.profile_changed();
        }
        Ok(dtos)
    }

    /// Delete a mod by ID: the launcher's copy and its link, and the Steam
    /// account's subscription to it (Steam then removes its own copy). Returns
    /// true when a copy in a Steam library remains: Steam was not running, or
    /// would not unsubscribe (the launcher never deletes in a Steam library).
    pub async fn delete_mod(&self, mod_id: u64) -> Result<bool, String> {
        self.delete_mods_bulk(vec![mod_id])
            .await
            .map(|kept| !kept.is_empty())
    }

    /// Delete several mods by ID, then unsubscribe from them in one Steam
    /// session. Returns the ids whose Steam copy remains.
    pub async fn delete_mods_bulk(&self, mod_ids: Vec<u64>) -> Result<Vec<u64>, String> {
        mocked!(self, w => Ok(w.kept_in_steam(&mod_ids)));
        let state = self.state()?;
        let ctl_clone = { state.read().await.ctl.clone_for_task() };
        spawn_blocking_mapped(move || -> std::result::Result<Vec<u64>, String> {
            let mut deleted = Vec::new();
            let mut in_steam = Vec::new();
            let mut failed = None;
            for id in mod_ids {
                match ctl_clone.delete_mod(id).cmd_err() {
                    Ok(outcome) => {
                        deleted.push(id);
                        if outcome == DeleteOutcome::KeptInSteam {
                            in_steam.push(id);
                        }
                    }
                    Err(e) => {
                        failed = Some(e);
                        break;
                    }
                }
            }
            // Even after a failure: what was deleted must not come back.
            let steam = dz_game::steam_download::unsubscribe(&deleted);
            // Copies Steam does not manage (not subscribed) go now; Steam
            // removes the unsubscribed ones itself.
            in_steam.retain(|id| {
                if steam.unsubscribed.contains(id) {
                    return false;
                }
                if steam.not_subscribed.contains(id) {
                    match ctl_clone.delete_steam_copies(*id) {
                        Ok(_) => return false,
                        Err(e) => eprintln!("[mods] Steam copy of {id} not deleted: {e}"),
                    }
                }
                true
            });
            if let Some(e) = failed {
                return Err(e);
            }
            Ok(in_steam)
        })
        .await
    }

    /// Toggle managed status of a mod.
    pub async fn toggle_mod_managed(&self, mod_id: u64) -> Result<bool, String> {
        mocked!(self, w => Ok(w.toggled_managed(mod_id)));
        let state = self.state()?;
        let ctl_clone = { state.read().await.ctl.clone_for_task() };
        spawn_blocking_mapped(move || ctl_clone.toggle_mod_managed(mod_id)).await
    }

    /// Delete every mod the launcher downloaded and every `@` link; mods in
    /// Steam libraries stay.
    pub async fn cleanup_mods(&self) -> Result<String, String> {
        mocked!(self => Ok("Removed 0 mods (0 B) and 0 symlinks".to_string()));
        let state = self.state()?;
        let ctl_clone = { state.read().await.ctl.clone_for_task() };
        let stats = spawn_blocking_mapped(move || ctl_clone.cleanup_mods()).await?;
        Ok(format!(
            "Removed {} mods ({}) and {} symlinks",
            stats.removed_count,
            mods::format_size(stats.removed_size),
            stats.symlinks_removed
        ))
    }

    /// Open the launcher's workshop folder (where SteamCMD downloads) in the
    /// system file manager.
    /// Returns the folder: the window opens it.
    pub async fn open_workshop_dir(&self) -> Result<std::path::PathBuf, String> {
        mocked!(self, w => w.folder("steamcmd-content/steamapps/workshop/content/221100"));
        let state = self.state()?;
        let path = state.read().await.ctl.workshop_path();
        tokio::fs::create_dir_all(&path).await.cmd_err()?;
        Ok(path)
    }

    /// Open the directory of the copy of a mod in use in the system file manager.
    /// Returns the folder: the window opens it.
    pub async fn open_mod_dir(&self, mod_id: u64) -> Result<std::path::PathBuf, String> {
        mocked!(self, w => w.folder(&format!("steamcmd-content/steamapps/workshop/content/221100/{mod_id}")));
        let state = self.state()?;
        let ctl = state.read().await.ctl.clone_for_task();
        let path = spawn_blocking_mapped(move || {
            ctl.mod_dir(mod_id)
                .ok_or_else(|| dz_common::Error::Mod(format!("Mod {mod_id} is not installed")))
        })
        .await?;
        Ok(path)
    }

    /// Create the `@<id>` links a listed server's mods need.
    pub async fn setup_mod_symlinks(&self, ip: String, port: i64) -> Result<(), String> {
        mocked!(self => Ok(()));
        let state = self.state()?;
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
}
