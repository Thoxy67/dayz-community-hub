use crate::Result;
use crate::errors::Error;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Strip ANSI/VT100 escape sequences from a string.
/// steamcmd embeds colour codes even when stdout is a pipe, e.g. `\x1b[1m`.
fn strip_ansi(s: &str) -> std::borrow::Cow<'_, str> {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let re = ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());
    re.replace_all(s, "")
}

/// Returns true if the line (after stripping ANSI codes) indicates steamcmd is
/// waiting for any kind of Steam Guard confirmation — either mobile app approval
/// or an email/SMS code entered via stdin.
///
/// Matches patterns emitted by steamcmd:
///   1. "Please confirm the login in the Steam Mobile app on your phone."
///   2. "Waiting for confirmation..." (repeated while polling for mobile)
///   3. "Steam Guard code:" (email/SMS — steamcmd reads the code from stdin)
fn is_steam_guard_prompt(line: &str) -> bool {
    let clean = strip_ansi(line);
    let lower = clean.to_lowercase();
    (lower.contains("please confirm") && (lower.contains("mobile") || lower.contains("phone")))
        || lower.contains("waiting for confirmation")
        || lower.contains("steam guard code:")
}

pub const DAYZ_GAME_ID: u32 = 221100;

/// Try to find Steam root directory by checking common locations.
/// Returns the path to the `steamapps` directory.
pub fn find_steam_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // Windows: Steam installs to Program Files (x86) by default,
        // or a user-chosen drive. Check common locations and registry env vars.
        let candidates: Vec<PathBuf> = {
            let mut v = Vec::new();
            // STEAM_PATH env override (power users)
            if let Ok(p) = std::env::var("STEAM_PATH") {
                v.push(PathBuf::from(&p).join("steamapps"));
            }
            // Read the install path from the Windows registry.
            // Steam writes its install directory to:
            //   HKCU\Software\Valve\Steam  →  SteamPath (REG_SZ)
            // This handles custom install drives/directories reliably.
            if let Some(reg_path) = query_steam_registry_path() {
                v.push(PathBuf::from(&reg_path).join("steamapps"));
            }
            // Default install locations (fallback when registry key is absent)
            for base in &["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"] {
                v.push(PathBuf::from(base).join("steamapps"));
            }
            // Per-user roaming / local variants
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                v.push(PathBuf::from(&local).join("Steam").join("steamapps"));
            }
            v
        };
        for candidate in &candidates {
            if candidate.exists() && candidate.is_dir() {
                return Some(candidate.clone());
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").ok()?;
        let home_path = PathBuf::from(home);
        let candidates = [
            home_path.join(".steam/steam/steamapps"),
            home_path.join(".local/share/Steam/steamapps"),
            home_path.join(".steam/root/steamapps"),
            home_path.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps"),
        ];
        for candidate in candidates.iter() {
            if candidate.exists() && candidate.is_dir() {
                return Some(candidate.to_path_buf());
            }
        }
        None
    }
}

