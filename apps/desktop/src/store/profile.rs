//! The player's profile and what changes it (= `stores/profile.svelte.ts`). Edits are
//! applied here first and then written; a failed write reloads the profile from disk, so
//! the screen never shows something that was not saved.
//!
//! The favourites and the history are moved out of each profile read into
//! [`Profile::favorites`] / [`Profile::history`] and edited there optimistically: read
//! them here, never `data.favorites` (left empty).

use dz_api::{Ipv4Addr, SocketAddrV4};
use dz_hub::Hub;
pub use dz_hub::features::profile::{FavoriteDto as Favorite, HistoryDto as History};
use dz_hub::features::profile::{ProfileDto, ProfileSettingsInput};
use dz_hub::features::steamworks::ModDownloaderDto;
pub use dz_profile::{FavoriteEdit, FavoriteGroup, FavoriteLabel, FavoriteMeta, LabelColor};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{Bus, ConfirmRequest, Signal};
use crate::i18n::{t, tf};
use crate::rt::{self, Task};

/// The account part of the settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountSettings {
    pub player: Option<String>,
    pub steam_login: Option<String>,
    pub steam_root: Option<String>,
    pub steamcmd_path: Option<String>,
    pub steam_api_key: Option<String>,
    pub steam_id: Option<String>,
    pub user_location: Option<(f64, f64)>,
}

/// The ping part of the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingSettings {
    pub timeout_auto: u32,
    pub timeout_manual: u32,
    pub max_retries: u32,
    pub scan_favorites: bool,
    pub scan_history: bool,
    pub scan_servers: bool,
}

/// A write's outcome: the toast for success, or the message for failure.
type WriteResult = Result<Option<String>, String>;

/// A launch option's write: what it was (`toggle:<key>` / `set:<key>`), the profile
/// before (restored on failure), the write.
type OptionWrite = (String, Option<ProfileDto>, Task<Result<(), String>>);

pub struct Profile {
    hub: Hub,
    bus: Bus,
    pub data: Option<ProfileDto>,
    pub favorites: Vec<Favorite>,
    /// The labels favourites can carry, moved out of each profile read like the
    /// favourites, and edited with them.
    pub labels: Vec<FavoriteLabel>,
    /// The favourites view's sections, in order.
    pub groups: Vec<FavoriteGroup>,
    pub history: Vec<History>,
    /// The Steam avatar as the hub gives it (a `data:` URI, kilobytes); set through
    /// [`Profile::set_avatar`], which works out [`Profile::avatar_key`] with it.
    pub avatar_url: Option<String>,
    /// The avatar's key for `parts::image::paint_keyed`, worked out once per avatar so
    /// the frame does not hash the URI.
    pub avatar_key: u64,
    /// Account writes that failed (the settings form puts the player's edits back).
    pub account_failures: u32,
    /// The favourites by address and the excluded IPs, asked by every row on screen
    /// each frame: an IPv4 one as numbers (what a listed row has), any other (a host
    /// name kept in the profile) by its text.
    favorite_addrs: FxHashSet<SocketAddrV4>,
    favorite_names: FxHashMap<String, Vec<u16>>,
    excluded: FxHashSet<Ipv4Addr>,
    excluded_names: FxHashSet<String>,
    load: Option<Task<Result<ProfileDto, String>>>,
    avatar: Option<Task<Option<String>>>,
    writes: Vec<Task<WriteResult>>,
    account: Option<Task<Result<(), String>>>,
    ping: Option<Task<Result<(), String>>>,
    ping_pending: Option<PingSettings>,
    option: Vec<OptionWrite>,
    /// The favourites' edits on their way to the hub, one batch at a time so they
    /// land in the order made (a label made, then given), and the ones made
    /// meanwhile with their success toasts.
    fav_write: Option<Task<WriteResult>>,
    fav_pending: Vec<(FavoriteEdit, Option<String>)>,
    /// Moves each time a write landed (favourites, history, settings): what is
    /// read from the hub's profile (the saved servers' mods) is read again.
    pub written: u64,
}

