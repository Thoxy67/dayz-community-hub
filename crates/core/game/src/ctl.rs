//! `DayzCtl`: the profile, steamcmd and HTTP client the app drives the game
//! with.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use dz_api::Server;
use dz_common::{Error, Result};
use dz_profile::{ModDownloader, Profile, Snapshot};
use dz_steamcmd::{ModProgress, PtyInputTx, SteamClient, SteamCmd, find_steam_root, find_steamcmd};
use reqwest::Client;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::launch;
use crate::mods::{self, InstalledMod, ModManagementStats};
use crate::operation::{Downloader, ModOpResult, ModOperation, spawn_mod_operation};

/// Returns the fallback steamapps path when Steam root is not configured.
fn default_steamapps_fallback() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from("C:\\Program Files (x86)\\Steam\\steamapps")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".steam/steam/steamapps")
    }
}

/// Normalize a user-supplied Steam path to a `steamapps` directory.
///
/// The launcher treats `steam_root` as the library's `steamapps` folder, but
/// users frequently point it at the *library* folder instead (e.g.
/// `/mnt/ssd2/SteamLibrary` rather than `/mnt/ssd2/SteamLibrary/steamapps`),
/// which silently breaks mod detection and install. Accept both forms.
fn normalize_steamapps(p: PathBuf) -> PathBuf {
    if p.file_name().is_some_and(|n| n == "steamapps") {
        p
    } else if p.join("steamapps").is_dir() {
        p.join("steamapps")
    } else {
        p
    }
}

/// The Steam client's `steamapps` directory that holds DayZ: the profile's,
/// or the one found on this machine. A saved folder that is not there (a
/// drive unplugged, a profile imported from the other OS) does not stop the
/// search.
fn resolve_steam_root(profile: &SteamInputs) -> PathBuf {
    profile
        .steam_root
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .map(normalize_steamapps)
        .or_else(find_steam_root)
        .unwrap_or_else(default_steamapps_fallback)
}

/// SteamCMD's own install directory; its workshop folder is the launcher's.
fn content_dir() -> PathBuf {
    dz_common::paths::steamcmd_content_dir()
}

/// Build a `SteamCmd` from the profile's settings, or `None` when steamcmd is
/// disabled or cannot be found. It installs into the launcher's own
/// directory, never into `steam_root`.
fn build_steamcmd(profile: &SteamInputs, steam_root: &Path) -> Option<Arc<SteamCmd>> {
    if !profile.steamcmd_enabled {
        return None;
    }
    let resolved_path = profile
        .steamcmd_path
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .or_else(find_steamcmd)?;
    let login = profile
        .steam_login
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    Some(Arc::new(
        SteamCmd::new(resolved_path, steam_root, content_dir(), Some(login))
            .with_saved_password(profile.steam_password.clone())
            .with_home(Some(dz_common::paths::steamcmd_home_dir())),
    ))
}

/// What finding Steam and SteamCMD reads from the profile: the Steam root,
/// SteamCMD on or off, its path, the login and its saved password. Equal
/// inputs, equal [`SteamSetup`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SteamInputs {
    pub steam_root: Option<String>,
    pub steamcmd_enabled: bool,
    pub steamcmd_path: Option<String>,
    pub steam_login: Option<String>,
    pub steam_password: Option<String>,
}

impl SteamInputs {
    pub fn of(p: &Profile) -> Self {
        Self {
            steam_root: p.steam_root.clone(),
            steamcmd_enabled: p.steamcmd_enabled,
            steamcmd_path: p.steamcmd_path.clone(),
            steam_login: p.steam_login.clone(),
            steam_password: p.steam_password.clone(),
        }
    }
}

/// Steam's folder and SteamCMD as found for some [`SteamInputs`]. Working it
/// out probes the disks (every drive on Windows), so a caller on a runtime
/// resolves it on a blocking thread and swaps it in with
/// [`DayzCtl::set_steam_setup`], holding no lock meanwhile.
pub struct SteamSetup {
    steam_root: PathBuf,
    steamcmd: Option<Arc<SteamCmd>>,
}

