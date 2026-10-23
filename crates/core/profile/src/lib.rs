//! The user's profile: account settings, favorites, history, launch options
//! and ping preferences, persisted as `profile.json` in the data directory.

pub mod favorites;
mod launch_options;
mod sessions;

pub use favorites::{FavoriteEdit, FavoriteGroup, FavoriteLabel, FavoriteMeta, LabelColor};
pub use launch_options::{LaunchOption, LaunchOptions};
pub use sessions::{PlayLog, SESSIONS_FILE, Session, SessionKind};

use dz_common::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub use dz_common::paths::{default_data_dir, default_profile_path};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Most history entries kept; newest first.
const MAX_HISTORY: usize = 200;

/// Every field has a default, so a profile written by any earlier version,
/// or edited by hand, still loads: what it lacks comes from [`Profile::default`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub steam_login: Option<String>,
    /// A Steam password saved by versions before 0.5. Never saved any more:
    /// typed once at SteamCMD's prompt so SteamCMD caches the login, then
    /// forgotten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_password: Option<String>,
    /// The account SteamCMD last logged in as, its login cached since.
    #[serde(default)]
    pub steamcmd_logged_in: Option<String>,
    #[serde(default)]
    pub steam_root: Option<String>,
    #[serde(default = "default_steamcmd_enabled")]
    pub steamcmd_enabled: bool,
    /// What downloads mods: SteamCMD (the default) or the Steam client.
    #[serde(default, deserialize_with = "ModDownloader::lenient")]
    pub mod_downloader: ModDownloader,
    /// Add the servers Steam's master lists (through the Steam client) to
    /// the launcher list. Steam shows DayZ running for the few seconds the
    /// addresses take to come.
    #[serde(default = "default_true")]
    pub steam_server_list: bool,
    pub player: Option<String>,
    /// Steam Web API key — used to fetch player avatar via GetPlayerSummaries.
    #[serde(default)]
    pub steam_api_key: Option<String>,
    /// Steam 64-bit account ID — used with the API key to resolve the avatar.
    #[serde(default)]
    pub steam_id: Option<String>,
    /// Optional explicit path to steamcmd binary (overrides auto-detection).
    #[serde(default)]
    pub steamcmd_path: Option<String>,
    /// User location for distance calculation (longitude, latitude).
    #[serde(default)]
    pub user_location: Option<(f64, f64)>,
    pub favorites: Vec<Favorite>,
    /// The labels favourites can carry, in the order they were made.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub favorite_labels: Vec<FavoriteLabel>,
    /// The favourites view's sections, in the player's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub favorite_groups: Vec<FavoriteGroup>,
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
    /// Times the scan asks a silent server again (0-5, default 1; 0 = never).
    /// One is enough at the A2S engine's pace, which loses few answers.
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
    /// Show what is being played on Discord (Rich Presence). Off by default:
    /// it tells others what the player does.
    #[serde(default)]
    pub discord_presence: bool,
    /// With the presence on, name the server played on (default true).
    #[serde(default = "default_true")]
    pub discord_show_server: bool,
    /// Workshop ids of the installed mods loaded with an offline mission
    /// (`-mod=`), in the order chosen; none by default.
    #[serde(default)]
    pub offline_mods: Vec<u64>,
    #[serde(
        deserialize_with = "launch_options::deserialize_launch_options",
        default = "LaunchOptions::defaults"
    )]
    pub options: LaunchOptions,
    pub version: String,
    #[serde(skip)]
    pub path: PathBuf,
}

impl Default for Profile {
    fn default() -> Self {
        Self::default_with_version(APP_VERSION)
    }
}

/// What downloads workshop mods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModDownloader {
    /// SteamCMD, into the launcher's own folder, logged in on its own.
    #[default]
    Steamcmd,
    /// The running Steam client (Steamworks): the account subscribes to
    /// each mod, Steam downloads it into its library and keeps it updated.
    Steamworks,
}

