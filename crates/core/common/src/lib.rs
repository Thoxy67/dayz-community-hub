//! What every DayZ Community Hub crate shares: one error type, where the app
//! keeps its files, and the clock.

pub mod error;
pub mod paths;
pub mod time;
#[cfg(windows)]
pub mod win;

pub use error::Error;

/// The result every crate in the workspace returns.
pub type Result<T> = std::result::Result<T, Error>;

/// Windows `CREATE_NO_WINDOW`: spawned helpers (steam, cmd, reg) must not
/// flash a console window over the launcher.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
