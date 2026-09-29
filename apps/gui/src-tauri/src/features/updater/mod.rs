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
///
/// On Windows the updater plugin checks and remembers the download. On
/// Linux the same release manifest is read directly, so the player still
/// learns that a new version is out; installing it is the package manager's.
#[tauri::command]
#[specta::specta]
pub(crate) async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    #[cfg(windows)]
    {
        windows::check(&app).await.map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        check_manifest(&app).await
    }
}

/// The release manifest (`latest.json`) as far as a version check needs it.
#[cfg(not(windows))]
#[derive(serde::Deserialize)]
struct Manifest {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
}

#[cfg(not(windows))]
async fn check_manifest(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    use crate::error::{HttpResultExt, send_ok};

    let endpoint = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("endpoints"))
        .and_then(|e| e.get(0))
        .and_then(|e| e.as_str())
        .ok_or("No update endpoint configured")?
        .to_owned();
    let manifest: Manifest = send_ok(crate::net::api().get(&endpoint))
        .await
        .http_err("Update check")?
        .json()
        .await
        .http_err("Update check")?;
    let current = app.package_info().version.to_string();
    Ok(is_newer(&manifest.version, &current).then(|| UpdateInfo {
        version: manifest.version.trim_start_matches('v').to_owned(),
        current_version: current,
        body: manifest.notes,
        date: manifest.pub_date,
    }))
}

/// Whether `candidate` is a later version than `current` ("1.2.3", an
/// optional leading "v", and a pre-release suffix that sorts before the
/// release it precedes: 0.5.0-pre < 0.5.0).
#[cfg_attr(windows, allow(dead_code))]
fn is_newer(candidate: &str, current: &str) -> bool {
    fn parse(v: &str) -> Option<([u64; 3], bool)> {
        let v = v.trim().trim_start_matches('v');
        let (core, pre) = match v.split_once('-') {
            Some((c, _)) => (c, true),
            None => (v, false),
        };
        let mut it = core.split('.').map(|p| p.parse::<u64>());
        let parts = [
            it.next()?.ok()?,
            it.next().unwrap_or(Ok(0)).ok()?,
            it.next().unwrap_or(Ok(0)).ok()?,
        ];
        Some((parts, pre))
    }
    match (parse(candidate), parse(current)) {
        (Some((a, a_pre)), Some((b, b_pre))) => a > b || (a == b && b_pre && !a_pre),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn versions_compare() {
        assert!(is_newer("0.5.0", "0.4.1"));
        assert!(is_newer("v0.4.2", "0.4.1"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.5.0", "0.5.0-pre"));
        assert!(!is_newer("0.5.0-pre", "0.5.0"));
        assert!(!is_newer("0.4.1", "0.4.1"));
        assert!(!is_newer("0.4.0", "0.4.1"));
        assert!(!is_newer("garbage", "0.4.1"));
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