impl SteamSetup {
    /// Find Steam and SteamCMD for these settings: blocking.
    pub fn resolve(inputs: &SteamInputs) -> Self {
        let steam_root = resolve_steam_root(inputs);
        let steamcmd = build_steamcmd(inputs, &steam_root);
        Self {
            steam_root,
            steamcmd,
        }
    }
}

pub struct DayzCtl {
    profile: Profile,
    /// The Steam client's `steamapps` holding DayZ (read only).
    steam_root: PathBuf,
    steamcmd: Option<Arc<SteamCmd>>,
    client: Client,
}

impl DayzCtl {
    /// Load the profile at `profile_path` (creating it if missing) and resolve
    /// steamcmd from it. `client` is the caller's, so the app keeps one pool
    /// of connections: it downloads the offline mode's archive, so it needs a
    /// stall timeout between reads but no total limit.
    pub async fn new(profile_path: impl AsRef<Path>, client: Client) -> Result<Self> {
        let profile = Profile::load_async(profile_path).await?;
        let inputs = SteamInputs::of(&profile);
        let setup = tokio::task::spawn_blocking(move || SteamSetup::resolve(&inputs))
            .await
            .map_err(|e| Error::Config(format!("finding Steam failed: {e}")))?;
        Ok(Self {
            profile,
            steam_root: setup.steam_root,
            steamcmd: setup.steamcmd,
            client,
        })
    }