impl ModDownloader {
    /// A value this version does not know (a newer one wrote it, or a hand
    /// edit) reads as the default instead of failing the whole profile.
    fn lenient<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(serde_json::from_value(v).unwrap_or_default())
    }
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
    1
}

fn default_true() -> bool {
    true
}

/// A saved server. What came after the address and password ([`favorites`]) is
/// optional on disk and left out when empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favorite {
    pub name: String,
    pub ip: String,
    pub port: u16,
    /// Optional server join password — saved so Direct Connect can auto-fill it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// The player's own words about it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Names of [`Profile::favorite_labels`] it carries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// The name of the [`Profile::favorite_groups`] section it is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The character name (`-name=`) to join it with, over the profile's player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    /// Launch arguments of its own, over and beside the global launch options
    /// (the merge is the hub's `features::launch::args`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub launch_args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub ts: i64,
    /// Workshop ids of the mods the server ran when it was last joined from
    /// here: what it needs while the server list does not show it. `None`
    /// when never recorded; `Some(vec![])` is a server known to run none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_ids: Option<Vec<u64>>,
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
            let mut profile = Self::fresh(dz_common::deck::deck().hardware);
            profile.path = path.to_path_buf();
            profile.snapshot()?.write_blocking()?;
            return Ok(profile);
        }
        let data = std::fs::read_to_string(path)?;
        let mut profile = match serde_json::from_str::<Profile>(&data) {
            Ok(p) => p,
            Err(e) => {
                set_aside(path, &e);
                let p = Self {
                    path: path.to_path_buf(),
                    ..Self::default()
                };
                p.snapshot()?.write_blocking()?;
                p
            }
        };
        profile.path = path.to_path_buf();
        profile.favorite_book().tidy();
        Ok(profile)
    }

    /// [`Profile::load`] without blocking the runtime.
    pub async fn load_async(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !tokio::fs::try_exists(path).await.unwrap_or(false) {
            let mut profile = Self::fresh(dz_common::deck::deck().hardware);
            profile.path = path.to_path_buf();
            profile.snapshot()?.write().await?;
            return Ok(profile);
        }
        let data = tokio::fs::read_to_string(path).await?;
        let mut profile = match serde_json::from_str::<Profile>(&data) {
            Ok(p) => p,
            Err(e) => {
                set_aside(path, &e);
                let p = Self {
                    path: path.to_path_buf(),
                    ..Self::default()
                };
                p.snapshot()?.write().await?;
                p
            }
        };
        profile.path = path.to_path_buf();
        profile.favorite_book().tidy();
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

    /// The profile a first start writes. On a Steam Deck the mods come through
    /// the Steam client: it is always running there, logged in, and SteamCMD
    /// would want the password typed on a pad.
    pub fn fresh(on_deck: bool) -> Self {
        let mut p = Self::default_with_version(APP_VERSION);
        if on_deck {
            p.mod_downloader = ModDownloader::Steamworks;
        }
        p
    }

    pub fn default_with_version(version: &str) -> Self {
        Self {
            steam_login: None,
            steam_password: None,
            steamcmd_logged_in: None,
            steam_root: None,
            steamcmd_enabled: true,
            mod_downloader: ModDownloader::Steamcmd,
            steam_server_list: true,
            steamcmd_path: None,
            user_location: None,
            player: None,
            steam_api_key: None,
            steam_id: None,
            favorites: Vec::new(),
            favorite_labels: Vec::new(),
            favorite_groups: Vec::new(),
            history: Vec::new(),
            excluded_ips: Vec::new(),
            ping_concurrency: default_ping_concurrency(),
            ping_timeout_auto: default_ping_timeout_auto(),
            ping_timeout_manual: default_ping_timeout_manual(),
            ping_max_retries: default_ping_max_retries(),
            ping_scan_favorites: true,
            ping_scan_history: true,
            ping_scan_servers: true,
            discord_presence: false,
            discord_show_server: true,
            offline_mods: Vec::new(),
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
                ..Favorite::default()
            });
        }
    }

    pub fn remove_favorite(&mut self, ip: &str, port: u16) {
        self.favorites.retain(|f| f.ip != ip || f.port != port);
    }

    pub fn is_favorite(&self, ip: &str, port: u16) -> bool {
        self.favorites.iter().any(|f| f.ip == ip && f.port == port)
    }

    /// Change a favourite's note, labels or group, or the labels and groups
    /// themselves ([`favorites::FavoriteBook::apply`]).
    pub fn edit_favorites(&mut self, edit: FavoriteEdit) -> std::result::Result<(), String> {
        self.edit_favorites_all(vec![edit])
    }

    /// [`Profile::edit_favorites`] for several edits, in order, all or none
    /// ([`favorites::FavoriteBook::apply_all`]).
    pub fn edit_favorites_all(
        &mut self,
        edits: Vec<FavoriteEdit>,
    ) -> std::result::Result<(), String> {
        self.favorite_book()
            .apply_all(edits)
            .map_err(|e| e.to_string())
    }

    fn favorite_book(&mut self) -> favorites::FavoriteBook<'_> {
        favorites::FavoriteBook {
            favorites: &mut self.favorites,
            labels: &mut self.favorite_labels,
            groups: &mut self.favorite_groups,
        }
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
                mod_ids: None,
                extra: None,
            },
        );
        // Bounded, so the profile JSON rewritten on every mutation stays small.
        self.history.truncate(MAX_HISTORY);
    }

    /// Remember the mods the history entry at `ip:port` ran (a join just
    /// recorded it); nothing when there is no such entry.
    pub fn set_history_mods(&mut self, ip: &str, port: u16, mod_ids: Vec<u64>) {
        if let Some(h) = self
            .history
            .iter_mut()
            .find(|h| h.ip == ip && h.port == port)
        {
            h.mod_ids = Some(mod_ids);
        }
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

/// A profile that no longer parses is kept beside the new one as
/// `profile.json.broken`, so nothing is lost and the app still starts.
fn set_aside(path: &Path, why: &serde_json::Error) {
    let broken = path.with_extension("json.broken");
    let _ = std::fs::rename(path, &broken);
    eprintln!(
        "profile at {} could not be read ({why}); kept as {} and started fresh",
        path.display(),
        broken.display()
    );
}

/// A serialized profile waiting to be written.
#[derive(Debug)]
pub struct Snapshot {
    path: PathBuf,
    data: String,
    seq: u64,
}

impl Snapshot {
    /// Write through a flushed temporary file and a rename
    /// ([`dz_common::fs::write_atomic`]), so neither a crash mid-write nor a power
    /// cut just after leaves a truncated `profile.json` behind.
    pub async fn write(&self) -> Result<()> {
        let (path, data) = (self.path.clone(), self.data.clone().into_bytes());
        tokio::task::spawn_blocking(move || dz_common::fs::write_atomic(&path, &data))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }

    /// [`Snapshot::write`] for synchronous callers.
    pub fn write_blocking(&self) -> Result<()> {
        dz_common::fs::write_atomic(&self.path, self.data.as_bytes())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_profile_loads_with_defaults() {
        let p: Profile = serde_json::from_str("{}").unwrap();
        assert!(p.favorites.is_empty());
        assert!(p.steamcmd_enabled);
        assert_eq!(p.mod_downloader, ModDownloader::Steamcmd);
        assert_eq!(p.ping_concurrency, 64);
        assert!(!p.options.window.description.is_empty());
    }

    #[test]
    fn a_first_start_on_a_deck_downloads_through_steam() {
        assert_eq!(Profile::fresh(true).mod_downloader, ModDownloader::Steamworks);
        assert_eq!(Profile::fresh(false).mod_downloader, ModDownloader::Steamcmd);
    }

    #[test]
    fn discord_presence_is_off_until_asked_and_names_the_server_once_on() {
        let p: Profile = serde_json::from_str("{}").unwrap();
        assert!(
            !p.discord_presence,
            "off by default: it tells others what one plays"
        );
        assert!(p.discord_show_server);
        let d = Profile::default();
        assert!(!d.discord_presence && d.discord_show_server);
    }

    #[test]
    fn the_mod_downloader_round_trips_and_an_unknown_one_reads_as_steamcmd() {
        let p: Profile = serde_json::from_str(r#"{"mod_downloader": "steamworks"}"#).unwrap();
        assert_eq!(p.mod_downloader, ModDownloader::Steamworks);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""mod_downloader":"steamworks""#));
        let p: Profile = serde_json::from_str(r#"{"mod_downloader": "torrent"}"#).unwrap();
        assert_eq!(p.mod_downloader, ModDownloader::Steamcmd);
        assert!(serde_json::from_str::<Profile>(r#"{"mod_downloader": 3}"#).is_ok());
    }

    #[test]
    fn an_old_profile_loads() {
        // The bash-era format: camelCase option keys, values of any JSON type,
        // history entries with extra fields, no ping settings at all.
        let json = r#"{
            "steam_login": "someone",
            "player": "Survivor",
            "favorites": [{"name": "A", "ip": "1.2.3.4", "port": 27016}],
            "history": [{"name": "B", "ip": "5.6.7.8", "port": 2302, "ts": 1700000000, "mods": [1, 2]}],
            "options": {
                "maxMem": {"enabled": true, "value": 8192},
                "doLogs": {"enabled": true, "value": true},
                "filePathing": {"enabled": false, "value": null}
            },
            "version": "0.1.0"
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(p.steam_login.as_deref(), Some("someone"));
        assert_eq!(p.favorites[0].port, 27016);
        assert_eq!(p.history[0].ts, 1_700_000_000);
        assert!(p.options.max_mem.enabled);
        assert_eq!(p.options.max_mem.value.as_deref(), Some("8192"));
        assert_eq!(p.options.do_logs.value.as_deref(), Some("true"));
        assert!(!p.options.file_patching.enabled);
        assert_eq!(p.ping_timeout_auto, 2000);
    }

    #[test]
    fn a_profile_round_trips() {
        let mut p = Profile::default();
        p.add_favorite("A".into(), "1.1.1.1".into(), 27016, Some("pw".into()));
        p.add_history("B".into(), "2.2.2.2".into(), 2302);
        p.add_excluded_ip("3.3.3.3".into());
        p.user_location = Some((2.35, 48.85));
        let back: Profile = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back.favorites[0].password.as_deref(), Some("pw"));
        assert_eq!(back.history[0].ip, "2.2.2.2");
        assert_eq!(back.excluded_ips, ["3.3.3.3"]);
        assert_eq!(back.user_location, Some((2.35, 48.85)));
    }

    #[test]
    fn a_joins_mods_and_the_offline_choice_are_kept() {
        let mut p = Profile::default();
        p.add_history("a".into(), "10.0.0.1".into(), 2302);
        p.set_history_mods("10.0.0.1", 2302, vec![7, 8]);
        p.set_history_mods("10.0.0.9", 2302, vec![9]);
        p.add_history("vanilla".into(), "10.0.0.2".into(), 2302);
        p.set_history_mods("10.0.0.2", 2302, Vec::new());
        p.offline_mods = vec![8];
        let json = serde_json::to_string(&p).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.history[1].mod_ids, Some(vec![7, 8]));
        // A server known to run no mod stays known after a reload.
        assert_eq!(back.history[0].mod_ids, Some(Vec::new()));
        assert_eq!(back.offline_mods, vec![8]);
        // A rejoin starts the entry afresh; an old entry without them reads as unknown.
        p.add_history("a".into(), "10.0.0.1".into(), 2302);
        assert_eq!(p.history[0].mod_ids, None);
        let old: History = serde_json::from_str(
            r#"{"name":"x","ip":"1.2.3.4","port":1,"ts":0,"mods":[{"id":1}]}"#,
        )
        .unwrap();
        assert_eq!(old.mod_ids, None);
    }

    #[test]
    fn history_is_newest_first_and_bounded() {
        let mut p = Profile::default();
        for i in 0..(MAX_HISTORY + 10) {
            p.add_history(
                format!("s{i}"),
                format!("10.0.0.{}", i % 250),
                (2302 + i) as u16,
            );
        }
        p.add_history("again".into(), "10.0.0.5".into(), 2307);
        assert_eq!(p.history.len(), MAX_HISTORY);
        assert_eq!(p.history[0].name, "again");
        assert_eq!(
            p.history
                .iter()
                .filter(|h| h.ip == "10.0.0.5" && h.port == 2307)
                .count(),
            1
        );
    }

    #[test]
    fn a_favorite_without_notes_labels_or_groups_loads_and_writes_none() {
        let json = r#"{"favorites": [{"name": "A", "ip": "1.2.3.4", "port": 2302}]}"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        let f = &p.favorites[0];
        assert!(f.note.is_empty() && f.labels.is_empty() && f.launch_args.is_empty());
        assert_eq!((f.group.as_ref(), f.character.as_ref()), (None, None));
        assert!(p.favorite_labels.is_empty() && p.favorite_groups.is_empty());
        // Empty fields stay off the disk: an older build reads the same file.
        let out = serde_json::to_value(&p).unwrap();
        assert_eq!(
            out["favorites"][0],
            serde_json::json!({"name": "A", "ip": "1.2.3.4", "port": 2302})
        );
        assert!(out.get("favorite_labels").is_none() && out.get("favorite_groups").is_none());
    }

    #[test]
    fn favorite_notes_labels_groups_and_launch_settings_round_trip() {
        let mut p = Profile::default();
        p.add_favorite("A".into(), "1.1.1.1".into(), 2302, None);
        p.edit_favorites(FavoriteEdit::SetLabel {
            old: None,
            name: "PvE".into(),
            color: LabelColor::Ok,
        })
        .unwrap();
        p.edit_favorites(FavoriteEdit::AddGroup {
            name: "Weekend".into(),
        })
        .unwrap();
        p.edit_favorites(FavoriteEdit::CollapseGroup {
            name: "Weekend".into(),
            collapsed: true,
        })
        .unwrap();
        p.edit_favorites(FavoriteEdit::Meta {
            ip: "1.1.1.1".into(),
            port: 2302,
            meta: FavoriteMeta {
                note: "trader at Green Mountain".into(),
                labels: vec!["PvE".into()],
                group: Some("Weekend".into()),
                character: Some("Alt".into()),
                launch_args: "-maxMem=4096".into(),
            },
        })
        .unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.favorites, p.favorites);
        assert_eq!(back.favorite_labels, p.favorite_labels);
        assert_eq!(back.favorite_groups, p.favorite_groups);
        assert!(back.favorite_groups[0].collapsed);
        // Saving the favourite again keeps what came with it.
        let mut again = back;
        again.add_favorite("A2".into(), "1.1.1.1".into(), 2302, Some("pw".into()));
        assert_eq!(again.favorites[0].note, "trader at Green Mountain");
        assert_eq!(again.favorites[0].character.as_deref(), Some("Alt"));
    }

    #[test]
    fn a_broken_profile_is_set_aside() {
        let dir = std::env::temp_dir().join(format!("dz-profile-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.json");
        std::fs::write(&path, "{ not json").unwrap();
        let p = Profile::load(&path).unwrap();
        assert!(p.favorites.is_empty());
        assert!(dir.join("profile.json.broken").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
