//! steamcmd, which downloads workshop mods into its own directory, and the
//! Steam client whose libraries it only reads.

mod client;
mod cmd;
mod detect;
mod download;
mod output;
mod progress;
mod pty;

pub use client::{SteamClient, appimage_env};
pub use cmd::SteamCmd;
pub use detect::{DayzInstall, detect_dayz, find_steam_root, find_steamcmd, steam_workshop_dirs};
pub use progress::{ModProgress, ProgressTx, PtyInputRx, PtyInputTx};

/// DayZ's Steam app id.
pub const DAYZ_GAME_ID: u32 = 221100;
