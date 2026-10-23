//! Joining a server (= `stores/connect.svelte.ts`): find out which of its mods are
//! missing or may be stale, ask the player whether to fetch them first, then launch.
//! Every way into a server (a row, a favourite, an address, the command line, a `.dzch`
//! file, a pasted link) ends here.
//!
//! A join is an async flow on the runtime: it asks the hub, asks the player through the
//! [`Bus`] (a password, "mods could not be read, go on?"), and comes back to the UI
//! thread with `bus.run` to show the mods dialog ([`Connect::request`]) or start a mod
//! operation. It starts from a [`Snapshot`] of what it needs from the other stores.

use dz_api::SocketAddrV4;
use dz_hub::Hub;
use dz_hub::features::browser::ServerRow;
use dz_hub::features::dzch_cli::CliArgs;
use dz_hub::features::steamcmd::ModOpType;
use rustc_hash::FxHashMap;

use super::mods::{Mods, OpArgs};
use super::profile::{Favorite, Profile};
use super::{Bus, ConfirmRequest};
use crate::i18n::{t, tf};
use crate::kit::toast::ToastTone;
use crate::rt;
use crate::views::ViewId;

/// The port players join on when none is given.
pub const DEFAULT_GAME_PORT: u16 = 2302;

/// Split `host:port` (or `[v6]:port`); the port is `None` when absent or invalid.
pub fn split_host_port(raw: &str) -> (String, Option<u16>) {
    let s = raw.trim();
    let port_of = |p: &str| -> Option<u16> {
        if p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse::<u16>().ok().filter(|n| *n >= 1)
    };
    if let Some(rest) = s.strip_prefix('[')
        && let Some((host, after)) = rest.split_once(']')
    {
        let port = after.strip_prefix(':').and_then(port_of);
        if after.is_empty() || port.is_some() {
            return (host.to_string(), port);
        }
    }
    if let Some(i) = s.find(':')
        && s.rfind(':') == Some(i)
        && let Some(port) = s.get(i + 1..).and_then(port_of)
    {
        return (s.get(..i).unwrap_or("").to_string(), Some(port));
    }
    (s.to_string(), None)
}

/// A pasted address: an IPv4 or a hostname, then a port (`^(ipv4|host):(\d{2,5})$`).
pub fn pasted_address(text: &str) -> Option<(String, u16)> {
    let (host, port) = text.trim().rsplit_once(':')?;
    if !(2..=5).contains(&port.len()) || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port: u16 = port.parse().ok().filter(|p| *p > 0)?;
    let ipv4 = {
        let parts: Vec<&str> = host.split('.').collect();
        parts.len() == 4
            && parts
                .iter()
                .all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
    };
    let label_ok =
        |l: &str| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let hostname = host.contains('.') && host.split('.').all(label_ok);
    (ipv4 || hostname).then(|| (host.to_string(), port))
}

/// A server as a `dzch://` link (a saved password stays out of it).
pub fn dzch_link(ip: &str, game_port: u16, query_port: Option<u16>, name: Option<&str>) -> String {
    let mut p = Vec::new();
    if let Some(q) = query_port.filter(|q| *q != game_port) {
        p.push(format!("qport={q}"));
    }
    if let Some(n) = name.filter(|n| !n.is_empty()) {
        p.push(format!("name={}", encode_component(n)));
    }
    let q = if p.is_empty() {
        String::new()
    } else {
        format!("?{}", p.join("&"))
    };
    format!("dzch://{ip}:{game_port}{q}")
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

/// A mod of the server, as the mods dialog shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectMod {
    pub id: u64,
    pub name: String,
    pub local_updated: Option<i64>,
    pub remote_updated: Option<i64>,
    pub size_human: Option<String>,
}

/// `Missing`: mods must be fetched to join. `Update`: they are here, maybe stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Missing,
    Update,
}

/// Straight to an address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub ip: String,
    pub port: u16,
    pub password: Option<String>,
    pub extra_args: Option<Vec<String>>,
}

/// What a join ends with, once the mods are dealt with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    Listed {
        /// Its query address.
        addr: SocketAddrV4,
        name: String,
        password: Option<String>,
    },
    Address(Target),
}

