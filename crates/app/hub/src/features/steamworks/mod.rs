//! Downloading mods through the running Steam client (dz-steamworks)
//! instead of SteamCMD: the choice, and whether it can work here.

use crate::Hub;
use dz_game::mods::WorkshopManifest;
use dz_profile::ModDownloader;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::spawn_blocking_mapped;
use crate::state::mutate_profile;

/// What downloads mods.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModDownloaderDto {
    /// SteamCMD, into the launcher's folder (the default).
    Steamcmd,
    /// The running Steam client, into the user's Steam library.
    Steamworks,
}

impl From<ModDownloader> for ModDownloaderDto {
    fn from(d: ModDownloader) -> Self {
        match d {
            ModDownloader::Steamcmd => Self::Steamcmd,
            ModDownloader::Steamworks => Self::Steamworks,
        }
    }
}

impl From<ModDownloaderDto> for ModDownloader {
    fn from(d: ModDownloaderDto) -> Self {
        match d {
            ModDownloaderDto::Steamcmd => Self::Steamcmd,
            ModDownloaderDto::Steamworks => Self::Steamworks,
        }
    }
}

/// Whether downloads through Steam can work on this machine.
#[derive(Serialize, Clone, Debug)]
pub struct SteamworksStatusDto {
    /// Valve's library loaded.
    pub library: bool,
    /// Why it did not, when it did not.
    pub error: Option<String>,
    /// The Steam client is running.
    pub steam_running: bool,
}

/// One Workshop item the Steam account is subscribed to, as Steam has it.
#[derive(Serialize, Clone, Debug)]
pub struct SteamSubscriptionDto {
    pub id: u64,
    pub subscribed: bool,
    pub installed: bool,
    pub needs_update: bool,
    pub downloading: bool,
    pub pending: bool,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

/// What the Workshop says about an item.
#[derive(Serialize, Clone, Debug)]
pub struct WorkshopItemDto {
    pub id: u64,
    pub title: String,
    /// The items it requires ("Required items" on its Workshop page).
    pub requires: Vec<u64>,
}

/// The account's subscriptions, or why they are not known.
#[derive(Serialize, Clone, Debug)]
pub struct SteamSubscriptionsDto {
    /// Steam answered.
    pub available: bool,
    /// Why it did not, when it did not.
    pub reason: Option<String>,
    pub items: Vec<SteamSubscriptionDto>,
    /// Title and requirements of the subscribed items, of the mods asked
    /// about, and of what they require.
    pub details: Vec<WorkshopItemDto>,
}

/// The last answer and when it came: Steam is asked at most this often.
static LAST: std::sync::Mutex<Option<(std::time::Instant, SteamSubscriptionsDto)>> =
    std::sync::Mutex::new(None);
const FRESH: std::time::Duration = std::time::Duration::from_secs(4);
/// Workshop details by id, asked once per run: titles and requirements
/// hardly change, and each question goes to Steam's servers.
static DETAILS: std::sync::Mutex<Option<std::collections::HashMap<u64, WorkshopItemDto>>> =
    std::sync::Mutex::new(None);
/// The ids whose details a session was asked for this run, answered or not:
/// an item gone from the Workshop is not the reason for a session on every
/// visit to the mods view.
static ASKED: std::sync::Mutex<Option<FxHashSet<u64>>> = std::sync::Mutex::new(None);

/// The Workshop's title for `id`, when Steam has been asked about it this run.
pub(crate) fn workshop_title(id: u64) -> Option<String> {
    DETAILS
        .lock()
        .ok()?
        .as_ref()?
        .get(&id)
        .map(|d| d.title.clone())
        .filter(|t| !t.trim().is_empty())
}

/// Every detail known this run.
fn details_known() -> Vec<WorkshopItemDto> {
    DETAILS
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(|d| d.values().cloned().collect()))
        .unwrap_or_default()
}