/// Read the Steam install path from the Windows registry.
///
/// Queries `HKCU\Software\Valve\Steam` → `SteamPath` (REG_SZ).
/// This is the most reliable way to find Steam when it was installed
/// to a non-default drive or directory (e.g. `D:\Games\Steam`).
#[cfg(target_os = "windows")]
fn query_steam_registry_path() -> Option<String> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("reg")
        .args(["query", "HKCU\\Software\\Valve\\Steam", "/v", "SteamPath"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // reg query output format:
    //   HKEY_CURRENT_USER\Software\Valve\Steam
    //       SteamPath    REG_SZ    C:/Program Files (x86)/Steam
    for line in stdout.lines() {
        let line = line.trim();
        if line.to_lowercase().starts_with("steampath") {
            // Split on "REG_SZ" and take the value after it
            if let Some(pos) = line.to_uppercase().find("REG_SZ") {
                let value = line[pos + "REG_SZ".len()..].trim();
                if !value.is_empty() {
                    // Steam writes forward slashes in the registry; normalise to backslashes.
                    return Some(value.replace('/', "\\"));
                }
            }
        }
    }
    None
}

/// Try to find steamcmd binary in PATH or common locations.
pub fn find_steamcmd() -> Option<PathBuf> {
    // Honour PATH first (works on all platforms)
    #[cfg(target_os = "windows")]
    let binary = "steamcmd.exe";
    #[cfg(not(target_os = "windows"))]
    let binary = "steamcmd";

    if let Ok(path) = which::which(binary) {
        return Some(path);
    }

    #[cfg(target_os = "windows")]
    {
        let candidates: Vec<PathBuf> = {
            let mut v = Vec::new();
            // If Steam is found via registry, its bundled steamcmd is in the same directory.
            if let Some(steam_path) = query_steam_registry_path() {
                v.push(PathBuf::from(&steam_path).join("steamcmd.exe"));
            }
            for base in &[
                "C:\\Program Files (x86)\\Steam\\steamcmd.exe",
                "C:\\Program Files (x86)\\SteamCMD\\steamcmd.exe",
                "C:\\SteamCMD\\steamcmd.exe",
            ] {
                v.push(PathBuf::from(base));
            }
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                v.push(
                    PathBuf::from(&local)
                        .join("Programs")
                        .join("steamcmd")
                        .join("steamcmd.exe"),
                );
            }
            v
        };
        for candidate in &candidates {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").ok()?;
        let home_path = PathBuf::from(home);
        let candidates = [
            home_path.join(".steam/steamcmd/steamcmd.sh"),
            home_path.join(".local/share/Steam/steamcmd/steamcmd.sh"),
            PathBuf::from("/usr/games/steamcmd"),
            PathBuf::from("/usr/local/games/steamcmd"),
            PathBuf::from("/usr/bin/steamcmd"),
        ];
        for candidate in candidates.iter() {
            if candidate.exists() {
                return Some(candidate.to_path_buf());
            }
        }
        None
    }
}

/// Progress messages sent during mod download/update operations.
#[derive(Debug, Clone)]
pub enum ModProgress {
    /// Steam is being shut down to avoid session conflict with steamcmd
    ShuttingDownSteam,
    /// steamcmd is waiting for Steam Guard Mobile confirmation on the user's phone
    SteamGuardMobileRequired,
    /// Starting download/update for a mod: (current_index, total_count, mod_id, mod_name)
    Starting {
        current: usize,
        total: usize,
        mod_id: u64,
        name: String,
    },
    /// A mod finished successfully
    Done {
        current: usize,
        total: usize,
        mod_id: u64,
        name: String,
    },
    /// A mod failed
    Failed {
        current: usize,
        total: usize,
        mod_id: u64,
        name: String,
        error: String,
    },
    /// A raw steamcmd output line (for the log panel in the UI)
    LogLine(String),
    /// All mods processed
    Finished {
        ok: usize,
        failed: usize,
        total: usize,
        /// Optional hint for the user (e.g. steamcmd re-login command)
        hint: Option<String>,
    },
}

/// Sender half for progress reporting.
pub type ProgressTx = mpsc::UnboundedSender<ModProgress>;

pub struct SteamCmd {
    steamcmd_path: PathBuf,
    steam_root: PathBuf,
    login: String,
    /// Optional password for non-interactive login. When set, steamcmd is
    /// invoked as `+login <user> <password>` so it doesn't rely on cached
    /// credentials. When absent, only the username is passed and steamcmd
    /// falls back to its credential cache.
    password: Option<String>,
    game_id: u32,
}

impl SteamCmd {
    pub fn new(
        steamcmd_path: impl AsRef<Path>,
        steam_root: impl AsRef<Path>,
        login: Option<String>,
    ) -> Self {
        let login = login.unwrap_or_else(|| "anonymous".to_string());
        Self {
            steamcmd_path: steamcmd_path.as_ref().to_path_buf(),
            steam_root: steam_root.as_ref().to_path_buf(),
            login,
            password: None,
            game_id: DAYZ_GAME_ID,
        }
    }

    pub fn with_password(mut self, password: Option<String>) -> Self {
        self.password = password;
        self
    }

    pub fn with_game_id(mut self, game_id: u32) -> Self {
        self.game_id = game_id;
        self
    }

    pub fn login(&self) -> &str {
        &self.login
    }

    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Push `+login <user>` or `+login <user> <password>` args onto a command.
    fn push_login_args(&self, cmd: &mut Command) {
        cmd.arg("+login").arg(&self.login);
        if let Some(ref pw) = self.password {
            cmd.arg(pw);
        }
    }

    pub fn steam_root(&self) -> &Path {
        &self.steam_root
    }

    /// Returns true if the login is non-anonymous (required for workshop downloads).
    pub fn has_real_login(&self) -> bool {
        !self.login.is_empty() && self.login != "anonymous"
    }

    /// Cache login credentials by running `steamcmd +login <user> [<password>] +quit`.
    /// If no password is saved, this is interactive and will prompt for password/2FA.
    pub async fn cache_login(&self) -> Result<()> {
        let mut cmd = Command::new(&self.steamcmd_path);
        self.push_login_args(&mut cmd);
        let status = cmd
            .arg("+quit")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await?;

        if status.success() {
            Ok(())
        } else {
            Err(Error::SteamCmd(format!(
                "steamcmd login failed. Try running: steamcmd +login {} +quit",
                self.login
            )))
        }
    }

    /// Download a mod from the Steam Workshop (always validates).
    /// This is the equivalent of the bash script's `workshopDownload()`.
    pub async fn download_mod(&self, workshop_id: u64) -> Result<()> {
        if !self.has_real_login() {
            return Err(Error::SteamCmd(
                "Workshop downloads require a non-anonymous Steam login. \
                 Set your steam_login in the profile."
                    .to_string(),
            ));
        }
        self.workshop_download_item(workshop_id).await
    }

    /// Update a mod (same as download -- steamcmd handles delta updates).
    pub async fn update_mod(&self, workshop_id: u64) -> Result<()> {
        self.download_mod(workshop_id).await
    }

    /// Internal: run steamcmd workshop_download_item with validate.
    /// Matches the bash script's command:
    /// `steamcmd +@ShutdownOnFailedCommand 1 +login <user> +workshop_download_item 221100 <id> validate +quit`
    ///
    /// Shuts down the Steam client first (shared auth session — both cannot run
    /// simultaneously without bumping the user offline).
    ///
    /// Streams stdout to detect "Cached credentials not found" -- if seen,
    /// kills the process immediately (it would hang waiting for password)
    /// and returns an error telling the user how to re-login.
    async fn workshop_download_item(&self, workshop_id: u64) -> Result<()> {
        // Steam and steamcmd share the same auth session. Shut Steam down
        // first so steamcmd doesn't bump the user offline mid-session.
        SteamClient::shutdown_for_steamcmd().await;

        let mut cmd = Command::new(&self.steamcmd_path);
        cmd.arg("+@ShutdownOnFailedCommand").arg("1");
        // Direct steamcmd to write workshop content into the real Steam steamapps tree,
        // not steamcmd's own directory. +force_install_dir must come before +login.
        if let Some(steam_parent) = self.steam_root.parent() {
            cmd.arg("+force_install_dir").arg(steam_parent);
        }
        self.push_login_args(&mut cmd);
        cmd.arg("+workshop_download_item")
            .arg(self.game_id.to_string())
            .arg(workshop_id.to_string())
            .arg("validate")
            .arg("+quit")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        let mut child = cmd.spawn()?;

        // Read both stdout and stderr as raw chunks (not lines) so we can detect
        // "Cached credentials not found" even if steamcmd doesn't terminate with '\n'.
        let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel::<String>();
        if let Some(stdout) = child.stdout.take() {
            let ctx = chunk_tx.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                let mut reader = BufReader::new(stdout);
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                            let _ = ctx.send(s);
                        }
                        Err(_) => break,
                    }
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let ctx = chunk_tx.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                let mut reader = BufReader::new(stderr);
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                            let _ = ctx.send(s);
                        }
                        Err(_) => break,
                    }
                }
            });
        }
        drop(chunk_tx);

        let mut all_output = String::new();
        while let Some(chunk) = chunk_rx.recv().await {
            all_output.push_str(&chunk);
            let clean = strip_ansi(&all_output);
            if clean.contains("Cached credentials not found") {
                let _ = child.kill().await;
                return Err(Error::CredentialsExpired(format!(
                    "steamcmd +login {} +quit",
                    self.login
                )));
            }
        }

        let status = child.wait().await?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::SteamCmd(format!(
                "steamcmd failed for mod {}. Try re-logging:\n  steamcmd +login {} +quit",
                workshop_id, self.login
            )))
        }
    }

    /// Path where workshop mods are stored: `steamapps/workshop/content/221100/`
    pub fn workshop_path(&self) -> PathBuf {
        self.steam_root
            .join("workshop")
            .join("content")
            .join(self.game_id.to_string())
    }

    /// Path to the DayZ game directory: `steamapps/common/DayZ`
    pub fn dayz_path(&self) -> PathBuf {
        self.steam_root.join("common").join("DayZ")
    }

    /// Update the DayZ game itself via steamcmd.
    pub async fn update_game(&self) -> Result<()> {
        let mut cmd = Command::new(&self.steamcmd_path);
        cmd.arg("+@ShutdownOnFailedCommand").arg("1");
        self.push_login_args(&mut cmd);
        cmd.arg("+app_update")
            .arg(self.game_id.to_string())
            .arg("+quit")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        let output = cmd.output().await?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(Error::SteamCmd(format!(
                "Failed to update game: {}",
                stderr
            )))
        }
    }

    /// Update multiple mods. Returns a Vec of (mod_id, Result) pairs.
    pub async fn update_all_mods(&self, mod_ids: &[u64]) -> Vec<(u64, Result<()>)> {
        let mut results = Vec::new();
        for &mod_id in mod_ids {
            let result = self.update_mod(mod_id).await;
            results.push((mod_id, result));
        }
        results
    }

    /// Download/update multiple mods with per-mod progress.
    ///
    /// Batches all mods into a single steamcmd invocation and streams stdout
    /// line-by-line via a PTY (Linux) / ConPTY (Windows). Both platforms
    /// receive real-time output so progress is reported as each mod finishes.
    ///
    /// `mods_info` is a list of (mod_id, display_name) pairs.
    pub async fn download_mods_with_progress(
        &self,
        mods_info: &[(u64, String)],
        tx: &ProgressTx,
    ) -> Vec<(u64, Result<()>)> {
        let total = mods_info.len();

        if total == 0 {
            let _ = tx.send(ModProgress::Finished {
                ok: 0,
                failed: 0,
                total: 0,
                hint: None,
            });
            return Vec::new();
        }

        if !self.has_real_login() {
            let err = "Workshop downloads require a non-anonymous Steam login".to_string();
            let results: Vec<(u64, Result<()>)> = mods_info
                .iter()
                .map(|(id, _)| (*id, Err(Error::SteamCmd(err.clone()))))
                .collect();
            let _ = tx.send(ModProgress::Finished {
                ok: 0,
                failed: total,
                total,
                hint: Some(
                    "Set your Steam login in the profile, then run:\n  steamcmd +login YOUR_USERNAME +quit"
                        .to_string(),
                ),
            });
            return results;
        }

        // Steam and steamcmd share the same auth session — running both at the same
        // time kicks the user offline. Shut Steam down cleanly before starting steamcmd.
        let _ = tx.send(ModProgress::ShuttingDownSteam);
        SteamClient::shutdown_for_steamcmd().await;

        // Use a single batched PTY invocation on all platforms.
        // ConPTY on Windows provides the same real-time line-buffered output
        // as a Linux PTY, so there is no need for the one-per-mod fallback.
        self.download_mods_batched(mods_info, tx, total).await
    }

    /// Spawn steamcmd under a PTY and stream its output as raw byte chunks.
    ///
    /// Returns `(child, chunk_rx)`. The caller drains `chunk_rx` until it closes
    /// (signals EOF / process exit), then calls `child.wait()`.
    ///
    /// Using a PTY forces steamcmd's stdio into line-buffered mode so every
    /// line arrives immediately (Linux PTY / Windows ConPTY).
    ///
    /// **Windows ConPTY caveat**: the master reader does *not* return EOF when
    /// the child exits — ConPTY keeps the handle open. We work around this by
    /// running a second watcher thread that calls `child.wait()` and then drops
    /// the master, which finally unblocks the reader and lets it return an error,
    /// breaking the read loop and closing `chunk_rx`.
    fn spawn_pty_streamed(
        &self,
        args: &[std::ffi::OsString],
    ) -> std::result::Result<
        (
            Box<dyn portable_pty::Child + Send + Sync>,
            mpsc::UnboundedReceiver<String>,
        ),
        String,
    > {
        use portable_pty::{CommandBuilder, PtySize, native_pty_system};
        use std::io::Read;
        use std::sync::{Arc, Mutex};

        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 220,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let mut cmd = CommandBuilder::new(&self.steamcmd_path);
        for arg in args {
            cmd.arg(arg);
        }

        let mut child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to start steamcmd: {}", e))?;
        drop(pty_pair.slave);

        let mut reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

        // Wrap the master in an Arc<Mutex<Option<...>>> so the watcher thread
        // can drop it (closing the ConPTY handle) once the child exits, which
        // unblocks the reader thread that is blocked on `reader.read()`.
        let master_holder: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> =
            Arc::new(Mutex::new(Some(pty_pair.master)));
        let master_for_watcher = Arc::clone(&master_holder);

        // Watcher thread: wait for the child to exit, then drop the master.
        // `child` is moved here; the caller receives a dummy handle below.
        let (exit_tx, exit_rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _ = child.wait();
            // Dropping the master closes the ConPTY output pipe, which makes
            // the reader return an error and exit its loop.
            drop(master_for_watcher.lock().unwrap().take());
            let _ = exit_tx.send(());
        });

        let (chunk_tx, chunk_rx) = mpsc::unbounded_channel::<String>();
        tokio::task::spawn_blocking(move || {
            // Keep _master_holder alive until the watcher drops it.
            let _master_holder = master_holder;
            let mut buf = [0u8; 256];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                        let _ = chunk_tx.send(s);
                    }
                }
            }
            // Drain the exit channel so the watcher thread is not leaked.
            let _ = exit_rx.recv();
        });

        // Return a no-op child since the real child is owned by the watcher thread.
        Ok((Box::new(NoopChild), chunk_rx))
    }

    /// Batch all mods into a single steamcmd invocation, stream stdout via PTY.
    /// Works on both Linux (PTY) and Windows (ConPTY).
    async fn download_mods_batched(
        &self,
        mods_info: &[(u64, String)],
        tx: &ProgressTx,
        total: usize,
    ) -> Vec<(u64, Result<()>)> {
        let _ = tx.send(ModProgress::Starting {
            current: 1,
            total,
            mod_id: mods_info[0].0,
            name: mods_info[0].1.clone(),
        });

        let mut args: Vec<std::ffi::OsString> = Vec::new();
        args.push("+@ShutdownOnFailedCommand".into());
        args.push("0".into());
        if let Some(steam_parent) = self.steam_root.parent() {
            args.push("+force_install_dir".into());
            args.push(steam_parent.as_os_str().into());
        }
        args.push("+login".into());
        args.push((&self.login).into());
        if let Some(ref pw) = self.password {
            args.push(pw.into());
        }
        for (mod_id, _) in mods_info {
            args.push("+workshop_download_item".into());
            args.push(self.game_id.to_string().into());
            args.push(mod_id.to_string().into());
            args.push("validate".into());
        }
        args.push("+quit".into());

        let (mut child, mut chunk_rx) = match self.spawn_pty_streamed(&args) {
            Ok(v) => v,
            Err(e) => {
                let results = mods_info
                    .iter()
                    .map(|(id, _)| (*id, Err(Error::SteamCmd(e.clone()))))
                    .collect();
                let _ = tx.send(ModProgress::Finished {
                    ok: 0,
                    failed: total,
                    total,
                    hint: None,
                });
                return results;
            }
        };

        // Accumulate all output so we can split on newlines ourselves.
        // Steam Guard mobile is detected by text: steamcmd flushes the
        // "Please confirm..." / "Waiting for confirmation..." lines once the
        // user approves on the phone.
        let mut succeeded = std::collections::HashSet::<u64>::new();
        let mut credentials_failed = false;
        let mut current_idx: usize = 0;
        let mut steam_guard_sent = false;
        let mut login_phase = true; // true until first download activity is seen
        let mut buf = String::new();
        let has_password = self.password.is_some();

        loop {
            let maybe_chunk = chunk_rx.recv().await;
            let raw_chunk = match maybe_chunk {
                Some(c) => c,
                None => break, // channel closed — steamcmd exited
            };
            buf.push_str(&raw_chunk);

            // Process any complete newline-terminated lines that have accumulated.
            while let Some(nl_pos) = buf.find('\n') {
                let raw_line = buf[..nl_pos].to_string();
                buf = buf[nl_pos + 1..].to_string();
                let line = strip_ansi(&raw_line).into_owned();

                // Forward every non-empty line to the UI log panel.
                if !line.trim().is_empty() {
                    let _ = tx.send(ModProgress::LogLine(line.clone()));
                }

                if line.contains("Cached credentials not found") {
                    credentials_failed = true;
                    let _ = child.kill();
                    // Drain is not needed — we'll break the outer loop below
                    break;
                }
                // Text-based Steam Guard detection on complete lines.
                if has_password && login_phase && !steam_guard_sent && is_steam_guard_prompt(&line)
                {
                    let _ = tx.send(ModProgress::SteamGuardMobileRequired);
                    steam_guard_sent = true;
                }
                if line.contains("Success. Downloaded item") || line.contains("already up to date")
                {
                    login_phase = false;
                    if let Some(id) = extract_mod_id_from_line(&line) {
                        succeeded.insert(id);
                        if let Some((idx, (_, name))) = mods_info
                            .iter()
                            .enumerate()
                            .find(|(_, (mid, _))| *mid == id)
                        {
                            let _ = tx.send(ModProgress::Done {
                                current: idx + 1,
                                total,
                                mod_id: id,
                                name: name.clone(),
                            });
                            current_idx = idx + 1;
                        }
                    }
                }
                if line.contains("Downloading item") {
                    login_phase = false;
                    if let Some(id) = extract_mod_id_from_line(&line) {
                        if let Some((idx, (_, name))) = mods_info
                            .iter()
                            .enumerate()
                            .find(|(_, (mid, _))| *mid == id)
                        {
                            if idx + 1 > current_idx {
                                let _ = tx.send(ModProgress::Starting {
                                    current: idx + 1,
                                    total,
                                    mod_id: id,
                                    name: name.clone(),
                                });
                                current_idx = idx + 1;
                            }
                        }
                    }
                }
            }
            // Also check the partial-line remainder (content not yet terminated
            // by '\n'). steamcmd writes some prompts without a trailing newline,
            // so they would never be processed by the loop above.
            if has_password && login_phase && !steam_guard_sent && is_steam_guard_prompt(&buf) {
                let _ = tx.send(ModProgress::SteamGuardMobileRequired);
                steam_guard_sent = true;
            }
            if credentials_failed {
                break;
            }
        }

        let _ = child.wait();

        if credentials_failed {
            let relogin_cmd = format!("steamcmd +login {} +quit", self.login);
            let hint = format!(
                "Cached credentials not found.\nRun this command to re-login:\n  {}",
                relogin_cmd
            );
            let results: Vec<(u64, Result<()>)> = mods_info
                .iter()
                .map(|(id, name)| {
                    if !succeeded.contains(id) {
                        let _ = tx.send(ModProgress::Failed {
                            current: 1,
                            total,
                            mod_id: *id,
                            name: name.clone(),
                            error: hint.clone(),
                        });
                    }
                    (*id, Err(Error::CredentialsExpired(relogin_cmd.clone())))
                })
                .collect();
            let _ = tx.send(ModProgress::Finished {
                ok: 0,
                failed: total,
                total,
                hint: Some(hint),
            });
            return results;
        }

        // Build final results — anything not seen as succeeded is failed
        let results: Vec<(u64, Result<()>)> = mods_info
            .iter()
            .enumerate()
            .map(|(idx, (id, name))| {
                if succeeded.contains(id) {
                    (*id, Ok(()))
                } else {
                    let _ = tx.send(ModProgress::Failed {
                        current: idx + 1,
                        total,
                        mod_id: *id,
                        name: name.clone(),
                        error: "Download failed".to_string(),
                    });
                    (*id, Err(Error::SteamCmd("Download failed".to_string())))
                }
            })
            .collect();

        let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
        let failed = results.iter().filter(|(_, r)| r.is_err()).count();
        let _ = tx.send(ModProgress::Finished {
            ok,
            failed,
            total,
            hint: if failed > 0 {
                Some(format!(
                    "Some downloads failed. Try:\n  steamcmd +login {} +quit",
                    self.login
                ))
            } else {
                None
            },
        });
        results
    }
}