/// The mod operation to run first, when the player says so.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetch {
    pub kind: ModOpType,
    pub args: OpArgs,
}

/// The mods dialog: shown until the player answers ([`Connect::answer`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectRequest {
    pub server_name: String,
    pub kind: RequestKind,
    pub mods: Vec<ConnectMod>,
    pub fetch: Fetch,
    pub launch: Launch,
}

/// Direct Connect's form, filled by "open in Direct Connect".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefill {
    pub ip: String,
    pub port: u16,
    pub query_port: Option<u16>,
    pub password: Option<String>,
}

/// What a join needs of the other stores, taken when it starts.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub initialized: bool,
    pub favorites: Vec<Favorite>,
    /// The last server played, if any.
    pub last: Option<(String, u16)>,
    /// Installed mods: id → (name, local_updated, remote_updated, size).
    pub mods: FxHashMap<u64, ConnectMod>,
    pub via_steam: bool,
}

impl Snapshot {
    pub fn of(initialized: bool, profile: &Profile, mods: &Mods) -> Self {
        Self {
            initialized,
            favorites: profile.favorites.clone(),
            last: profile.history.first().map(|h| (h.ip.clone(), h.port)),
            mods: mods
                .installed
                .iter()
                .map(|m| {
                    (
                        m.id,
                        ConnectMod {
                            id: m.id,
                            name: m.name.clone(),
                            local_updated: Some(m.local_updated),
                            remote_updated: m.remote_updated,
                            size_human: Some(m.size_human.clone()),
                        },
                    )
                })
                .collect(),
            via_steam: profile.via_steam(),
        }
    }
}

pub struct Connect {
    hub: Hub,
    bus: Bus,
    pub request: Option<ConnectRequest>,
    pub prefill: Option<Prefill>,
    pending_cli: Option<CliArgs>,
}

impl Connect {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        Self {
            hub,
            bus,
            request: None,
            prefill: None,
            pending_cli: None,
        }
    }

    /// A server from the list.
    pub fn server(&self, s: ServerRow, known: Option<String>, snap: Snapshot) {
        let (hub, bus) = (self.hub.clone(), self.bus.clone());
        rt::fire(async move { flow_server(hub, bus, s, known, snap).await });
    }

    /// An address from a favourite or the history: the list's entry if there is one.
    pub fn address(&self, ip: String, port: u16, password: Option<String>, snap: Snapshot) {
        let (hub, bus) = (self.hub.clone(), self.bus.clone());
        rt::fire(async move { flow_address(hub, bus, ip, port, password, snap).await });
    }

    /// Straight to an address (Direct Connect, the command line).
    pub fn direct(&self, t: Target, snap: Snapshot) {
        let (hub, bus) = (self.hub.clone(), self.bus.clone());
        rt::fire(async move { flow_direct(hub, bus, t, snap).await });
    }

    /// The server played last.
    pub fn rejoin(&self, snap: Snapshot) {
        match snap.last.clone() {
            Some((ip, port)) => self.address(ip, port, None, snap),
            None => self.bus.warn(t("shell", "noHistory")),
        }
    }

    /// The player answered the mods dialog: fetch them first, or go as they are.
    /// Returns the operation to start, if any; the caller hands it to the mods store.
    pub fn answer(&mut self, fetch: bool) -> Option<(Fetch, Launch)> {
        let r = self.request.take()?;
        if fetch {
            Some((r.fetch, r.launch))
        } else {
            self.launch(r.launch);
            None
        }
    }

    /// Launch, now that the mods are dealt with.
    pub fn launch(&self, l: Launch) {
        let (hub, bus) = (self.hub.clone(), self.bus.clone());
        rt::fire(async move { do_launch(&hub, &bus, l).await });
    }

    /// Open a server in Direct Connect, to query it or join with a password.
    pub fn open_in_direct(
        &mut self,
        ip: String,
        port: u16,
        query_port: Option<u16>,
        password: Option<String>,
    ) {
        self.prefill = Some(Prefill {
            ip,
            port,
            query_port,
            password,
        });
        self.bus.go(ViewId::Connect);
    }

    /// The command line, a second launch, a `dzch://` link.
    pub fn cli(&mut self, args: CliArgs, snap: Snapshot) {
        if !snap.initialized {
            self.pending_cli = Some(args);
            return;
        }
        if let Some(open) = args.open {
            self.open_dzch(open, snap);
        } else if let Some(c) = args.connect {
            let (host, port) = split_host_port(&c);
            self.bus.go(ViewId::Connect);
            self.direct(
                Target {
                    ip: host,
                    port: port.unwrap_or(DEFAULT_GAME_PORT),
                    password: None,
                    extra_args: None,
                },
                snap,
            );
        } else if args.reconnect {
            if snap.last.is_some() {
                self.rejoin(snap);
            } else {
                self.bus.warn(t("connect", "cliNoHistory"));
            }
        }
    }

    /// Arguments that arrived before the app was ready.
    pub fn flush_cli(&mut self, snap: Snapshot) {
        if snap.initialized
            && let Some(a) = self.pending_cli.take()
        {
            self.cli(a, snap);
        }
    }

    /// A `dzch://` link or a `.dzch` file path.
    pub fn open_dzch(&self, raw: String, snap: Snapshot) {
        let (hub, bus) = (self.hub.clone(), self.bus.clone());
        rt::fire(async move {
            let c = if raw.starts_with("dzch://") {
                hub.parse_dzch_url(raw).await
            } else {
                hub.read_dzch_file(raw).await
            };
            match c {
                Ok(c) => {
                    bus.go(ViewId::Connect);
                    let t = Target {
                        ip: c.ip,
                        port: c.port,
                        password: c.password,
                        extra_args: None,
                    };
                    flow_direct(hub, bus, t, snap).await;
                }
                Err(e) => bus.err(tf("connect", "cliDzchOpenFailed", &[("error", &e)])),
            }
        });
    }

    /// A server as a `dzch://` link on the clipboard, to send to a friend.
    pub fn copy_link(
        &self,
        ctx: &egui::Context,
        ip: &str,
        game_port: u16,
        query_port: Option<u16>,
        name: Option<&str>,
    ) {
        ctx.copy_text(dzch_link(ip, game_port, query_port, name));
        self.bus.ok(t("connect", "linkCopied"));
    }
}

