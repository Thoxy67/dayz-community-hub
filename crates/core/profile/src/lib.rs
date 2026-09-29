//! The user's profile: account settings, favorites, history, launch options
//! and ping preferences, persisted as `profile.json` in the data directory.

mod launch_options;

pub use launch_options::{LaunchOption, LaunchOptions};

use dz_common::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub use dz_common::paths::{default_data_dir, default_profile_path};

const APP_VERSION: &str = "0.1.0";

/// Most history entries kept; newest first.
const MAX_HISTORY: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub steam_login: Option<String>,
    /// Steam account password — stored in plaintext so steamcmd can log in
    /// non-interactively without relying on cached credentials.
    #[serde(default)]
    pub steam_password: Option<String>,
    #[serde(default)]
    pub steam_root: Option<String>,
    #[serde(default = "default_steamcmd_enabled")]
    pub steamcmd_enabled: bool,
    pub player: Option<String>,
    /// Steam Web API key — used to fetch player avatar via GetPlayerSummaries.
    #[serde(default)]
    pub steam_api_key: Option<String>,
    /// Steam 64-bit account ID — used with the API key to resolve the avatar.
    #[serde(default)]
    pub steam_id: Option<String>,
    /// BattleMetrics personal access token — used to fetch server rank, uptime and player history.
    #[serde(default)]
    pub battlemetrics_api_key: Option<String>,
    /// Optional explicit path to steamcmd binary (overrides auto-detection).
    #[serde(default)]
    pub steamcmd_path: Option<String>,
    /// User location for distance calculation (longitude, latitude).
    #[serde(default)]
    pub user_location: Option<(f64, f64)>,
    pub favorites: Vec<Favorite>,
    pub history: Vec<History>,
    /// IPs excluded from the server browser (persisted across restarts).
    #[serde(default)]
    pub excluded_ips: Vec<String>,
    /// Ping concurrency level (5-100, default 25).
    #[serde(default = "default_ping_concurrency")]
    pub ping_concurrency: u32,
    /// Auto ping timeout in milliseconds (1000-5000, default 2000).
    #[serde(default = "default_ping_timeout_auto")]
    pub ping_timeout_auto: u32,
    /// Manual ping timeout in milliseconds (1000-30000, default 10000).
    #[serde(default = "default_ping_timeout_manual")]
    pub ping_timeout_manual: u32,
    /// Max consecutive timeouts before stopping auto-retry (0-5, default 3; 0 = disable).
    #[serde(default = "default_ping_max_retries")]
    pub ping_max_retries: u32,
    /// Whether to include favorites in auto ping scan (default true).
    #[serde(default = "default_true")]
    pub ping_scan_favorites: bool,
    /// Whether to include history in auto ping scan (default true).
    #[serde(default = "default_true")]
    pub ping_scan_history: bool,
    /// Whether to include all servers in auto ping scan (default true).
    #[serde(default = "default_true")]
    pub ping_scan_servers: bool,
    #[serde(
        deserialize_with = "launch_options::deserialize_launch_options",
        default = "LaunchOptions::defaults"
    )]
    pub options: LaunchOptions,
    pub version: String,
    #[serde(skip)]
    pub path: PathBuf,
}

fn default_steamcmd_enabled() -> bool {
    true
}

fn default_ping_concurrency() -> u32 {
    64
}

fn default_ping_timeout_auto() -> u32 {
    2000
}

fn default_ping_timeout_manual() -> u32 {
    10000
}

