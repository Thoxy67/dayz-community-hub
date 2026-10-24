//! A list of saved servers (= `features/favorites/SavedServers.svelte`), shared by the
//! favourites and the history: the toolbar (search, "in the list only", the count), the
//! header's tools (shortcuts, the details toggle), the list with the browser's columns and
//! row, and the shared server panel beside it.
//!
//! A saved list is a few dozen servers, so it is filtered, sorted and counted here, each
//! frame, from the profile (optimistic: a removal shows at once) and the hub's snapshot
//! (each saved server is looked up by address; the listed ones show their live state).
//! Only the rows on screen are drawn; those that never answered a ping are pinged 200 ms
//! after the rows on screen settle.
//!
//! The view's choices live in one [`State`] per list in egui's memory, so its keys, the
//! palette, the pad and the control channel change the same thing as the buttons.

pub mod cmds;

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use dz_api::SocketAddrV4;

use egui::mutex::Mutex;
use egui::{CornerRadius, Id, Sense, Ui, UiBuilder, Vec2};
use rustc_hash::{FxHashMap, FxHasher};
use serde_json::{Value, json};

use crate::app::Ctx;
use crate::commands::{self, Scope};
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::{Button, IconButton, Input, Side, TipExt, Tone, Variant, VirtualList};
use crate::parts::{
    Empty, Host, ListColumns, MasterDetail, PageHeader, RowAction, RowButton, RowCache, RowData,
    RowExtra, RowLabel, RowMenuItem, RowTag, ServerListRow, ShortcutsButton, SortHead, TableHead,
    draw, head_cell, ph,
};
use crate::store::servers::Live;
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::ViewId;
use crate::views::servers::detail;

pub use crate::views::servers::{Run, run_all};

const HEAD_H: f32 = 28.0;
const OVERSCAN: usize = 1;
/// Rows on screen without a ping are pinged once they have stayed this long (s).
const PING_SETTLE: f64 = 0.2;
/// A ping at or past this is a timeout (sorted last, left out of the figures).
const PING_TIMEOUT: u32 = 5000;

/// Which saved list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Favorites,
    History,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Favorites => "favorites",
            Kind::History => "history",
        }
    }

    pub fn view(self) -> ViewId {
        match self {
            Kind::Favorites => ViewId::Favorites,
            Kind::History => ViewId::History,
        }
    }

    /// The list a view shows, for the `list.*` commands.
    pub fn of_view(v: ViewId) -> Option<Kind> {
        match v {
            ViewId::Favorites => Some(Kind::Favorites),
            ViewId::History => Some(Kind::History),
            _ => None,
        }
    }

    /// Its commands' ids.
    fn cmd(self, what: Cmd) -> &'static str {
        match (self, what) {
            (Kind::Favorites, Cmd::Detail) => "favorites.toggle_detail",
            (Kind::Favorites, Cmd::Listed) => "favorites.toggle_listed_only",
            (Kind::Favorites, Cmd::Search) => "favorites.search",
            (Kind::Favorites, Cmd::Sort) => "favorites.sort_by",
            (Kind::Favorites, Cmd::Remove) => "favorites.remove",
            (Kind::Favorites, Cmd::Select) => "favorites.select",
            (Kind::History, Cmd::Detail) => "history.toggle_detail",
            (Kind::History, Cmd::Listed) => "history.toggle_listed_only",
            (Kind::History, Cmd::Search) => "history.search",
            (Kind::History, Cmd::Sort) => "history.sort_by",
            (Kind::History, Cmd::Remove) => "history.remove",
            (Kind::History, Cmd::Select) => "history.select",
        }
    }
}

#[derive(Clone, Copy)]
enum Cmd {
    Detail,
    Listed,
    Search,
    Sort,
    Remove,
    Select,
}

/// The list's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortCol {
    Name,
    Players,
    Ping,
    Map,
    Time,
    Mods,
    /// History: when last played.
    Recent,
}

impl SortCol {
    pub fn id(self) -> &'static str {
        match self {
            SortCol::Name => "name",
            SortCol::Players => "players",
            SortCol::Ping => "ping",
            SortCol::Map => "map",
            SortCol::Time => "time",
            SortCol::Mods => "mods",
            SortCol::Recent => "recent",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Some(match s {
            "name" => SortCol::Name,
            "players" => SortCol::Players,
            "ping" => SortCol::Ping,
            "map" => SortCol::Map,
            "time" => SortCol::Time,
            "mods" => SortCol::Mods,
            "recent" => SortCol::Recent,
            _ => return None,
        })
    }

    /// A new column starts ascending for words and pings, descending for counts.
    fn first_asc(self) -> bool {
        matches!(
            self,
            SortCol::Name | SortCol::Map | SortCol::Time | SortCol::Ping
        )
    }
}

/// A saved server: a favourite or a history entry.
#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub ip: String,
    /// The port it was saved with (joining goes through it).
    pub port: u16,
    pub name: String,
    /// Favourites: the join password kept with it.
    pub password: Option<String>,
    /// History: when it was last joined (Unix seconds).
    pub ts: Option<i64>,
    /// Favourites: the player's note, labels and group.
    pub note: String,
    pub labels: Vec<String>,
    pub group: Option<String>,
}

impl Entry {
    pub fn key(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }

    /// The arguments the `list.*` commands take for it.
    pub fn args(&self, game_port: i64) -> Value {
        json!({
            "ip": self.ip,
            "port": self.port,
            "gamePort": game_port,
            "name": self.name,
        })
    }
}

/// The entries of a list, from the profile (already changed by a removal).
pub fn entries(cx: &Ctx, kind: Kind) -> Vec<Entry> {
    let p = &cx.stores.profile;
    match kind {
        Kind::Favorites => p
            .favorites
            .iter()
            .map(|f| Entry {
                ip: f.ip.clone(),
                port: f.port,
                name: f.name.clone(),
                password: f.password.clone(),
                ts: None,
                note: f.note.clone(),
                labels: f.labels.clone(),
                group: f.group.clone(),
            })
            .collect(),
        Kind::History => p
            .history
            .iter()
            .map(|h| Entry {
                ip: h.ip.clone(),
                port: h.port,
                name: h.name.clone(),
                ts: Some(h.ts),
                ..Entry::default()
            })
            .collect(),
    }
}

/// An entry as the list sees it this frame.
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub key: String,
    /// What pings know it by: the listed query port, else the saved port.
    pub ping_key: String,
    /// The same address as numbers, what the list and friends are asked by each frame;
    /// `None` for one saved as a host name (never listed).
    pub addr: Option<SocketAddrV4>,
    pub listed: bool,
    pub game_port: i64,
    pub live: Live,
    /// `None` never pinged; at or past 5 s a timeout.
    pub ping: Option<u32>,
    pub map: String,
    pub time: String,
    pub mods: i64,
}

impl Line {
    fn ping_rank(&self) -> u32 {
        self.ping.filter(|p| *p < PING_TIMEOUT).unwrap_or(u32::MAX)
    }
}