/// The password to join with: the one given, else the favourite's, else asked for.
/// `Ok(None)` for none needed is not a case here (callers ask only for locked
/// servers); `Err(())` when the player gives up.
async fn password_for(
    bus: &Bus,
    snap: &Snapshot,
    name: &str,
    ip: &str,
    ports: &[u16],
    known: Option<String>,
) -> Result<String, ()> {
    if let Some(k) = known.filter(|k| !k.is_empty()) {
        return Ok(k);
    }
    let fav = snap
        .favorites
        .iter()
        .find(|f| f.ip == ip && ports.contains(&f.port) && f.password.is_some());
    if let Some(p) = fav.and_then(|f| f.password.clone()) {
        return Ok(p);
    }
    let a = bus.password(name.to_string()).await.ok_or(())?;
    if a.save {
        let (name, ip, pw) = (name.to_string(), ip.to_string(), a.password.clone());
        let port = ports
            .first()
            .copied()
            .filter(|p| *p != 0)
            .unwrap_or(DEFAULT_GAME_PORT);
        bus.run(move |cx| cx.stores.profile.add_favorite(&name, &ip, port, Some(pw)));
    }
    Ok(a.password)
}

/// The server's mods, by its query address; `None` when they could not be read and
/// the player said stop.
async fn mods_of(
    hub: &Hub,
    bus: &Bus,
    addr: SocketAddrV4,
    count: usize,
    name: &str,
) -> Option<Vec<(u64, String)>> {
    if count == 0 {
        return Some(Vec::new());
    }
    match hub.get_server_details(addr).await {
        Ok(d) => Some(
            d.mods
                .into_iter()
                .filter_map(|m| {
                    u64::try_from(m.steam_workshop_id)
                        .ok()
                        .map(|id| (id, m.name))
                })
                .collect(),
        ),
        Err(e) => {
            let req = ConfirmRequest::new(
                t("connect", "modsUnknownTitle"),
                tf(
                    "connect",
                    "modsUnknownMessage",
                    &[("name", &name), ("error", &e)],
                ),
            )
            .confirm_label(t("connect", "modsUnknownGo"));
            bus.confirm(req).await.then(Vec::new)
        }
    }
}

