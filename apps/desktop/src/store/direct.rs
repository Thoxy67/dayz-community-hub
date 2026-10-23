//! The Direct Connect form (= `features/connect/direct.svelte.ts`): an address, what the
//! server says about itself when asked, and the extra launch arguments (mods the query
//! did not report, any flag) sent along when joining. A store, so the form survives
//! leaving the view and coming back (not a restart), and so the commands reach it.
//!
//! What it resolves to (the listed entry, the favourite, the ports, the server's mods)
//! is computed from the other stores when asked ([`Direct::resolve`]): nothing is copied
//! that could go stale. A query is one flight, superseded by the next: an older answer
//! is dropped with its task.

use std::sync::Arc;

use dz_game::{DzchConfig, DzchMod};
use dz_hub::Hub;
use dz_hub::features::a2s::A2sDetailsDto;
use dz_hub::features::browser::ServerRow;
use dz_hub::features::servers::ServerDto;

use super::connect::{DEFAULT_GAME_PORT, Target, split_host_port};
use super::mods::Mods;
use super::profile::{Favorite, Profile};
use super::servers::Servers;
use super::{Bus, Stores};
use crate::i18n::{t, tf};
use crate::rt::{self, Task};

/// What an argument is: a mod (`-mod=@id`) or a flag, verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    Mod,
    Custom,
}

/// One argument appended to the launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchArg {
    pub id: u64,
    pub kind: ArgKind,
    pub value: String,
    pub label: String,
    /// Reported by the server's query; listed first.
    pub from_server: bool,
    pub enabled: bool,
}

impl LaunchArg {
    /// As the command line writes it.
    pub fn arg(&self) -> String {
        match self.kind {
            ArgKind::Mod => format!("-mod=@{}", self.value),
            ArgKind::Custom if self.value.starts_with('-') => self.value.clone(),
            ArgKind::Custom => format!("-{}", self.value),
        }
    }
}

/// Whether one of the server's mods is here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModStatus {
    Installed,
    Stale,
    Missing,
}

/// A mod the server runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerMod {
    pub id: i64,
    pub name: String,
}

/// `^(25[0-5]|2[0-4]\d|1?\d?\d)(\.…){3}$`: only such addresses are looked up in the list.
pub fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            (1..=3).contains(&p.len())
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok_and(|n| n <= 255)
                && !(p.len() == 3 && p.starts_with('0'))
        })
}

/// `encodeURIComponent`.
fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `dzchLink` (`lib/dzch.ts`): the query port only when it differs, then the name, the
/// password and the mods' ids.
pub fn dzch_link(c: &DzchConfig) -> String {
    let mut p = Vec::new();
    if let Some(q) = c.query_port.filter(|q| *q != c.port) {
        p.push(format!("qport={q}"));
    }
    if !c.name.is_empty() {
        p.push(format!("name={}", encode_component(&c.name)));
    }
    if let Some(pw) = c.password.as_deref().filter(|p| !p.is_empty()) {
        p.push(format!("password={}", encode_component(pw)));
    }
    if !c.mods.is_empty() {
        let ids: Vec<String> = c.mods.iter().map(|m| m.id.to_string()).collect();
        p.push(format!("mods={}", ids.join(",")));
    }
    let q = if p.is_empty() {
        String::new()
    } else {
        format!("?{}", p.join("&"))
    };
    format!("dzch://{}:{}{q}", c.ip, c.port)
}

/// The file name an export suggests: the server's name made safe (60 characters), else
/// `ip_port`.
pub fn export_name(c: &DzchConfig) -> String {
    if c.name.is_empty() {
        return format!("{}_{}.dzch", c.ip, c.port);
    }
    let safe: String = c
        .name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ' ' {
                ch
            } else {
                '_'
            }
        })
        .take(60)
        .collect();
    format!("{safe}.dzch")
}

/// The first part of a query: the listed entry (looked up when not known), the server's
/// own answer, and the details fetch already on its way.
struct Answer {
    listed: Option<ServerRow>,
    a2s: Result<Arc<A2sDetailsDto>, String>,
    details: Option<Task<Option<ServerDto>>>,
}

