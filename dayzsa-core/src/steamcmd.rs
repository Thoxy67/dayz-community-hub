use crate::Result;
use crate::errors::Error;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

pub const DAYZ_GAME_ID: u32 = 221100;

/// Try to find Steam root directory by checking common locations.
/// Returns the path to the `steamapps` directory.
pub fn find_steam_root() -> Option<PathBuf> {
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

/// Try to find steamcmd binary in PATH or common locations
pub fn find_steamcmd() -> Option<PathBuf> {
    if let Ok(path) = which::which("steamcmd") {
        return Some(path);
    }

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

/// Progress messages sent during mod download/update operations.
#[derive(Debug, Clone)]
pub enum ModProgress {
    /// Steam is being shut down to avoid session conflict with steamcmd
    ShuttingDownSteam,
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
            game_id: DAYZ_GAME_ID,
        }
    }

    pub fn with_game_id(mut self, game_id: u32) -> Self {
        self.game_id = game_id;
        self
    }

    pub fn login(&self) -> &str {
        &self.login
    }

    pub fn steam_root(&self) -> &Path {
        &self.steam_root
    }

    /// Returns true if the login is non-anonymous (required for workshop downloads).
    pub fn has_real_login(&self) -> bool {
        !self.login.is_empty() && self.login != "anonymous"
    }

    /// Cache login credentials by running `steamcmd +login <user> +quit`.
    /// This is interactive and will prompt for password/2FA in the terminal.
    pub async fn cache_login(&self) -> Result<()> {
        let status = Command::new(&self.steamcmd_path)
            .arg("+login")
            .arg(&self.login)
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

        let mut child = Command::new(&self.steamcmd_path)
            .arg("+@ShutdownOnFailedCommand")
            .arg("1")
            .arg("+login")
            .arg(&self.login)
            .arg("+workshop_download_item")
            .arg(self.game_id.to_string())
            .arg(workshop_id.to_string())
            .arg("validate")
            .arg("+quit")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Stream stdout to catch credential issues early
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("Cached credentials not found") {
                    let _ = child.kill().await;
                    return Err(Error::CredentialsExpired(format!(
                        "steamcmd +login {} +quit",
                        self.login
                    )));
                }
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
        let output = Command::new(&self.steamcmd_path)
            .arg("+@ShutdownOnFailedCommand")
            .arg("1")
            .arg("+login")
            .arg(&self.login)
            .arg("+app_update")
            .arg(self.game_id.to_string())
            .arg("+quit")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

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

    /// Download/update multiple mods in a single steamcmd invocation with progress.
    ///
    /// Batches all `+workshop_download_item` commands into one process to avoid
    /// repeated login overhead. Streams stdout to detect per-mod completion.
    /// SteamCMD only downloads changed files (delta updates), so this is efficient
    /// for both fresh installs and updates.
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
                hint: Some(format!(
                    "Set your Steam login in the profile, then run:\n  steamcmd +login YOUR_USERNAME +quit"
                )),
            });
            return results;
        }

        // Steam and steamcmd share the same auth session — running both at the same
        // time kicks the user offline. Shut Steam down cleanly before starting steamcmd.
        let _ = tx.send(ModProgress::ShuttingDownSteam);
        SteamClient::shutdown_for_steamcmd().await;

        // Report starting the first mod
        let _ = tx.send(ModProgress::Starting {
            current: 1,
            total,
            mod_id: mods_info[0].0,
            name: mods_info[0].1.clone(),
        });

        // Build a single steamcmd command with all mods batched:
        // steamcmd +login <user> +workshop_download_item 221100 <id1> validate \
        //                        +workshop_download_item 221100 <id2> validate ... +quit
        let mut cmd = Command::new(&self.steamcmd_path);
        cmd.arg("+@ShutdownOnFailedCommand").arg("0"); // Don't abort on single mod failure
        cmd.arg("+login").arg(&self.login);

        for (mod_id, _) in mods_info {
            cmd.arg("+workshop_download_item")
                .arg(self.game_id.to_string())
                .arg(mod_id.to_string())
                .arg("validate");
        }

        cmd.arg("+quit");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err_str = format!("Failed to start steamcmd: {}", e);
                let results: Vec<(u64, Result<()>)> = mods_info
                    .iter()
                    .map(|(id, name)| {
                        let _ = tx.send(ModProgress::Failed {
                            current: 1,
                            total,
                            mod_id: *id,
                            name: name.clone(),
                            error: err_str.clone(),
                        });
                        (*id, Err(Error::SteamCmd(err_str.clone())))
                    })
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

        // Build a lookup: mod_id -> (index, name)
        let mod_lookup: std::collections::HashMap<u64, (usize, String)> = mods_info
            .iter()
            .enumerate()
            .map(|(i, (id, name))| (*id, (i, name.clone())))
            .collect();

        // Track which mods completed successfully
        let mut succeeded: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut current_mod_idx: usize = 0;
        let mut credentials_not_found = false;

        // Stream stdout line by line to detect per-mod progress
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // Detect expired/missing login: steamcmd prints this then
                // hangs waiting for interactive password input on stdin.
                if line.contains("Cached credentials not found") {
                    credentials_not_found = true;
                    let _ = child.kill().await;
                    break;
                }

                // SteamCMD prints lines like:
                //   "Downloading item 1559212036 ..."
                //   "Success. Downloaded item 1559212036 to ..."
                //   "ERROR! Download item 1559212036 failed (..."

                if line.contains("Success. Downloaded item")
                    || line.contains("already up to date")
                {
                    if let Some(id) = extract_mod_id_from_line(&line) {
                        succeeded.insert(id);
                        if let Some((idx, name)) = mod_lookup.get(&id) {
                            let _ = tx.send(ModProgress::Done {
                                current: idx + 1,
                                total,
                                mod_id: id,
                                name: name.clone(),
                            });

                            let next_idx = idx + 1;
                            if next_idx < total {
                                let next = &mods_info[next_idx];
                                let _ = tx.send(ModProgress::Starting {
                                    current: next_idx + 1,
                                    total,
                                    mod_id: next.0,
                                    name: next.1.clone(),
                                });
                                current_mod_idx = next_idx;
                            }
                        }
                    }
                } else if line.contains("ERROR! Download item") || line.contains("ERROR!") {
                    if let Some(id) = extract_mod_id_from_line(&line) {
                        if let Some((idx, name)) = mod_lookup.get(&id) {
                            let _ = tx.send(ModProgress::Failed {
                                current: idx + 1,
                                total,
                                mod_id: id,
                                name: name.clone(),
                                error: line.clone(),
                            });

                            let next_idx = idx + 1;
                            if next_idx < total {
                                let next = &mods_info[next_idx];
                                let _ = tx.send(ModProgress::Starting {
                                    current: next_idx + 1,
                                    total,
                                    mod_id: next.0,
                                    name: next.1.clone(),
                                });
                                current_mod_idx = next_idx;
                            }
                        }
                    }
                } else if line.contains("Downloading item") {
                    if let Some(id) = extract_mod_id_from_line(&line) {
                        if let Some((idx, name)) = mod_lookup.get(&id) {
                            if *idx != current_mod_idx || *idx > 0 {
                                let _ = tx.send(ModProgress::Starting {
                                    current: idx + 1,
                                    total,
                                    mod_id: id,
                                    name: name.clone(),
                                });
                                current_mod_idx = *idx;
                            }
                        }
                    }
                }
            }
        }

        // If credentials not found, report all mods as failed with a typed error
        if credentials_not_found {
            let relogin_cmd = format!("steamcmd +login {} +quit", self.login);
            let hint = format!(
                "Cached credentials not found.\nRun this command to re-login:\n  {}",
                relogin_cmd
            );
            let results: Vec<(u64, Result<()>)> = mods_info
                .iter()
                .map(|(id, name)| {
                    let _ = tx.send(ModProgress::Failed {
                        current: 1,
                        total,
                        mod_id: *id,
                        name: name.clone(),
                        error: hint.clone(),
                    });
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

        // Wait for process to finish
        let status = child.wait().await;

        // Build results
        let results: Vec<(u64, Result<()>)> = mods_info
            .iter()
            .map(|(id, name)| {
                if succeeded.contains(id) {
                    (*id, Ok(()))
                } else {
                    // Check if the mod directory exists (it might have been downloaded
                    // even if we didn't parse the success line)
                    let mod_dir = self.workshop_path().join(id.to_string());
                    if mod_dir.exists() {
                        // Assume success if directory exists
                        (*id, Ok(()))
                    } else {
                        let err = match &status {
                            Ok(s) if !s.success() => format!("steamcmd exited with {}", s),
                            Err(e) => format!("steamcmd error: {}", e),
                            _ => format!("Mod {} download status unknown", id),
                        };
                        let (idx, _) = mod_lookup.get(id).map(|v| (v.0, &v.1)).unwrap_or((0, name));
                        let _ = tx.send(ModProgress::Failed {
                            current: idx + 1,
                            total,
                            mod_id: *id,
                            name: name.clone(),
                            error: err.clone(),
                        });
                        (*id, Err(Error::SteamCmd(err)))
                    }
                }
            })
            .collect();

        let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
        let failed = results.iter().filter(|(_, r)| r.is_err()).count();
        let hint = if failed > 0 {
            Some(format!(
                "Some downloads failed. If your steamcmd login expired, run:\n  steamcmd +login {} +quit",
                self.login
            ))
        } else {
            None
        };
        let _ = tx.send(ModProgress::Finished {
            ok,
            failed,
            total,
            hint,
        });

        results
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
    /// Check if the Steam client is currently running.
    ///
    /// Matches the main Steam client process only — not helpers that can
    /// outlive a crashed Steam (steamwebhelper) and not container wrappers
    /// used by Flatpak (bwrap, pressure-vessel-wrap).
    pub fn is_running() -> bool {
        use sysinfo::System;
        let mut system = System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        system.processes().values().any(|process| {
            let name = process.name().to_string_lossy().to_lowercase();
            // "steam" covers native Linux and the Flatpak entry-point script.
            // "steam.exe" covers Wine/Proton scenarios.
            // We intentionally exclude "steamwebhelper" — it can outlive a
            // crashed Steam client and would cause a false-positive.
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
        std::process::Command::new("nohup")
            .arg("steam")
            .arg("-nofriendsui")
            .arg("-silent")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(false)
    }

    /// Graceful shutdown via `steam -shutdown`.
    pub async fn shutdown() -> Result<()> {
        let _ = Command::new("steam")
            .arg("-shutdown")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?
            .wait()
            .await;
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
        let _ = Command::new("steam")
            .arg("-shutdown")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

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
