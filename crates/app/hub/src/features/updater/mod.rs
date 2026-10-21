//! Updating the launcher itself, from the one release manifest (`latest.json`
//! at the endpoint `tauri.conf.json` named, Tauri's updater format, with a
//! `windows-x86_64` and a `linux-x86_64` entry). What tauri-plugin-updater
//! did is plain Rust here (`release`): read the manifest, compare versions,
//! download, check the minisign signature against the one public key.
//!
//! - **Windows portable zip**: the zip's executable is swapped in place of
//!   this one (see `windows.rs`).
//! - **Linux AppImage** (release build, started as an AppImage, `$APPIMAGE`
//!   set): the new image is written over the old one, the old one kept aside
//!   until the new one is in place.
//! - **deb / rpm / AUR packages** belong to the package manager, and **dev
//!   builds** are the source tree: a newer version is still reported, but
//!   installing is refused, and [`Hub::update_support`] says why.
//!
//! After an install the window calls `restart_app`. A download in flight is
//! the backend's: its progress goes out as `HubEvent::UpdateDownload` and is
//! kept here too, so a window asks [`Hub::update_download_state`] instead of
//! losing the bar or starting the download again.

mod release;
#[cfg(windows)]
mod windows;

use serde::Serialize;
use std::sync::{LazyLock, Mutex};

use crate::{Hub, HubEvent};
pub(crate) use release::Update;

/// How far an update's download has got, sent as `HubEvent::UpdateDownload`,
/// in the shape tauri-plugin-updater's JS side used:
/// `{ event: "Started", data: { contentLength } }`.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started { content_length: Option<u64> },
    #[serde(rename_all = "camelCase")]
    Progress { chunk_length: usize },
    /// Downloaded, verified and in place: restart to use it.
    Finished,
}

/// A download as it stands.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DownloadState {
    /// Downloading, verifying or putting the new version in place.
    pub running: bool,
    /// Bytes received, of `total` when the server said.
    pub received: u64,
    pub total: Option<u64>,
    /// In place: restart to use it.
    pub done: bool,
    pub error: Option<String>,
}

impl DownloadState {
    fn apply(&mut self, ev: &DownloadEvent) {
        match ev {
            DownloadEvent::Started { content_length } => {
                self.total = *content_length;
                self.received = 0;
            }
            DownloadEvent::Progress { chunk_length } => self.received += *chunk_length as u64,
            DownloadEvent::Finished => {
                self.running = false;
                self.done = true;
            }
        }
    }
}

static DOWNLOAD: LazyLock<Mutex<DownloadState>> = LazyLock::new(Mutex::default);

fn download() -> std::sync::MutexGuard<'static, DownloadState> {
    DOWNLOAD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Where a download's steps go: the state above, then the window.
#[derive(Clone)]
pub(crate) struct Reporter(Hub);

impl Reporter {
    pub(crate) fn send(&self, ev: DownloadEvent) {
        download().apply(&ev);
        self.0.emit(HubEvent::UpdateDownload(ev));
    }
}

/// A newer version than the one running.
#[derive(Clone, Serialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateKind {
    Appimage,
    WindowsZip,
    PackageManager,
    Dev,
}

/// Whether this copy can install an update, and if not, why.
#[derive(Clone, Serialize)]
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

impl Hub {
    /// Whether this copy can update itself.
    pub async fn update_support(&self) -> UpdateSupport {
        mocked!(self => crate::mock::fixtures::update_support());
        support()
    }

    /// Look for a newer version. `None` when this one is the latest. Works on
    /// every kind of copy, so a package user still learns a release is out.
    pub async fn check_for_update(&self) -> Result<Option<UpdateInfo>, String> {
        mocked!(self => crate::mock::fixtures::check_update());
        #[cfg(windows)]
        windows::remove_leftovers();
        let installer = (kind() == UpdateKind::Appimage).then_some("appimage");
        let update = release::check(env!("CARGO_PKG_VERSION"), installer)
            .await
            .map_err(|e| format!("Update check: {e}"))?;
        let info = update.as_ref().map(|u| UpdateInfo {
            version: u.version.clone(),
            current_version: u.current_version.clone(),
            body: u.body.clone(),
            date: u.date.clone(),
        });
        self.inner.pending.set(update);
        Ok(info)
    }

