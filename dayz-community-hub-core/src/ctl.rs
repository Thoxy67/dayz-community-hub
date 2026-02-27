use crate::{
    Result,
    api::{self, Server, ServerList},
    config::Profile,
    errors::Error,
    launch,
    mods::{self, InstalledMod, ModManagementStats},
    steamcmd::{ModProgress, PtyInputTx, SteamClient, SteamCmd, find_steam_root, find_steamcmd},
};
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Returns the user's home directory path, cross-platform.
fn home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let p = std::env::var("USERPROFILE")
            .or_else(|_| {
                std::env::var("HOMEDRIVE").and_then(|d| std::env::var("HOMEPATH").map(|p| d + &p))
            })
            .unwrap_or_else(|_| "C:\\Users\\Public".to_string());
        PathBuf::from(p)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let p = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(p)
    }
}

/// Returns the fallback steamapps path when Steam root is not configured.
fn default_steamapps_fallback() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from("C:\\Program Files (x86)\\Steam\\steamapps")
    }
    #[cfg(not(target_os = "windows"))]
    {
        home_dir().join(".steam/steam/steamapps")
    }
}

pub struct DayzCtl {
    profile: Profile,
    steamcmd: Option<Arc<SteamCmd>>,
    client: Client,
    server_list: Option<ServerList>,
}

