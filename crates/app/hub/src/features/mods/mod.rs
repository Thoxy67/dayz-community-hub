//! Installed workshop mods: list them, check them against the Workshop,
//! delete them, link and unlink them, and open their folders.

use crate::Hub;
use dz_game::mods::{self, DeleteOutcome, InstalledMod, ModSource};
use futures_util::StreamExt;
use rustc_hash::FxHashMap;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{HttpResultExt, send_ok};
use crate::error::{ResultExt, spawn_blocking_mapped};

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
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

/// Keep a read of the mods on disk, and the Workshop's answer if one came:
/// the browser's flags are written again only when either changed, so the
/// rechecks while a deletion settles (every few seconds) cost the browser
/// nothing.
fn set_mods_known(
    s: &mut crate::state::AppState,
    disk: FxHashMap<u64, i64>,
    remote: Option<FxHashMap<u64, i64>>,
) {
    let changed = s.mods_on_disk.as_ref() != Some(&disk)
        || remote.as_ref().is_some_and(|r| *r != s.mod_update_cache);
    s.mods_on_disk = Some(disk);
    if let Some(r) = remote {
        s.mod_update_cache = r;
    }
    if changed {
        s.profile_changed();
    }
}

/// How long a read of the mods on disk serves `check_mod_updates`.
const SCAN_FRESH: Duration = Duration::from_secs(10);

/// The last read of the mods on disk, if it is recent enough to use again.
fn recent_scan(scan: Option<&(Instant, Arc<Vec<InstalledMod>>)>) -> Option<Arc<Vec<InstalledMod>>> {
    scan.filter(|(at, _)| at.elapsed() < SCAN_FRESH)
        .map(|(_, mods)| Arc::clone(mods))
}

/// The mods on disk changed (deleted, made managed, cleaned up, an
/// operation ran): the next check reads them again.
pub(crate) async fn mods_changed(state: &crate::state::SharedState) {
    state.write().await.mod_scan = None;
}

/// The Workshop's keyless details endpoint: a form POST, hundreds of ids
/// per request, the same `time_updated` as `IPublishedFileService/GetDetails`.
const REMOTE_STORAGE_DETAILS: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";
/// Ids per keyless request (Steam takes a few hundred).
const KEYLESS_CHUNK: usize = 250;
/// Ids per keyed `GetDetails` request.
const KEYED_CHUNK: usize = 100;
/// Requests in flight at once, for a user with a great many mods.
const WEB_CONCURRENT: usize = 4;

/// `(id, time_updated)` of each item of a details answer (either endpoint:
/// both answer `response.publishedfiledetails[]`). An item Steam could not
/// give (private, removed: `result` other than 1) is left out.
fn parse_details(resp: &serde_json::Value) -> Vec<(u64, i64)> {
    let Some(files) = resp["response"]["publishedfiledetails"].as_array() else {
        return Vec::new();
    };
    files
        .iter()
        .filter(|f| f["result"].as_i64().is_none_or(|r| r == 1))
        .filter_map(|f| {
            let id = &f["publishedfileid"];
            let id = id
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .or_else(|| id.as_u64())?;
            let ts = f["time_updated"].as_i64().filter(|&t| t > 0)?;
            Some((id, ts))
        })
        .collect()
}

/// The keyless request's form body for `ids`.
fn keyless_form(ids: &[u64]) -> String {
    let mut body = format!("itemcount={}", ids.len());
    for (i, id) in ids.iter().enumerate() {
        // `publishedfileids[i]`, its brackets percent-encoded.
        body.push_str(&format!("&publishedfileids%5B{i}%5D={id}"));
    }
    body
}

/// One request of [`updates_from_web`]: the keyless endpoint, or `GetDetails`
/// with a key.
async fn details_chunk(chunk: &[u64], key: Option<&str>) -> Result<Vec<(u64, i64)>, String> {
    let client = crate::net::api();
    let req = match key {
        None => client
            .post(REMOTE_STORAGE_DETAILS)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(keyless_form(chunk)),
        Some(key) => {
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
            client
                .get("https://api.steampowered.com/IPublishedFileService/GetDetails/v1/")
                .query(&params)
        }
    };
    let resp = send_ok(req)
        .await
        .http_err("Steam Workshop")?
        .json::<serde_json::Value>()
        .await
        .http_err("Steam Workshop")?;
    Ok(parse_details(&resp))
}

