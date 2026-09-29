//! Background mod operations: install or update through steamcmd, then link.
//!
//! Everything downloads into the launcher's own folder. A mod that is only in
//! a Steam library is updated by downloading a launcher copy, which is then
//! the newer and the one linked: the Steam client only updates the items its
//! account is subscribed to, and the launcher never writes in its libraries.

use std::path::PathBuf;
use std::sync::Arc;

use dz_api::Server;
use dz_common::Result;
use dz_steamcmd::{ModProgress, PtyInputTx, SteamCmd};
use tokio::sync::mpsc;

use crate::mods::{self, InstalledMod, ModDirs};

/// Describes what kind of background mod operation to perform.
#[derive(Clone)]
pub enum ModOperation {
    /// Install the mods a server needs that are missing, then link them all.
    InstallServer { server: Server },
    /// Update (re-download) every mod a server uses, then link them all.
    UpdateServer { server: Server },
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
    /// Download these mods again and check every file (`validate`): slow,
    /// for a mod that does not load.
    Repair { mods: Vec<(u64, String)> },
    /// Log SteamCMD in and quit, so later downloads use its cached login.
    Login,
}

/// Emit a "nothing to do" finished message. Used by operation branches that
/// have an empty input list and can skip the steamcmd invocation entirely.
fn emit_nothing_to_do(tx: &mpsc::UnboundedSender<ModProgress>) {
    let _ = tx.send(ModProgress::Finished {
        ok: 0,
        failed: 0,
        total: 0,
        hint: None,
    });
}

/// Spawn a background mod operation with progress reporting.
/// Returns a (receiver, join_handle). The caller polls the receiver for progress messages.
/// When the operation completes, the final message is `ModProgress::Finished`.
pub fn spawn_mod_operation(
    steamcmd: Arc<SteamCmd>,
    dirs: ModDirs,
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
        // Take the PTY input receiver once — only one match arm executes.
        let mut pty_rx = Some(pty_input_rx);
        // Helper: take the receiver or create a dummy (never-resolving) one.
        macro_rules! take_pty_rx {
            () => {
                pty_rx.take().unwrap_or_else(|| mpsc::unbounded_channel().1)
            };
        }

        match op {
            ModOperation::InstallServer { server } => {
                let server_mod_ids = server.mod_ids();
                let missing = mods::get_missing_mods(&server_mod_ids, &installed_mods);

                if missing.is_empty() {
                    let _ = mods::create_mod_symlinks(&dirs, &dayz_path, &server_mod_ids);
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

                let results = steamcmd
                    .download_mods_with_progress(&mods_info, false, &tx, take_pty_rx!())
                    .await;

                let mut installed_ids = Vec::new();
                let mut failed = Vec::new();
                for (mod_id, result) in &results {
                    match result {
                        Ok(_) => {
                            installed_ids.push(*mod_id);
                        }
                        Err(e) => {
                            failed.push((*mod_id, format!("{e}")));
                        }
                    }
                }

                let _ = mods::create_mod_symlinks(&dirs, &dayz_path, &server_mod_ids);

                ModOpResult::InstallDone(InstallResult {
                    installed: installed_ids,
                    failed,
                    total_server_mods: server_mod_ids.len(),
                })
            }

            ModOperation::UpdateServer { server } => {
                let mods_info: Vec<(u64, String)> = server
                    .mods
                    .iter()
                    .map(|m| (m.steam_workshop_id as u64, m.name.clone()))
                    .collect();

                let results = steamcmd
                    .download_mods_with_progress(&mods_info, false, &tx, take_pty_rx!())
                    .await;

                let _ = mods::create_mod_symlinks(&dirs, &dayz_path, &server.mod_ids());

                let per_mod: Vec<(u64, Result<()>)> = results;
                ModOpResult::UpdateDone(per_mod)
            }

            ModOperation::UpdateAll => {
                let mods_info: Vec<(u64, String)> = installed_mods
                    .iter()
                    .map(|m| (m.id, m.name.clone()))
                    .collect();

                let results = steamcmd
                    .download_mods_with_progress(&mods_info, false, &tx, take_pty_rx!())
                    .await;
                updated(&dirs, &dayz_path, results)
            }

            ModOperation::UpdateStale { stale_mods } => {
                if stale_mods.is_empty() {
                    emit_nothing_to_do(&tx);
                    return ModOpResult::UpdateDone(vec![]);
                }
                let results = steamcmd
                    .download_mods_with_progress(&stale_mods, false, &tx, take_pty_rx!())
                    .await;
                updated(&dirs, &dayz_path, results)
            }

            ModOperation::UpdateOne { mod_id, name } => {
                let mods_info = vec![(mod_id, name)];
                let results = steamcmd
                    .download_mods_with_progress(&mods_info, false, &tx, take_pty_rx!())
                    .await;
                updated(&dirs, &dayz_path, results)
            }

            ModOperation::UpdateSelected { mods } => {
                if mods.is_empty() {
                    emit_nothing_to_do(&tx);
                    return ModOpResult::UpdateDone(vec![]);
                }
                let results = steamcmd
                    .download_mods_with_progress(&mods, false, &tx, take_pty_rx!())
                    .await;
                updated(&dirs, &dayz_path, results)
            }

            ModOperation::Repair { mods } => {
                if mods.is_empty() {
                    emit_nothing_to_do(&tx);
                    return ModOpResult::UpdateDone(vec![]);
                }
                let results = steamcmd
                    .download_mods_with_progress(&mods, true, &tx, take_pty_rx!())
                    .await;
                updated(&dirs, &dayz_path, results)
            }

            ModOperation::Login => {
                steamcmd.login_with_progress(&tx, take_pty_rx!()).await;
                ModOpResult::UpdateDone(vec![])
            }

            ModOperation::InstallManual { mods: mods_info } => {
                if mods_info.is_empty() {
                    emit_nothing_to_do(&tx);
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
                    let _ = mods::create_mod_symlinks(&dirs, &dayz_path, &all_ids);
                    emit_nothing_to_do(&tx);
                    return ModOpResult::InstallDone(InstallResult {
                        installed: Vec::new(),
                        failed: Vec::new(),
                        total_server_mods: mods_info.len(),
                    });
                }

                let results = steamcmd
                    .download_mods_with_progress(&missing, false, &tx, take_pty_rx!())
                    .await;

                let mut installed_new = Vec::new();
                let mut failed = Vec::new();
                for (mod_id, result) in &results {
                    match result {
                        Ok(_) => {
                            installed_new.push(*mod_id);
                        }
                        Err(e) => {
                            failed.push((*mod_id, format!("{e}")));
                        }
                    }
                }

                // Create symlinks for all requested mods (including previously installed ones)
                let all_ids: Vec<u64> = mods_info.iter().map(|(id, _)| *id).collect();
                let _ = mods::create_mod_symlinks(&dirs, &dayz_path, &all_ids);

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

/// The mods downloaded are now the newest copies: their links move to them.
fn updated(
    dirs: &ModDirs,
    dayz_path: &std::path::Path,
    results: Vec<(u64, Result<()>)>,
) -> ModOpResult {
    let ok: Vec<u64> = results
        .iter()
        .filter(|(_, r)| r.is_ok())
        .map(|(id, _)| *id)
        .collect();
    mods::relink_linked(dirs, dayz_path, &ok);
    ModOpResult::UpdateDone(results)
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
