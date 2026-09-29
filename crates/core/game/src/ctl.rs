//! `DayzCtl`: the profile, steamcmd and HTTP client the app drives the game
//! with.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use dz_api::Server;
use dz_common::{Error, Result};
use dz_profile::{Profile, Snapshot};
use dz_steamcmd::{ModProgress, PtyInputTx, SteamClient, SteamCmd, find_steam_root, find_steamcmd};
use reqwest::Client;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::launch;
use crate::mods::{self, InstalledMod, ModManagementStats};
use crate::operation::{ModOpResult, ModOperation, spawn_mod_operation};

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

/// Build a `SteamCmd` from the profile's settings, or `None` when steamcmd is
/// disabled or cannot be found.
fn build_steamcmd(profile: &Profile) -> Option<Arc<SteamCmd>> {
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
    let steam_root = profile
        .steam_root
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .map(normalize_steamapps)
        .or_else(find_steam_root)
        .unwrap_or_else(default_steamapps_fallback);
    let login = profile
        .steam_login
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let password = profile.steam_password.clone();
    Some(Arc::new(
        SteamCmd::new(resolved_path, steam_root, Some(login)).with_password(password),
    ))
}

pub struct DayzCtl {
    profile: Profile,
    steamcmd: Option<Arc<SteamCmd>>,
    client: Client,
}

impl DayzCtl {
    /// Load the profile at `profile_path` (creating it if missing) and resolve
    /// steamcmd from it.
    pub async fn new(profile_path: impl AsRef<Path>) -> Result<Self> {
        let profile = Profile::load_async(profile_path).await?;
        let steamcmd = build_steamcmd(&profile);
        Ok(Self {
            profile,
            steamcmd,
            client: Client::new(),
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

    pub fn workshop_path(&self) -> Result<PathBuf> {
        self.steamcmd_ref().map(|sc| sc.workshop_path())
    }

    pub fn dayz_path(&self) -> Result<PathBuf> {
        self.steamcmd_ref().map(|sc| sc.dayz_path())
    }

    pub fn has_steamcmd(&self) -> bool {
        self.steamcmd.is_some()
    }

    /// Rebuild the `SteamCmd` instance from the current profile.
    /// Call this after mutating `steam_login`, `steam_password`, `steam_root`,
    /// `steamcmd_path` or `steamcmd_enabled` so the new values take effect
    /// without restarting the app.
    pub fn rebuild_steamcmd(&mut self) {
        self.steamcmd = build_steamcmd(&self.profile);
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
        let steamcmd = self.steamcmd_ref()?.clone();
        let workshop_path = self.workshop_path()?;
        let dayz_path = self.dayz_path()?;
        let installed = self.get_installed_mods().unwrap_or_default();
        Ok(spawn_mod_operation(
            steamcmd,
            workshop_path,
            dayz_path,
            op,
            installed,
        ))
    }

    pub fn steamcmd_login(&self) -> Option<&str> {
        self.steamcmd.as_ref().map(|sc| sc.login())
    }

    // --- Mod management (blocking filesystem work: call off the runtime) ---

    pub fn get_installed_mods(&self) -> Result<Vec<InstalledMod>> {
        mods::scan_workshop_dir(&self.workshop_path()?)
    }

    pub fn delete_mod(&self, mod_id: u64, only_managed: bool) -> Result<()> {
        mods::delete_mod(&self.workshop_path()?, mod_id, only_managed)
    }

    pub fn toggle_mod_managed(&self, mod_id: u64) -> Result<bool> {
        mods::toggle_mod_managed(&self.workshop_path()?, &self.dayz_path()?, mod_id)
    }

    pub fn cleanup_mods(&self) -> Result<ModManagementStats> {
        mods::cleanup_mods(&self.workshop_path()?, &self.dayz_path()?)
    }

    /// Create the `@<id>` links DayZ loads a server's mods through.
    pub fn setup_mod_symlinks(&self, server: &Server) -> Result<Vec<u64>> {
        mods::create_mod_symlinks(
            &self.workshop_path()?,
            &self.dayz_path()?,
            &server.mod_ids(),
        )
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

    /// Join `server` through `steam -applaunch`:
    /// 1. start Steam if it is not running and wait for it to be ready,
    /// 2. hand Steam the launch arguments.
    ///
    /// Recording the server in the history is the caller's job, on the
    /// profile it actually keeps.
    pub async fn launch_game(
        &self,
        server: &Server,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Result<()> {
        // `start` spawns a process and scans the process table (sysinfo), both
        // blocking, so run them off the async runtime thread.
        let already_running = tokio::task::spawn_blocking(SteamClient::start)
            .await
            .map_err(|e| Error::Other(format!("steam start task failed: {e}")))??;

        // A cold-started Steam discards -applaunch until its IPC socket is up
        // (which is why a second attempt always used to work): wait for the
        // process, then give it a moment to register the launch handler.
        if !already_running {
            for _ in 0..30u8 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let running = tokio::task::spawn_blocking(SteamClient::is_running)
                    .await
                    .unwrap_or(false);
                if running {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    break;
                }
            }
        }

        let args = self.build_steam_launch_args(server, password, extra_args);

        let mut cmd = Command::new(SteamClient::steam_exe_path());
        cmd.args(&args).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(dz_common::CREATE_NO_WINDOW);
        cmd.spawn()?;
        Ok(())
    }
}