/// Which of the server's mods are missing, and which are here.
fn split(server_mods: &[(u64, String)], snap: &Snapshot) -> (Vec<ConnectMod>, Vec<ConnectMod>) {
    let mut missing = Vec::new();
    let mut present = Vec::new();
    for (id, name) in server_mods {
        match snap.mods.get(id) {
            Some(m) => present.push(m.clone()),
            None => missing.push(ConnectMod {
                id: *id,
                name: if name.is_empty() {
                    format!("Workshop {id}")
                } else {
                    name.clone()
                },
                local_updated: None,
                remote_updated: None,
                size_human: None,
            }),
        }
    }
    (missing, present)
}

fn show(bus: &Bus, r: ConnectRequest) {
    bus.run(move |cx| cx.stores.connect.request = Some(r));
}

async fn flow_server(hub: Hub, bus: Bus, s: ServerRow, known: Option<String>, snap: Snapshot) {
    let mut password = None;
    let addr = SocketAddrV4::new(s.ip, s.query_port);
    if s.password {
        match password_for(
            &bus,
            &snap,
            &s.name,
            &s.ip.to_string(),
            &[s.query_port, s.game_port],
            known,
        )
        .await
        {
            Ok(p) => password = Some(p),
            Err(()) => return,
        }
    }
    let Some(server_mods) = mods_of(&hub, &bus, addr, s.mods_count, &s.name).await else {
        return;
    };
    let launch = Launch::Listed {
        addr,
        name: s.name.clone(),
        password,
    };
    let (missing, present) = split(&server_mods, &snap);
    let args = OpArgs {
        ip: Some(s.ip.to_string()),
        port: Some(i64::from(s.query_port)),
        ..OpArgs::default()
    };
    if !missing.is_empty() {
        show(
            &bus,
            ConnectRequest {
                server_name: s.name.clone(),
                kind: RequestKind::Missing,
                mods: missing,
                fetch: Fetch {
                    kind: ModOpType::InstallServer,
                    args,
                },
                launch,
            },
        );
    } else if !server_mods.is_empty() {
        show(
            &bus,
            ConnectRequest {
                server_name: s.name.clone(),
                kind: RequestKind::Update,
                mods: present,
                fetch: Fetch {
                    kind: ModOpType::UpdateServer,
                    args,
                },
                launch,
            },
        );
    } else {
        do_launch(&hub, &bus, launch).await;
    }
}

async fn flow_address(
    hub: Hub,
    bus: Bus,
    ip: String,
    port: u16,
    password: Option<String>,
    snap: Snapshot,
) {
    let row = super::servers::lookup_one(&hub, &ip, i64::from(port)).await;
    match row {
        Some(s) => flow_server(hub, bus, s, password, snap).await,
        None => {
            let t = Target {
                ip,
                port,
                password,
                extra_args: None,
            };
            flow_direct(hub, bus, t, snap).await;
        }
    }
}

async fn flow_direct(hub: Hub, bus: Bus, mut t: Target, snap: Snapshot) {
    let listed = super::servers::lookup_one(&hub, &t.ip, i64::from(t.port)).await;
    let name = listed
        .as_ref()
        .map_or_else(|| format!("{}:{}", t.ip, t.port), |l| l.name.clone());
    if let Some(l) = &listed
        && l.password
    {
        match password_for(
            &bus,
            &snap,
            &name,
            &t.ip,
            &[l.query_port, l.game_port],
            t.password.clone(),
        )
        .await
        {
            Ok(p) => t.password = Some(p),
            Err(()) => return,
        }
    }
    let server_mods = match &listed {
        Some(l) => match mods_of(
            &hub,
            &bus,
            SocketAddrV4::new(l.ip, l.query_port),
            l.mods_count,
            &name,
        )
        .await
        {
            Some(m) => m,
            None => return,
        },
        None => Vec::new(),
    };
    if server_mods.is_empty() {
        do_launch(&hub, &bus, Launch::Address(t)).await;
        return;
    }
    let (missing, present) = split(&server_mods, &snap);
    let ids = |list: &[(u64, String)]| OpArgs {
        mod_ids: Some(list.iter().map(|m| m.0).collect()),
        mod_names: Some(list.iter().map(|m| m.1.clone()).collect()),
        ..OpArgs::default()
    };
    if !missing.is_empty() {
        let list: Vec<(u64, String)> = missing.iter().map(|m| (m.id, m.name.clone())).collect();
        show(
            &bus,
            ConnectRequest {
                server_name: name,
                kind: RequestKind::Missing,
                mods: missing,
                fetch: Fetch {
                    kind: ModOpType::UpdateSelected,
                    args: ids(&list),
                },
                launch: Launch::Address(t),
            },
        );
    } else {
        let all: Vec<(u64, String)> = server_mods
            .iter()
            .map(|(id, n)| {
                (
                    *id,
                    if n.is_empty() {
                        id.to_string()
                    } else {
                        n.clone()
                    },
                )
            })
            .collect();
        show(
            &bus,
            ConnectRequest {
                server_name: name,
                kind: RequestKind::Update,
                mods: present,
                fetch: Fetch {
                    kind: ModOpType::UpdateSelected,
                    args: ids(&all),
                },
                launch: Launch::Address(t),
            },
        );
    }
}