/// A no-op `portable_pty::Child` returned by `spawn_pty_streamed`.
///
/// The real child process is owned by the watcher thread inside
/// `spawn_pty_streamed`, which calls `child.wait()` and then drops the PTY
/// master to unblock the reader. Callers receive this stub so they can still
/// call `.kill()` (no-op) and `.wait()` (no-op — the watcher already waited).
#[derive(Debug)]
struct NoopChild;

impl portable_pty::ChildKiller for NoopChild {
    fn kill(&mut self) -> std::io::Result<()> {
        Ok(())
    }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        Box::new(NoopChild)
    }
}

impl portable_pty::Child for NoopChild {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        Ok(Some(portable_pty::ExitStatus::with_exit_code(0)))
    }
    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        Ok(portable_pty::ExitStatus::with_exit_code(0))
    }
    fn process_id(&self) -> Option<u32> {
        None
    }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        None
    }
}

/// Regex for extracting a workshop mod ID from a steamcmd output line — compiled once.
static MOD_ID_RE: OnceLock<Regex> = OnceLock::new();

/// Extract a workshop mod ID (number) from a steamcmd output line.
/// Matches patterns like "item 1559212036" or "item:1559212036".
fn extract_mod_id_from_line(line: &str) -> Option<u64> {
    let re = MOD_ID_RE.get_or_init(|| Regex::new(r"item[: ]+(\d+)").unwrap());
    re.captures(line)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
}