    // --- Profile ---

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        &mut self.profile
    }

    /// The profile serialized as it is now; see [`dz_profile::Writer`].
    pub fn profile_snapshot(&self) -> Result<Snapshot> {
        self.profile.snapshot()
    }

    /// Reload the profile from disk, replacing the in-memory state.
    pub fn reload_profile(&mut self, path: &Path) -> Result<()> {
        self.profile = Profile::load(path)?;
        Ok(())
    }

    // --- Paths ---

    /// The launcher's workshop folder, where SteamCMD downloads.
    pub fn workshop_path(&self) -> PathBuf {
        self.mod_dirs_without_steam().launcher
    }

    /// SteamCMD's own install directory.
    pub fn steamcmd_content_dir(&self) -> PathBuf {
        content_dir()
    }

    pub fn dayz_path(&self) -> Result<PathBuf> {
        Ok(self.steam_root.join("common").join("DayZ"))
    }

    /// The installed game's version (`1.30.164200`). Blocking: reads the
    /// executable.
    pub fn game_version(&self) -> Option<String> {
        crate::version::installed(&self.steam_root.join("common").join("DayZ"))
    }

    /// Where BattlEye stands. Blocking: see [`crate::battleye::status`].
    pub fn battleye_status(&self) -> crate::battleye::BattlEyeStatus {
        crate::battleye::status(
            &self.steam_root.join("common").join("DayZ"),
            &self.steam_root,
        )
    }

    /// Repair BattlEye (a UAC prompt on Windows), after deleting the game's
    /// data outside its folder when `clear` is set. Blocking.
    pub fn repair_battleye(&self, clear: bool) -> Result<()> {
        if clear {
            crate::battleye::clear_game_data(&self.steam_root)?;
        }
        crate::battleye::repair(&self.dayz_path()?)
    }

    /// The game's data outside its folder, which a thorough repair deletes.
    /// Blocking.
    pub fn game_data_dirs(&self) -> Vec<PathBuf> {
        crate::battleye::game_data_dirs(&self.steam_root)
    }

    fn mod_dirs_without_steam(&self) -> mods::ModDirs {
        mods::ModDirs {
            launcher: content_dir()
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join(dz_steamcmd::DAYZ_GAME_ID.to_string()),
            steam: Vec::new(),
        }
    }

    /// Where installed mods are: the launcher's folder and every Steam
    /// library's workshop folder. Reads `libraryfolders.vdf`: blocking.
    pub fn mod_dirs(&self) -> mods::ModDirs {
        mods::ModDirs {
            steam: dz_steamcmd::steam_workshop_dirs(&self.steam_root),
            ..self.mod_dirs_without_steam()
        }
    }

    pub fn has_steamcmd(&self) -> bool {
        self.steamcmd.is_some()
    }

    /// Mods download through the Steam client rather than SteamCMD.
    pub fn downloads_through_steam(&self) -> bool {
        self.profile.mod_downloader == ModDownloader::Steamworks
    }

    /// Rebuild the `SteamCmd` instance from the current profile.
    /// Call this after mutating `steam_login`, `steam_password`, `steam_root`,
    /// `steamcmd_path` or `steamcmd_enabled` so the new values take effect
    /// without restarting the app. Probes the disks: blocking. Under a lock on
    /// a runtime, resolve a [`SteamSetup`] outside it and swap it in with
    /// [`DayzCtl::set_steam_setup`] instead.
    pub fn rebuild_steamcmd(&mut self) {
        let setup = SteamSetup::resolve(&SteamInputs::of(&self.profile));
        self.set_steam_setup(setup);
    }

    /// Take a Steam folder and SteamCMD resolved elsewhere in.
    pub fn set_steam_setup(&mut self, setup: SteamSetup) {
        self.steam_root = setup.steam_root;
        self.steamcmd = setup.steamcmd;
    }

    /// The shared HTTP client.
    pub fn http_client(&self) -> &Client {
        &self.client
    }

    /// A clone for a background task (launch, filesystem scan). Cheap apart
    /// from the profile: steamcmd is behind an `Arc` and the HTTP client is
    /// reference counted.
    pub fn clone_for_task(&self) -> Self {
        Self {
            profile: self.profile.clone(),
            steam_root: self.steam_root.clone(),
            steamcmd: self.steamcmd.clone(),
            client: self.client.clone(),
        }
    }

    fn steamcmd_ref(&self) -> Result<&Arc<SteamCmd>> {
        self.steamcmd
            .as_ref()
            .ok_or_else(|| Error::Config("SteamCMD not configured".to_string()))
    }

    /// Start a background mod operation with progress reporting.
    ///
    /// Returns the progress receiver, the sender for input (a password or a
    /// Steam Guard code) to the steamcmd PTY, and the task's handle.
    #[allow(clippy::type_complexity)]
    pub fn start_mod_operation(
        &self,
        op: ModOperation,
    ) -> Result<(
        mpsc::UnboundedReceiver<ModProgress>,
        PtyInputTx,
        tokio::task::JoinHandle<ModOpResult>,
    )> {
        let installed = self.mod_dirs().scan(None);
        self.start_mod_operation_with(op, installed)
    }

    /// [`DayzCtl::start_mod_operation`] with the mods on disk the caller has
    /// already read, so a start does not scan the folders again.
    pub fn start_mod_operation_with(
        &self,
        op: ModOperation,
        installed: Vec<InstalledMod>,
    ) -> Result<(
        mpsc::UnboundedReceiver<ModProgress>,
        PtyInputTx,
        tokio::task::JoinHandle<ModOpResult>,
    )> {
        // Logging in is SteamCMD's alone, whichever downloads.
        let downloader = match (&op, self.profile.mod_downloader) {
            (ModOperation::Login, _) | (_, ModDownloader::Steamcmd) => {
                Downloader::SteamCmd(self.steamcmd_ref()?.clone())
            }
            (_, ModDownloader::Steamworks) => Downloader::Steam,
        };
        let dirs = self.mod_dirs();
        let dayz_path = self.dayz_path()?;
        Ok(spawn_mod_operation(
            downloader, dirs, dayz_path, op, installed,
        ))
    }

    pub fn steamcmd_login(&self) -> Option<&str> {
        self.steamcmd.as_ref().map(|sc| sc.login())
    }

    // --- Mod management (blocking filesystem work: call off the runtime) ---

    /// Every installed mod, launcher's and Steam's, one entry per id.
    pub fn get_installed_mods(&self) -> Result<Vec<InstalledMod>> {
        Ok(self.mod_dirs().scan(self.dayz_path().ok().as_deref()))
    }

    /// Delete a mod's launcher copy and its `@<id>` link. A copy in a Steam
    /// library is never deleted: see [`mods::DeleteOutcome::KeptInSteam`].
    pub fn delete_mod(&self, mod_id: u64) -> Result<mods::DeleteOutcome> {
        mods::delete_mod(&self.mod_dirs(), self.dayz_path().ok().as_deref(), mod_id)
    }

    /// Delete a mod's copies in the Steam libraries: only for a mod the
    /// account is not subscribed to (see [`mods::delete_steam_copies`]).
    pub fn delete_steam_copies(&self, mod_id: u64) -> Result<()> {
        mods::delete_steam_copies(&self.mod_dirs(), mod_id)
    }

    /// The directory of the copy of a mod in use.
    pub fn mod_dir(&self, mod_id: u64) -> Option<PathBuf> {
        self.mod_dirs().copy_of(mod_id).map(|(p, _)| p)
    }

    pub fn toggle_mod_managed(&self, mod_id: u64) -> Result<bool> {
        mods::toggle_mod_managed(&self.mod_dirs(), &self.dayz_path()?, mod_id)
    }

    pub fn cleanup_mods(&self) -> Result<ModManagementStats> {
        mods::cleanup_mods(&self.mod_dirs(), &self.dayz_path()?)
    }

    /// Create the `@<id>` links DayZ loads a server's mods through.
    pub fn setup_mod_symlinks(&self, server: &Server) -> Result<Vec<u64>> {
        mods::create_mod_symlinks(&self.mod_dirs(), &self.dayz_path()?, &server.mod_ids())
    }

    // --- Launch ---

    /// Full Steam command-line arguments to join `server`.
    pub fn build_steam_launch_args(
        &self,
        server: &Server,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Vec<String> {
        let args = launch::build_launch_args(
            server,
            &server.mod_ids(),
            password,
            &self.profile.options,
            extra_args,
        );
        launch::build_steam_applaunch_args(
            dz_steamcmd::DAYZ_GAME_ID,
            &args,
            self.profile.player.as_deref(),
        )
    }

    /// Join `server` through `steam -applaunch`. Recording the server in the
    /// history is the caller's job, on the profile it actually keeps. A
    /// BattlEye server first has BattlEye checked, and repaired if need be:
    /// without it the game would only be kicked.
    pub async fn launch_game(
        &self,
        server: &Server,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Result<()> {
        if server.battl_eye == Some(true) {
            let (dayz, steam_root) = (self.dayz_path()?, self.steam_root.clone());
            tokio::task::spawn_blocking(move || crate::battleye::ensure(&dayz, &steam_root))
                .await
                .map_err(|e| Error::Other(format!("battleye task failed: {e}")))??;
        }
        run_through_steam(self.build_steam_launch_args(server, password, extra_args)).await
    }
}

/// Hand Steam a command line (`-applaunch 221100 …`):
/// 1. start Steam if it is not running and wait for it to be ready,
/// 2. run it with the arguments.
pub async fn run_through_steam(args: Vec<String>) -> Result<()> {
    // A Steamworks session still open is DayZ as far as Steam is concerned:
    // the game would not start. End it (its process) first.
    let closed = tokio::task::spawn_blocking(|| {
        dz_steamworks::close_session(std::time::Duration::from_secs(3))
    })
    .await
    .unwrap_or(false);
    if closed {
        // Give Steam a moment to see that "DayZ" went away.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    }
    // `start` spawns a process and scans the process table (sysinfo), both
    // blocking, so run them off the async runtime thread.
    let already_running = tokio::task::spawn_blocking(SteamClient::start)
        .await
        .map_err(|e| Error::Other(format!("steam start task failed: {e}")))??;

    // A cold-started Steam discards -applaunch until its IPC socket is up
    // (which is why a second attempt always used to work): wait for the
    // process, then for its login where the system says (Windows), else
    // give it a moment to register the launch handler.
    if !already_running {
        for _ in 0..30u8 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let (running, ready) = tokio::task::spawn_blocking(|| {
                (SteamClient::is_running(), SteamClient::is_ready())
            })
            .await
            .unwrap_or((false, None));
            match (running, ready) {
                (true, Some(true)) => {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    break;
                }
                (true, None) => {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    break;
                }
                // Not up, or not logged in yet: at most the 30 s, then the
                // launch goes anyway.
                _ => {}
            }
        }
    }

    let (program, pre) = SteamClient::launcher().ok_or_else(SteamClient::not_found)?;
    let mut cmd = Command::new(program);
    cmd.args(&pre)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    dz_steamcmd::scrub_child_env(cmd.as_std_mut());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(dz_common::CREATE_NO_WINDOW);
    cmd.spawn()?;
    Ok(())
}