/// `time_updated` of each of `ids`, from the Workshop's Web API: keyless
/// (`ISteamRemoteStorage`) when `key` is `None`, else `GetDetails` with it.
/// The chunks go a few at once: they are independent and the shared client
/// pools connections.
async fn updates_from_web(ids: &[u64], key: Option<&str>) -> Result<FxHashMap<u64, i64>, String> {
    let size = if key.is_some() {
        KEYED_CHUNK
    } else {
        KEYLESS_CHUNK
    };
    // Owned chunks: a stream of borrowed slices trips the `Send` check of the
    // window's task.
    let chunks: Vec<Vec<u64>> = ids.chunks(size).map(<[u64]>::to_vec).collect();
    let chunk_results: Vec<Result<Vec<(u64, i64)>, String>> = futures_util::stream::iter(chunks)
        .map(|chunk| async move { details_chunk(&chunk, key).await })
        .buffer_unordered(WEB_CONCURRENT)
        .collect()
        .await;

    let mut remote_map: FxHashMap<u64, i64> = FxHashMap::default();
    for chunk in chunk_results {
        remote_map.extend(chunk?);
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
            let Some(srv) = dz_api::addr(ip, i64::from(port)).and_then(|a| s.find_flexible(a))
            else {
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

        let mods = Arc::new(spawn_blocking_mapped(move || ctl_clone.get_installed_mods()).await?);
        {
            let mut s = state.write().await;
            set_mods_known(&mut s, on_disk(&mods), None);
            s.mod_scan = Some((Instant::now(), Arc::clone(&mods)));
        }

        Ok(mods
            .iter()
            .map(|m| installed_mod_to_dto(m, &update_cache))
            .collect())
    }

    /// Fetch `time_updated` for all installed mods and cache the results:
    /// from the Workshop's keyless Web API, else `GetDetails` with the
    /// profile's key, and only when the Web API cannot be reached, from the
    /// Steam client. Returns the enriched mod list (same as `get_installed_mods`).
    pub async fn check_mod_updates(&self) -> Result<Vec<InstalledModDto>, String> {
        mocked!(self, w => Ok(w.installed_mods()));
        let state = self.state()?;
        let (ctl_clone, recent) = {
            let s = state.read().await;
            (s.ctl.clone_for_task(), recent_scan(s.mod_scan.as_ref()))
        };

        // The window reads the disk (`get_installed_mods`) just before it
        // asks: that read serves, unless it is old or the mods changed since.
        let installed_mods = match recent {
            Some(mods) => mods,
            None => Arc::new(
                spawn_blocking_mapped(move || ctl_clone.get_installed_mods())
                    .await
                    .unwrap_or_default(),
            ),
        };

        if installed_mods.is_empty() {
            let mut s = state.write().await;
            set_mods_known(&mut s, FxHashMap::default(), None);
            return Ok(vec![]);
        }

        let mod_ids: Vec<u64> = installed_mods.iter().map(|m| m.id).collect();
        // The keyless Web API answers everyone in one request or a few; with a
        // key, `GetDetails` is a second try. The Steam client answers the same,
        // at the cost of a worker process and a moment of Steam showing DayZ
        // running, so it is the last resort.
        let remote_map = match updates_from_web(&mod_ids, None).await {
            Ok(map) => map,
            Err(keyless) => {
                let keyed = match crate::features::steam::access::web_key(&state).await {
                    Some(key) => updates_from_web(&mod_ids, Some(&key)).await,
                    None => Err(keyless),
                };
                match keyed {
                    Ok(map) => map,
                    Err(web) => updates_from_steam(mod_ids)
                        .await
                        .map_err(|steam| format!("{web} {steam}"))?,
                }
            }
        };

        let dtos = installed_mods
            .iter()
            .map(|m| installed_mod_to_dto(m, &remote_map))
            .collect();
        {
            let mut s = state.write().await;
            set_mods_known(&mut s, on_disk(&installed_mods), Some(remote_map));
            s.mod_scan = Some((Instant::now(), installed_mods));
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
        let kept = spawn_blocking_mapped(move || -> std::result::Result<Vec<u64>, String> {
            // The folders (Steam's libraries read from its .vdf) are worked out
            // once for every mod, not once per mod.
            let dirs = ctl_clone.mod_dirs();
            let dayz = ctl_clone.dayz_path().ok();
            let mut deleted = Vec::new();
            let mut in_steam = Vec::new();
            let mut failed = None;
            for (id, r) in mods::delete_mods(&dirs, dayz.as_deref(), &mod_ids) {
                match r.cmd_err() {
                    Ok(outcome) => {
                        deleted.push(id);
                        if outcome == DeleteOutcome::KeptInSteam {
                            in_steam.push(id);
                        }
                    }
                    Err(e) => failed = Some(e),
                }
            }
            // Even after a failure: what was deleted must not come back.
            let steam = dz_game::steam_download::unsubscribe(&deleted);
            // Copies Steam does not manage (not subscribed) go now; Steam
            // removes the unsubscribed ones itself.
            let loose: Vec<u64> = in_steam
                .iter()
                .copied()
                .filter(|id| !steam.unsubscribed.contains(id) && steam.not_subscribed.contains(id))
                .collect();
            let mut gone = Vec::new();
            for (id, r) in mods::delete_steam_copies_of(&dirs, &loose) {
                match r {
                    Ok(()) => gone.push(id),
                    Err(e) => eprintln!("[mods] Steam copy of {id} not deleted: {e}"),
                }
            }
            in_steam.retain(|id| !steam.unsubscribed.contains(id) && !gone.contains(id));
            if let Some(e) = failed {
                return Err(e);
            }
            Ok(in_steam)
        })
        .await;
        mods_changed(&state).await;
        kept
    }

    /// Toggle managed status of a mod.
    pub async fn toggle_mod_managed(&self, mod_id: u64) -> Result<bool, String> {
        mocked!(self, w => Ok(w.toggled_managed(mod_id)));
        let state = self.state()?;
        let ctl_clone = { state.read().await.ctl.clone_for_task() };
        let managed = spawn_blocking_mapped(move || ctl_clone.toggle_mod_managed(mod_id)).await;
        mods_changed(&state).await;
        managed
    }

    /// Delete every mod the launcher downloaded and every `@` link; mods in
    /// Steam libraries stay.
    pub async fn cleanup_mods(&self) -> Result<String, String> {
        mocked!(self => Ok("Removed 0 mods (0 B) and 0 symlinks".to_string()));
        let state = self.state()?;
        let ctl_clone = { state.read().await.ctl.clone_for_task() };
        let stats = spawn_blocking_mapped(move || ctl_clone.cleanup_mods()).await;
        mods_changed(&state).await;
        let stats = stats?;
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
    pub async fn setup_mod_symlinks(&self, addr: dz_api::SocketAddrV4) -> Result<(), String> {
        mocked!(self => Ok(()));
        let state = self.state()?;
        let (server, ctl) = {
            let state = state.read().await;
            let server = state
                .find_by_query_port(addr)
                .cloned()
                .ok_or_else(|| "Server not found".to_string())?;
            (server, state.ctl.clone_for_task())
        };
        let linked =
            spawn_blocking_mapped(move || ctl.setup_mod_symlinks(&server).map(|_| ())).await;
        mods_changed(&state).await;
        linked
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod scan_tests {
    use super::*;

    #[test]
    fn details_keep_answered_items_only() {
        let resp = serde_json::json!({"response": {"result": 1, "resultcount": 4,
        "publishedfiledetails": [
            {"publishedfileid": "1559212036", "result": 1, "time_updated": 1700000000},
            {"publishedfileid": "1564026768", "result": 9},
            {"publishedfileid": "2", "result": 1, "time_updated": 0},
            {"publishedfileid": 3, "time_updated": 42},
        ]}});
        assert_eq!(
            parse_details(&resp),
            [(1_559_212_036, 1_700_000_000), (3, 42)]
        );
        assert!(parse_details(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn the_keyless_form_numbers_every_id() {
        assert_eq!(
            keyless_form(&[10, 20]),
            "itemcount=2&publishedfileids%5B0%5D=10&publishedfileids%5B1%5D=20"
        );
    }

    #[test]
    fn a_recent_read_serves_and_an_old_one_does_not() {
        let mods = Arc::new(Vec::new());
        assert!(recent_scan(None).is_none(), "nothing known: read the disk");
        let now = (Instant::now(), Arc::clone(&mods));
        assert!(recent_scan(Some(&now)).is_some_and(|m| Arc::ptr_eq(&m, &mods)));
        if let Some(past) = Instant::now().checked_sub(SCAN_FRESH + Duration::from_secs(1)) {
            assert!(recent_scan(Some(&(past, mods))).is_none());
        }
    }
}