/// The ids of `ids` no session was asked about yet this run.
fn not_asked(ids: impl Iterator<Item = u64>) -> Vec<u64> {
    let asked = ASKED.lock().ok();
    let asked = asked.as_ref().and_then(|a| a.as_ref());
    let details = DETAILS.lock().ok();
    let known = details.as_ref().and_then(|d| d.as_ref());
    let mut out: Vec<u64> = ids
        .filter(|id| {
            !asked.is_some_and(|a| a.contains(id)) && !known.is_some_and(|d| d.contains_key(id))
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The account's subscriptions as the Steam client's workshop manifests
/// in `workshops` (each library's `steamapps/workshop`) record them: no
/// session, so Steam does not show DayZ running. `None` when no library
/// has a manifest. The bytes of a download are not in them.
fn from_manifests(workshops: &[PathBuf]) -> Option<Vec<SteamSubscriptionDto>> {
    let mut any = false;
    let mut by_id: FxHashMap<u64, SteamSubscriptionDto> = FxHashMap::default();
    for workshop in workshops {
        let Some(m) = WorkshopManifest::read(workshop) else {
            continue;
        };
        any = true;
        for (id, item) in subscriptions_in(&m, workshop) {
            by_id
                .entry(id)
                .and_modify(|had| {
                    had.installed |= item.installed;
                    had.needs_update |= item.needs_update;
                    had.downloading |= item.downloading;
                    had.pending |= item.pending;
                })
                .or_insert(item);
        }
    }
    any.then(|| {
        let mut items: Vec<SteamSubscriptionDto> = by_id.into_values().collect();
        items.sort_unstable_by_key(|i| i.id);
        items
    })
}

/// The subscribed items of one library's manifest. Steam downloads an item
/// into `workshop/downloads/221100/<id>` before it moves it into place: the
/// folder there means it is downloading.
fn subscriptions_in(m: &WorkshopManifest, workshop: &Path) -> Vec<(u64, SteamSubscriptionDto)> {
    let downloads = workshop
        .join("downloads")
        .join(dz_steamcmd::DAYZ_GAME_ID.to_string());
    m.subscribed
        .iter()
        .map(|&id| {
            let installed = m.installed.contains_key(&id);
            let needs_update = m.needs_update.contains(&id);
            let downloading = downloads.join(id.to_string()).is_dir();
            let pending = !downloading && m.needs_download && (!installed || needs_update);
            (
                id,
                SteamSubscriptionDto {
                    id,
                    subscribed: true,
                    installed,
                    needs_update,
                    downloading,
                    pending,
                    bytes_done: 0,
                    bytes_total: 0,
                },
            )
        })
        .collect()
}

/// Ask the running Steam client in a short session (Steam shows DayZ
/// running for a moment): its subscriptions with live states and bytes, and
/// the details of those and of `ids` not known yet.
fn ask_session(ids: &[u64]) -> Result<SteamSubscriptionsDto, String> {
    let known: Vec<u64> = DETAILS
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(|d| d.keys().copied().collect()))
        .unwrap_or_default();
    let (items, found) = dz_steamworks::subscriptions(ids, &known)?;
    if let Ok(mut asked) = ASKED.lock() {
        let asked = asked.get_or_insert_with(Default::default);
        asked.extend(ids.iter().copied());
        asked.extend(items.iter().map(|s| s.id));
    }
    let details = DETAILS
        .lock()
        .map(|mut d| {
            let d = d.get_or_insert_with(Default::default);
            for f in found {
                d.insert(
                    f.id,
                    WorkshopItemDto {
                        id: f.id,
                        title: f.title,
                        requires: f.requires,
                    },
                );
            }
            d.values().cloned().collect()
        })
        .unwrap_or_default();
    Ok(SteamSubscriptionsDto {
        available: true,
        reason: None,
        items: items
            .into_iter()
            .map(|s| SteamSubscriptionDto {
                id: s.id,
                subscribed: s.state.subscribed(),
                installed: s.state.installed(),
                needs_update: s.state.needs_update(),
                downloading: s.state.downloading(),
                pending: s.state.pending(),
                bytes_done: s.bytes.0,
                bytes_total: s.bytes.1,
            })
            .collect(),
        details,
    })
}

impl Hub {
    /// Choose what downloads mods; the next operation uses it.
    pub async fn set_mod_downloader(&self, downloader: ModDownloaderDto) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| {
            s.ctl.profile_mut().mod_downloader = downloader.into();
            Ok(())
        })
        .await
    }

    /// Load Valve's library (the first call writes it to disk) and look for a
    /// running Steam, without connecting to it.
    pub async fn steamworks_status(&self) -> Result<SteamworksStatusDto, String> {
        mocked!(self => Ok(crate::mock::fixtures::steamworks_status()));
        spawn_blocking_mapped(|| {
            let (loaded, steam_running) = dz_steamworks::status();
            Ok::<_, String>(SteamworksStatusDto {
                library: loaded.is_ok(),
                error: loaded.err(),
                steam_running,
            })
        })
        .await
    }

    /// Connect to Steam as DayZ and disconnect at once: rejects with the
    /// sentence a download would fail with.
    pub async fn steamworks_check(&self) -> Result<(), String> {
        mocked!(self => Ok(()));
        // The session lives and dies on this one blocking thread.
        spawn_blocking_mapped(dz_steamworks::check).await
    }

    /// The DayZ Workshop items the Steam account is subscribed to, with what
    /// Steam is doing with each (installed, downloading, waiting), and the
    /// Workshop's details (title, required items) of those and of `ids` (the
    /// mods on disk).
    ///
    /// Read from the workshop manifests the Steam client keeps in each
    /// library, which costs a file read. The running Steam client is asked
    /// in a short session (Steam shows DayZ running for a moment) only for
    /// what the files do not hold: the details of items not asked about yet
    /// this run, and the bytes of a download under way; or for everything
    /// when no library has a manifest. At most every few seconds; while
    /// another session is open (a download through Steam), the files, else
    /// the last answer.
    pub async fn steam_subscriptions(
        &self,
        ids: Vec<u64>,
    ) -> Result<SteamSubscriptionsDto, String> {
        mocked!(self, w => Ok(w.steam_subscriptions()));
        let state = self.state()?;
        let ctl = state.read().await.ctl.clone_for_task();
        spawn_blocking_mapped(move || {
            let last = LAST.lock().map(|l| l.clone()).unwrap_or(None);
            if let Some((at, dto)) = &last
                && at.elapsed() < FRESH
            {
                return Ok::<_, String>(dto.clone());
            }
            let unavailable = |reason: String| SteamSubscriptionsDto {
                available: false,
                reason: Some(reason),
                items: Vec::new(),
                details: Vec::new(),
            };
            let dto = match from_manifests(&ctl.mod_dirs().steam_workshops()) {
                Some(items) => {
                    let active = items.iter().any(|i| i.downloading || i.pending);
                    let unknown = not_asked(ids.iter().copied().chain(items.iter().map(|i| i.id)));
                    let from_files = SteamSubscriptionsDto {
                        available: true,
                        reason: None,
                        items,
                        details: details_known(),
                    };
                    if (active || !unknown.is_empty())
                        && !dz_steamworks::session_open()
                        && dz_steamworks::steam_running()
                    {
                        ask_session(&ids).unwrap_or_else(|e| {
                            eprintln!("[steamworks] subscriptions: {e}");
                            from_files
                        })
                    } else {
                        from_files
                    }
                }
                None => {
                    if !dz_steamworks::steam_running() {
                        return Ok(unavailable("Steam is not running.".into()));
                    }
                    if dz_steamworks::session_open()
                        && let Some((_, dto)) = last
                    {
                        return Ok(dto);
                    }
                    match ask_session(&ids) {
                        Ok(dto) => dto,
                        // A session busy elsewhere: the last answer is still the best.
                        Err(e) => {
                            eprintln!("[steamworks] subscriptions: {e}");
                            match last {
                                Some((_, dto)) => dto,
                                None => unavailable(e),
                            }
                        }
                    }
                }
            };
            if dto.available
                && let Ok(mut l) = LAST.lock()
            {
                *l = Some((std::time::Instant::now(), dto.clone()));
            }
            Ok(dto)
        })
        .await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_says_what_steam_does_with_each_subscription() {
        let root = std::env::temp_dir().join(format!("dzch-acf-subs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let workshop = root.join("steamapps").join("workshop");
        std::fs::create_dir_all(workshop.join("downloads").join("221100").join("3")).unwrap();
        std::fs::write(
            workshop.join(dz_game::mods::MANIFEST),
            r#""AppWorkshop"
{
	"NeedsDownload"		"1"
	"WorkshopItemsInstalled"
	{
		"1" { "size" "10" "timeupdated" "100" }
		"2" { "size" "10" "timeupdated" "100" }
		"5" { "size" "10" "timeupdated" "100" }
	}
	"WorkshopItemDetails"
	{
		"1" { "timeupdated" "100" "subscribedby" "7" "latest_timeupdated" "100" }
		"2" { "timeupdated" "100" "subscribedby" "7" "latest_timeupdated" "200" }
		"3" { "timeupdated" "0" "subscribedby" "7" }
		"4" { "timeupdated" "0" "subscribedby" "7" }
		"5" { "timeupdated" "100" "latest_timeupdated" "100" }
	}
}"#,
        )
        .unwrap();
        let items = from_manifests(&[workshop, root.join("none")]).expect("a manifest");
        let got: Vec<_> = items
            .iter()
            .map(|i| (i.id, i.installed, i.needs_update, i.downloading, i.pending))
            .collect();
        assert_eq!(
            got,
            [
                (1, true, false, false, false),
                (2, true, true, false, true),
                (3, false, false, true, false),
                (4, false, false, false, true),
            ],
            "5 is installed but nobody's subscription"
        );
        assert!(
            from_manifests(&[root.join("none")]).is_none(),
            "no file: ask Steam"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
