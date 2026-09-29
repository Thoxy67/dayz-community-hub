//! Downloading mods through the running Steam client (dz-steamworks)
//! instead of SteamCMD: the choice, and whether it can work here.

use dz_profile::ModDownloader;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::spawn_blocking_mapped;
use crate::state::{SharedState, mutate_profile};

/// What downloads mods.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
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

/// Choose what downloads mods; the next operation uses it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_mod_downloader(
    downloader: ModDownloaderDto,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    mutate_profile(&state, |s| {
        s.ctl.profile_mut().mod_downloader = downloader.into();
        Ok(())
    })
    .await
}

/// Whether downloads through Steam can work on this machine.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct SteamworksStatusDto {
    /// Valve's library loaded.
    pub library: bool,
    /// Why it did not, when it did not.
    pub error: Option<String>,
    /// The Steam client is running.
    pub steam_running: bool,
}

/// Load Valve's library (the first call writes it to disk) and look for a
/// running Steam, without connecting to it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn steamworks_status() -> Result<SteamworksStatusDto, String> {
    spawn_blocking_mapped(|| {
        let loaded = dz_steamworks::available();
        Ok::<_, String>(SteamworksStatusDto {
            library: loaded.is_ok(),
            error: loaded.err(),
            steam_running: dz_steamworks::steam_running(),
        })
    })
    .await
}

/// Connect to Steam as DayZ and disconnect at once: rejects with the
/// sentence a download would fail with.
#[tauri::command]
#[specta::specta]
pub(crate) async fn steamworks_check() -> Result<(), String> {
    // The session lives and dies on this one blocking thread.
    spawn_blocking_mapped(dz_steamworks::check).await
}

/// One Workshop item the Steam account is subscribed to, as Steam has it.
#[derive(Serialize, Clone, Debug, specta::Type)]
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
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct WorkshopItemDto {
    pub id: u64,
    pub title: String,
    /// The items it requires ("Required items" on its Workshop page).
    pub requires: Vec<u64>,
}

/// The account's subscriptions, or why they are not known.
#[derive(Serialize, Clone, Debug, specta::Type)]
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

/// The DayZ Workshop items the Steam account is subscribed to, with what
/// Steam is doing with each (installed, downloading, waiting), and the
/// Workshop's details (title, required items) of those and of `ids` (the
/// mods on disk). Asks the running Steam client in a short session (Steam
/// shows DayZ running for a moment), at most every few seconds; while
/// another session is open (a download through Steam), the last answer.
#[tauri::command]
#[specta::specta]
pub(crate) async fn steam_subscriptions(ids: Vec<u64>) -> Result<SteamSubscriptionsDto, String> {
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
        if !dz_steamworks::steam_running() {
            return Ok(unavailable("Steam is not running.".into()));
        }
        if dz_steamworks::session_open()
            && let Some((_, dto)) = last
        {
            return Ok(dto);
        }
        let known: std::collections::HashSet<u64> = DETAILS
            .lock()
            .ok()
            .and_then(|d| d.as_ref().map(|d| d.keys().copied().collect()))
            .unwrap_or_default();
        let dto = match dz_steamworks::subscriptions(&ids, &|id| known.contains(&id)) {
            Ok((items, found)) => {
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
                SteamSubscriptionsDto {
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
                }
            }
            // A session busy elsewhere: the last answer is still the best.
            Err(e) => {
                eprintln!("[steamworks] subscriptions: {e}");
                match last {
                    Some((_, dto)) => dto,
                    None => unavailable(e),
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