/// What the form resolves to now, read from the other stores.
#[derive(Clone, Debug, Default)]
pub struct Resolved {
    pub ip: String,
    pub port_num: Option<u16>,
    pub address_error: bool,
    pub port_error: bool,
    pub valid: bool,
    pub listed: Option<Arc<ServerRow>>,
    pub favorite: Option<Favorite>,
    pub game_port: u16,
    pub query_port: Option<u16>,
    pub name: String,
}

pub struct Direct {
    hub: Hub,
    bus: Bus,
    pub address: String,
    pub port: String,
    /// Only when the server answers queries on a port of its own.
    pub query_port: String,
    pub password: String,
    /// The password came from a favourite, not the keyboard.
    pub password_from_favorite: bool,

    pub a2s: Option<Arc<A2sDetailsDto>>,
    pub details: Option<ServerDto>,
    pub querying: bool,
    pub details_loading: bool,
    pub error: String,
    pub resolved_query_port: Option<i64>,

    pub args: Vec<LaunchArg>,
    next_id: u64,

    // What the view shows that commands also switch.
    pub show_password: bool,
    /// A submit, Enter or Connect was tried: errors show from now on.
    pub touched: bool,
    pub ports_open: bool,
    pub args_open: bool,
    /// The installed mod picked to add, and the custom argument being typed.
    pub pick: Option<u64>,
    pub custom: String,

    /// The favourite whose password filled the field last (fills once per favourite).
    filled_from: Option<(String, u16)>,
    query: Option<Task<Answer>>,
    details_task: Option<Task<Option<ServerDto>>>,
    export: Option<Task<Result<Option<String>, String>>>,
}

