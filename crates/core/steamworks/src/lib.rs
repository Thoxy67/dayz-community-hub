//! Workshop downloads through the running Steam client, as the DZSA
//! launcher does them: the user's account subscribes to each mod, Steam
//! downloads it into its own library (`steamapps/workshop/content/221100`)
//! and keeps it updated from then on.
//!
//! Valve's library (`libsteam_api.so`, `steam_api64.dll`) is not linked: it is
//! embedded at build time, written to disk and loaded the first time this
//! mode is used, so the app starts and runs without it, and a failure to load
//! it is an error this mode reports, nothing more.
//!
//! The launcher itself never loads it: every call runs in a short-lived
//! child process (see [`worker`]), since Steam shows DayZ running until the
//! process that connected as DayZ exits. `main` must call
//! [`serve_if_worker`] first thing.

mod api;
mod redist;
mod session;
mod state;
mod worker;

pub use session::{Details, Event, ItemResult, Subscribed, Unsubscribed};
pub use state::{ItemState, percent, result_text};
pub use worker::{
    WORKER_ARG, check, close_session, download, serve_if_worker, session_open, status,
    steam_running, subscriptions, unsubscribe,
};

/// DayZ's Steam app id, the game a session connects as.
pub const DAYZ_APP_ID: u32 = 221100;

/// Whether this mode can work here: the library loads. Loads it (once) if
/// it has not been. The error is a sentence. Worker side.
fn available() -> Result<(), String> {
    api::get().map(|_| ()).map_err(|e| format!("{e}."))
}

/// Whether the Steam client is running, as Valve's library sees it (without
/// connecting). False when the library does not load. Worker side.
fn local_steam_running() -> bool {
    // SAFETY: SteamAPI_IsSteamRunning takes nothing and only reads Steam's
    // registry / pid file; it is callable without a session.
    api::get().is_ok_and(|api| unsafe { (api.is_steam_running)() })
}