impl Profile {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        Self {
            hub,
            bus,
            data: None,
            favorites: Vec::new(),
            labels: Vec::new(),
            groups: Vec::new(),
            history: Vec::new(),
            avatar_url: None,
            avatar_key: 0,
            account_failures: 0,
            favorite_addrs: FxHashSet::default(),
            favorite_names: FxHashMap::default(),
            excluded: FxHashSet::default(),
            excluded_names: FxHashSet::default(),
            load: None,
            avatar: None,
            writes: Vec::new(),
            account: None,
            ping: None,
            ping_pending: None,
            option: Vec::new(),
            fav_write: None,
            fav_pending: Vec::new(),
            written: 0,
        }
    }

    fn set(&mut self, mut p: ProfileDto) {
        self.favorites = std::mem::take(&mut p.favorites);
        self.labels = std::mem::take(&mut p.favorite_labels);
        self.groups = std::mem::take(&mut p.favorite_groups);
        self.history = std::mem::take(&mut p.history);
        self.data = Some(p);
        self.derive();
    }

    fn derive(&mut self) {
        self.favorite_addrs.clear();
        self.favorite_names.clear();
        for f in &self.favorites {
            match f.ip.trim().parse() {
                Ok(ip) => {
                    self.favorite_addrs.insert(SocketAddrV4::new(ip, f.port));
                }
                Err(_) => self
                    .favorite_names
                    .entry(f.ip.clone())
                    .or_default()
                    .push(f.port),
            }
        }
        self.excluded.clear();
        self.excluded_names.clear();
        for e in self.data.iter().flat_map(|p| &p.excluded_ips) {
            match e.trim().parse() {
                Ok(ip) => {
                    self.excluded.insert(ip);
                }
                Err(_) => {
                    self.excluded_names.insert(e.clone());
                }
            }
        }
    }

    /// Whether `ip:port` (as text: a favourite, what was typed) is a favourite.
    pub fn is_favorite(&self, ip: &str, port: u16) -> bool {
        match ip.trim().parse() {
            Ok(ip) => self.is_favorite_at(SocketAddrV4::new(ip, port)),
            Err(_) => self
                .favorite_names
                .get(ip)
                .is_some_and(|ports| ports.contains(&port)),
        }
    }

    /// Whether a listed server's address (query or game port) is a favourite.
    pub fn is_favorite_at(&self, addr: SocketAddrV4) -> bool {
        self.favorite_addrs.contains(&addr)
    }

    /// A cheap fingerprint of the favourites, the history and the excluded addresses
    /// (no allocation): what a view caches from them is made again when it moves,
    /// optimistic edits included.
    pub fn stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = rustc_hash::FxHasher::default();
        self.favorites.len().hash(&mut h);
        for f in &self.favorites {
            (&f.ip, f.port, &f.name, &f.password).hash(&mut h);
            (&f.note, &f.labels, &f.group).hash(&mut h);
        }
        (&self.labels, &self.groups).hash(&mut h);
        self.history.len().hash(&mut h);
        for e in &self.history {
            (&e.ip, e.port, &e.name, e.ts).hash(&mut h);
        }
        (self.excluded.len(), self.excluded_names.len()).hash(&mut h);
        h.finish()
    }

    /// Whether `ip` (as text) is excluded.
    pub fn is_excluded(&self, ip: &str) -> bool {
        match ip.trim().parse() {
            Ok(ip) => self.is_excluded_ip(ip),
            Err(_) => self.excluded_names.contains(ip),
        }
    }

    /// Whether a listed server's IP is excluded.
    pub fn is_excluded_ip(&self, ip: Ipv4Addr) -> bool {
        self.excluded.contains(&ip)
    }

    pub fn has_api_key(&self) -> bool {
        self.data
            .as_ref()
            .is_some_and(|p| p.steam_api_key.as_deref().is_some_and(|k| !k.is_empty()))
    }

    /// A Web API key and a Steam id: friends can be asked behind other views.
    pub fn has_web_api(&self) -> bool {
        self.has_api_key()
            && self
                .data
                .as_ref()
                .is_some_and(|p| p.steam_id.as_deref().is_some_and(|k| !k.is_empty()))
    }

    /// Mods download through the running Steam client rather than SteamCMD.
    pub fn via_steam(&self) -> bool {
        self.data
            .as_ref()
            .is_some_and(|p| p.mod_downloader == ModDownloaderDto::Steamworks)
    }

    /// (background timeout, manual timeout), with the defaults.
    pub fn ping_settings(&self) -> (u32, u32) {
        self.data.as_ref().map_or((2000, 10_000), |p| {
            (p.ping_timeout_auto, p.ping_timeout_manual)
        })
    }

    pub fn load(&mut self) {
        let hub = self.hub.clone();
        self.load = Some(rt::spawn(async move { hub.get_profile().await }));
    }

    /// A new avatar (or none), and its key.
    pub fn set_avatar(&mut self, a: Option<String>) {
        if a == self.avatar_url {
            return;
        }
        self.avatar_key = a.as_deref().map_or(0, crate::parts::image::key_of);
        self.avatar_url = a;
    }

    pub fn load_avatar(&mut self) {
        if !self.has_web_api() {
            self.set_avatar(None);
            return;
        }
        let hub = self.hub.clone();
        self.avatar = Some(rt::spawn(async move {
            hub.fetch_steam_avatar().await.ok().flatten()
        }));
    }

    /// Apply locally (already done by the caller), write, and on failure reload and say why.
    fn write<F>(&mut self, fut: F, done: Option<String>, fail_prefix: Option<String>)
    where
        F: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        let prefix = fail_prefix.unwrap_or_else(|| t("shell", "errorFailed").to_string());
        self.writes.push(rt::spawn(async move {
            match fut.await {
                Ok(()) => Ok(done),
                Err(e) => Err(format!("{prefix}: {e}")),
            }
        }));
    }

    fn settings(&self) -> ProfileSettingsInput {
        let p = self.data.as_ref();
        ProfileSettingsInput {
            player: p.and_then(|p| p.player.clone()),
            steam_login: p.and_then(|p| p.steam_login.clone()),
            steam_root: p.and_then(|p| p.steam_root.clone()),
            steamcmd_enabled: p.is_none_or(|p| p.steamcmd_enabled),
            steamcmd_path: p.and_then(|p| p.steamcmd_path.clone()),
            steam_api_key: p.and_then(|p| p.steam_api_key.clone()),
            steam_id: p.and_then(|p| p.steam_id.clone()),
            user_location: p.and_then(|p| p.user_location),
            ping_timeout_auto: p.map_or(2000, |p| p.ping_timeout_auto),
            ping_timeout_manual: p.map_or(10_000, |p| p.ping_timeout_manual),
            ping_max_retries: p.map_or(0, |p| p.ping_max_retries),
            ping_scan_favorites: p.is_none_or(|p| p.ping_scan_favorites),
            ping_scan_history: p.is_none_or(|p| p.ping_scan_history),
            ping_scan_servers: p.is_none_or(|p| p.ping_scan_servers),
        }
    }

    // ── account ─────────────────────────────────────────────────────────────

    /// Optimistic: the profile takes the values at once; a failed write reloads it.
    pub fn save_account(&mut self, a: AccountSettings) {
        if let Some(p) = &mut self.data {
            p.player.clone_from(&a.player);
            p.steam_login.clone_from(&a.steam_login);
            p.steam_root.clone_from(&a.steam_root);
            p.steamcmd_path.clone_from(&a.steamcmd_path);
            p.steam_api_key.clone_from(&a.steam_api_key);
            p.steam_id.clone_from(&a.steam_id);
            p.user_location = a.user_location;
        }
        let mut s = self.settings();
        s.player = a.player;
        s.steam_login = a.steam_login;
        s.steam_root = a.steam_root;
        s.steamcmd_path = a.steamcmd_path;
        s.steam_api_key = a.steam_api_key;
        s.steam_id = a.steam_id;
        s.user_location = a.user_location;
        let hub = self.hub.clone();
        self.account = Some(rt::spawn(async move { hub.save_profile_settings(s).await }));
    }

    /// Ping settings are written as the sliders move, without a toast; a write waits for
    /// the one in flight, and only the newest waiting one is sent.
    pub fn save_ping(&mut self, p: PingSettings) {
        if self.data.is_none() {
            return;
        }
        if let Some(d) = &mut self.data {
            d.ping_timeout_auto = p.timeout_auto;
            d.ping_timeout_manual = p.timeout_manual;
            d.ping_max_retries = p.max_retries;
            d.ping_scan_favorites = p.scan_favorites;
            d.ping_scan_history = p.scan_history;
            d.ping_scan_servers = p.scan_servers;
        }
        if self.ping.is_some() {
            self.ping_pending = Some(p);
            return;
        }
        let s = self.settings();
        let hub = self.hub.clone();
        self.ping = Some(rt::spawn(async move { hub.save_profile_settings(s).await }));
    }

    /// SteamCMD or the Steam client: takes effect at once, from the next operation.
    pub fn set_mod_downloader(&mut self, d: ModDownloaderDto) {
        if let Some(p) = &mut self.data {
            p.mod_downloader = d;
        }
        let hub = self.hub.clone();
        self.write(async move { hub.set_mod_downloader(d).await }, None, None);
    }

    /// Steam's own server list is added to the launcher's.
    pub fn set_steam_server_list(&mut self, enabled: bool) {
        if let Some(p) = &mut self.data {
            p.steam_server_list = enabled;
        }
        let hub = self.hub.clone();
        self.write(
            async move { hub.set_steam_server_list(enabled).await },
            None,
            None,
        );
    }

    /// Discord Rich Presence, and whether it names the server: saved, applied at once.
    pub fn set_discord(&mut self, enabled: bool, show_server: bool) {
        if let Some(p) = &mut self.data {
            p.discord_presence = enabled;
            p.discord_show_server = show_server;
        }
        let hub = self.hub.clone();
        self.write(
            async move { hub.set_discord_presence(enabled, show_server).await },
            None,
            None,
        );
    }

    // ── favourites ──────────────────────────────────────────────────────────

    pub fn add_favorite(&mut self, name: &str, ip: &str, port: u16, password: Option<String>) {
        let prev = self
            .favorites
            .iter()
            .position(|f| f.ip == ip && f.port == port)
            .map(|i| self.favorites.remove(i));
        // Saved again: what the player kept with it stays.
        let prev = prev.unwrap_or_default();
        self.favorites.push(Favorite {
            name: name.to_string(),
            ip: ip.to_string(),
            port,
            password: password.clone().or(prev.password),
            note: prev.note,
            labels: prev.labels,
            group: prev.group,
            character: prev.character,
            launch_args: prev.launch_args,
        });
        self.derive();
        let hub = self.hub.clone();
        let (n, i) = (name.to_string(), ip.to_string());
        let done = tf("favorites", "added", &[("name", &name)]);
        self.write(
            async move { hub.add_favorite(n, i, port, password).await },
            Some(done),
            None,
        );
    }

    pub fn remove_favorite(&mut self, ip: &str, port: u16) {
        self.favorites.retain(|f| !(f.ip == ip && f.port == port));
        self.derive();
        let hub = self.hub.clone();
        let i = ip.to_string();
        self.write(
            async move { hub.remove_favorite(i, port).await },
            Some(t("favorites", "removed").to_string()),
            None,
        );
    }

    pub fn toggle_favorite(&mut self, name: &str, ip: &str, port: u16) {
        if self.is_favorite(ip, port) {
            self.remove_favorite(ip, port);
        } else {
            self.add_favorite(name, ip, port, None);
        }
    }

    /// The favourite saved as `ip:port`.
    pub fn favorite(&self, ip: &str, port: u16) -> Option<&Favorite> {
        self.favorites.iter().find(|f| f.ip == ip && f.port == port)
    }

    /// The favourite a server is saved as: under `ip` with any of `ports` (saved,
    /// query or game port alike).
    pub fn favorite_of(&self, ip: &str, ports: &[u16]) -> Option<&Favorite> {
        self.favorites
            .iter()
            .find(|f| f.ip == ip && ports.contains(&f.port))
    }

    /// The label named `name`.
    pub fn label(&self, name: &str) -> Option<&FavoriteLabel> {
        self.labels.iter().find(|l| l.name == name)
    }

    /// Change a favourite's note, labels, group or launch settings, or the labels and
    /// groups: applied here at once with the hub's own rules, then written; a refused
    /// edit says why and changes nothing, a failed write reloads the profile.
    pub fn edit_favorites(&mut self, edit: FavoriteEdit, done: Option<String>) {
        let applied = dz_profile::favorites::FavoriteBook {
            favorites: &mut self.favorites,
            labels: &mut self.labels,
            groups: &mut self.groups,
        }
        .apply(edit.clone());
        if let Err(e) = applied {
            self.bus.err(edit_error(&e));
            return;
        }
        self.fav_pending.push((edit, done));
        self.send_favorite_edits();
    }

    /// The edits waiting, as one batch, unless one is on its way already (it sends
    /// the rest when it lands).
    fn send_favorite_edits(&mut self) {
        if self.fav_write.is_some() || self.fav_pending.is_empty() {
            return;
        }
        let (edits, done): (Vec<FavoriteEdit>, Vec<Option<String>>) =
            std::mem::take(&mut self.fav_pending).into_iter().unzip();
        // One toast for the batch: the last edit's that has one.
        let done = done.into_iter().flatten().last();
        let hub = self.hub.clone();
        let prefix = t("shell", "errorFailed").to_string();
        self.fav_write = Some(rt::spawn(async move {
            match hub.edit_favorites(edits).await {
                Ok(()) => Ok(done),
                Err(e) => Err(format!("{prefix}: {e}")),
            }
        }));
    }

    /// Remove, after asking.
    pub fn confirm_remove_favorite(&self, fav: &Favorite) {
        let req = ConfirmRequest::new(
            t("favorites", "removeTitle"),
            tf("favorites", "removeMessage", &[("name", &fav.name)]),
        )
        .danger();
        let (ip, port) = (fav.ip.clone(), fav.port);
        self.bus
            .confirm_then(req, move |cx| cx.stores.profile.remove_favorite(&ip, port));
    }

    // ── history ─────────────────────────────────────────────────────────────

    /// Remove an entry, after asking.
    pub fn remove_history(&self, h: &History) {
        let req = ConfirmRequest::new(
            t("history", "removeTitle"),
            tf("history", "removeMessage", &[("name", &h.name)]),
        )
        .danger();
        let (ip, port) = (h.ip.clone(), h.port);
        self.bus.confirm_then(req, move |cx| {
            cx.stores.profile.remove_history_now(&ip, port)
        });
    }

    fn remove_history_now(&mut self, ip: &str, port: u16) {
        self.history.retain(|e| !(e.ip == ip && e.port == port));
        let hub = self.hub.clone();
        let i = ip.to_string();
        self.write(
            async move { hub.remove_history_entry(i, port).await },
            Some(t("history", "removed").to_string()),
            None,
        );
    }

    /// Clear the whole history, after asking.
    pub fn clear_history(&self) {
        let count = self.history.len();
        let req = ConfirmRequest::new(
            t("history", "clearTitle"),
            tf("history", "clearMessage", &[("count", &count)]),
        )
        .danger();
        self.bus
            .confirm_then(req, move |cx| cx.stores.profile.clear_history_now());
    }

    fn clear_history_now(&mut self) {
        self.history.clear();
        let hub = self.hub.clone();
        self.write(
            async move { hub.clear_history().await },
            Some(t("history", "cleared").to_string()),
            None,
        );
    }

    // ── excluded IPs ────────────────────────────────────────────────────────

    pub fn exclude_ip(&mut self, ip: &str) {
        if let Some(p) = &mut self.data
            && !p.excluded_ips.iter().any(|e| e == ip)
        {
            p.excluded_ips.push(ip.to_string());
        }
        self.derive();
        let hub = self.hub.clone();
        let i = ip.to_string();
        self.write(
            async move { hub.add_excluded_ip(i).await },
            Some(tf("favorites", "ipExcluded", &[("ip", &ip)])),
            Some(t("favorites", "ipExcludeFailed").to_string()),
        );
    }

    pub fn unexclude_ip(&mut self, ip: &str) {
        if let Some(p) = &mut self.data {
            p.excluded_ips.retain(|e| e != ip);
        }
        self.derive();
        let hub = self.hub.clone();
        let i = ip.to_string();
        self.write(
            async move { hub.remove_excluded_ip(i).await },
            Some(tf("favorites", "ipUnexcluded", &[("ip", &ip)])),
            None,
        );
    }

    // ── launch options ──────────────────────────────────────────────────────

    pub fn toggle_option(&mut self, key: &str) {
        let before = self.data.clone();
        if let Some(o) = self
            .data
            .as_mut()
            .and_then(|p| p.options.iter_mut().find(|o| o.key == key))
        {
            o.enabled = !o.enabled;
        }
        let hub = self.hub.clone();
        let k = key.to_string();
        let task = rt::spawn(async move { hub.toggle_launch_option(k).await.map(|_| ()) });
        self.option.push((format!("toggle:{key}"), before, task));
    }

    pub fn set_option_value(&mut self, key: &str, value: Option<String>) {
        let before = self.data.clone();
        if let Some(o) = self
            .data
            .as_mut()
            .and_then(|p| p.options.iter_mut().find(|o| o.key == key))
        {
            if value.is_some() {
                o.enabled = true;
            }
            o.value.clone_from(&value);
        }
        let hub = self.hub.clone();
        let k = key.to_string();
        let task = rt::spawn(async move { hub.set_launch_option_value(k, value).await });
        self.option.push((format!("set:{key}"), before, task));
    }

    // ── the profile file ────────────────────────────────────────────────────

    /// Export to a file the player picks.
    pub fn export_to(&self, include_mods: bool) {
        let hub = self.hub.clone();
        let bus = self.bus.clone();
        rt::fire(async move {
            let pick = crate::parts::file_picker::Pick::save(
                t("settings", "profileExportTitle"),
                "dayz-community-hub-profile.dchub",
            )
            .filter(t("settings", "profileFilterName"), &["dchub"]);
            let Some(path) = crate::parts::file_picker::pick(pick).await else {
                return;
            };
            let path = path.to_string_lossy().into_owned();
            match hub.export_profile(path, include_mods).await {
                Ok(()) => bus.ok(t("settings", "profileExported")),
                Err(e) => bus.err(tf("settings", "profileExportFailed", &[("error", &e)])),
            }
        });
    }

    /// Import a file the player picks, after asking, then restart.
    pub fn import_from(&self) {
        let hub = self.hub.clone();
        let bus = self.bus.clone();
        rt::fire(async move {
            let pick = crate::parts::file_picker::Pick::file(t("settings", "profileImportTitle"))
                .filter(t("settings", "profileFilterName"), &["dchub"]);
            let Some(path) = crate::parts::file_picker::pick(pick).await else {
                return;
            };
            let req = ConfirmRequest::new(
                t("settings", "profileImportConfirmTitle"),
                t("settings", "profileImportConfirmMessage"),
            );
            if !bus.confirm(req).await {
                return;
            }
            let path = path.to_string_lossy().into_owned();
            // Restarted as the update restarts: the hub starts the new instance
            // (which waits for this one to be gone), then this window closes the
            // normal way, its GPU and drivers torn down by the main thread rather
            // than by an exit racing a frame. The mock starts nothing.
            // The import writes its own interface.json: this window's pending
            // preferences must not land over it while it closes.
            crate::prefs::hold_writes(true);
            match hub.import_profile(path).await.map(|_| ()) {
                Ok(()) => match hub.relaunch_app().await {
                    Ok(true) => {
                        if let Some(rt) = rt::Rt::get() {
                            crate::platform::life::quit(rt.ctx());
                        }
                    }
                    Ok(false) => crate::prefs::hold_writes(false),
                    Err(e) => {
                        crate::prefs::hold_writes(false);
                        bus.err(format!("{}: {e}", t("settings", "profileImportFailed")));
                    }
                },
                Err(e) => {
                    crate::prefs::hold_writes(false);
                    bus.err(format!("{}: {e}", t("settings", "profileImportFailed")));
                }
            }
        });
    }

    /// Back to factory defaults, after asking, then restart.
    pub fn reset(&self) {
        let hub = self.hub.clone();
        let bus = self.bus.clone();
        rt::fire(async move {
            let req = ConfirmRequest::new(
                t("settings", "profileResetTitle"),
                t("settings", "profileResetMessage"),
            )
            .danger();
            if !bus.confirm(req).await {
                return;
            }
            // An immediate exit, on purpose: closing the normal way would write the
            // preferences and the window's place back into the data directory just
            // deleted.
            let r = match hub.reset_profile().await {
                Ok(()) => hub.restart_app().await,
                Err(e) => Err(e),
            };
            if let Err(e) = r {
                bus.err(format!("{}: {e}", t("settings", "profileResetFailed")));
            }
        });
    }

    // ── the frame ───────────────────────────────────────────────────────────

    pub fn poll(&mut self, signals: &mut Vec<Signal>) -> bool {
        let mut changed = false;
        if let Some(task) = &mut self.load
            && let Some(r) = task.poll()
        {
            self.load = None;
            changed = true;
            match r {
                Ok(p) => {
                    self.set(p);
                    signals.push(Signal::ProfileLoaded);
                }
                Err(e) => self
                    .bus
                    .err(tf("settings", "profileLoadFailed", &[("error", &e)])),
            }
        }
        if let Some(task) = &mut self.avatar
            && let Some(a) = task.poll()
        {
            self.avatar = None;
            self.set_avatar(a);
            changed = true;
        }
        let mut reload = false;
        if let Some(task) = &mut self.fav_write
            && let Some(r) = task.poll()
        {
            self.fav_write = None;
            self.written = self.written.wrapping_add(1);
            match r {
                Ok(done) => {
                    if let Some(d) = done {
                        self.bus.ok(d);
                    }
                }
                Err(e) => {
                    // The hub refused or failed: what was applied here no longer
                    // holds, and the edits made since built on it.
                    self.bus.err(e);
                    self.fav_pending.clear();
                    reload = true;
                }
            }
            self.send_favorite_edits();
        }
        let bus = self.bus.clone();
        let mut landed = 0u64;
        self.writes.retain_mut(|task| match task.poll() {
            Some(Ok(done)) => {
                if let Some(d) = done {
                    bus.ok(d);
                }
                landed += 1;
                false
            }
            Some(Err(e)) => {
                bus.err(e);
                reload = true;
                landed += 1;
                false
            }
            None => task.is_pending(),
        });
        self.written = self.written.wrapping_add(landed);
        if let Some(task) = &mut self.account
            && let Some(r) = task.poll()
        {
            self.account = None;
            changed = true;
            match r {
                Ok(()) => {
                    self.bus.ok(t("settings", "profileSettingsSaved"));
                    signals.push(Signal::AccountSaved);
                }
                Err(e) => {
                    self.bus.err(tf(
                        "settings",
                        "profileSettingsSaveFailed",
                        &[("error", &e)],
                    ));
                    self.account_failures = self.account_failures.wrapping_add(1);
                    self.load();
                }
            }
        }
        if let Some(task) = &mut self.ping
            && let Some(r) = task.poll()
        {
            self.ping = None;
            if let Err(e) = r {
                self.bus.err(tf(
                    "settings",
                    "profileSettingsSaveFailed",
                    &[("error", &e)],
                ));
            }
            if let Some(p) = self.ping_pending.take() {
                self.save_ping(p);
            }
        }
        let mut failed = Vec::new();
        self.option
            .retain_mut(|(what, before, task)| match task.poll() {
                Some(Ok(())) => false,
                Some(Err(e)) => {
                    failed.push((what.clone(), before.take(), e));
                    false
                }
                None => task.is_pending(),
            });
        for (what, before, e) in failed {
            changed = true;
            let key = what.split_once(':').map_or("", |(_, k)| k);
            if let (Some(p), Some(b)) = (&mut self.data, before)
                && let (Some(o), Some(prev)) = (
                    p.options.iter_mut().find(|o| o.key == key),
                    b.options.into_iter().find(|o| o.key == key),
                )
            {
                *o = prev;
            }
            let word = if what.starts_with("toggle:") {
                "profileOptionToggleFailed"
            } else {
                "profileOptionSetFailed"
            };
            self.bus.err(tf("settings", word, &[("error", &e)]));
        }
        if reload {
            self.load();
            changed = true;
        }
        changed
    }
}

/// A refused favourites edit, in the player's language.
pub fn edit_error(e: &dz_profile::favorites::EditError) -> String {
    use dz_profile::favorites::{EditError as E, Kind::Group, Kind::Label};
    match e {
        E::NotFavorite { .. } => t("favorites", "notFavorite").to_string(),
        E::NoName(Label) => t("favorites", "errLabelNoName").to_string(),
        E::NoName(Group) => t("favorites", "errGroupNoName").to_string(),
        E::Taken(Label, n) => tf("favorites", "errLabelTaken", &[("name", n)]),
        E::Taken(Group, n) => tf("favorites", "errGroupTaken", &[("name", n)]),
        E::TooMany(Label, max) => tf("favorites", "errLabelTooMany", &[("max", max)]),
        E::TooMany(Group, max) => tf("favorites", "errGroupTooMany", &[("max", max)]),
        E::Missing(Label, n) => tf("favorites", "errLabelMissing", &[("name", n)]),
        E::Missing(Group, n) => tf("favorites", "errGroupMissing", &[("name", n)]),
    }
}