impl Direct {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        Self {
            hub,
            bus,
            address: String::new(),
            port: DEFAULT_GAME_PORT.to_string(),
            query_port: String::new(),
            password: String::new(),
            password_from_favorite: false,
            a2s: None,
            details: None,
            querying: false,
            details_loading: false,
            error: String::new(),
            resolved_query_port: None,
            args: Vec::new(),
            next_id: 0,
            show_password: false,
            touched: false,
            ports_open: false,
            args_open: false,
            pick: None,
            custom: String::new(),
            filled_from: None,
            query: None,
            details_task: None,
            export: None,
        }
    }

    // ── what the form resolves to ───────────────────────────────────────────

    fn port_num(&self) -> Option<u16> {
        self.port.trim().parse::<u16>().ok().filter(|p| *p >= 1)
    }

    /// An address and a port are filled in (what `resolve` calls valid).
    pub fn valid(&self) -> bool {
        !self.address.trim().is_empty() && self.port_num().is_some()
    }

    /// The listed entry, the favourite, the ports and the name, now. Strict IPv4 only is
    /// looked up in the list, so typing does not ask for every prefix.
    pub fn resolve(&self, servers: &Servers, profile: &Profile) -> Resolved {
        let ip = self.address.trim().to_string();
        let port_num = self.port_num();
        let address_error = ip.is_empty();
        let port_error = port_num.is_none();
        let valid = !address_error && !port_error;
        let listed = match port_num {
            Some(p) if valid && is_ipv4(&ip) => {
                servers.row(&ip, i64::from(p)).map(|r| Arc::new(r.to_row()))
            }
            _ => None,
        };
        let favorite = port_num.filter(|_| valid).and_then(|p| {
            profile
                .favorites
                .iter()
                .find(|f| {
                    f.ip == ip
                        && (f.port == p
                            || listed.as_ref().is_some_and(|l| {
                                i64::from(f.port) == l.game_port
                                    || i64::from(f.port) == l.query_port
                            }))
                })
                .cloned()
        });
        let to_u16 = |n: i64| u16::try_from(n).ok().filter(|p| *p >= 1);
        let game_port = listed
            .as_ref()
            .and_then(|l| to_u16(l.game_port))
            .or_else(|| self.a2s.as_ref().and_then(|a| a.game_port.and_then(to_u16)))
            .or(port_num)
            .unwrap_or(0);
        let query_port = listed
            .as_ref()
            .and_then(|l| to_u16(l.query_port))
            .or_else(|| self.resolved_query_port.and_then(to_u16))
            .or_else(|| {
                self.query_port
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .filter(|p| *p >= 1)
            });
        let name = self
            .a2s
            .as_ref()
            .map(|a| a.server_name.clone())
            .or_else(|| listed.as_ref().map(|l| l.name.clone()))
            .unwrap_or_default();
        Resolved {
            ip,
            port_num,
            address_error,
            port_error,
            valid,
            listed,
            favorite,
            game_port,
            query_port,
            name,
        }
    }

    /// The server's mods: the list's (with proper names) first, then any only the query saw.
    pub fn server_mods(&self) -> Vec<ServerMod> {
        let a2s_mods = self.a2s.as_ref().map(|a| a.mods.as_slice()).unwrap_or(&[]);
        let base = match &self.details {
            Some(d) if !d.mods.is_empty() => d.mods.as_slice(),
            _ => a2s_mods,
        };
        let mut out: Vec<ServerMod> = Vec::with_capacity(base.len());
        let mut seen = rustc_hash::FxHashSet::default();
        let more = self
            .a2s
            .as_ref()
            .map(|a| a.mods.iter().chain(a.mods_a2s.iter()))
            .into_iter()
            .flatten();
        for m in base.iter().chain(more) {
            if seen.insert(m.steam_workshop_id) {
                out.push(ServerMod {
                    id: m.steam_workshop_id,
                    name: m.name.clone(),
                });
            }
        }
        out
    }

    pub fn mod_status(mods: &Mods, id: i64) -> ModStatus {
        match u64::try_from(id).ok().and_then(|id| mods.by_id(id)) {
            None => ModStatus::Missing,
            Some(m) if m.update_available => ModStatus::Stale,
            Some(_) => ModStatus::Installed,
        }
    }

    /// The arguments that will actually be passed, in order.
    pub fn launch_args(&self) -> Vec<String> {
        self.args
            .iter()
            .filter(|a| a.enabled)
            .map(LaunchArg::arg)
            .collect()
    }

    /// The server shown on the right: its list entry, or the address the query answered on.
    pub fn detail_target(&self, r: &Resolved) -> Option<(String, i64)> {
        if let Some(l) = &r.listed {
            return Some((l.ip.clone(), l.query_port));
        }
        match (&self.a2s, self.resolved_query_port) {
            (Some(_), Some(q)) => Some((r.ip.clone(), q)),
            _ => None,
        }
    }

    // ── editing ─────────────────────────────────────────────────────────────

    /// "1.2.3.4:2402" typed or pasted into the address field: split the port off.
    pub fn split_address(&mut self) {
        let (host, port) = split_host_port(&self.address);
        if let Some(p) = port {
            self.address = host;
            self.port = p.to_string();
        }
    }

    /// A favourite's saved password fills an empty field (once per favourite, so the
    /// player can still empty it).
    pub fn fill_from_favorite(&mut self, fav: Option<&Favorite>) {
        let key = fav.map(|f| (f.ip.clone(), f.port));
        if key == self.filled_from {
            return;
        }
        self.filled_from = key;
        if let Some(pw) = fav
            .and_then(|f| f.password.as_deref())
            .filter(|p| !p.is_empty())
            && self.password.is_empty()
        {
            self.password = pw.to_string();
            self.password_from_favorite = true;
        }
    }

    fn reset(&mut self) {
        self.details_loading = false;
        self.a2s = None;
        self.details = None;
        self.error.clear();
        self.resolved_query_port = None;
        self.query = None;
        self.details_task = None;
        self.querying = false;
    }

    /// Start from an address (a recent server, a prefill), then ask it.
    pub fn load(
        &mut self,
        ip: &str,
        port: u16,
        query_port: Option<u16>,
        password: Option<String>,
        servers: &mut Servers,
        profile: &Profile,
    ) {
        self.address = ip.to_string();
        self.port = port.to_string();
        self.query_port = query_port
            .filter(|q| *q != port)
            .map(|q| q.to_string())
            .unwrap_or_default();
        self.password = password.unwrap_or_default();
        self.password_from_favorite = false;
        self.reset();
        self.query(servers, profile);
    }

    pub fn clear(&mut self) {
        self.address.clear();
        self.port = DEFAULT_GAME_PORT.to_string();
        self.query_port.clear();
        self.password.clear();
        self.password_from_favorite = false;
        self.filled_from = None;
        self.args.retain(|a| !a.from_server);
        self.reset();
    }

    // ── asking the server ───────────────────────────────────────────────────

    /// Ask the server: the list's entry (looked up if not known), its own answer on the
    /// listed query port, the typed query port or the typed port; the details fetched
    /// alongside. A newer query drops this one's answers.
    pub fn query(&mut self, servers: &mut Servers, profile: &Profile) {
        self.split_address();
        let r = self.resolve(servers, profile);
        let Some(typed) = r.port_num.filter(|_| r.valid) else {
            return;
        };
        self.reset();
        self.querying = true;
        let hub = self.hub.clone();
        let ip = r.ip.clone();
        let known = r.listed.as_deref().cloned();
        let typed_qp = self
            .query_port
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|p| *p > 0);
        self.query = Some(rt::spawn(async move {
            let listed = match known {
                Some(l) => Some(l),
                None => super::servers::lookup_one(&hub, &ip, i64::from(typed)).await,
            };
            let details_of = |ip: String, qp: i64| {
                let hub = hub.clone();
                rt::spawn(async move { hub.get_server_details(ip, qp).await.ok() })
            };
            let mut details = listed
                .as_ref()
                .filter(|l| l.mods_count > 0)
                .map(|l| details_of(l.ip.clone(), l.query_port));
            let qp = listed
                .as_ref()
                .map(|l| l.query_port)
                .or(typed_qp)
                .unwrap_or(i64::from(typed));
            let target = listed.as_ref().map_or_else(|| ip.clone(), |l| l.ip.clone());
            let a2s = hub
                .query_a2s(target, qp, listed.as_ref().map(|l| l.game_port))
                .await;
            if listed.is_none()
                && let Ok(a) = &a2s
            {
                details = Some(details_of(ip.clone(), a.query_port));
            }
            Answer {
                listed,
                a2s,
                details,
            }
        }));
    }

    /// After a query: the server's mods become arguments (enabled, first), ones that
    /// disappeared are dropped, and whatever the player added is kept.
    fn sync_server_args(&mut self) {
        let detected = self.server_mods();
        let ids: rustc_hash::FxHashSet<String> =
            detected.iter().map(|m| m.id.to_string()).collect();
        let kept: Vec<LaunchArg> = self
            .args
            .iter()
            .filter(|a| a.from_server && ids.contains(&a.value))
            .cloned()
            .collect();
        let kept_ids: rustc_hash::FxHashSet<String> =
            kept.iter().map(|a| a.value.clone()).collect();
        let mut fresh = Vec::new();
        for m in detected
            .iter()
            .filter(|m| !kept_ids.contains(&m.id.to_string()))
        {
            self.next_id += 1;
            fresh.push(LaunchArg {
                id: self.next_id,
                kind: ArgKind::Mod,
                value: m.id.to_string(),
                label: if m.name.is_empty() {
                    m.id.to_string()
                } else {
                    m.name.clone()
                },
                from_server: true,
                enabled: true,
            });
        }
        let manual: Vec<LaunchArg> = self
            .args
            .iter()
            .filter(|a| !a.from_server)
            .cloned()
            .collect();
        self.args = kept.into_iter().chain(fresh).chain(manual).collect();
    }

    /// Take finished answers in. True when something on screen changed.
    pub fn poll(&mut self, servers: &mut Servers, profile: &Profile) -> bool {
        let mut changed = false;
        if let Some(a) = self.query.as_mut().and_then(Task::poll) {
            self.query = None;
            changed = true;
            self.querying = false;
            let typed = self.port_num().map_or(0, i64::from);
            match a.a2s {
                Ok(a2s) => {
                    self.resolved_query_port = Some(a2s.query_port);
                    let game = a.listed.as_ref().map(|l| l.game_port).or(a2s.game_port);
                    if let Some(g) = game {
                        self.port = g.to_string();
                    }
                    if a2s.query_port != game.unwrap_or(typed) {
                        self.query_port = a2s.query_port.to_string();
                    }
                    let ip = a
                        .listed
                        .as_ref()
                        .map_or_else(|| self.address.trim().to_string(), |l| l.ip.clone());
                    let (_, _, manual) = profile.ping_settings();
                    servers.ping_one(&ip, a2s.query_port, manual);
                    self.a2s = Some(a2s);
                }
                Err(e) => self.error = e,
            }
            self.details_loading = a.details.is_some();
            match a.details {
                Some(t) => self.details_task = Some(t),
                None => self.sync_server_args(),
            }
        }
        if let Some(d) = self.details_task.as_mut().and_then(Task::poll) {
            self.details_task = None;
            self.details = d;
            self.details_loading = false;
            self.sync_server_args();
            changed = true;
        }
        if let Some(r) = self.export.as_mut().and_then(Task::poll) {
            self.export = None;
            match r {
                Ok(Some(file)) => self.bus.ok(tf("connect", "exported", &[("file", &file)])),
                Ok(None) => {}
                Err(e) => self
                    .bus
                    .err(tf("connect", "exportFailed", &[("error", &e)])),
            }
        }
        changed
    }

    // ── launch arguments ────────────────────────────────────────────────────

    pub fn add_mod(&mut self, mods: &Mods, id: u64) {
        let Some(m) = mods.by_id(id) else { return };
        let v = id.to_string();
        if self
            .args
            .iter()
            .any(|a| a.kind == ArgKind::Mod && a.value == v)
        {
            return;
        }
        self.next_id += 1;
        self.args.push(LaunchArg {
            id: self.next_id,
            kind: ArgKind::Mod,
            value: v,
            label: m.name.clone(),
            from_server: false,
            enabled: true,
        });
    }

    pub fn add_custom(&mut self, raw: &str) {
        let value = raw.trim();
        if value.is_empty() {
            return;
        }
        self.next_id += 1;
        self.args.push(LaunchArg {
            id: self.next_id,
            kind: ArgKind::Custom,
            value: value.to_string(),
            label: value.to_string(),
            from_server: false,
            enabled: true,
        });
    }

    pub fn remove_arg(&mut self, id: u64) {
        self.args.retain(|a| a.id != id);
    }

    pub fn toggle_arg(&mut self, id: u64) {
        if let Some(a) = self.args.iter_mut().find(|a| a.id == id) {
            a.enabled = !a.enabled;
        }
    }

    pub fn move_arg(&mut self, id: u64, dir: i32) {
        let Some(i) = self.args.iter().position(|a| a.id == id) else {
            return;
        };
        let Some(j) = i.checked_add_signed(dir as isize) else {
            return;
        };
        if j < self.args.len() {
            self.args.swap(i, j);
        }
    }

    /// The installed mods not added yet, by name.
    pub fn available_mods<'m>(&self, mods: &'m Mods) -> Vec<(u64, &'m str)> {
        let mut v: Vec<(u64, &str)> = mods
            .installed
            .iter()
            .filter(|m| {
                let id = m.id.to_string();
                !self
                    .args
                    .iter()
                    .any(|a| a.kind == ArgKind::Mod && a.value == id)
            })
            .map(|m| (m.id, m.name.as_str()))
            .collect();
        v.sort_by_cached_key(|(_, n)| n.to_lowercase());
        v
    }

    // ── what is shared ──────────────────────────────────────────────────────

    /// The form as a `.dzch` config (v1).
    pub fn config(&self, r: &Resolved) -> DzchConfig {
        DzchConfig {
            version: 1,
            ip: r.ip.clone(),
            port: r.game_port,
            query_port: r.query_port.filter(|q| *q != r.game_port),
            name: r.name.clone(),
            password: Some(self.password.clone()).filter(|p| !p.is_empty()),
            mods: self
                .server_mods()
                .into_iter()
                .map(|m| DzchMod {
                    id: m.id,
                    name: m.name,
                })
                .collect(),
        }
    }

    /// Ask where, then write the file; a toast says how it went.
    pub fn export(&mut self, c: DzchConfig) {
        let hub = self.hub.clone();
        let file = export_name(&c);
        self.export = Some(rt::spawn(async move {
            let Some(f) = rfd::AsyncFileDialog::new()
                .set_title(t("connect", "exportTitle"))
                .set_file_name(file)
                .add_filter(t("connect", "dzchFilter"), &["dzch"])
                .save_file()
                .await
            else {
                return Ok(None);
            };
            let path = f.path().to_path_buf();
            let shown = path.file_name().map_or_else(
                || path.to_string_lossy().into_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            hub.write_dzch_file(path.to_string_lossy().into_owned(), c)
                .await
                .map(|()| Some(shown))
        }));
    }

    /// Where the join goes: the game port, the password, the enabled arguments.
    pub fn target(&self, r: &Resolved) -> Target {
        let args = self.launch_args();
        Target {
            ip: r.ip.clone(),
            port: r.game_port,
            password: Some(self.password.clone()).filter(|p| !p.is_empty()),
            extra_args: (!args.is_empty()).then_some(args),
        }
    }
}