/// Every entry looked up in the list (and in the answers of direct queries).
pub fn lines(cx: &mut Ctx, entries: &[Entry]) -> Vec<Line> {
    let s = &mut *cx.stores;
    entries
        .iter()
        .map(|e| {
            let port = i64::from(e.port);
            let key = e.key();
            let ping = s.servers.ping_ms(&e.ip, port);
            let saved = dz_api::addr(&e.ip, port);
            match saved.and_then(|a| s.servers.row(a)) {
                Some(r) => Line {
                    ping_key: r.addr().to_string(),
                    addr: Some(r.addr()),
                    key,
                    listed: true,
                    game_port: i64::from(r.game_port()),
                    live: Live {
                        players: r.players(),
                        max: r.max_players(),
                        bots: r.bots(),
                    },
                    ping: r.ping_ms().or(ping),
                    map: r.map.to_string(),
                    time: r.server.time.clone(),
                    mods: i64::try_from(r.mods_count()).unwrap_or(i64::MAX),
                },
                None => Line {
                    ping_key: key.clone(),
                    addr: saved,
                    key,
                    listed: false,
                    game_port: port,
                    live: s.server_data.players(&s.servers, &e.ip, port),
                    ping,
                    map: String::new(),
                    time: String::new(),
                    mods: -1,
                },
            }
        })
        .collect()
}

/// The figures a view header shows over the list.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub total: usize,
    pub listed: usize,
    pub players: i64,
    pub best_ping: Option<u32>,
    pub avg_ping: Option<u32>,
    pub last_ts: Option<i64>,
    pub this_week: usize,
}

pub fn stats(entries: &[Entry], lines: &[Line], now_s: i64) -> Stats {
    let mut s = Stats {
        total: lines.len(),
        ..Stats::default()
    };
    let mut sum: u64 = 0;
    let mut n: u64 = 0;
    let week_ago = now_s.saturating_sub(7 * 86_400);
    for (e, l) in entries.iter().zip(lines) {
        if l.listed {
            s.listed += 1;
            s.players = s.players.saturating_add(l.live.players.max(0));
        }
        if let Some(p) = l.ping.filter(|p| *p < PING_TIMEOUT) {
            s.best_ping = Some(s.best_ping.map_or(p, |b| b.min(p)));
            sum += u64::from(p);
            n += 1;
        }
        if let Some(ts) = e.ts {
            s.last_ts = Some(s.last_ts.map_or(ts, |l| l.max(ts)));
            if ts >= week_ago {
                s.this_week += 1;
            }
        }
    }
    s.avg_ping = (sum + n / 2)
        .checked_div(n)
        .and_then(|v| u32::try_from(v).ok());
    s
}

/// What the list remembers for the life of the window.
pub struct State {
    pub query: String,
    pub listed_only: bool,
    pub sort: SortCol,
    pub asc: bool,
    /// The key (`ip:saved port`) of the server picked.
    pub sel: Option<String>,
    pub show_detail: bool,
    /// Bring this row into view on the next frame.
    pub scroll_to: Option<usize>,
    /// Rows a screenful holds (PgUp/PgDn), from the last frame.
    pub page_rows: usize,
    /// The keys of the rows shown last frame, in order (the commands walk them).
    pub shown: Vec<String>,
    /// Where each of [`State::shown`] is among the list's rows (group headings
    /// come between them).
    pub row_of_shown: Vec<usize>,
    /// Favourites: the "other favourites" section is folded (for the life of the
    /// window; the player's groups keep theirs in the profile).
    pub ungrouped_collapsed: bool,
    /// Focus the search on the next frame.
    pub focus_search: bool,
    /// History: the rail's Rejoin card asked for the last server played; done on the
    /// first frame that has entries.
    pub focus_last: bool,
    /// The rows on screen: what pings know them by, when they came, and whether asked.
    on_screen: Vec<String>,
    on_screen_hash: u64,
    on_screen_since: f64,
    pinged_hash: u64,
    /// The list as last made, and the key it was made under.
    listing: Option<(u64, Arc<Listing>)>,
    /// History: each join's "how long ago" and date, by its time, with the second and
    /// the language they were written in (written again when either moves).
    times: FxHashMap<i64, (u64, String, String)>,
    /// Favourites: the row menu's "Move to {group}" words, with the key (the groups'
    /// names and the language) they were written under.
    move_words: (u64, Vec<String>),
}

impl State {
    fn new(kind: Kind) -> Self {
        let history = kind == Kind::History;
        Self {
            query: String::new(),
            listed_only: false,
            sort: if history {
                SortCol::Recent
            } else {
                SortCol::Name
            },
            asc: !history,
            sel: None,
            show_detail: true,
            scroll_to: None,
            page_rows: 10,
            shown: Vec::new(),
            row_of_shown: Vec::new(),
            ungrouped_collapsed: false,
            focus_search: false,
            focus_last: false,
            on_screen: Vec::new(),
            on_screen_hash: 0,
            on_screen_since: 0.0,
            pinged_hash: 0,
            listing: None,
            times: FxHashMap::default(),
            move_words: (0, Vec::new()),
        }
    }

    /// The rail's Rejoin card: search and filter cleared, newest first, the newest entry
    /// picked with its details open. `false` while there is nothing to pick.
    pub fn open_last(&mut self, entries: &[Entry]) -> bool {
        let Some(newest) = entries.iter().max_by_key(|e| e.ts.unwrap_or(0)) else {
            return false;
        };
        self.query.clear();
        self.listed_only = false;
        self.sort = SortCol::Recent;
        self.asc = false;
        self.sel = Some(newest.key());
        self.show_detail = true;
        self.scroll_to = Some(0);
        true
    }

    /// A click on a heading: the other way round, or a new column its own way.
    pub fn sort_by(&mut self, col: SortCol) {
        if self.sort == col {
            self.asc = !self.asc;
        } else {
            self.sort = col;
            self.asc = col.first_asc();
        }
    }
}

pub type Shared = Arc<Mutex<State>>;

pub fn shared(ctx: &egui::Context, kind: Kind) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new(("dzch.saved", kind.id())), || {
            Arc::new(Mutex::new(State::new(kind)))
        })
        .clone()
    })
}

