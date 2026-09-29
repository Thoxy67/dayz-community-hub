//! The steamcmd install this app drives, and where it puts things.

use std::path::{Path, PathBuf};

use crate::DAYZ_GAME_ID;

pub struct SteamCmd {
    pub(crate) steamcmd_path: PathBuf,
    pub(crate) steam_root: PathBuf,
    pub(crate) login: String,
    /// Optional password for non-interactive login. When set, steamcmd is
    /// invoked as `+login <user> <password>` so it doesn't rely on cached
    /// credentials. When absent, only the username is passed and steamcmd
    /// falls back to its credential cache.
    pub(crate) password: Option<String>,
    pub(crate) game_id: u32,
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
            password: None,
            game_id: DAYZ_GAME_ID,
        }
    }

    pub fn with_password(mut self, password: Option<String>) -> Self {
        self.password = password;
        self
    }

    pub fn login(&self) -> &str {
        &self.login
    }

    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    pub fn steam_root(&self) -> &Path {
        &self.steam_root
    }

    /// Returns true if the login is non-anonymous (required for workshop downloads).
    pub fn has_real_login(&self) -> bool {
        !self.login.is_empty() && self.login != "anonymous"
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
}