    /// Download, verify and install the version `check_for_update` found.
    /// Its steps go out as `HubEvent::UpdateDownload`, ending with
    /// `Finished`; the window then calls `restart_app`.
    pub async fn install_update(&self) -> Result<(), String> {
        mocked!(self => mock_install(self).await);
        let s = support();
        if !s.supported {
            return Err(s.reason.unwrap_or_default());
        }
        let update = self
            .inner
            .pending
            .get()
            .ok_or("No update to install: check for updates first")?;
        {
            let mut d = download();
            // One download is enough.
            if d.running {
                return Err("The update is already downloading".into());
            }
            *d = DownloadState {
                running: true,
                ..DownloadState::default()
            };
        }
        let on_event = Reporter(self.clone());

        #[cfg(windows)]
        let r = windows::install(&update, on_event).await;
        #[cfg(not(windows))]
        let r = install_appimage(&update, on_event).await;
        let mut d = download();
        d.running = false;
        if let Err(e) = &r {
            d.error = Some(e.clone());
        }
        r
    }

    /// The update's download as it stands, for a window that has just loaded.
    pub async fn update_download_state(&self) -> DownloadState {
        download().clone()
    }
}

/// The mock's update: 80 MB downloaded in a few seconds, then in place.
/// Nothing is fetched or written.
#[cfg(feature = "mock")]
async fn mock_install(hub: &Hub) -> Result<(), String> {
    const TOTAL: u64 = 80_000_000;
    const CHUNKS: u64 = 20;
    {
        let mut d = download();
        if d.running {
            return Err("The update is already downloading".into());
        }
        *d = DownloadState {
            running: true,
            ..DownloadState::default()
        };
    }
    let reporter = Reporter(hub.clone());
    tokio::spawn(async move {
        reporter.send(DownloadEvent::Started {
            content_length: Some(TOTAL),
        });
        for _ in 0..CHUNKS {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            reporter.send(DownloadEvent::Progress {
                chunk_length: (TOTAL / CHUNKS) as usize,
            });
        }
        reporter.send(DownloadEvent::Finished);
    });
    Ok(())
}

/// Download the new AppImage, check its signature, and write it over
/// `$APPIMAGE` (what tauri-plugin-updater did on Linux).
#[cfg(not(windows))]
async fn install_appimage(update: &Update, on_event: Reporter) -> Result<(), String> {
    let image = std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .ok_or("Not started as an AppImage ($APPIMAGE is not set)")?;
    let bytes = release::download(&update.download_url, &on_event)
        .await
        .map_err(|e| format!("Update install failed: {e}"))?;
    let (signature, version) = (update.signature.clone(), update.version.clone());
    tokio::task::spawn_blocking(move || {
        release::verify_signature(&bytes, &signature, release::PUBKEY, true, Some(&version))?;
        release::replace_appimage(&image, &bytes)
    })
    .await
    .map_err(|e| format!("Update install failed: {e}"))?
    .map_err(|e| format!("Update install failed: {e}"))?;
    on_event.send(DownloadEvent::Finished);
    Ok(())
}

/// Start this executable again with the same arguments, and exit. An
/// AppImage runs from a mount that goes away with the process, so the image
/// itself (`$APPIMAGE`, the new one after an update) is started instead.
pub(crate) fn restart() -> Result<(), String> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    #[cfg(target_os = "linux")]
    if let Some(image) = std::env::var_os("APPIMAGE")
        && spawn_detached(std::path::Path::new(&image), &args).is_ok()
    {
        std::process::exit(0);
    }
    let exe = std::env::current_exe().map_err(|e| format!("Restart: {e}"))?;
    spawn_detached(&exe, &args).map_err(|e| format!("Restart: {e}"))?;
    std::process::exit(0);
}

fn spawn_detached(program: &std::path::Path, args: &[std::ffi::OsString]) -> std::io::Result<()> {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(drop)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
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
    fn a_download_adds_up_its_chunks() {
        let mut d = DownloadState {
            running: true,
            ..DownloadState::default()
        };
        d.apply(&DownloadEvent::Started {
            content_length: Some(100),
        });
        d.apply(&DownloadEvent::Progress { chunk_length: 30 });
        d.apply(&DownloadEvent::Progress { chunk_length: 70 });
        assert_eq!((d.received, d.total, d.running), (100, Some(100), true));
        d.apply(&DownloadEvent::Finished);
        assert!(d.done && !d.running);
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
