//! Where steamcmd is, watching for it to appear during setup, and (on
//! Windows) installing it.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_specta::Event;

#[cfg(target_os = "windows")]
use crate::error::{HttpResultExt, ResultExt};
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

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let candidate = std::path::PathBuf::from(appdata)
                .join("dayz-community-hub")
                .join("steamcmd")
                .join("steamcmd.exe");
            if candidate.exists() {
                return SteamcmdStatusDto {
                    found: true,
                    path: Some(candidate.to_string_lossy().to_string()),
                    platform: platform.into(),
                };
            }
        }
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
    tokio::task::spawn_blocking(move || detect_steamcmd_sync(&explicit_path))
        .await
        .map_err(|e| format!("Task join error: {e}"))
}

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
    let state_clone = (*state).clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            let explicit_path = state_clone.read().await.ctl.profile().steamcmd_path.clone();
            let status =
                match tokio::task::spawn_blocking(move || detect_steamcmd_sync(&explicit_path))
                    .await
                {
                    Ok(s) => s,
                    Err(_) => continue,
                };
            if status.found {
                let _ = crate::events::SteamcmdDetected(status).emit(&app);
                return;
            }
        }
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
                let out_path = install_dir_clone.join(file.name());
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

        {
            use std::os::windows::process::CommandExt;
            let steamcmd_steamapps = install_dir.join("steamapps");
            if !steamcmd_steamapps.exists() {
                if let Some(steam_root) = dz_steamcmd::find_steam_root() {
                    let _ = std::process::Command::new("cmd")
                        .args([
                            "/c",
                            "mklink",
                            "/J",
                            &steamcmd_steamapps.to_string_lossy().to_string(),
                            &steam_root.to_string_lossy().to_string(),
                        ])
                        .creation_flags(dz_common::CREATE_NO_WINDOW)
                        .output();
                }
            }
        }

        Ok(exe_path.to_string_lossy().to_string())
    }
}