/// The rows to show: those matching the search (and listed, if asked), in order.
/// Indices into `entries` / `lines`. Each word is lowercased once, not in the comparator.
pub fn shown(entries: &[Entry], lines: &[Line], st: &State) -> Vec<usize> {
    let q = st.query.trim().to_lowercase();
    let n = entries.len().min(lines.len());
    let by_name = st.sort == SortCol::Name;
    let names: Vec<String> = if q.is_empty() && !by_name {
        Vec::new()
    } else {
        entries
            .iter()
            .take(n)
            .map(|e| e.name.to_lowercase())
            .collect()
    };
    // The note and the labels, searched too (favourites).
    let marks: Vec<String> = if q.is_empty() {
        Vec::new()
    } else {
        entries
            .iter()
            .take(n)
            .map(|e| {
                let mut s = e.note.to_lowercase();
                for l in &e.labels {
                    s.push('\n');
                    s.push_str(&l.to_lowercase());
                }
                s
            })
            .collect()
    };
    let maps: Vec<String> = if q.is_empty() {
        Vec::new()
    } else {
        lines
            .iter()
            .take(n)
            .map(|l| {
                if l.listed {
                    l.map.to_lowercase()
                } else {
                    String::new()
                }
            })
            .collect()
    };
    let mut out: Vec<usize> = (0..n)
        .filter(|&i| {
            let Some(l) = lines.get(i) else {
                return false;
            };
            (!st.listed_only || l.listed)
                && (q.is_empty()
                    || names.get(i).is_some_and(|n| n.contains(&q))
                    || l.key.contains(&q)
                    || marks.get(i).is_some_and(|m| m.contains(&q))
                    || (l.listed && maps.get(i).is_some_and(|m| m.contains(&q))))
        })
        .collect();
    // Unlisted servers have no map, clock or mods: they go after the others.
    let text = |l: &Line, s: &str| {
        if l.listed {
            s.to_lowercase()
        } else {
            "~".to_string()
        }
    };
    let words: Vec<String> = match st.sort {
        SortCol::Map => lines.iter().take(n).map(|l| text(l, &l.map)).collect(),
        SortCol::Time => lines.iter().take(n).map(|l| text(l, &l.time)).collect(),
        _ => Vec::new(),
    };
    let cmp = |a: usize, b: usize| -> std::cmp::Ordering {
        let (Some(ea), Some(eb), Some(la), Some(lb)) =
            (entries.get(a), entries.get(b), lines.get(a), lines.get(b))
        else {
            return std::cmp::Ordering::Equal;
        };
        match st.sort {
            SortCol::Name => names
                .get(a)
                .cmp(&names.get(b))
                .then_with(|| ea.name.cmp(&eb.name)),
            SortCol::Players => la.live.players.cmp(&lb.live.players),
            SortCol::Ping => la.ping_rank().cmp(&lb.ping_rank()),
            SortCol::Map | SortCol::Time => words.get(a).cmp(&words.get(b)),
            SortCol::Mods => la.mods.cmp(&lb.mods),
            SortCol::Recent => ea.ts.unwrap_or(0).cmp(&eb.ts.unwrap_or(0)),
        }
    };
    out.sort_by(|&a, &b| {
        let o = cmp(a, b);
        if st.asc { o } else { o.reverse() }
    });
    out
}

/// One row of the list: a group's heading or an entry (an index into the entries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Head(usize),
    Entry(usize),
}

/// A group's heading: its name (`None` for the favourites in no group), how many of its
/// favourites match, whether it is folded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub name: Option<String>,
    pub count: usize,
    pub collapsed: bool,
}

/// The rows to draw: without groups, the order as it is; with them, each group's heading
/// then its entries (in the order), in the groups' order, then the favourites in no group
/// (or in one that no longer exists) under their own heading. While a search or "in the
/// list only" filters, a group with nothing matching is left out; while searching, a
/// folded group shows what matches.
pub fn group_rows(
    entries: &[Entry],
    order: &[usize],
    groups: &[dz_profile::FavoriteGroup],
    filtering: bool,
    searching: bool,
    ungrouped_collapsed: bool,
) -> (Vec<Row>, Vec<Head>) {
    if groups.is_empty() {
        return (order.iter().map(|&i| Row::Entry(i)).collect(), Vec::new());
    }
    let group_of = |i: usize| entries.get(i).and_then(|e| e.group.as_deref());
    let known = |g: Option<&str>| g.is_some_and(|g| groups.iter().any(|x| x.name == g));
    let mut rows = Vec::with_capacity(order.len() + groups.len() + 1);
    let mut heads = Vec::new();
    let mut section = |name: Option<String>, collapsed: bool, members: Vec<usize>| {
        if members.is_empty() && (filtering || name.is_none()) {
            return;
        }
        rows.push(Row::Head(heads.len()));
        heads.push(Head {
            name,
            count: members.len(),
            collapsed,
        });
        if !collapsed || searching {
            rows.extend(members.into_iter().map(Row::Entry));
        }
    };
    // A hand-edited or imported profile may name two groups alike: the first one
    // holds them, so no favourite is listed twice.
    let mut seen: Vec<&str> = Vec::with_capacity(groups.len());
    for g in groups {
        if seen.contains(&g.name.as_str()) {
            continue;
        }
        seen.push(g.name.as_str());
        let members = order
            .iter()
            .copied()
            .filter(|&i| group_of(i) == Some(g.name.as_str()))
            .collect();
        section(Some(g.name.clone()), g.collapsed, members);
    }
    let rest = order
        .iter()
        .copied()
        .filter(|&i| !known(group_of(i)))
        .collect();
    section(None, ungrouped_collapsed, rest);
    (rows, heads)
}

/// A list as last made: its entries, how each stands, the order shown, and the rows drawn
/// (the order under the groups' headings).
#[derive(Default)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub lines: Vec<Line>,
    pub order: Vec<usize>,
    pub rows: Vec<Row>,
    pub heads: Vec<Head>,
}

/// The list for this frame: the one made before while nothing it reads has moved (the
/// list's generation, the profile's favourites and history, each entry's own state, the
/// search, the order, "in the list only"), else made again, with the keys the commands
/// walk. A ping landing on a server elsewhere in the list does not make it again.
fn listing(cx: &mut Ctx, kind: Kind, st: &mut State) -> Arc<Listing> {
    let key = {
        let mut h = FxHasher::default();
        (
            kind,
            cx.stores.servers.snapshot().generation(),
            cx.stores.profile.stamp(),
            &st.query,
            st.listed_only,
            st.sort.id(),
            st.asc,
            st.ungrouped_collapsed,
        )
            .hash(&mut h);
        entries_state(cx, kind, &mut h);
        h.finish()
    };
    if let Some((k, l)) = &st.listing
        && *k == key
    {
        return Arc::clone(l);
    }
    let entries = entries(cx, kind);
    let lines = lines(cx, &entries);
    let order = shown(&entries, &lines, st);
    let groups: &[dz_profile::FavoriteGroup] = match kind {
        Kind::Favorites => &cx.stores.profile.groups,
        Kind::History => &[],
    };
    let searching = !st.query.trim().is_empty();
    let (rows, heads) = group_rows(
        &entries,
        &order,
        groups,
        searching || st.listed_only,
        searching,
        st.ungrouped_collapsed,
    );
    st.shown.clear();
    st.row_of_shown.clear();
    for (r, row) in rows.iter().enumerate() {
        if let Row::Entry(i) = row
            && let Some(l) = lines.get(*i)
        {
            st.shown.push(l.key.clone());
            st.row_of_shown.push(r);
        }
    }
    let l = Arc::new(Listing {
        entries,
        lines,
        order,
        rows,
        heads,
    });
    st.listing = Some((key, Arc::clone(&l)));
    l
}

/// What each saved entry's line reads, hashed: a listed server's live and profile words,
/// else its ping asked on request and its own query's answer.
fn entries_state(cx: &Ctx, kind: Kind, h: &mut FxHasher) {
    let s = &cx.stores;
    let mut one = |ip: &str, port: u16| {
        let port = i64::from(port);
        match s.servers.row_of(ip, port) {
            Some(r) => r.state_bits().hash(h),
            None => {
                s.servers.ping_ms(ip, port).hash(h);
                let live = s.server_data.players(&s.servers, ip, port);
                (live.players, live.max, live.bots).hash(h);
            }
        }
    };
    match kind {
        Kind::Favorites => s.profile.favorites.iter().for_each(|f| one(&f.ip, f.port)),
        Kind::History => s.profile.history.iter().for_each(|e| one(&e.ip, e.port)),
    }
}

/// Words a list's empty state says (it has no entries at all).
pub struct EmptyWords<'a> {
    pub icon: Icon,
    pub title: &'a str,
    pub hint: &'a str,
}

