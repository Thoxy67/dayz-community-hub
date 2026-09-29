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
