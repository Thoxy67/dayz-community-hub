//! What a mod download reports while it runs.

use tokio::sync::mpsc;

/// Progress messages sent during mod download/update operations.
#[derive(Debug, Clone)]
pub enum ModProgress {
    /// SteamCMD logged in: its login is cached from now on, so a password
    /// saved by an earlier version is no longer needed.
    LoggedIn,
    /// Steam refused the password saved by an earlier version: it is no
    /// use and can be forgotten.
    SavedPasswordRefused,
    /// steamcmd is waiting for Steam Guard Mobile confirmation on the user's phone
    SteamGuardMobileRequired,
    /// steamcmd is prompting for a password (no cached login, or the saved
    /// one was refused). The UI should show a password input and send the
    /// password via the `PtyInputTx` channel.
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
    /// A transient progress line that overwrites itself via carriage returns
    /// (e.g. steamcmd's "Update state … progress: 45.5"). The UI replaces the
    /// previous transient line instead of appending, so download progress
    /// updates in place rather than flooding the log.
    LogProgress(String),
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