/// One frame of a saved list, as `SavedServers.svelte` lays it out: the header (its
/// figures from `figures`, the tools, the toolbar), then the empty state or the list
/// beside the server panel. Runs what was asked once the frame is over.
pub fn show(
    ui: &mut Ui,
    cx: &mut Ctx,
    kind: Kind,
    cache: &mut RowCache,
    figures: impl FnOnce(&mut Ui, &Stats),
    empty: EmptyWords<'_>,
) {
    show_with_actions(ui, cx, kind, cache, figures, |_, _, _| {}, empty);
}

/// [`show`], with the list's own buttons after the header's tools (history: "Clear all").
pub fn show_with_actions(
    ui: &mut Ui,
    cx: &mut Ctx,
    kind: Kind,
    cache: &mut RowCache,
    figures: impl FnOnce(&mut Ui, &Stats),
    actions: impl FnOnce(&mut Ui, &Stats, &mut Run),
    empty: EmptyWords<'_>,
) {
    let shared = shared(ui.ctx(), kind);
    let mut run = Run::new();
    let list;
    let selected;
    {
        let mut st = shared.lock();
        let mut l = listing(cx, kind, &mut st);
        if st.focus_last && st.open_last(&l.entries) {
            st.focus_last = false;
            l = listing(cx, kind, &mut st);
        }
        list = l;
        let (entries, lines, order) = (&list.entries, &list.lines, &list.order);
        if cx.nav.take_focus_search() {
            st.focus_search = true;
        }
        keys(ui.ctx(), cx, kind, &mut st, entries, lines, &mut run);
        selected = st.sel.as_ref().and_then(|k| {
            order
                .iter()
                .copied()
                .find(|&i| lines.get(i).is_some_and(|l| l.key == *k))
        });
    }
    let (entries, lines, order) = (&list.entries, &list.lines, &list.order);
    // The panel's model and its debounced fetches, before the frame borrows.
    let model = selected
        .and_then(|i| entries.get(i))
        .map(|e| detail::prepare(cx, &e.ip, i64::from(e.port), &e.name));
    let now_s = crate::store::now_ms() / 1000;
    let st_figures = stats(entries, lines, now_s);
    {
        let mut st = shared.lock();
        let data = Frame {
            kind,
            entries,
            lines,
            order,
            rows: &list.rows,
            heads: &list.heads,
            selected,
            stats: &st_figures,
        };
        frame(
            ui,
            cx,
            &mut st,
            &data,
            cache,
            model.as_deref(),
            figures,
            actions,
            empty,
            &mut run,
        );
    }
    ping_on_screen(cx, &shared);
    run_all(cx, run);
}

/// What one frame draws from.
struct Frame<'a> {
    kind: Kind,
    entries: &'a [Entry],
    lines: &'a [Line],
    order: &'a [usize],
    rows: &'a [Row],
    heads: &'a [Head],
    selected: Option<usize>,
    stats: &'a Stats,
}

/// Pings the rows on screen that never answered, once they stayed 200 ms.
fn ping_on_screen(cx: &mut Ctx, shared: &Shared) {
    let now = cx.egui.input(|i| i.time);
    let keys = {
        let mut st = shared.lock();
        if st.on_screen_hash == st.pinged_hash || st.on_screen.is_empty() {
            return;
        }
        let age = now - st.on_screen_since;
        if age < PING_SETTLE {
            cx.egui
                .request_repaint_after(std::time::Duration::from_secs_f64(PING_SETTLE - age));
            return;
        }
        st.pinged_hash = st.on_screen_hash;
        st.on_screen.clone()
    };
    let (concurrency, timeout, _) = cx.stores.profile.ping_settings();
    cx.stores.servers.ping_visible(&keys, concurrency, timeout);
}