// ── actions that need more than one store ───────────────────────────────────

impl Stores {
    /// What the Direct Connect form resolves to now.
    pub fn direct_resolved(&mut self) -> Resolved {
        self.direct.resolve(&self.servers, &self.profile)
    }

    pub fn direct_query(&mut self) {
        self.direct.touched = true;
        self.direct.query(&mut self.servers, &self.profile);
    }

    pub fn direct_load(&mut self, ip: &str, port: u16, qp: Option<u16>, pw: Option<String>) {
        self.direct
            .load(ip, port, qp, pw, &mut self.servers, &self.profile);
    }

    /// Join the address in the form (needs a profile).
    pub fn direct_join(&mut self) {
        self.direct.touched = true;
        self.direct.split_address();
        let r = self.direct_resolved();
        if !r.valid || self.profile.data.is_none() {
            return;
        }
        let t = self.direct.target(&r);
        let snap = self.cli_snapshot();
        self.connect.direct(t, snap);
    }

    pub fn direct_favorite(&mut self) {
        let r = self.direct_resolved();
        if !r.valid {
            return;
        }
        let name = if r.name.is_empty() {
            format!("{}:{}", r.ip, r.game_port)
        } else {
            r.name.clone()
        };
        let pw = Some(self.direct.password.clone()).filter(|p| !p.is_empty());
        self.profile.add_favorite(&name, &r.ip, r.game_port, pw);
    }