/// Steam client management (detect, start, shutdown).
pub struct SteamClient;

impl SteamClient {
    /// Returns the Steam executable name for the current platform.
    pub fn steam_exe() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "steam.exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "steam"
        }
    }

    /// Check if the Steam client is currently running.
    pub fn is_running() -> bool {
        use sysinfo::System;
        let mut system = System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        system.processes().values().any(|process| {
            let name = process.name().to_string_lossy().to_lowercase();
            // "steam" — native Linux / macOS / Flatpak entry-point.
            // "steam.exe" — Windows and Wine/Proton on Linux.
            // Exclude "steamwebhelper" — it can outlive a crashed client.
            name == "steam" || name == "steam.exe"
        })
    }

    /// Start Steam in silent mode (no friends UI).
    ///
    /// Returns `true` if Steam was already running, `false` if it was just
    /// started (so the caller knows whether to wait for it to become ready).
    pub fn start() -> Result<bool> {
        if Self::is_running() {
            return Ok(true);
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // On Windows spawn steam.exe directly; no nohup equivalent needed
            // because detached processes persist after the parent exits.
            std::process::Command::new(Self::steam_exe())
                .arg("-nofriendsui")
                .arg("-silent")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            // On Linux/macOS use nohup so Steam keeps running if the spawner exits.
            std::process::Command::new("nohup")
                .arg(Self::steam_exe())
                .arg("-nofriendsui")
                .arg("-silent")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }

        Ok(false)
    }

    /// Graceful shutdown via `steam -shutdown`.
    pub async fn shutdown() -> Result<()> {
        let mut cmd = Command::new(Self::steam_exe());
        cmd.arg("-shutdown")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        let _ = cmd.spawn()?.wait().await;
        // Give it time to shut down
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        Ok(())
    }

    /// Force kill all Steam processes.
    pub fn shutdown_force() -> Result<()> {
        use sysinfo::System;
        let mut system = System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        for process in system.processes().values() {
            let name = process.name().to_string_lossy().to_lowercase();
            if name == "steam"
                || name == "steam.exe"
                || name == "steamwebhelper"
                || name.starts_with("steam")
            {
                process.kill();
            }
        }
        Ok(())
    }

    /// Shut down Steam gracefully before running steamcmd.
    ///
    /// steamcmd and the Steam client share the same auth session — running both
    /// simultaneously causes Steam to kick you offline. This function:
    ///   1. Does nothing if Steam is not running.
    ///   2. Sends `steam -shutdown` and waits up to 15 s for all processes to exit.
    ///   3. Force-kills any remaining Steam processes if they didn't exit in time.
    pub async fn shutdown_for_steamcmd() {
        if !Self::is_running() {
            return;
        }

        // Ask Steam to shut down gracefully
        let mut cmd = Command::new(Self::steam_exe());
        cmd.arg("-shutdown")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        let _ = cmd.spawn();

        // Poll every 500 ms for up to 15 s
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if !Self::is_running() {
                return;
            }
        }

        // Still running — force kill
        Self::shutdown_force().ok();

        // Brief pause to let OS release file locks before steamcmd starts
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