/// The list's keys (registry scopes `list` and `saved`), while nothing else has them.
fn keys(
    ctx: &egui::Context,
    cx: &Ctx,
    kind: Kind,
    st: &mut State,
    entries: &[Entry],
    lines: &[Line],
    run: &mut Run,
) {
    let overlay = cx.stores.dialogs.open()
        || egui::Popup::is_any_open(ctx)
        || ctx.memory(|m| m.top_modal_layer().is_some())
        || cx.stores.connect.request.is_some();
    if overlay {
        return;
    }
    let (down, page_down, end) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::PageDown),
            i.key_pressed(egui::Key::End),
        )
    });
    let focused_widget = ctx.memory(|m| m.focused().is_some());
    let Some(id) =
        commands::scoped(ctx, Scope::List).or_else(|| commands::scoped(ctx, Scope::Saved))
    else {
        return;
    };
    ctx.request_repaint();
    let page = i64::try_from(st.page_rows.saturating_sub(2).max(1)).unwrap_or(1);
    match id {
        "list.move" => cmds::move_by(st, if down { 1 } else { -1 }),
        "list.page" => cmds::move_by(st, if page_down { page } else { -page }),
        "list.edges" => cmds::move_by(st, if end { i64::MAX / 2 } else { i64::MIN / 2 }),
        "list.close" => st.sel = None,
        "list.detail" => st.show_detail = !st.show_detail,
        "list.join" if focused_widget => {}
        other => {
            // Acts on the row picked, while it is shown.
            let Some(k) = st.sel.as_ref().filter(|k| st.shown.contains(k)) else {
                return;
            };
            let Some((e, l)) = lines
                .iter()
                .position(|l| l.key == *k)
                .and_then(|i| Some((entries.get(i)?, lines.get(i)?)))
            else {
                return;
            };
            let args = e.args(l.game_port);
            if other == "saved.remove" {
                run.push((kind.cmd(Cmd::Remove), args));
            } else {
                run.push((other, args));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn frame(
    ui: &mut Ui,
    cx: &Ctx,
    st: &mut State,
    d: &Frame<'_>,
    cache: &mut RowCache,
    model: Option<&detail::model::Model>,
    figures: impl FnOnce(&mut Ui, &Stats),
    actions: impl FnOnce(&mut Ui, &Stats, &mut Run),
    empty: EmptyWords<'_>,
    run: &mut Run,
) {
    let tk = ui.tokens();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, CornerRadius::ZERO, tk.panel);
    let mut v = ui.new_child(
        UiBuilder::new()
            .max_rect(full)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    v.spacing_mut().item_spacing = Vec2::ZERO;
    header(&mut v, st, d, figures, actions, run);

    let rest = v.available_rect_before_wrap();
    let mut body = v.new_child(
        UiBuilder::new()
            .max_rect(rest)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    if d.entries.is_empty() {
        Empty::new()
            .icon(empty.icon)
            .title(empty.title)
            .text(empty.hint)
            .show(&mut body, |ui| {
                let b = Button::new(t("favorites", "browseServers")).variant(Variant::Accent);
                if ui.add(b).clicked() {
                    run.push(("view.servers", Value::Null));
                }
            });
        return;
    }
    let mut detail_run = Run::new();
    let show_detail = st.show_detail;
    MasterDetail::new(format!("{}-detail", d.kind.id()))
        .detail_shown(show_detail)
        .selected(d.selected.is_some())
        .initial(380.0)
        .range(300.0, 620.0)
        .breakpoint(980.0)
        .show(
            &mut body,
            |ui| list(ui, cx, st, d, cache, run),
            |ui| match model {
                Some(m) => detail_run = detail::panel(ui, cx, m, true),
                None => Empty::new()
                    .icon(lucide::SERVER)
                    .title(t("favorites", "selectHint"))
                    .show(ui, |_| {}),
            },
        );
    run.extend(detail_run);
}

fn header(
    ui: &mut Ui,
    st: &mut State,
    d: &Frame<'_>,
    figures: impl FnOnce(&mut Ui, &Stats),
    actions: impl FnOnce(&mut Ui, &Stats, &mut Run),
    run: &mut Run,
) {
    let sheet_kbd = commands::shortcut("sheet.toggle")
        .and_then(commands::Shortcut::key_text)
        .unwrap_or_default();
    let detail_kbd = commands::shortcut("list.detail")
        .and_then(commands::Shortcut::key_text)
        .unwrap_or_default();
    let show_detail = st.show_detail;
    let kind = d.kind;
    let run_cell = std::cell::RefCell::new(std::mem::take(run));
    let toolbar = Some(|ui: &mut Ui| {
        ui.add_space(-1.0);
        toolbar(ui, st, d, &mut run_cell.borrow_mut());
        ui.add_space(1.0);
    });
    PageHeader::new(t("nav", kind.id())).show(
        ui,
        |ui| figures(ui, d.stats),
        |ui| {
            let mut r = run_cell.borrow_mut();
            if ui.add(ShortcutsButton::new(&sheet_kbd)).clicked() {
                r.push(("sheet.toggle", Value::Null));
            }
            if ui
                .add(
                    IconButton::new(lucide::PANEL_RIGHT, t("servers", "toggleDetails"))
                        .kbd(&detail_kbd)
                        .active(show_detail)
                        .side(Side::Bottom),
                )
                .clicked()
            {
                r.push((kind.cmd(Cmd::Detail), Value::Null));
            }
            actions(ui, d.stats, &mut r);
        },
        toolbar,
    );
    *run = run_cell.into_inner();
}

/// Search, "in the list only", and the count at the right end, as the browser's row.
fn toolbar(ui: &mut Ui, st: &mut State, d: &Frame<'_>, run: &mut Run) {
    let tk = ui.tokens();
    let win_w = ui.ctx().content_rect().width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let count_l = format::num(d.order.len() as f64);
        let count_r = format!(" / {}", format::num(d.entries.len() as f64));
        let mono = font(Face::Mono, size::TEXT_2XS);
        let gl = ph(ui, &count_l, mono.clone());
        let gr = ph(ui, &count_r, mono);
        let count_w = gl.size().x + gr.size().x + 4.0;
        // As tall as the search beside it.
        let chip = Button::chip(t("favorites", "onlineOnly"), st.listed_only)
            .size(crate::kit::Size::Sm)
            .tip(t("favorites", "onlineOnlyTitle"));
        let chip_w = chip.natural_width(ui);
        let max_search = if win_w >= 1280.0 {
            320.0
        } else {
            f32::INFINITY
        };
        let rest = chip_w + count_w + 6.0 * 3.0;
        let search_w = (ui.available_width() - rest).clamp(128.0, max_search);
        let r = ui.add(
            Input::new(&mut st.query)
                .search()
                .placeholder(t("favorites", "searchPlaceholder"))
                .clear_label(t("servers", "clearSearch"))
                .width(search_w)
                .char_limit(200)
                .id_salt(("saved-search", d.kind.id())),
        );
        if std::mem::take(&mut st.focus_search) {
            r.request_focus();
        }
        if r.changed() {
            st.scroll_to = Some(0);
            ui.ctx().request_repaint();
        }
        let chip = ui.add(chip);
        if chip.clicked() {
            run.push((d.kind.cmd(Cmd::Listed), Value::Null));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(count_w, size::CONTROL), Sense::hover());
            let x = rect.right() - gr.size().x - gl.size().x;
            draw(ui.painter(), &gl, x, rect.center().y, tk.fg);
            draw(
                ui.painter(),
                &gr,
                x + gl.size().x,
                rect.center().y,
                tk.fg_faint,
            );
        });
    });
}

/// The heading: the browser's columns, sorted here; history adds "last played".
fn heading(ui: &mut Ui, kind: Kind, st: &State, run: &mut Run) {
    let wide = kind == Kind::History;
    TableHead::new(wide).show(ui, |ui, cols| {
        let mut head = |ui: &mut Ui, label: &str, col: SortCol, tip: &str| {
            let r = ui.add(SortHead::new(label, st.sort == col, st.asc).tip(tip));
            if r.clicked() {
                run.push((kind.cmd(Cmd::Sort), json!({ "col": col.id() })));
            }
        };
        head_cell(ui, cols.ping, |ui| {
            head(ui, t("servers", "colPing"), SortCol::Ping, "")
        });
        head_cell(ui, cols.players, |ui| {
            head(ui, t("servers", "colPlayers"), SortCol::Players, "")
        });
        head_cell(ui, cols.server, |ui| {
            head(ui, t("servers", "colServer"), SortCol::Name, "")
        });
        if let Some(m) = cols.map {
            head_cell(ui, m, |ui| {
                head(ui, t("servers", "colMap"), SortCol::Map, "");
                head(
                    ui,
                    t("servers", "colTime"),
                    SortCol::Time,
                    t("servers", "colTimeTitle"),
                );
            });
        }
        head_cell(ui, cols.mods, |ui| {
            head(ui, t("servers", "colMods"), SortCol::Mods, "")
        });
        if let Some(x) = cols.extra {
            head_cell(ui, x, |ui| {
                head(ui, t("history", "colLastPlayed"), SortCol::Recent, "")
            });
        }
    });
}

const BTN_DIRECT: u32 = 1;
const BTN_REMOVE: u32 = 2;
const BTN_EDIT: u32 = 3;
/// "Take out of its group".
const BTN_UNGROUP: u32 = 4;
/// "Remove this server's mods" (a listed server with one of them installed).
const MENU_REMOVE_MODS: u32 = 5;
/// "Move to group n": `BTN_GROUP + n`.
const BTN_GROUP: u32 = 100;

/// What a group's heading was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeadAction {
    Toggle,
    Move(i32),
    Rename,
    Remove,
}

