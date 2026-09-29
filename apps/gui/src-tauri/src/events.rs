//! The events Rust emits to the window, typed once. Each name is the
//! struct's name in kebab-case (`LaunchDone` is `launch-done`), which is what
//! the window listens to.

use serde::{Deserialize, Serialize};
use tauri_specta::Event;

pub use crate::features::dzch_cli::CliArgs;
pub use crate::features::steamcmd::detect::SteamcmdStatusDto;

/// A launch reached Steam; carries the server's name.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct LaunchDone(pub String);

/// A launch failed; carries the error.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct LaunchError(pub String);

/// The offline mode finished downloading.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct OfflineModeUpdated;

/// The offline mode download failed; carries the error.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct OfflineModeError(pub String);

/// steamcmd appeared while `watch_steamcmd` was polling.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct SteamcmdDetected(pub SteamcmdStatusDto);

/// Every event, for the builder.
pub(crate) fn collect() -> tauri_specta::Events {
    tauri_specta::collect_events![
        LaunchDone,
        LaunchError,
        OfflineModeUpdated,
        OfflineModeError,
        SteamcmdDetected,
        CliArgs,
        crate::features::browser::ServersChanged,
        crate::features::gamepad::GamepadInput,
        crate::features::gamepad::GamepadPads,
    ]
}
