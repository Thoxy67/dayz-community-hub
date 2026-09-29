//! Workshop downloads through the running Steam client, as the DZSA
//! launcher does them: the user's account subscribes to each mod, Steam
//! downloads it into its own library (`steamapps/workshop/content/221100`)
//! and keeps it updated from then on.
//!
//! Valve's library (`libsteam_api.so`, `steam_api64.dll`) is not linked: it is
//! embedded at build time, written to disk and loaded the first time this
//! mode is used, so the app starts and runs without it, and a failure to load
//! it is an error this mode reports, nothing more.

mod api;
mod redist;
mod session;
mod state;

pub use session::{Event, ItemResult, Unsubscribed, check, download, session_open, unsubscribe};
pub use state::{ItemState, percent, result_text};

/// DayZ's Steam app id, the game a session connects as.
pub const DAYZ_APP_ID: u32 = 221100;

/// Whether this mode can work here: the library loads. Loads it (once) if
/// it has not been. The error is a sentence.
pub fn available() -> Result<(), String> {
    api::get().map(|_| ()).map_err(|e| format!("{e}."))
}

/// Whether the Steam client is running, as Valve's library sees it (without
/// connecting). False when the library does not load.
pub fn steam_running() -> bool {
    // SAFETY: SteamAPI_IsSteamRunning takes nothing and only reads Steam's
    // registry / pid file; it is callable without a session.
    api::get().is_ok_and(|api| unsafe { (api.is_steam_running)() })
}