/// A group's heading, one row tall: the fold chevron, the name and the count on a band
/// at the row's foot (the space above parts it from the group before). A click folds or
/// unfolds it; the right-click menu moves, renames or deletes the group. The list's first
/// row has no group above it to part from: its band fills the row, with no empty strip
/// under the column heading.
fn head_row(ui: &mut Ui, head: &Head, at: usize, first: bool) -> Option<HeadAction> {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, crate::parts::row_px()), Sense::hover());
    let top = if first {
        rect.top()
    } else {
        rect.bottom() - 30.0
    };
    let band = egui::Rect::from_min_max(egui::pos2(rect.left(), top), rect.right_bottom());
    let id = ui.id().with(("saved-head", at));
    let resp = ui.interact(band, id, Sense::click());
    let hover = crate::kit::paint::ease(ui, id.with("hover"), resp.hovered());
    let p = ui.painter();
    p.rect_filled(
        band,
        CornerRadius::ZERO,
        crate::kit::paint::alpha(tk.raised, 0.35 + 0.35 * hover),
    );
    p.hline(
        band.x_range(),
        band.bottom() - 0.5,
        egui::Stroke::new(1.0, tk.border),
    );
    let cy = band.center().y;
    let chevron = if head.collapsed {
        lucide::CHEVRON_RIGHT
    } else {
        lucide::CHEVRON_DOWN
    };
    chevron.paint(
        p,
        egui::Rect::from_center_size(egui::pos2(band.left() + 16.0, cy), Vec2::splat(12.0)),
        tk.fg_muted,
    );
    let name = head
        .name
        .as_deref()
        .unwrap_or_else(|| t("favorites", "ungrouped"));
    let count = format::num(head.count as f64);
    let cg = ph(ui, &count, font(Face::Mono, size::TEXT_2XS));
    let room = (band.width() - 28.0 - cg.size().x - 20.0).max(16.0);
    let ng = crate::parts::ph_trunc(ui, name, font(Face::SansSemibold, size::TEXT_XS), room);
    let x = band.left() + 28.0;
    let p = ui.painter();
    draw(
        p,
        &ng,
        x,
        cy,
        if head.name.is_some() {
            tk.fg
        } else {
            tk.fg_muted
        },
    );
    draw(p, &cg, x + ng.size().x + 8.0, cy, tk.fg_faint);
    let fold = if head.collapsed {
        t("favorites", "expand")
    } else {
        t("favorites", "collapse")
    };
    let mut out = None;
    if resp.clicked() {
        out = Some(HeadAction::Toggle);
    }
    let named = head.name.is_some();
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if named {
        if let Some(inner) = crate::kit::menu::context(&resp, |ui| {
            let mut a = None;
            let item = |ui: &mut Ui, label: &str, icon| {
                ui.add(crate::kit::MenuItem::new(label).icon(icon))
                    .clicked()
            };
            if item(ui, fold, chevron) {
                a = Some(HeadAction::Toggle);
            }
            crate::kit::menu::separator(ui);
            if item(ui, t("favorites", "moveUp"), lucide::ARROW_UP) {
                a = Some(HeadAction::Move(-1));
            }
            if item(ui, t("favorites", "moveDown"), lucide::ARROW_DOWN) {
                a = Some(HeadAction::Move(1));
            }
            if item(ui, t("favorites", "rename"), lucide::PENCIL) {
                a = Some(HeadAction::Rename);
            }
            crate::kit::menu::separator(ui);
            if ui
                .add(
                    crate::kit::MenuItem::new(t("favorites", "removeGroup"))
                        .icon(lucide::TRASH_2)
                        .danger(),
                )
                .clicked()
            {
                a = Some(HeadAction::Remove);
            }
            a
        }) && let Some(a) = inner.inner
        {
            out = Some(a);
        }
    } else {
        let _ = resp.tip_kbd(fold, "", Side::Top);
    }
    out
}

