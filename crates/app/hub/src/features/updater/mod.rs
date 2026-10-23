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
#[cfg(test)]
mod testkit;
#[cfg(windows)]
mod windows;

use serde::Serialize;
use std::sync::{LazyLock, Mutex};

/// The check every downloaded update passes before it is installed: `data` against
/// the base64 `.sig` text (`latest.json`'s `signature`) and a base64 minisign public
/// key (the launcher's own is built in). Public so the release tooling's signer
/// (`cargo xtask sign`) is tested against exactly this.
pub fn verify_update_signature(data: &[u8], signature: &str, pubkey: &str) -> Result<(), String> {
    release::verify_signature(data, signature, pubkey, false, None)
}

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

/// Where a download's steps go: the state above, then the window (or, in a
/// test, wherever the test listens).
#[derive(Clone)]
pub(crate) struct Reporter(std::sync::Arc<dyn Fn(DownloadEvent) + Send + Sync>);

impl Reporter {
    fn hub(hub: Hub) -> Self {
        Self(std::sync::Arc::new(move |ev| {
            download().apply(&ev);
            hub.emit(HubEvent::UpdateDownload(ev));
        }))
    }

    pub(crate) fn send(&self, ev: DownloadEvent) {
        (self.0)(ev);
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
        let update = release::check(crate::VERSION, installer)
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
        let on_event = Reporter::hub(self.clone());

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

    /// Start a new instance of this copy (the updated one after an install),
    /// without exiting: `true` when one was started and this window should
    /// now close (its preferences saved, as a restart from Tauri did). The new
    /// instance waits for this one to be gone before it takes over. The mock
    /// starts nothing and answers `false`.
    pub async fn relaunch_app(&self) -> Result<bool, String> {
        mocked!(self => Ok(false));
        relaunch().map(|()| true)
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
    let reporter = Reporter::hub(hub.clone());
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
    // Through a link, the file it names is the one replaced.
    let image = std::fs::canonicalize(&image).unwrap_or(image);
    release::require_https(&update.download_url)
        .map_err(|e| format!("Update install failed: {e}"))?;
    install_appimage_at(update, &image, release::PUBKEY, on_event).await
}

/// [`install_appimage`] onto `image`, checked against `pubkey`.
#[cfg(not(windows))]
async fn install_appimage_at(
    update: &Update,
    image: &std::path::Path,
    pubkey: &str,
    on_event: Reporter,
) -> Result<(), String> {
    let bytes = release::download(&update.download_url, &on_event)
        .await
        .map_err(|e| format!("Update install failed: {e}"))?;
    let (signature, version) = (update.signature.clone(), update.version.clone());
    let (image, pubkey) = (image.to_path_buf(), pubkey.to_owned());
    tokio::task::spawn_blocking(move || {
        release::verify_signature(&bytes, &signature, &pubkey, true, Some(&version))?;
        release::replace_appimage(&image, &bytes)
    })
    .await
    .map_err(|e| format!("Update install failed: {e}"))?
    .map_err(|e| format!("Update install failed: {e}"))?;
    on_event.send(DownloadEvent::Finished);
    Ok(())
}

/// Set by a restart for the instance it starts: the process id to wait for.
const RESTART_OF: &str = "DZCH_RESTART_OF";
/// How long a restarted instance waits for the previous one to be gone.
const RESTART_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

/// The executable a restart starts when an update put the new one at a path
/// the running process may no longer report as its own (Windows: the running
/// file was renamed aside).
static RELAUNCH_TARGET: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

#[cfg(windows)]
fn set_relaunch_target(exe: std::path::PathBuf) {
    *RELAUNCH_TARGET
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(exe);
}

/// The arguments a restart starts the new instance with: the window's own
/// (`--data-dir`, `--instance`, `--mock`, `--control`), set by `main` from what
/// it parsed. Never the original command line: a `dzch://` link, a `.dzch`
/// file, `--connect` or `--reconnect` would join a server or start the game
/// again after every update, profile import or reset. Unset, nothing is passed.
static RESTART_ARGS: Mutex<Vec<std::ffi::OsString>> = Mutex::new(Vec::new());

/// Keep the arguments a restart passes on (see [`RESTART_ARGS`]).
pub fn set_restart_args(args: Vec<std::ffi::OsString>) {
    *RESTART_ARGS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = args;
}

/// Start this copy again, without exiting, with the window's own arguments
/// (see [`set_restart_args`]). An AppImage runs from a mount that goes away
/// with the process, so the image itself (`$APPIMAGE`, the new one after an
/// update) is started instead.
pub(crate) fn relaunch() -> Result<(), String> {
    let args = RESTART_ARGS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    #[cfg(target_os = "linux")]
    if let Some(image) = std::env::var_os("APPIMAGE")
        && spawn_detached(std::path::Path::new(&image), &args).is_ok()
    {
        return Ok(());
    }
    let target = RELAUNCH_TARGET
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let exe = match target {
        Some(t) => t,
        None => std::env::current_exe().map_err(|e| format!("Restart: {e}"))?,
    };
    spawn_detached(&exe, &args).map_err(|e| format!("Restart: {e}"))
}

/// Start this executable again with the window's own arguments (see
/// [`relaunch`]), and exit at once: the control channel's restart and the
/// reset's, whose window must not write its files back into the data folder
/// just deleted. An update and a profile import use [`relaunch`] and let the
/// window close the normal way.
pub(crate) fn restart() -> Result<(), String> {
    relaunch()?;
    std::process::exit(0);
}

/// The process a restart started this instance after, read once and taken out
/// of the environment, so nothing this instance starts (Steam, then a launcher
/// Steam starts) inherits a stale id. Call it at the very start of `main`,
/// while the process has one thread.
pub fn take_restart_of() -> Option<u32> {
    let pid = std::env::var(RESTART_OF)
        .ok()
        .and_then(|p| p.trim().parse::<u32>().ok());
    if std::env::var_os(RESTART_OF).is_some() {
        // SAFETY: called first thing in `main`, before any other thread exists
        // (the runtime, the hub and the window come later), so nothing reads the
        // environment at the same time.
        unsafe { std::env::remove_var(RESTART_OF) };
    }
    pid.filter(|p| *p != std::process::id())
}

/// How long a restarted instance waits for the previous one: also the bound on
/// waiting for its single-instance lock.
pub fn restart_wait() -> std::time::Duration {
    RESTART_WAIT
}

/// Wait (at most 15 s) for process `pid` to be gone. For an instance that takes
/// no single-instance lock (`--mock` without `--instance`); the others wait for
/// the previous one's lock instead, which no reused pid can fool.
pub fn wait_for_previous(pid: u32) {
    wait_for_exit(pid, RESTART_WAIT);
}

/// At start: when a restart started this instance, wait (at most 15 s) for
/// the previous one to be gone, so it does not find that one still running
/// and hand its arguments over to a window about to close.
pub fn wait_for_previous_instance() {
    if let Some(pid) = take_restart_of() {
        wait_for_previous(pid);
    }
}

/// Wait until process `pid` is gone, at most `timeout`; whether it is.
fn wait_for_exit(pid: u32, timeout: std::time::Duration) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let mut sys = sysinfo::System::new();
    let until = std::time::Instant::now() + timeout;
    loop {
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        if sys.process(pid).is_none() {
            return true;
        }
        if std::time::Instant::now() >= until {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn spawn_detached(program: &std::path::Path, args: &[std::ffi::OsString]) -> std::io::Result<()> {
    std::process::Command::new(program)
        .args(args)
        .env(RESTART_OF, std::process::id().to_string())
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

    fn collector() -> (Reporter, std::sync::Arc<Mutex<Vec<DownloadEvent>>>) {
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let s = seen.clone();
        let r = Reporter(std::sync::Arc::new(move |ev| s.lock().unwrap().push(ev)));
        (r, seen)
    }

    /// The whole Linux path against a local release server: the manifest
    /// read, a newer version found, the AppImage downloaded with progress,
    /// its signature checked, the image replaced. A tampered download leaves
    /// the old image alone.
    #[cfg(not(windows))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_update_installs_end_to_end_from_a_local_release() {
        use testkit::{TAURI_COMMENT, TestKey, serve};
        let key = TestKey::new(11);
        let payload: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let good = key.sign(&payload, true, TAURI_COMMENT);
        let mut bad_payload = payload.clone();
        bad_payload[1234] ^= 0xff;
        // The manifest is built after the server knows its own address.
        let platform = format!(
            "{}-{}",
            "linux",
            if cfg!(target_arch = "aarch64") {
                "aarch64"
            } else {
                "x86_64"
            }
        );
        let base = serve(vec![
            ("/good.AppImage".into(), payload.clone()),
            ("/bad.AppImage".into(), bad_payload),
        ])
        .await;
        let manifest = serde_json::json!({
            "version": "99.0.0",
            "notes": "What is new",
            "pub_date": "2026-10-13T01:13:52Z",
            "platforms": {
                "windows-x86_64": { "signature": "x", "url": format!("{base}/win.zip") },
                platform.clone(): { "signature": good, "url": format!("{base}/good.AppImage") },
            }
        });
        let latest = serve(vec![(
            "/latest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        )])
        .await;

        let endpoint = format!("{latest}/latest.json");
        let update = release::check_at(&endpoint, "0.5.4", Some("appimage"))
            .await
            .unwrap()
            .expect("a newer version");
        assert_eq!(update.version, "99.0.0");
        assert_eq!(update.body.as_deref(), Some("What is new"));
        // Already the latest: nothing.
        assert!(
            release::check_at(&endpoint, "99.0.0", None)
                .await
                .unwrap()
                .is_none()
        );
        // A missing manifest is an error the card shows.
        assert!(
            release::check_at(&format!("{latest}/nope.json"), "0.5.4", None)
                .await
                .is_err()
        );

        let dir = std::env::temp_dir().join(format!("dzch-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("DayZ-Community-Hub.AppImage");
        std::fs::write(&image, b"old image").unwrap();

        let (reporter, seen) = collector();
        install_appimage_at(&update, &image, &key.pubkey(), reporter)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&image).unwrap(), payload);
        let seen = seen.lock().unwrap().clone();
        assert!(matches!(
            seen.first(),
            Some(DownloadEvent::Started { content_length: Some(n) }) if *n == payload.len() as u64
        ));
        assert!(matches!(seen.last(), Some(DownloadEvent::Finished)));
        let mut state = DownloadState {
            running: true,
            ..DownloadState::default()
        };
        for ev in &seen {
            state.apply(ev);
        }
        assert_eq!(state.received, payload.len() as u64);
        assert!(state.done);

        // The tampered download: refused, the image as it was, no Finished.
        std::fs::write(&image, b"old image").unwrap();
        let tampered = Update {
            download_url: format!("{base}/bad.AppImage"),
            ..update.clone()
        };
        let (reporter, seen) = collector();
        let err = install_appimage_at(&tampered, &image, &key.pubkey(), reporter)
            .await
            .unwrap_err();
        assert!(err.contains("signature"), "{err}");
        assert_eq!(std::fs::read(&image).unwrap(), b"old image");
        assert!(
            !seen
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, DownloadEvent::Finished))
        );
        // Signed by another key: refused too.
        let (reporter, _) = collector();
        assert!(
            install_appimage_at(&update, &image, &TestKey::new(12).pubkey(), reporter)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(&image).unwrap(), b"old image");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_restarted_instance_waits_for_the_previous_one() {
        let mut child = std::process::Command::new("sleep")
            .arg("0.3")
            .spawn()
            .unwrap();
        let pid = child.id();
        // Reaped as soon as it ends, as the previous instance's exit is.
        let reaper = std::thread::spawn(move || child.wait());
        let t = std::time::Instant::now();
        assert!(wait_for_exit(pid, std::time::Duration::from_secs(10)));
        assert!(t.elapsed() >= std::time::Duration::from_millis(200));
        reaper.join().unwrap().unwrap();
        // Still running past the wait: gives up.
        assert!(!wait_for_exit(
            std::process::id(),
            std::time::Duration::from_millis(100)
        ));
    }
}
