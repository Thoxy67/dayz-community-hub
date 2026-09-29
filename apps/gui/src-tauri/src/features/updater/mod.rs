//! Updating the launcher itself. Windows builds replace their own executable;
//! Linux packages are updated by the package manager, so there the commands
//! say so instead of being absent, and the command set is the same on both.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::PendingUpdate;

use serde::Serialize;
use tauri::{AppHandle, ipc::Channel};

/// How far an update's download has got, sent over a `Channel`, in the shape
/// tauri-plugin-updater's JS side uses: `{ event: "Started", data: { contentLength } }`.
/// (The variant names stay as they are; only the fields are camelCase.)
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Serialize, specta::Type)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
    },
    Finished,
}

/// A newer version than the one running.
#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Release notes.
    pub body: Option<String>,
    /// Release date, ISO 8601.
    pub date: Option<String>,
}

#[cfg(not(windows))]
const UNSUPPORTED: &str =
    "The launcher updates itself on Windows only; update it through your package manager.";

/// Look for a newer version. `null` when this one is the latest.
#[tauri::command]
#[specta::specta]
pub(crate) async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    #[cfg(windows)]
    {
        windows::check(&app).await.map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(UNSUPPORTED.into())
    }
}

/// Download, verify and install the version `check_for_update` found, then
/// restart into it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn install_update(
    app: AppHandle,
    on_event: Channel<DownloadEvent>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::install(&app, on_event)
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, on_event);
        Err(UNSUPPORTED.into())
    }
}
