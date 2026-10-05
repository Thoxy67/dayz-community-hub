//! Where steamcmd is, watching for it to appear during setup, and (on
//! Windows) installing it.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_specta::Event;

#[cfg(target_os = "windows")]
use crate::error::HttpResultExt;
use crate::state::SharedState;

#[derive(Serialize, serde::Deserialize, Clone, Debug, specta::Type)]
pub struct SteamcmdStatusDto {
    pub found: bool,
    pub path: Option<String>,
    pub platform: String,
}

/// Core detection logic shared by `detect_steamcmd` (one-shot) and
/// `watch_steamcmd` (polling).
fn detect_steamcmd_sync(explicit_path: &Option<String>) -> SteamcmdStatusDto {
    #[cfg(target_os = "windows")]
    let platform = "windows";
    #[cfg(target_os = "linux")]
    let platform = "linux";
    #[cfg(target_os = "macos")]
    let platform = "macos";

    if let Some(p) = explicit_path
        && !p.is_empty()
        && std::path::Path::new(p).exists()
    {
        return SteamcmdStatusDto {
            found: true,
            path: Some(p.clone()),
            platform: platform.into(),
        };
    }

    #[cfg(target_os = "windows")]
    let binary_name = "steamcmd.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "steamcmd";

    if let Ok(found_path) = which::which(binary_name) {
        return SteamcmdStatusDto {
            found: true,
            path: Some(found_path.to_string_lossy().to_string()),
            platform: platform.into(),
        };
    }

    // The controller's own search: Valve's tarball in ~/.steam/steamcmd,
    // Debian's /usr/games (often off a desktop session's PATH), Snap,
    // Flatpak, C:\SteamCMD, the copy this app downloads… so the setup and
    // Settings find what the controller would use.
    if let Some(found) = dz_steamcmd::find_steamcmd() {
        return SteamcmdStatusDto {
            found: true,
            path: Some(found.to_string_lossy().to_string()),
            platform: platform.into(),
        };
    }

    SteamcmdStatusDto {
        found: false,
        path: None,
        platform: platform.into(),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn detect_steamcmd(
    state: State<'_, SharedState>,
) -> Result<SteamcmdStatusDto, String> {
    let explicit_path = {
        let s = state.read().await;
        s.ctl.profile().steamcmd_path.clone()
    };
    // Push the FS walk (which::which + path.exists) onto the blocking pool —
    // it touches the filesystem and on Windows can spawn `reg query` for the
    // registry lookup, neither of which belong on the runtime thread.
    let status = tokio::task::spawn_blocking(move || detect_steamcmd_sync(&explicit_path))
        .await
        .map_err(|e| format!("Task join error: {e}"))?;
    adopt_if_found(&state, &status).await;
    Ok(status)
}

/// steamcmd was found but the controller was built without it (installed
/// after startup, during setup): take it up now, so mod operations work and
/// the "SteamCMD not found" warning goes away without a settings save.
async fn adopt_if_found(state: &SharedState, status: &SteamcmdStatusDto) {
    if status.found && !state.read().await.ctl.has_steamcmd() {
        state.write().await.ctl.rebuild_steamcmd();
    }
}

/// Whether a `watch_steamcmd` poller is running, so repeated calls (the setup
/// wizard mounting again) do not stack pollers.
static WATCHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A poller gives up after this long: setup is not left open for hours.
const WATCH_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Start a background task that polls for steamcmd every 3 seconds.
/// Each tick re-reads `steamcmd_path` from the live profile, so a user
/// editing the path during the wizard takes effect immediately rather than
/// being stuck with the value captured at watch start.
#[tauri::command]
#[specta::specta]
pub(crate) async fn watch_steamcmd(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if WATCHING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let state_clone = (*state).clone();

    tokio::spawn(async move {
        let started = std::time::Instant::now();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        while started.elapsed() < WATCH_FOR {
            interval.tick().await;
            let explicit_path = state_clone.read().await.ctl.profile().steamcmd_path.clone();
            let Ok(status) =
                tokio::task::spawn_blocking(move || detect_steamcmd_sync(&explicit_path)).await
            else {
                continue;
            };
            if status.found {
                adopt_if_found(&state_clone, &status).await;
                let _ = crate::events::SteamcmdDetected(status).emit(&app);
                break;
            }
        }
        WATCHING.store(false, Ordering::Release);
    });

    Ok(())
}

/// Windows-only: download steamcmd.zip from Valve and unzip it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn download_steamcmd_windows() -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    return Err("Only available on Windows".into());

    #[cfg(target_os = "windows")]
    {
        use std::io::Cursor;

        let appdata =
            std::env::var("APPDATA").map_err(|_| "APPDATA env var not found".to_string())?;
        let install_dir = std::path::PathBuf::from(&appdata)
            .join("dayz-community-hub")
            .join("steamcmd");
        tokio::fs::create_dir_all(&install_dir)
            .await
            .map_err(|e| format!("Cannot create steamcmd dir: {e}"))?;

        let exe_path = install_dir.join("steamcmd.exe");
        if tokio::fs::try_exists(&exe_path).await.unwrap_or(false) {
            return Ok(exe_path.to_string_lossy().to_string());
        }

        let bytes = crate::error::send_ok(
            crate::net::download()
                .get("https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip"),
        )
        .await
        .http_err("SteamCMD download")?
        .bytes()
        .await
        .http_err("SteamCMD download")?;

        let install_dir_clone = install_dir.clone();
        tokio::task::spawn_blocking(move || {
            let cursor = Cursor::new(bytes);
            let mut archive =
                zip::ZipArchive::new(cursor).map_err(|e| format!("ZIP open failed: {e}"))?;
            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| format!("ZIP entry error: {e}"))?;
                // Only names that stay inside the install folder.
                let Some(name) = file.enclosed_name() else {
                    continue;
                };
                let out_path = install_dir_clone.join(name);
                if file.is_dir() {
                    std::fs::create_dir_all(&out_path).map_err(|e| format!("mkdir failed: {e}"))?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("mkdir failed: {e}"))?;
                    }
                    let mut out = std::fs::File::create(&out_path)
                        .map_err(|e| format!("File create failed: {e}"))?;
                    std::io::copy(&mut file, &mut out)
                        .map_err(|e| format!("File write failed: {e}"))?;
                }
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))??;

        // No link from SteamCMD's `steamapps` to the Steam client's library
        // (an earlier version made one): SteamCMD would write its manifests
        // there. It installs into the launcher's own directory instead.

        Ok(exe_path.to_string_lossy().to_string())
    }
}

/// Where DayZ is on this machine, for the setup to show and correct.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct DayzDetectDto {
    /// The `steamapps` folder in use (the library holding DayZ, if one does).
    pub steamapps: Option<String>,
    /// The game's folder, when it is installed there.
    pub dayz_dir: Option<String>,
    /// Mods the Steam client has already downloaded, across its libraries.
    pub workshop_mods: u32,
}

/// Look for DayZ in `path` (a Steam library or its `steamapps`, as picked by
/// the player) or, without one, wherever Steam is installed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn detect_dayz(path: Option<String>) -> Result<DayzDetectDto, String> {
    tokio::task::spawn_blocking(move || {
        let explicit = path
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from);
        let found = dz_steamcmd::detect_dayz(explicit.as_deref());
        let workshop_mods = found
            .workshop_dirs
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flat_map(|entries| entries.flatten())
            .filter(|e| e.path().is_dir())
            .count() as u32;
        DayzDetectDto {
            steamapps: found.steamapps.map(|p| p.to_string_lossy().into_owned()),
            dayz_dir: found.dayz.map(|p| p.to_string_lossy().into_owned()),
            workshop_mods,
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))
}