/// The list pane: nothing matching, or the rows on screen.
fn list(ui: &mut Ui, cx: &Ctx, st: &mut State, d: &Frame<'_>, cache: &mut RowCache, run: &mut Run) {
    let tk = ui.tokens();
    let kind = d.kind;
    let wide = kind == Kind::History;
    if d.order.is_empty() {
        heading(ui, kind, st, run);
        // What hides every entry (the search, "in the list only"), and the way out.
        let query = st.query.trim().to_string();
        let mut why: Vec<String> = Vec::new();
        if !query.is_empty() {
            why.push(tf("servers", "filterSearch", &[("query", &query)]));
        }
        if st.listed_only {
            why.push(t("favorites", "onlineOnly").to_string());
        }
        let text = tf(
            "servers",
            "noMatchFilters",
            &[("filters", &why.join(" · "))],
        );
        let listed_only = st.listed_only;
        Empty::new()
            .icon(lucide::SEARCH_X)
            .title(t("favorites", "noMatch"))
            .text(if why.is_empty() { "" } else { &text })
            .show(ui, |ui| {
                if !why.is_empty()
                    && ui
                        .add(Button::new(t("servers", "clearFilters")).variant(Variant::Accent))
                        .clicked()
                {
                    if !query.is_empty() {
                        run.push((kind.cmd(Cmd::Search), json!({ "text": "" })));
                    }
                    if listed_only {
                        run.push((kind.cmd(Cmd::Listed), json!({ "on": false })));
                    }
                }
            });
        st.on_screen.clear();
        return;
    }
    let servers = &cx.stores.servers;
    let data = &cx.stores.server_data;
    let profile = &cx.stores.profile;
    let mods = &cx.stores.mods;
    let friends = &cx.stores.friends;
    let has_friends = !friends.list().is_empty();
    let direct = t("favorites", "openDirect");
    let remove = match kind {
        Kind::Favorites => t("servers", "removeFavorite"),
        Kind::History => t("history", "remove"),
    };
    let edit = t("favorites", "edit");
    let button = |id, icon, label, danger| RowButton {
        id,
        icon,
        label,
        exclude: false,
        danger,
        tint: None,
    };
    let fav_buttons = [
        button(BTN_DIRECT, lucide::PLUG_ZAP, direct, false),
        button(BTN_EDIT, lucide::PENCIL, edit, false),
        button(BTN_REMOVE, lucide::TRASH_2, remove, true),
    ];
    let history_buttons = [
        button(BTN_DIRECT, lucide::PLUG_ZAP, direct, false),
        button(BTN_REMOVE, lucide::TRASH_2, remove, true),
    ];
    let buttons: &[RowButton] = if wide { &history_buttons } else { &fav_buttons };
    // Favourites: "Edit…", then a "Move to …" per other group, "Take out of the group".
    let groups = &profile.groups;
    let words_key = {
        let mut h = FxHasher::default();
        crate::i18n::locale().hash(&mut h);
        for g in groups {
            g.name.hash(&mut h);
        }
        h.finish()
    };
    let mut move_words = std::mem::take(&mut st.move_words);
    if move_words.0 != words_key || move_words.1.len() != groups.len() {
        move_words = (
            words_key,
            groups
                .iter()
                .map(|g| tf("favorites", "moveToGroup", &[("group", &g.name)]))
                .collect(),
        );
    }
    let ungroup = t("favorites", "ungroup");
    let remove_kbd = commands::shortcut("saved.remove")
        .and_then(commands::Shortcut::key_text)
        .unwrap_or_default();
    let remove_item = RowMenuItem {
        id: BTN_REMOVE,
        icon: lucide::TRASH_2,
        label: remove,
        danger: true,
        kbd: &remove_kbd,
    };
    let remove_mods = RowMenuItem {
        id: MENU_REMOVE_MODS,
        icon: lucide::TRASH_2,
        label: t("mods", "removeServerMods"),
        danger: true,
        kbd: "",
    };
    // Filled again for each row on screen, so a row allocates nothing.
    let fav_menu = |items: &mut Vec<_>, group: Option<&str>| {
        items.clear();
        items.push(RowMenuItem {
            id: BTN_EDIT,
            icon: lucide::PENCIL,
            label: edit,
            danger: false,
            kbd: "",
        });
        for (i, (g, w)) in groups.iter().zip(&move_words.1).enumerate() {
            if group != Some(g.name.as_str()) {
                items.push(RowMenuItem {
                    id: BTN_GROUP + u32::try_from(i).unwrap_or(0),
                    icon: lucide::LAYERS,
                    label: w,
                    danger: false,
                    kbd: "",
                });
            }
        }
        if group.is_some_and(|g| groups.iter().any(|x| x.name == g)) {
            items.push(RowMenuItem {
                id: BTN_UNGROUP,
                icon: lucide::LAYERS,
                label: ungroup,
                danger: false,
                kbd: "",
            });
        }
    };
    let mut row_menu: Vec<RowMenuItem> = Vec::new();
    let mut labels: Vec<RowLabel> = Vec::new();
    let tag = RowTag {
        text: t("favorites", "notInList"),
        tone: Tone::Warn,
        tip: t("favorites", "serverOfflineHint"),
    };
    let sel = st.sel.clone();
    let scroll_to = st.scroll_to.take();
    let cursor = sel
        .as_ref()
        .and_then(|k| st.shown.iter().position(|s| s == k))
        .and_then(|i| st.row_of_shown.get(i).copied());
    // Asked as a place among the entries shown; the list scrolls by rows.
    let scroll_to = scroll_to.map(|i| st.row_of_shown.get(i).copied().unwrap_or(i));
    let mut head_acts: Vec<(HeadAction, usize)> = Vec::new();
    let mut acts: Vec<(RowAction, usize)> = Vec::new();
    let mut times = std::mem::take(&mut st.times);
    if times.len() > 512 {
        times.clear();
    }
    let times_stamp = {
        let mut h = FxHasher::default();
        (crate::i18n::locale(), chrono::Local::now().timestamp()).hash(&mut h);
        h.finish()
    };
    let out =
        VirtualList::new(("saved-list", kind.id()), d.rows.len())
            .row_height(crate::parts::row_px())
            .overscan(OVERSCAN)
            .header(HEAD_H)
            .scroll_to(scroll_to)
            .cursor(cursor)
            .show(
                ui,
                |ui| heading(ui, kind, st, run),
                |ui, i| {
                    let at = match d.rows.get(i) {
                        Some(Row::Entry(at)) => *at,
                        Some(Row::Head(h)) => {
                            if let Some(head) = d.heads.get(*h)
                                && let Some(a) = head_row(ui, head, *h, i == 0)
                            {
                                head_acts.push((a, *h));
                            }
                            return;
                        }
                        None => return,
                    };
                    let (Some(e), Some(l)) = (d.entries.get(at), d.lines.get(at)) else {
                        return;
                    };
                    let port = i64::from(e.port);
                    let listed = l.addr.and_then(|a| servers.row(a));
                    let mut rd = match &listed {
                        Some(r) => RowData::from_server(r),
                        None => RowData {
                            ip: Host::of(&e.ip),
                            query_port: port,
                            game_port: port,
                            players: u32::try_from(l.live.players.max(0)).unwrap_or(0),
                            max_players: u32::try_from(l.live.max.max(0)).unwrap_or(0),
                            bots: l.live.bots,
                            ping_ms: l.ping,
                            listed: false,
                            ..RowData::default()
                        },
                    };
                    rd.name = &e.name;
                    rd.join_port = port;
                    rd.flags.saved_password = e.password.as_deref().is_some_and(|p| !p.is_empty());
                    rd.ping_pending = servers.pending.contains(&l.ping_key);
                    rd.players_loading = data.a2s_loading(&l.ping_key);
                    // Saved, query and game port alike; a listed one asked by numbers.
                    match &listed {
                        Some(r) => {
                            rd.favorite = [e.port, r.query_port(), r.game_port()]
                                .into_iter()
                                .any(|p| profile.is_favorite_at(SocketAddrV4::new(r.ip(), p)));
                            rd.excluded = profile.is_excluded_ip(r.ip());
                        }
                        None => {
                            rd.favorite = profile.is_favorite(&e.ip, e.port);
                            rd.excluded = profile.is_excluded(&e.ip);
                        }
                    }
                    let names: Vec<&str> = match l.addr.filter(|_| has_friends) {
                        Some(a) => friends.on(a).map(|f| f.name.as_str()).collect(),
                        None => Vec::new(),
                    };
                    let (top, bottom) = match e.ts.filter(|_| wide) {
                        Some(ts) => {
                            let cell = times.entry(ts).or_default();
                            if cell.0 != times_stamp || cell.1.is_empty() {
                                *cell = (times_stamp, format::relative(ts), format::date_time(ts));
                            }
                            (cell.1.as_str(), cell.2.as_str())
                        }
                        None => ("", ""),
                    };
                    let w = ui.available_width();
                    let cols = ListColumns::new(
                        egui::Rangef::new(ui.max_rect().left(), ui.max_rect().left() + w),
                        wide,
                    );
                    labels.clear();
                    labels.extend(e.labels.iter().filter_map(|n| profile.label(n)).map(|l| {
                        RowLabel {
                            text: &l.name,
                            color: crate::views::favorites::label_color(&tk, l.color),
                        }
                    }));
                    if wide {
                        row_menu.clear();
                        row_menu.push(remove_item);
                    } else {
                        fav_menu(&mut row_menu, e.group.as_deref());
                    }
                    // "Remove this server's mods" when one of them is installed.
                    if listed.as_ref().is_some_and(|r| {
                        r.server
                            .mods
                            .iter()
                            .any(|m| mods.by_id(m.steam_workshop_id as u64).is_some())
                    }) {
                        row_menu.push(remove_mods);
                    }
                    let menu: &[RowMenuItem] = &row_menu;
                    let mut row = ServerListRow::new(rd)
                        .selected(sel.as_ref() == Some(&l.key))
                        .friends(&names)
                        .buttons(buttons)
                        .menu(menu)
                        .marks(&labels, &e.note);
                    if !l.listed {
                        row = row.tag(tag);
                    }
                    if wide && !top.is_empty() {
                        row = row.extra(RowExtra {
                            top,
                            bottom,
                            tip: bottom,
                        });
                    }
                    let resp = row.show(ui, &cols, cache);
                    if let Some(a) = resp.action {
                        acts.push((a, at));
                    }
                },
            );
    st.times = times;
    drop((row_menu, labels));
    st.move_words = move_words;
    st.page_rows = ((out.viewport.height() / crate::parts::row_px()).floor() as usize).max(1);
    // The rows on screen, for their pings.
    let on: Vec<String> = out
        .visible
        .filter_map(|i| match d.rows.get(i) {
            Some(Row::Entry(at)) => d.lines.get(*at),
            _ => None,
        })
        .filter(|l| l.ping.is_none())
        .map(|l| l.ping_key.clone())
        .collect();
    let mut h = FxHasher::default();
    on.hash(&mut h);
    let hash = h.finish();
    if hash != st.on_screen_hash {
        st.on_screen_hash = hash;
        st.on_screen_since = ui.input(|i| i.time);
        st.on_screen = on;
        st.pinged_hash = if st.on_screen.is_empty() { hash } else { 0 };
    }
    for (a, h) in head_acts {
        let Some(head) = d.heads.get(h) else {
            continue;
        };
        let name = head.name.clone().map_or(Value::Null, Value::String);
        match a {
            HeadAction::Toggle => run.push(("favorites.group_toggle", json!({ "name": name }))),
            HeadAction::Move(delta) => run.push((
                "favorites.group_move",
                json!({ "name": name, "delta": delta }),
            )),
            HeadAction::Rename => run.push(("favorites.organize", Value::Null)),
            HeadAction::Remove => run.push(("favorites.group_remove", json!({ "name": name }))),
        }
    }
    for (a, at) in acts {
        let (Some(e), Some(l)) = (d.entries.get(at), d.lines.get(at)) else {
            continue;
        };
        let args = e.args(l.game_port);
        let id = match a {
            RowAction::Select => {
                run.push((kind.cmd(Cmd::Select), args));
                continue;
            }
            RowAction::Details => {
                let mut args = args;
                if let Some(o) = args.as_object_mut() {
                    o.insert("detail".into(), Value::Bool(true));
                }
                run.push((kind.cmd(Cmd::Select), args));
                continue;
            }
            RowAction::ShowMods => {
                let mut args = args;
                if let Some(o) = args.as_object_mut() {
                    o.insert("detail".into(), Value::Bool(true));
                }
                run.push((kind.cmd(Cmd::Select), args));
                run.push(("servers.detail.tab", json!({ "tab": "mods" })));
                continue;
            }
            RowAction::Join => "list.join",
            RowAction::ToggleFavorite => "list.fav",
            RowAction::Ping => "list.ping",
            RowAction::RefreshPlayers => "servers.row.refresh_players",
            RowAction::CopyLink => "list.link",
            RowAction::CopyAddress => "list.copy",
            RowAction::DesktopShortcut => "list.shortcut",
            RowAction::DirectConnect | RowAction::Custom(BTN_DIRECT) => "list.direct",
            RowAction::ToggleExclude => "servers.row.exclude_ip",
            RowAction::Custom(BTN_REMOVE) => kind.cmd(Cmd::Remove),
            RowAction::Custom(BTN_EDIT) => "favorites.edit",
            RowAction::Custom(MENU_REMOVE_MODS) => {
                // Asked by the listed server's address (its mods are the list's).
                if let Some(a) = l.addr {
                    run.push((
                        "servers.row.remove_mods",
                        json!({ "ip": a.ip().to_string(), "port": a.port() }),
                    ));
                }
                continue;
            }
            RowAction::Custom(BTN_UNGROUP) => {
                let mut args = args;
                if let Some(o) = args.as_object_mut() {
                    o.insert("group".into(), Value::Null);
                }
                run.push(("favorites.move_to_group", args));
                continue;
            }
            RowAction::Custom(n) if n >= BTN_GROUP => {
                let i = usize::try_from(n - BTN_GROUP).unwrap_or(usize::MAX);
                let Some(g) = profile.groups.get(i) else {
                    continue;
                };
                let mut args = args;
                if let Some(o) = args.as_object_mut() {
                    o.insert("group".into(), Value::String(g.name.clone()));
                }
                run.push(("favorites.move_to_group", args));
                continue;
            }
            RowAction::Custom(_) => continue,
        };
        run.push((id, args));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, port: u16) -> Entry {
        Entry {
            ip: "1.2.3.4".into(),
            port,
            name: name.into(),
            ..Entry::default()
        }
    }

    fn line(e: &Entry, listed: bool, players: i64, ping: Option<u32>, map: &str) -> Line {
        Line {
            key: e.key(),
            ping_key: e.key(),
            listed,
            live: Live {
                players,
                ..Live::default()
            },
            ping,
            map: map.into(),
            mods: if listed { 0 } else { -1 },
            ..Line::default()
        }
    }

    #[test]
    fn search_sort_and_figures() {
        let es = [entry("bravo", 1), entry("Alpha", 2), entry("charlie", 3)];
        let ls = [
            line(&es[0], true, 10, Some(80), "enoch"),
            line(&es[1], false, 0, Some(6000), ""),
            line(&es[2], true, 30, Some(40), "sakhal"),
        ];
        let mut st = State::new(Kind::Favorites);
        assert_eq!(shown(&es, &ls, &st), vec![1, 0, 2]);
        st.sort_by(SortCol::Ping);
        assert!(st.asc);
        // A timeout sorts last.
        assert_eq!(shown(&es, &ls, &st), vec![2, 0, 1]);
        st.sort_by(SortCol::Players);
        assert!(!st.asc);
        assert_eq!(shown(&es, &ls, &st), vec![2, 0, 1]);
        st.listed_only = true;
        st.query = "SAK".into();
        assert_eq!(shown(&es, &ls, &st), vec![2]);
        let s = stats(&es, &ls, 0);
        assert_eq!((s.total, s.listed, s.players), (3, 2, 40));
        assert_eq!((s.best_ping, s.avg_ping), (Some(40), Some(60)));
    }

    #[test]
    fn moving_walks_the_rows_shown() {
        let mut st = State::new(Kind::Favorites);
        move_down(&mut st);
        assert!(st.sel.is_none());
        st.shown = vec!["a".into(), "b".into(), "c".into()];
        move_down(&mut st);
        assert_eq!(st.sel.as_deref(), Some("a"));
        cmds::move_by(&mut st, i64::MAX / 2);
        assert_eq!(st.sel.as_deref(), Some("c"));
        assert_eq!(st.scroll_to, Some(2));
    }

    #[test]
    fn open_last_picks_the_newest_and_clears_the_filters() {
        let mut st = State::new(Kind::History);
        assert!(!st.open_last(&[]));
        let mut a = entry("a", 1);
        a.ts = Some(10);
        let mut b = entry("b", 2);
        b.ts = Some(30);
        st.query = "x".into();
        st.listed_only = true;
        st.show_detail = false;
        st.sort_by(SortCol::Name);
        assert!(st.open_last(&[a, b.clone()]));
        assert_eq!(st.sel, Some(b.key()));
        assert!(st.query.is_empty() && !st.listed_only && st.show_detail);
        assert_eq!((st.sort, st.asc), (SortCol::Recent, false));
    }

    #[test]
    fn groups_head_their_entries_and_fold() {
        let mut es = vec![entry("a", 1), entry("b", 2), entry("c", 3), entry("d", 4)];
        es[0].group = Some("G1".into());
        es[2].group = Some("G1".into());
        es[3].group = Some("gone".into());
        let groups = vec![
            dz_profile::FavoriteGroup {
                name: "G1".into(),
                collapsed: false,
            },
            dz_profile::FavoriteGroup {
                name: "Empty".into(),
                collapsed: false,
            },
        ];
        let order = [0, 1, 2, 3];
        let (rows, heads) = group_rows(&es, &order, &[], false, false, false);
        assert!(heads.is_empty() && rows.len() == 4);
        let (rows, heads) = group_rows(&es, &order, &groups, false, false, false);
        use Row::{Entry as E, Head as H};
        assert_eq!(rows, vec![H(0), E(0), E(2), H(1), H(2), E(1), E(3)]);
        assert_eq!(heads[0].count, 2);
        assert_eq!(heads[2].name, None);
        // Filtering leaves out an empty group; folding hides entries unless searching.
        let mut folded = groups;
        folded[0].collapsed = true;
        let (rows, _) = group_rows(&es, &order, &folded, true, false, true);
        assert_eq!(rows, vec![H(0), H(1)]);
        let (rows, _) = group_rows(&es, &order, &folded, true, true, true);
        assert_eq!(rows, vec![H(0), E(0), E(2), H(1), E(1), E(3)]);
        // Two groups of one name (a hand-edited profile): listed once, under the first.
        let twice = vec![folded[0].clone(), folded[0].clone()];
        let (rows, heads) = group_rows(&es, &order, &twice, false, true, false);
        assert_eq!(heads.len(), 2);
        assert_eq!(rows, vec![H(0), E(0), E(2), H(1), E(1), E(3)]);
    }

    fn move_down(st: &mut State) {
        cmds::move_by(st, 1);
    }
}
