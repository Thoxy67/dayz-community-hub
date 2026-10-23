//! What the hub tells the window, unasked. One variant per event the Tauri
//! shell emitted (`tauri_specta::Event`, named in kebab-case there:
//! `LaunchDone` was `launch-done`), with the same payload, plus the streams
//! that went over a `Channel` (a mod operation's steps, an update's download,
//! an explicit ping list's results).

use serde::Serialize;

pub use crate::features::browser::ServersChanged;
pub use crate::features::dzch_cli::CliArgs;
pub use crate::features::gamepad::{GamepadInput, GamepadPads};
pub use crate::features::ping::{PingResultDto, PingScanProgress};
pub use crate::features::servers::refresh::RefreshChanged;
pub use crate::features::servers::sources::{Phase as SourcePhase, SourcesChanged};
pub use crate::features::stats::PlaySessionsChanged;
pub use crate::features::steamcmd::ModProgressEvent;
pub use crate::features::steamcmd::detect::SteamcmdStatusDto;
pub use crate::features::updater::DownloadEvent;

/// Every event, as the sink receives it.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "event", content = "payload", rename_all = "kebab-case")]
pub enum HubEvent {
    /// A launch reached Steam; carries the server's name.
    LaunchDone(String),
    /// A launch failed; carries the error.
    LaunchError(String),
    /// The offline mode finished downloading.
    OfflineModeUpdated,
    /// The offline mode download failed; carries the error.
    OfflineModeError(String),
    /// steamcmd appeared while `watch_steamcmd` was polling.
    SteamcmdDetected(SteamcmdStatusDto),
    /// A second launch, or a `dzch://` link, asked for something.
    CliArgs(CliArgs),
    /// The list, the profile's flags or live data changed (≤ ~30/s): the window
    /// reads the new state from `Hub::servers_snapshot` on its next frame.
    ServersChanged(ServersChanged),
    /// Where each source of the list has got (≤ 4/s).
    SourcesChanged(Box<SourcesChanged>),
    /// A refresh of the list started or ended.
    RefreshChanged(RefreshChanged),
    /// How far the whole-list ping scan has got (≤ 2/s).
    PingScanProgress(PingScanProgress),
    /// A play session started or ended.
    PlaySessionsChanged(PlaySessionsChanged),
    /// DayZ started or stopped (whether launched from here or not), as the
    /// play tracker saw it; also its first look, at start.
    GameRunning(bool),
    /// A pad's input, for the gamepad navigation mode.
    GamepadInput(GamepadInput),
    /// The pads connected changed.
    GamepadPads(GamepadPads),
    /// One step of the running mod operation (was its `Channel`). Steps
    /// numbered at or below a `mod_op_attach` snapshot's `seq` are in it.
    ModProgress(Box<ModProgressEvent>),
    /// How far an update's download has got (was its `Channel`).
    UpdateDownload(DownloadEvent),
    /// A batch of results of an explicit ping list (`ping_servers`, was its
    /// `Channel`); they are in the live store too.
    PingResults(Vec<PingResultDto>),
}
