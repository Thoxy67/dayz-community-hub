//! Updating the launcher itself, from the one release manifest (`latest.json`
//! at the endpoint in `tauri.conf.json`, Tauri's updater format, with a
//! `windows-x86_64` and a `linux-x86_64` entry).
//!
//! - **Windows portable zip**: the plugin checks the manifest; our own code
//!   downloads the zip, verifies its minisign signature, swaps the executable
//!   (see `windows.rs`).
//! - **Linux AppImage** (release build, started as an AppImage, `$APPIMAGE`
//!   set): the plugin downloads, verifies and writes the new image over the
//!   old one.
//! - **deb / rpm / AUR packages** belong to the package manager, and **dev
//!   builds** are the source tree: a newer version is still reported, but
//!   installing is refused, and [`update_support`] says why.
//!
//! Every path verifies against the one public key, `plugins.updater.pubkey`
//! in `tauri.conf.json`. After an install the window calls `restart_app`.

#[cfg(windows)]
mod windows;

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};

/// How far an update's download has got, sent over a `Channel`, in the shape
/// tauri-plugin-updater's JS side uses: `{ event: "Started", data: { contentLength } }`.
#[derive(Clone, Serialize, specta::Type)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started { content_length: Option<u64> },
    #[serde(rename_all = "camelCase")]
    Progress { chunk_length: usize },
    /// Downloaded, verified and in place: restart to use it.
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

/// What kind of copy this is, as far as updating goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateKind {
    Appimage,
    WindowsZip,
    PackageManager,
    Dev,
}

/// Whether this copy can install an update, and if not, why.
#[derive(Clone, Serialize, specta::Type)]
pub struct UpdateSupport {
    pub supported: bool,
    pub kind: UpdateKind,
    pub reason: Option<String>,
}

fn kind() -> UpdateKind {
    if cfg!(debug_assertions) {
        UpdateKind::Dev
    } else if cfg!(windows) {
        UpdateKind::WindowsZip
    } else if std::env::var_os("APPIMAGE").is_some() {
        UpdateKind::Appimage
    } else {
        UpdateKind::PackageManager
    }
}

fn support() -> UpdateSupport {
    let kind = kind();
    let reason = match kind {
        UpdateKind::Appimage | UpdateKind::WindowsZip => None,
        UpdateKind::PackageManager => Some(
            "This copy was installed by a package manager (deb, rpm, AUR): update it there."
                .to_string(),
        ),
        UpdateKind::Dev => Some("A development build does not update itself.".to_string()),
    };
    UpdateSupport {
        supported: reason.is_none(),
        kind,
        reason,
    }
}

/// The update the last check found, kept for `install_update`.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

impl Pending {
    fn set(&self, u: Option<Update>) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = u;
    }
    fn get(&self) -> Option<Update> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Whether this copy can update itself.
#[tauri::command]
#[specta::specta]
pub(crate) async fn update_support() -> UpdateSupport {
    support()
}

/// Look for a newer version. `null` when this one is the latest. Works on
/// every kind of copy, so a package user still learns a release is out.
#[tauri::command]
#[specta::specta]
pub(crate) async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let update = app
        .updater()
        .map_err(|e| format!("Update check: {e}"))?
        .check()
        .await
        .map_err(|e| format!("Update check: {e}"))?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        body: u.body.clone(),
        date: u.date.map(|d| {
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                d.year(),
                d.month() as u8,
                d.day(),
                d.hour(),
                d.minute(),
                d.second(),
            )
        }),
    });
    app.state::<Pending>().set(update);
    Ok(info)
}

/// Download, verify and install the version `check_for_update` found. Ends
/// with a `Finished` event; the window then calls `restart_app`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn install_update(
    app: AppHandle,
    on_event: Channel<DownloadEvent>,
) -> Result<(), String> {
    let s = support();
    if !s.supported {
        return Err(s.reason.unwrap_or_default());
    }
    let update = app
        .state::<Pending>()
        .get()
        .ok_or("No update to install: check for updates first")?;

    #[cfg(windows)]
    {
        windows::install(&app, &update, on_event)
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        install_appimage(&update, on_event).await
    }
}

/// The plugin writes the new AppImage over `$APPIMAGE` after checking its
/// signature against the configured key.
#[cfg(not(windows))]
async fn install_appimage(update: &Update, on_event: Channel<DownloadEvent>) -> Result<(), String> {
    let (progress, finished) = (on_event.clone(), on_event.clone());
    let mut started = false;
    let mut pending = 0usize;
    let mut last = std::time::Instant::now();
    update
        .download_and_install(
            move |chunk, total| {
                if !started {
                    started = true;
                    let _ = progress.send(DownloadEvent::Started {
                        content_length: total,
                    });
                }
                pending += chunk;
                if last.elapsed() >= std::time::Duration::from_millis(100) {
                    let _ = progress.send(DownloadEvent::Progress {
                        chunk_length: std::mem::take(&mut pending),
                    });
                    last = std::time::Instant::now();
                }
            },
            move || {
                let _ = finished.send(DownloadEvent::Progress { chunk_length: 0 });
            },
        )
        .await
        .map_err(|e| format!("Update install failed: {e}"))?;
    let _ = on_event.send(DownloadEvent::Finished);
    Ok(())
}

/// Restart into the installed version. An AppImage runs from a mount that
/// goes away with the process, so the image itself (`$APPIMAGE`, now the new
/// one) is started instead of the running executable.
pub(crate) fn restart(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    if let Some(image) = std::env::var_os("APPIMAGE")
        && std::process::Command::new(&image)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    {
        app.exit(0);
        return;
    }
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dev_build_says_why_it_cannot_update() {
        // Tests are debug builds.
        let s = support();
        assert_eq!(s.kind, UpdateKind::Dev);
        assert!(!s.supported);
        assert!(s.reason.is_some());
    }

    #[test]
    fn kinds_serialize_as_the_window_expects() {
        let json = serde_json::to_string(&[
            UpdateKind::Appimage,
            UpdateKind::WindowsZip,
            UpdateKind::PackageManager,
            UpdateKind::Dev,
        ])
        .unwrap();
        assert_eq!(
            json,
            r#"["appimage","windows-zip","package-manager","dev"]"#
        );
    }
}