fn default_ping_max_retries() -> u32 {
    3
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Favorite {
    pub name: String,
    pub ip: String,
    pub port: u16,
    /// Optional server join password — saved so Direct Connect can auto-fill it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub ts: i64,
    // Old profiles may have extra fields like "mods" -- ignore them
    #[serde(flatten)]
    pub extra: Option<serde_json::Value>,
}

impl History {
    /// A human-readable relative time ("2 hours ago").
    pub fn relative_time(&self) -> String {
        dz_common::time::format_relative_time(self.ts)
    }
}

impl Profile {
    /// Read the profile at `path`, creating a default one there if none exists.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            let mut profile = Self::default_with_version(APP_VERSION);
            profile.path = path.to_path_buf();
            profile.snapshot()?.write_blocking()?;
            return Ok(profile);
        }
        let data = std::fs::read_to_string(path)?;
        let mut profile: Profile = serde_json::from_str(&data)?;
        profile.path = path.to_path_buf();
        Ok(profile)
    }

    /// [`Profile::load`] without blocking the runtime.
    pub async fn load_async(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !tokio::fs::try_exists(path).await.unwrap_or(false) {
            let mut profile = Self::default_with_version(APP_VERSION);
            profile.path = path.to_path_buf();
            profile.snapshot()?.write().await?;
            return Ok(profile);
        }
        let data = tokio::fs::read_to_string(path).await?;
        let mut profile: Profile = serde_json::from_str(&data)?;
        profile.path = path.to_path_buf();
        Ok(profile)
    }

    /// The profile serialized as it is now, ready to be written after the
    /// caller has released whatever lock guards the profile.
    pub fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            path: self.path.clone(),
            data: serde_json::to_string_pretty(self)?,
            seq: 0,
        })
    }

    pub fn default_with_version(version: &str) -> Self {
        Self {
            steam_login: None,
            steam_password: None,
            steam_root: None,
            steamcmd_enabled: true,
            steamcmd_path: None,
            user_location: None,
            player: None,
            steam_api_key: None,
            steam_id: None,
            battlemetrics_api_key: None,
            favorites: Vec::new(),
            history: Vec::new(),
            excluded_ips: Vec::new(),
            ping_concurrency: default_ping_concurrency(),
            ping_timeout_auto: default_ping_timeout_auto(),
            ping_timeout_manual: default_ping_timeout_manual(),
            ping_max_retries: default_ping_max_retries(),
            ping_scan_favorites: true,
            ping_scan_history: true,
            ping_scan_servers: true,
            options: LaunchOptions::defaults(),
            version: version.to_string(),
            path: PathBuf::new(),
        }
    }

    /// Add a favorite, or update the name and password of an existing one.
    pub fn add_favorite(&mut self, name: String, ip: String, port: u16, password: Option<String>) {
        if let Some(existing) = self
            .favorites
            .iter_mut()
            .find(|f| f.ip == ip && f.port == port)
        {
            existing.name = name;
            existing.password = password;
        } else {
            self.favorites.push(Favorite {
                name,
                ip,
                port,
                password,
            });
        }
    }

    pub fn remove_favorite(&mut self, ip: &str, port: u16) {
        self.favorites.retain(|f| f.ip != ip || f.port != port);
    }

    pub fn is_favorite(&self, ip: &str, port: u16) -> bool {
        self.favorites.iter().any(|f| f.ip == ip && f.port == port)
    }

    /// Put a server at the top of the history (moving it if already there).
    pub fn add_history(&mut self, name: String, ip: String, port: u16) {
        let ts = dz_common::time::now_secs() as i64;
        self.history.retain(|h| h.ip != ip || h.port != port);
        self.history.insert(
            0,
            History {
                name,
                ip,
                port,
                ts,
                extra: None,
            },
        );
        // Bounded, so the profile JSON rewritten on every mutation stays small.
        self.history.truncate(MAX_HISTORY);
    }

    pub fn remove_history(&mut self, ip: &str, port: u16) {
        self.history.retain(|h| h.ip != ip || h.port != port);
    }

    /// Add an IP to the excluded list (no-op if already present).
    pub fn add_excluded_ip(&mut self, ip: String) {
        if !self.excluded_ips.contains(&ip) {
            self.excluded_ips.push(ip);
        }
    }

    pub fn remove_excluded_ip(&mut self, ip: &str) {
        self.excluded_ips.retain(|e| e != ip);
    }
}

/// A serialized profile waiting to be written.
#[derive(Debug)]
pub struct Snapshot {
    path: PathBuf,
    data: String,
    seq: u64,
}

impl Snapshot {
    /// Write through a temporary file and a rename, so a crash mid-write never
    /// leaves a truncated `profile.json` behind.
    pub async fn write(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let tmp = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &self.data).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
    }

    /// [`Snapshot::write`] for synchronous callers.
    pub fn write_blocking(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, &self.data)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// Orders profile writes.
///
/// A snapshot is taken while the profile's lock is held and written after it
/// is released, so the disk write never blocks other commands. Two commands
/// can then race to the disk; the writer numbers snapshots in the order they
/// were taken and drops any that is older than one already written, so the
/// file always ends at the latest state.
#[derive(Debug, Default)]
pub struct Writer {
    next: AtomicU64,
    written: tokio::sync::Mutex<u64>,
}

impl Writer {
    /// Number a snapshot. Call while the profile's lock is still held.
    pub fn stamp(&self, mut snapshot: Snapshot) -> Snapshot {
        snapshot.seq = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        snapshot
    }

    /// Write a stamped snapshot unless a newer one already reached the disk.
    pub async fn write(&self, snapshot: Snapshot) -> Result<()> {
        let mut written = self.written.lock().await;
        if snapshot.seq <= *written {
            return Ok(());
        }
        snapshot.write().await?;
        *written = snapshot.seq;
        Ok(())
    }
}