impl DayzCtl {
    pub async fn new(profile_path: impl AsRef<Path>) -> Result<Self> {
        let profile = Profile::load(profile_path)?;
        let steamcmd = if profile.steamcmd_enabled {
            // Prefer explicit user-configured path, then auto-detect
            let resolved_path = profile
                .steamcmd_path
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .or_else(find_steamcmd);
            if let Some(steamcmd_path) = resolved_path {
                let steam_root = profile
                    .steam_root
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| PathBuf::from(s))
                    .or_else(find_steam_root)
                    .unwrap_or_else(default_steamapps_fallback);
                let login = profile
                    .steam_login
                    .clone()
                    .unwrap_or_else(|| "anonymous".to_string());
                let password = profile.steam_password.clone();
                Some(Arc::new(
                    SteamCmd::new(steamcmd_path, steam_root, Some(login)).with_password(password),
                ))
            } else {
                None
            }
        } else {
            None
        };
        let client = Client::new();
        Ok(Self {
            profile,
            steamcmd,
            client,
            server_list: None,
        })
    }

    // --- Server list ---

    pub async fn fetch_servers(&mut self) -> Result<&ServerList> {
        let list = api::fetch_servers(&self.client).await?;
        self.server_list = Some(list);
        Ok(self.server_list.as_ref().unwrap())
    }

    pub fn get_server_list(&self) -> Option<&ServerList> {
        self.server_list.as_ref()
    }

    pub fn find_server(&self, ip: &str, port: u16) -> Option<&Server> {
        self.server_list
            .as_ref()
            .and_then(|sl| sl.find_server(ip, port))
    }

    pub fn find_server_by_game_port(&self, ip: &str, game_port: u16) -> Option<&Server> {
        self.server_list
            .as_ref()
            .and_then(|sl| sl.find_server_by_game_port(ip, game_port))
    }

    /// Find a server by IP, trying both endpoint port and game port.
    pub fn find_server_flexible(&self, ip: &str, port: u16) -> Option<&Server> {
        self.find_server(ip, port)
            .or_else(|| self.find_server_by_game_port(ip, port))
    }

    // --- Profile ---

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        &mut self.profile
    }

    pub fn save_profile(&self) -> Result<()> {
        self.profile.save()
    }

    /// Reload the profile from disk, replacing the in-memory state.
    pub fn reload_profile(&mut self, path: &std::path::Path) -> Result<()> {
        self.profile = Profile::load(path)?;
        Ok(())
    }

    pub fn add_favorite(&mut self, name: String, ip: String, port: u16, password: Option<String>) {
        self.profile.add_favorite(name, ip, port, password);
    }

    pub fn remove_favorite(&mut self, ip: &str, port: u16) {
        self.profile.remove_favorite(ip, port);
    }

    pub fn add_history(&mut self, name: String, ip: String, port: u16) {
        self.profile.add_history(name, ip, port);
    }

    pub fn get_favorites_with_status(&self) -> Vec<crate::config::FavoriteStatus<'_>> {
        let servers = self
            .server_list
            .as_ref()
            .map(|sl| sl.result.as_slice())
            .unwrap_or(&[]);
        self.profile.get_favorites_with_status(servers)
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
    /// Call this after mutating `profile.steam_login`, `steam_password`,
    /// `steam_root`, or `steamcmd_enabled` so the new values take effect
    /// without restarting the app.
    pub fn rebuild_steamcmd(&mut self) {
        self.steamcmd = if self.profile.steamcmd_enabled {
            let resolved_path = self
                .profile
                .steamcmd_path
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .or_else(find_steamcmd);
            resolved_path.map(|steamcmd_path| {
                let steam_root = self
                    .profile
                    .steam_root
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .or_else(find_steam_root)
                    .unwrap_or_else(default_steamapps_fallback);
                let login = self
                    .profile
                    .steam_login
                    .clone()
                    .unwrap_or_else(|| "anonymous".to_string());
                let password = self.profile.steam_password.clone();
                Arc::new(
                    SteamCmd::new(steamcmd_path, steam_root, Some(login)).with_password(password),
                )
            })
        } else {
            None
        };
    }

    /// Expose the shared HTTP client for background tasks.
    pub fn http_client(&self) -> &Client {
        &self.client
    }

    /// Create a lightweight clone of this controller suitable for use in a
    /// background task that only needs to call `launch_game`.
    /// The server list is not cloned (it can be large).
    pub fn clone_for_launch(&self) -> Self {
        Self {
            profile: self.profile.clone(),
            steamcmd: self.steamcmd.clone(),
            client: self.client.clone(),
            server_list: None,
        }
    }

    /// Get a shared reference to steamcmd for background operations.
    pub fn steamcmd_arc(&self) -> Option<Arc<SteamCmd>> {
        self.steamcmd.clone()
    }

    /// Private helper: get a reference to SteamCmd or return a config error.
    fn steamcmd_ref(&self) -> Result<&Arc<SteamCmd>> {
        self.steamcmd
            .as_ref()
            .ok_or_else(|| Error::Config("SteamCMD not configured".to_string()))
    }

    /// Start a background mod operation with progress reporting.
    /// Returns (progress_receiver, pty_input_sender, join_handle) or an error
    /// if steamcmd is not configured.
    ///
    /// The `PtyInputTx` can be used to send a password (or Steam Guard code)
    /// to the running steamcmd PTY when a `PasswordRequired` progress event
    /// is received.
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

    pub fn has_real_login(&self) -> bool {
        self.steamcmd
            .as_ref()
            .map(|sc| sc.has_real_login())
            .unwrap_or(false)
    }

    // --- Mod management ---

    pub fn get_installed_mods(&self) -> Result<Vec<InstalledMod>> {
        let workshop_path = self.workshop_path()?;
        mods::scan_workshop_dir(&workshop_path)
    }

    /// Get the list of mod IDs required by a server that are not installed.
    pub fn get_missing_mod_ids(&self, server: &Server) -> Result<Vec<u64>> {
        let installed = self.get_installed_mods()?;
        let server_mod_ids: Vec<u64> = server
            .mods
            .iter()
            .map(|m| m.steam_workshop_id as u64)
            .collect();
        Ok(mods::get_missing_mods(&server_mod_ids, &installed))
    }

    /// Install missing mods for a server using SteamCMD.
    /// This is the main workflow matching the bash script's `getNotInstalledMods()`.
    ///
    /// 1. Determine which mods are missing
    /// 2. Download each via steamcmd
    /// 3. Mark each as managed (`.dayz-community-hub` marker)
    /// 4. Create symlinks in the DayZ directory
    /// 5. Verify all mods are present
    pub async fn install_missing_mods(&self, server: &Server) -> Result<InstallResult> {
        let steamcmd = self.steamcmd_ref()?;

        if !steamcmd.has_real_login() {
            return Err(Error::SteamCmd(
                "Workshop downloads require a non-anonymous Steam login".to_string(),
            ));
        }

        let installed = self.get_installed_mods()?;
        let server_mod_ids: Vec<u64> = server
            .mods
            .iter()
            .map(|m| m.steam_workshop_id as u64)
            .collect();
        let missing = mods::get_missing_mods(&server_mod_ids, &installed);

        if missing.is_empty() {
            // Still create symlinks for all server mods
            let workshop_path = self.workshop_path()?;
            let dayz_path = self.dayz_path()?;
            mods::create_mod_symlinks(&workshop_path, &dayz_path, &server_mod_ids)?;

            return Ok(InstallResult {
                installed: Vec::new(),
                failed: Vec::new(),
                total_server_mods: server_mod_ids.len(),
            });
        }

        let mut installed_ids = Vec::new();
        let mut failed = Vec::new();

        for &mod_id in &missing {
            match steamcmd.download_mod(mod_id).await {
                Ok(_) => {
                    // Mark as managed
                    let workshop_path = self.workshop_path()?;
                    let _ = mods::mark_mod_as_managed(&workshop_path, mod_id);
                    installed_ids.push(mod_id);
                }
                Err(e) => {
                    failed.push((mod_id, format!("{}", e)));
                }
            }
        }

        // Create symlinks for ALL server mods (not just newly installed ones)
        let workshop_path = self.workshop_path()?;
        let dayz_path = self.dayz_path()?;
        mods::create_mod_symlinks(&workshop_path, &dayz_path, &server_mod_ids)?;

        // Verify all mods exist
        let still_missing = mods::verify_mods(&workshop_path, &server_mod_ids);
        if !still_missing.is_empty() && failed.is_empty() {
            for id in still_missing {
                failed.push((id, "Mod directory not found after download".to_string()));
            }
        }

        Ok(InstallResult {
            installed: installed_ids,
            failed,
            total_server_mods: server_mod_ids.len(),
        })
    }

    /// Download and install a specific mod by ID.
    pub async fn download_mod(&self, workshop_id: u64) -> Result<()> {
        let steamcmd = self.steamcmd_ref()?;
        steamcmd.download_mod(workshop_id).await?;
        let workshop_path = self.workshop_path()?;
        mods::mark_mod_as_managed(&workshop_path, workshop_id)?;
        Ok(())
    }

    /// Update a single mod.
    pub async fn update_mod(&self, workshop_id: u64) -> Result<()> {
        self.steamcmd_ref()?.update_mod(workshop_id).await
    }

    /// Update all installed mods. Returns results per mod.
    pub async fn update_all_mods(&self) -> Result<Vec<(u64, Result<()>)>> {
        let steamcmd = self.steamcmd_ref()?;
        let installed = self.get_installed_mods()?;
        let ids: Vec<u64> = installed.iter().map(|m| m.id).collect();
        Ok(steamcmd.update_all_mods(&ids).await)
    }

    /// Update all mods required by a specific server.
    pub async fn update_server_mods(&self, server: &Server) -> Result<Vec<(u64, Result<()>)>> {
        let steamcmd = self.steamcmd_ref()?;
        let ids: Vec<u64> = server
            .mods
            .iter()
            .map(|m| m.steam_workshop_id as u64)
            .collect();
        Ok(steamcmd.update_all_mods(&ids).await)
    }

    pub fn delete_mod(&self, mod_id: u64, only_managed: bool) -> Result<()> {
        let workshop_path = self.workshop_path()?;
        mods::delete_mod(&workshop_path, mod_id, only_managed)
    }

    pub fn toggle_mod_managed(&self, mod_id: u64) -> Result<bool> {
        let workshop_path = self.workshop_path()?;
        mods::toggle_mod_managed(&workshop_path, mod_id)
    }

    pub fn cleanup_mods(&self) -> Result<ModManagementStats> {
        let workshop_path = self.workshop_path()?;
        let dayz_path = self.dayz_path()?;
        mods::cleanup_mods(&workshop_path, &dayz_path)
    }

    /// Create symlinks for a server's mods.
    pub fn setup_mod_symlinks(&self, server: &Server) -> Result<Vec<u64>> {
        let workshop_path = self.workshop_path()?;
        let dayz_path = self.dayz_path()?;
        let mod_ids: Vec<u64> = server
            .mods
            .iter()
            .map(|m| m.steam_workshop_id as u64)
            .collect();
        mods::create_mod_symlinks(&workshop_path, &dayz_path, &mod_ids)
    }

    // --- Launch ---

    /// Build DayZ launch arguments for a server (without Steam wrapper).
    pub fn build_launch_args(&self, server: &Server, password: Option<&str>) -> Vec<String> {
        self.build_launch_args_with_extra(server, password, &[])
    }

    /// Like `build_launch_args` but appends `extra_args` after the standard flags.
    pub fn build_launch_args_with_extra(
        &self,
        server: &Server,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Vec<String> {
        let mod_ids: Vec<u64> = server
            .mods
            .iter()
            .map(|m| m.steam_workshop_id as u64)
            .collect();
        launch::build_launch_args(server, &mod_ids, password, &self.profile.options, extra_args)
    }

    /// Build full Steam launch command arguments.
    pub fn build_steam_launch_args(&self, server: &Server, password: Option<&str>) -> Vec<String> {
        self.build_steam_launch_args_with_extra(server, password, &[])
    }

    /// Like `build_steam_launch_args` but appends `extra_args` after the standard flags.
    pub fn build_steam_launch_args_with_extra(
        &self,
        server: &Server,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Vec<String> {
        let args = self.build_launch_args_with_extra(server, password, extra_args);
        launch::build_steam_applaunch_args(
            crate::steamcmd::DAYZ_GAME_ID,
            &args,
            self.profile.player.as_deref(),
        )
    }

    /// Full launch workflow matching the bash script's flow:
    /// 1. Ensure Steam is running (wait for it to be ready if just started)
    /// 2. Build launch args
    /// 3. Launch via `steam -applaunch`
    /// 4. Record in history
    pub async fn launch_game(&mut self, server: &Server, password: Option<&str>) -> Result<()> {
        self.launch_game_with_extra(server, password, &[]).await
    }

    /// Like `launch_game` but appends `extra_args` to the DayZ command line.
    /// Used for direct-connect with user-configured server modes.
    pub async fn launch_game_with_extra(
        &mut self,
        server: &Server,
        password: Option<&str>,
        extra_args: &[String],
    ) -> Result<()> {
        // Start Steam if needed, and find out whether it was already up.
        let already_running = SteamClient::start()?;

        // If Steam was just cold-started, wait for its IPC socket to become
        // ready before sending -applaunch.  Sending too early causes the
        // command to be silently discarded — which is exactly why a second
        // attempt always works.
        if !already_running {
            // Poll until the Steam process is visible in the process table,
            // then add a small extra buffer for IPC initialisation.
            for _ in 0..30u8 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if SteamClient::is_running() {
                    // Steam process is up; give it a couple more seconds to
                    // open its socket and register the game-launch handler.
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    break;
                }
            }
        }

        // Build launch arguments
        let args = self.build_steam_launch_args_with_extra(server, password, extra_args);

        let mut cmd = Command::new("steam");
        cmd.args(&args).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);

        cmd.spawn()?;

        // Record in history
        self.add_history(
            server.name.clone(),
            server.endpoint.ip.clone(),
            server.game_port as u16,
        );
        self.save_profile()?;

        Ok(())
    }
}