async fn do_launch(hub: &Hub, bus: &Bus, l: Launch) {
    match l {
        Launch::Listed {
            addr,
            name,
            password,
        } => {
            bus.step(
                "launch",
                tf("connect", "connectLaunchingServer", &[("name", &name)]),
                ToastTone::Neutral,
            );
            let _ = hub.setup_mod_symlinks(addr).await;
            match hub.launch_server(addr, password).await {
                Ok(()) => bus.step(
                    "launch",
                    t("connect", "connectWaitingSteam"),
                    ToastTone::Neutral,
                ),
                Err(e) => {
                    bus.step(
                        "launch",
                        tf("connect", "connectLaunchFailed", &[("error", &e)]),
                        ToastTone::Err,
                    );
                    crate::platform::life::note_launch_failed(bus.ctx(), &e);
                }
            }
        }
        Launch::Address(t) => {
            let address = format!("{}:{}", t.ip, t.port);
            bus.step(
                "launch",
                tf(
                    "connect",
                    "connectLaunchingAddress",
                    &[("address", &address)],
                ),
                ToastTone::Neutral,
            );
            match hub
                .launch_direct(t.ip, t.port, t.password, t.extra_args)
                .await
            {
                Ok(()) => bus.step(
                    "launch",
                    crate::i18n::t("connect", "connectWaitingSteam"),
                    ToastTone::Neutral,
                ),
                Err(e) => {
                    bus.step(
                        "launch",
                        tf("connect", "connectLaunchFailed", &[("error", &e)]),
                        ToastTone::Err,
                    );
                    crate::platform::life::note_launch_failed(bus.ctx(), &e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_host_and_port() {
        assert_eq!(
            split_host_port("1.2.3.4:2402"),
            ("1.2.3.4".into(), Some(2402))
        );
        assert_eq!(split_host_port(" host "), ("host".into(), None));
        assert_eq!(split_host_port("[::1]:2302"), ("::1".into(), Some(2302)));
        assert_eq!(split_host_port("a:b:c"), ("a:b:c".into(), None));
        assert_eq!(split_host_port("h:0"), ("h:0".into(), None));
    }

    #[test]
    fn recognises_pasted_addresses() {
        assert_eq!(
            pasted_address("1.2.3.4:2302"),
            Some(("1.2.3.4".into(), 2302))
        );
        assert_eq!(
            pasted_address("play.example.com:2402"),
            Some(("play.example.com".into(), 2402))
        );
        assert_eq!(pasted_address("hello"), None);
        assert_eq!(pasted_address("localhost:2302"), None);
        assert_eq!(pasted_address("1.2.3.4:9"), None);
    }

    #[test]
    fn writes_links() {
        assert_eq!(
            dzch_link("1.2.3.4", 2302, Some(2302), None),
            "dzch://1.2.3.4:2302"
        );
        assert_eq!(
            dzch_link("1.2.3.4", 2302, Some(27016), Some("A B")),
            "dzch://1.2.3.4:2302?qport=27016&name=A%20B"
        );
    }
}
