use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("SteamCMD error: {0}")]
    SteamCmd(String),
    /// Cached steamcmd credentials are missing or expired.
    /// The inner string is the exact command the user should run to fix it.
    #[error("SteamCMD credentials expired. Run: {0}")]
    CredentialsExpired(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Mod error: {0}")]
    Mod(String),
    #[error("Server error: {0}")]
    Server(String),
    #[error("Unknown error: {0}")]
    Other(String),
    #[error("A2S query error: {0}")]
    A2sQuery(String),
}
