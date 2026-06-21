use crate::Result;
use crate::errors::Error;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Bundle of handles handed back from `spawn_pty_streamed`:
/// (no-op child wrapper, stdout chunk receiver, PTY writer for stdin).
type PtyStreamHandles = (
    Box<dyn portable_pty::Child + Send + Sync>,
    mpsc::UnboundedReceiver<String>,
    Box<dyn std::io::Write + Send>,
);

/// Strip ANSI/VT100 escape sequences from a string.
/// steamcmd embeds colour codes even when stdout is a pipe, e.g. `\x1b[1m`.
///
/// Fast path: most steamcmd output lines contain no ANSI escapes at all,
/// so we short-circuit with a single byte scan and return the original
/// borrowed slice without invoking the regex engine.
fn strip_ansi(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.as_bytes().contains(&0x1b) {
        return std::borrow::Cow::Borrowed(s);
    }
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let re = ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());
    re.replace_all(s, "")
}

/// ASCII-only case-insensitive `contains`.  Avoids the per-line
/// `to_lowercase()` String allocation that the previous implementation made
/// for every steamcmd line we matched against multiple patterns.
fn ascii_contains_ci(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() {
        return true;
    }
    if h.len() < n.len() {
        return false;
    }
    'outer: for start in 0..=h.len() - n.len() {
        for (i, &nb) in n.iter().enumerate() {
            if !h[start + i].eq_ignore_ascii_case(&nb) {
                continue 'outer;
            }
        }
        return true;
    }
    false
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
    let l = clean.as_ref();
    (ascii_contains_ci(l, "please confirm")
        && (ascii_contains_ci(l, "mobile") || ascii_contains_ci(l, "phone")))
        || ascii_contains_ci(l, "waiting for confirmation")
        || ascii_contains_ci(l, "steam guard code:")
}