/// Describes what kind of background mod operation to perform.
#[derive(Clone)]
pub enum ModOperation {
    /// Install missing mods for a server, then launch
    InstallThenLaunch {
        server: Server,
        password: Option<String>,
    },
    /// Install missing mods (no launch)
    InstallOnly { server: Server },
    /// Update all server mods, then launch
    UpdateThenLaunch {
        server: Server,
        password: Option<String>,
    },
    /// Update all installed mods
    UpdateAll,
    /// Update only mods that are known to be stale (remote_updated > local_updated).
    /// `stale_ids` is the pre-filtered list of (mod_id, name) pairs from the GUI.
    UpdateStale { stale_mods: Vec<(u64, String)> },
    /// Update a single mod
    UpdateOne { mod_id: u64, name: String },
    /// Update a user-specified subset of mods (pre-resolved to (id, name) pairs)
    UpdateSelected { mods: Vec<(u64, String)> },
    /// Install mods by Workshop ID (manual install from Mods tab).
    /// Downloads, marks as managed, and creates symlinks — same as InstallOnly
    /// but driven by a user-provided list instead of a server's mod list.
    InstallManual { mods: Vec<(u64, String)> },
}

/// Spawn a background mod operation with progress reporting.
/// Returns a (receiver, join_handle). The caller polls the receiver for progress messages.
/// When the operation completes, the final message is `ModProgress::Finished`.
pub fn spawn_mod_operation(
    steamcmd: Arc<SteamCmd>,
    workshop_path: PathBuf,
    dayz_path: PathBuf,
    op: ModOperation,
    installed_mods: Vec<InstalledMod>,
) -> (
    mpsc::UnboundedReceiver<ModProgress>,
    PtyInputTx,
    tokio::task::JoinHandle<ModOpResult>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (pty_input_tx, pty_input_rx) = mpsc::unbounded_channel::<String>();

    let handle = tokio::spawn(async move {
        // Wrap in Option so we can move it into whichever branch needs it.
        let mut pty_input_rx = Some(pty_input_rx);

        match op {
            ModOperation::InstallOnly { server }
            | ModOperation::InstallThenLaunch { server, .. } => {
                let server_mod_ids: Vec<u64> = server
                    .mods
                    .iter()
                    .map(|m| m.steam_workshop_id as u64)
                    .collect();
                let missing = mods::get_missing_mods(&server_mod_ids, &installed_mods);

                if missing.is_empty() {
                    let _ = mods::create_mod_symlinks(&workshop_path, &dayz_path, &server_mod_ids);
                    let _ = tx.send(ModProgress::Finished {
                        ok: 0,
                        failed: 0,
                        total: 0,
                        hint: None,
                    });
                    return ModOpResult::InstallDone(InstallResult {
                        installed: Vec::new(),
                        failed: Vec::new(),
                        total_server_mods: server_mod_ids.len(),
                    });
                }

                // Build (id, name) pairs for progress display
                let mods_info: Vec<(u64, String)> = missing
                    .iter()
                    .map(|&id| {
                        let name = server
                            .mods
                            .iter()
                            .find(|m| m.steam_workshop_id as u64 == id)
                            .map(|m| m.name.clone())
                            .unwrap_or_else(|| id.to_string());
                        (id, name)
                    })
                    .collect();

                let pty_rx = pty_input_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1);
                let results = steamcmd.download_mods_with_progress(&mods_info, &tx, pty_rx).await;

                let mut installed_ids = Vec::new();
                let mut failed = Vec::new();
                for (mod_id, result) in &results {
                    match result {
                        Ok(_) => {
                            let _ = mods::mark_mod_as_managed(&workshop_path, *mod_id);
                            installed_ids.push(*mod_id);
                        }
                        Err(e) => {
                            failed.push((*mod_id, format!("{}", e)));
                        }
                    }
                }

                let _ = mods::create_mod_symlinks(&workshop_path, &dayz_path, &server_mod_ids);

                ModOpResult::InstallDone(InstallResult {
                    installed: installed_ids,
                    failed,
                    total_server_mods: server_mod_ids.len(),
                })
            }

            ModOperation::UpdateThenLaunch { server, .. } => {
                let mods_info: Vec<(u64, String)> = server
                    .mods
                    .iter()
                    .map(|m| (m.steam_workshop_id as u64, m.name.clone()))
                    .collect();

                let pty_rx = pty_input_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1);
                let results = steamcmd.download_mods_with_progress(&mods_info, &tx, pty_rx).await;

                let server_mod_ids: Vec<u64> = server
                    .mods
                    .iter()
                    .map(|m| m.steam_workshop_id as u64)
                    .collect();
                let _ = mods::create_mod_symlinks(&workshop_path, &dayz_path, &server_mod_ids);

                let per_mod: Vec<(u64, Result<()>)> = results;
                ModOpResult::UpdateDone(per_mod)
            }

            ModOperation::UpdateAll => {
                let mods_info: Vec<(u64, String)> = installed_mods
                    .iter()
                    .map(|m| (m.id, m.name.clone()))
                    .collect();

                let pty_rx = pty_input_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1);
                let results = steamcmd.download_mods_with_progress(&mods_info, &tx, pty_rx).await;
                ModOpResult::UpdateDone(results)
            }

            ModOperation::UpdateStale { stale_mods } => {
                if stale_mods.is_empty() {
                    // Nothing stale — emit finished immediately, skip Steam restart
                    let _ = tx.send(crate::steamcmd::ModProgress::Finished {
                        ok: 0,
                        failed: 0,
                        total: 0,
                        hint: None,
                    });
                    return ModOpResult::UpdateDone(vec![]);
                }
                let pty_rx = pty_input_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1);
                let results = steamcmd.download_mods_with_progress(&stale_mods, &tx, pty_rx).await;
                ModOpResult::UpdateDone(results)
            }

            ModOperation::UpdateOne { mod_id, name } => {
                let mods_info = vec![(mod_id, name)];
                let pty_rx = pty_input_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1);
                let results = steamcmd.download_mods_with_progress(&mods_info, &tx, pty_rx).await;
                ModOpResult::UpdateDone(results)
            }

            ModOperation::UpdateSelected { mods } => {
                if mods.is_empty() {
                    let _ = tx.send(crate::steamcmd::ModProgress::Finished {
                        ok: 0,
                        failed: 0,
                        total: 0,
                        hint: None,
                    });
                    return ModOpResult::UpdateDone(vec![]);
                }
                let pty_rx = pty_input_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1);
                let results = steamcmd.download_mods_with_progress(&mods, &tx, pty_rx).await;
                ModOpResult::UpdateDone(results)
            }

            ModOperation::InstallManual { mods: mods_info } => {
                if mods_info.is_empty() {
                    let _ = tx.send(crate::steamcmd::ModProgress::Finished {
                        ok: 0,
                        failed: 0,
                        total: 0,
                        hint: None,
                    });
                    return ModOpResult::InstallDone(InstallResult {
                        installed: Vec::new(),
                        failed: Vec::new(),
                        total_server_mods: 0,
                    });
                }

                // Filter out mods that are already installed
                let installed_ids: std::collections::HashSet<u64> =
                    installed_mods.iter().map(|m| m.id).collect();
                let missing: Vec<(u64, String)> = mods_info
                    .iter()
                    .filter(|(id, _)| !installed_ids.contains(id))
                    .cloned()
                    .collect();

                if missing.is_empty() {
                    // All requested mods are already installed — just create symlinks
                    let all_ids: Vec<u64> = mods_info.iter().map(|(id, _)| *id).collect();
                    let _ = mods::create_mod_symlinks(&workshop_path, &dayz_path, &all_ids);
                    let _ = tx.send(crate::steamcmd::ModProgress::Finished {
                        ok: 0,
                        failed: 0,
                        total: 0,
                        hint: None,
                    });
                    return ModOpResult::InstallDone(InstallResult {
                        installed: Vec::new(),
                        failed: Vec::new(),
                        total_server_mods: mods_info.len(),
                    });
                }

                let pty_rx = pty_input_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1);
                let results = steamcmd.download_mods_with_progress(&missing, &tx, pty_rx).await;

                let mut installed_new = Vec::new();
                let mut failed = Vec::new();
                for (mod_id, result) in &results {
                    match result {
                        Ok(_) => {
                            let _ = mods::mark_mod_as_managed(&workshop_path, *mod_id);
                            installed_new.push(*mod_id);
                        }
                        Err(e) => {
                            failed.push((*mod_id, format!("{}", e)));
                        }
                    }
                }

                // Create symlinks for all requested mods (including previously installed ones)
                let all_ids: Vec<u64> = mods_info.iter().map(|(id, _)| *id).collect();
                let _ = mods::create_mod_symlinks(&workshop_path, &dayz_path, &all_ids);

                ModOpResult::InstallDone(InstallResult {
                    installed: installed_new,
                    failed,
                    total_server_mods: mods_info.len(),
                })
            }
        }
    });

    (rx, pty_input_tx, handle)
}

/// Result from a completed background mod operation.
pub enum ModOpResult {
    InstallDone(InstallResult),
    UpdateDone(Vec<(u64, Result<()>)>),
}

/// Result of installing missing mods for a server.
pub struct InstallResult {
    pub installed: Vec<u64>,
    pub failed: Vec<(u64, String)>,
    pub total_server_mods: usize,
}

impl InstallResult {
    pub fn all_success(&self) -> bool {
        self.failed.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.installed.is_empty() && self.failed.is_empty() {
            "All mods already installed".to_string()
        } else if self.failed.is_empty() {
            format!("Successfully installed {} mods", self.installed.len())
        } else {
            format!(
                "Installed {} mods, {} failed",
                self.installed.len(),
                self.failed.len()
            )
        }
    }
}