    pub fn direct_copy_link(&mut self, ctx: &egui::Context) {
        let r = self.direct_resolved();
        if !r.valid {
            return;
        }
        ctx.copy_text(dzch_link(&self.direct.config(&r)));
        self.bus.ok(t("connect", "linkCopied"));
    }

    pub fn direct_export(&mut self) {
        let r = self.direct_resolved();
        if r.valid {
            let c = self.direct.config(&r);
            self.direct.export(c);
        }
    }

    pub fn direct_add_mod(&mut self, id: u64) {
        self.direct.add_mod(&self.mods, id);
    }

    /// Take the form's answers in.
    pub fn direct_poll(&mut self) -> bool {
        self.direct.poll(&mut self.servers, &self.profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_ipv4() {
        assert!(is_ipv4("1.2.3.4"));
        assert!(is_ipv4("255.255.255.0"));
        assert!(!is_ipv4("256.1.1.1"));
        assert!(!is_ipv4("1.2.3"));
        assert!(!is_ipv4("1.2.3."));
        assert!(!is_ipv4("play.example.com"));
    }

    #[test]
    fn writes_links_and_names() {
        let mut c = DzchConfig {
            version: 1,
            ip: "1.2.3.4".into(),
            port: 2302,
            query_port: Some(2302),
            name: String::new(),
            password: None,
            mods: Vec::new(),
        };
        assert_eq!(dzch_link(&c), "dzch://1.2.3.4:2302");
        assert_eq!(export_name(&c), "1.2.3.4_2302.dzch");
        c.query_port = Some(27016);
        c.name = "A B/C".into();
        c.password = Some("p w".into());
        c.mods = vec![DzchMod {
            id: 1,
            name: "x".into(),
        }];
        assert_eq!(
            dzch_link(&c),
            "dzch://1.2.3.4:2302?qport=27016&name=A%20B%2FC&password=p%20w&mods=1"
        );
        assert_eq!(export_name(&c), "A B_C.dzch");
    }

    #[test]
    fn writes_args() {
        let a = |kind, value: &str| LaunchArg {
            id: 0,
            kind,
            value: value.into(),
            label: String::new(),
            from_server: false,
            enabled: true,
        };
        assert_eq!(a(ArgKind::Mod, "12").arg(), "-mod=@12");
        assert_eq!(a(ArgKind::Custom, "doLogs").arg(), "-doLogs");
        assert_eq!(a(ArgKind::Custom, "-doLogs").arg(), "-doLogs");
    }
}