/// Returns true if the partial buffer ends with a `password:` prompt.
/// steamcmd emits this without a trailing newline when cached credentials
/// are not found and it falls back to interactive login.
fn is_password_prompt(buf: &str) -> bool {
    let clean = strip_ansi(buf);
    let trimmed = clean.trim_end();
    trimmed.ends_with("password:") || trimmed.ends_with("password: ")
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
        // Cover the major Linux Steam install sources: native, Flatpak, Snap.
        let candidates = [
            home_path.join(".steam/steam/steamapps"),
            home_path.join(".local/share/Steam/steamapps"),
            home_path.join(".steam/root/steamapps"),
            // Flatpak Steam (com.valvesoftware.Steam)
            home_path.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps"),
            home_path.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps"),
            // Snap Steam
            home_path.join("snap/steam/common/.local/share/Steam/steamapps"),
            home_path.join("snap/steam/current/.local/share/Steam/steamapps"),
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
        // Linux + macOS: covers the most common package sources users hit.
        // Snap and Flatpak ship steamcmd in non-PATH locations, so users
        // with those installs were previously falling through to "not found"
        // even when a working binary was present.
        let candidates = [
            home_path.join(".steam/steamcmd/steamcmd.sh"),
            home_path.join(".local/share/Steam/steamcmd/steamcmd.sh"),
            // Flatpak (steamcmd packaged separately from the Steam flatpak)
            home_path.join(".var/app/com.valvesoftware.SteamCMD/data/steamcmd.sh"),
            home_path.join(".var/app/com.valvesoftware.SteamCMD/.local/share/Steam/steamcmd.sh"),
            // Standard Linux distribution paths
            PathBuf::from("/usr/games/steamcmd"),
            PathBuf::from("/usr/local/games/steamcmd"),
            PathBuf::from("/usr/bin/steamcmd"),
            PathBuf::from("/usr/local/bin/steamcmd"),
            // Snap (snap shadows /usr/bin in PATH on some distros, but the
            // direct path is more reliable for detection)
            PathBuf::from("/snap/bin/steamcmd"),
            PathBuf::from("/var/lib/snapd/snap/bin/steamcmd"),
            // AUR package — installs into /opt
            PathBuf::from("/opt/steamcmd/steamcmd.sh"),
            // macOS Homebrew
            PathBuf::from("/usr/local/opt/steamcmd/bin/steamcmd"),
            PathBuf::from("/opt/homebrew/bin/steamcmd"),
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
    /// steamcmd is prompting for a password (cached credentials not found).
    /// The UI should show a password input and send the password via the
    /// `PtyInputTx` channel.
    PasswordRequired,
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

/// Channel for the frontend to send input (e.g. password) to the running
/// steamcmd PTY. The receiver lives inside the download task.
pub type PtyInputTx = mpsc::UnboundedSender<String>;
/// Receiver half — held by the download task.
pub type PtyInputRx = mpsc::UnboundedReceiver<String>;

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
        pty_input: PtyInputRx,
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
        self.download_mods_batched(mods_info, tx, total, pty_input)
            .await
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
    /// PTY writer handle — wrapped so it can be shared across threads.
    /// The download task uses this to send password / Steam Guard codes to
    /// the running steamcmd process.
    ///
    /// Bundles the three handles spawn_pty_streamed hands back: the (no-op)
    /// child, the stdout chunk receiver, and the PTY writer.
    fn spawn_pty_streamed(
        &self,
        args: &[std::ffi::OsString],
    ) -> std::result::Result<PtyStreamHandles, String> {
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

        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

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
            // 4 KiB read buffer — 16x fewer syscalls and channel sends than
            // the prior 256-byte buffer, which dominated CPU during big mod
            // downloads.  steamcmd's PTY output is line-oriented and already
            // small per line, so this only batches what would otherwise be
            // multiple syscalls back-to-back.
            let mut buf = [0u8; 4096];
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
        Ok((Box::new(NoopChild), chunk_rx, writer))
    }

    /// Batch all mods into a single steamcmd invocation, stream stdout via PTY.
    /// Works on both Linux (PTY) and Windows (ConPTY).
    async fn download_mods_batched(
        &self,
        mods_info: &[(u64, String)],
        tx: &ProgressTx,
        total: usize,
        mut pty_input: PtyInputRx,
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

        let (mut child, mut chunk_rx, mut pty_writer) = match self.spawn_pty_streamed(&args) {
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
        let mut succeeded =
            rustc_hash::FxHashSet::<u64>::with_capacity_and_hasher(mods_info.len(), Default::default());
        // O(1) mod_id → (idx, name) lookup so we don't linear-scan mods_info
        // on every "Downloading"/"Success" line.  For N mods downloading with
        // ~3 progress matches each, this turns O(N²) into O(N).
        let mod_lookup: rustc_hash::FxHashMap<u64, (usize, &str)> = mods_info
            .iter()
            .enumerate()
            .map(|(idx, (id, name))| (*id, (idx, name.as_str())))
            .collect();
        let mut current_idx: usize = 0;
        let mut steam_guard_sent = false;
        let mut password_prompt_sent = false;
        let mut steam_guard_timed_out = false;
        let mut invalid_password = false;
        let mut sg_start: Option<std::time::Instant> = None;
        let mut login_phase = true; // true until first download activity is seen
        let mut buf = String::new();

        loop {
            let maybe_chunk = chunk_rx.recv().await;
            let raw_chunk = match maybe_chunk {
                Some(c) => c,
                None => break, // channel closed — steamcmd exited
            };
            // Prevent unbounded buffer growth (1 MB limit)
            const MAX_BUF_SIZE: usize = 1024 * 1024;
            if buf.len() + raw_chunk.len() > MAX_BUF_SIZE {
                buf.clear();
                eprintln!("[SteamCmd] output buffer exceeded limit, clearing");
            }
            buf.push_str(&raw_chunk);

            // Process any complete newline-terminated lines that have accumulated.
            // Hot path: a single mod download produces hundreds of progress lines.
            // Previously each line allocated 4–6 Strings (raw_line, buf realloc,
            // strip_ansi.into_owned, line.clone for log, two to_lowercase copies).
            // We now slice into `buf` in place, drain after we're done reading,
            // and use ASCII case-insensitive contains so the only allocation
            // per line is the one the channel send genuinely requires.
            while let Some(nl_pos) = buf.find('\n') {
                // Scope the borrow on `buf` so we can `.drain` after.
                let advance_to = nl_pos + 1;
                {
                    let stripped = strip_ansi(&buf[..nl_pos]);
                    let line: &str = stripped.as_ref();

                    // Forward every non-empty line to the UI log panel.
                    if !line.trim().is_empty() {
                        let _ = tx.send(ModProgress::LogLine(line.to_owned()));
                    }

                    // Text-based Steam Guard detection on complete lines.
                    // NOTE: no `has_password` guard — Steam Guard can appear
                    // after the interactive password prompt flow too.
                    if login_phase && !steam_guard_sent && is_steam_guard_prompt(line) {
                        let _ = tx.send(ModProgress::SteamGuardMobileRequired);
                        steam_guard_sent = true;
                        sg_start = Some(std::time::Instant::now());
                        eprintln!("[SteamGuard] prompt detected (t=0.0s)");
                    }

                    // Detect login errors from complete lines.
                    let elapsed = sg_start.map(|s| s.elapsed().as_secs_f32()).unwrap_or(0.0);
                    if ascii_contains_ci(line, "timed out waiting for confirmation")
                        || ascii_contains_ci(line, "wait for confirmation timed out")
                        || (ascii_contains_ci(line, "error") && ascii_contains_ci(line, "timeout"))
                    {
                        steam_guard_timed_out = true;
                        eprintln!("[SteamGuard] TIMED OUT (t={elapsed:.1}s)");
                    }
                    if ascii_contains_ci(line, "invalid password") {
                        invalid_password = true;
                    }
                    // Log when Steam Guard is resolved.
                    if steam_guard_sent
                        && !steam_guard_timed_out
                        && ascii_contains_ci(line, "waiting for")
                        && ascii_contains_ci(line, "ok")
                    {
                        eprintln!("[SteamGuard] confirmed OK (t={elapsed:.1}s)");
                    }
                    if ascii_contains_ci(line, "retrying") {
                        eprintln!("[SteamGuard] retrying (t={elapsed:.1}s)");
                    }

                    if line.contains("Success. Downloaded item")
                        || line.contains("already up to date")
                    {
                        login_phase = false;
                        if let Some(id) = extract_mod_id_from_line(line)
                            && let Some(&(idx, name)) = mod_lookup.get(&id)
                        {
                            succeeded.insert(id);
                            let _ = tx.send(ModProgress::Done {
                                current: idx + 1,
                                total,
                                mod_id: id,
                                name: name.to_owned(),
                            });
                            current_idx = idx + 1;
                        }
                    }
                    if line.contains("Downloading item") {
                        login_phase = false;
                        if let Some(id) = extract_mod_id_from_line(line)
                            && let Some(&(idx, name)) = mod_lookup.get(&id)
                            && idx + 1 > current_idx
                        {
                            let _ = tx.send(ModProgress::Starting {
                                current: idx + 1,
                                total,
                                mod_id: id,
                                name: name.to_owned(),
                            });
                            current_idx = idx + 1;
                        }
                    }
                }
                // Borrow on buf released; advance in place (no realloc).
                buf.drain(..advance_to);
            }

            // Check the partial-line remainder (content not yet terminated
            // by '\n'). steamcmd writes prompts like "password:" without a
            // trailing newline, so they would never be processed by the loop
            // above.
            // Partial-buffer Steam Guard check (no `has_password` guard — works
            // for both saved-password and interactive-password flows).
            if login_phase && !steam_guard_sent && is_steam_guard_prompt(&buf) {
                let _ = tx.send(ModProgress::SteamGuardMobileRequired);
                steam_guard_sent = true;
            }

            // Detect the `password:` prompt (no trailing newline).
            // steamcmd emits this when cached credentials are missing.
            if login_phase && !password_prompt_sent && is_password_prompt(&buf) {
                password_prompt_sent = true;
                let _ = tx.send(ModProgress::PasswordRequired);

                // Wait for the user to provide a password via the input channel.
                // If the channel is closed (user cancelled / dropped), kill the
                // process and treat it as a credential failure.
                match pty_input.recv().await {
                    Some(password) => {
                        // Write the password followed by Enter to the PTY.
                        use std::io::Write;
                        let _ = pty_writer.write_all(password.as_bytes());
                        let _ = pty_writer.write_all(b"\n");
                        let _ = pty_writer.flush();
                        buf.clear();
                    }
                    None => {
                        // User cancelled — kill the process
                        let _ = child.kill();
                        break;
                    }
                }
            }
        }

        let _ = child.wait();

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
            hint: if invalid_password {
                Some(
                    "Invalid password. Check your Steam password in Settings and try again."
                        .to_string(),
                )
            } else if steam_guard_timed_out {
                Some(
                    "Steam Guard confirmation timed out. Open the Steam Mobile app faster next time, or try again."
                        .to_string(),
                )
            } else if failed > 0 {
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
            let name = process.name().to_string_lossy();
            // "steam" — native Linux / macOS / Flatpak entry-point.
            // "steam.exe" — Windows and Wine/Proton on Linux.
            // Exclude "steamwebhelper" — it can outlive a crashed client.
            // Case-insensitive compare avoids allocating a lowercased String
            // for every process in the table on every poll.
            name.eq_ignore_ascii_case("steam") || name.eq_ignore_ascii_case("steam.exe")
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
            let name = process.name().to_string_lossy();
            // Matches "steam", "steam.exe", "steamwebhelper", … — any process
            // whose name starts with "steam" (case-insensitive). Avoids
            // allocating a lowercased String per process.
            let bytes = name.as_bytes();
            if bytes.len() >= 5 && bytes[..5].eq_ignore_ascii_case(b"steam") {
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
