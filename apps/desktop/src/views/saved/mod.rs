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

use egui::mutex::Mutex;
use egui::{CornerRadius, Id, Sense, Ui, UiBuilder, Vec2};
use rustc_hash::FxHasher;
use serde_json::{Value, json};

use crate::app::Ctx;
use crate::commands::{self, Scope};
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::{Button, IconButton, Input, Side, Tone, Variant, VirtualList};
use crate::parts::{
    Empty, ListColumns, MasterDetail, PageHeader, ROW_PX, RowAction, RowButton, RowCache, RowData,
    RowExtra, RowMenuItem, RowTag, ServerListRow, ShortcutsButton, SortHead, TableHead, draw,
    head_cell, ph,
};
use crate::store::servers::Live;
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::ViewId;
use crate::views::servers::detail;

pub use crate::views::servers::{Run, run_all};

const HEAD_H: f32 = 28.0;
const OVERSCAN: usize = 8;
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
            })
            .collect(),
        Kind::History => p
            .history
            .iter()
            .map(|h| Entry {
                ip: h.ip.clone(),
                port: h.port,
                name: h.name.clone(),
                password: None,
                ts: Some(h.ts),
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
            match s.servers.row(&e.ip, port) {
                Some(r) => Line {
                    ping_key: format!("{}:{}", r.ip(), r.query_port()),
                    key,
                    listed: true,
                    game_port: r.game_port(),
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
                    key,
                    listed: false,
                    game_port: port,
                    live: s.server_data.players(&mut s.servers, &e.ip, port),
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
            focus_search: false,
            focus_last: false,
            on_screen: Vec::new(),
            on_screen_hash: 0,
            on_screen_since: 0.0,
            pinged_hash: 0,
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
/// Indices into `entries` / `lines`.
pub fn shown(entries: &[Entry], lines: &[Line], st: &State) -> Vec<usize> {
    let q = st.query.trim().to_lowercase();
    let mut out: Vec<usize> = (0..entries.len().min(lines.len()))
        .filter(|&i| {
            let (Some(e), Some(l)) = (entries.get(i), lines.get(i)) else {
                return false;
            };
            (!st.listed_only || l.listed)
                && (q.is_empty()
                    || e.name.to_lowercase().contains(&q)
                    || l.key.contains(&q)
                    || (l.listed && l.map.to_lowercase().contains(&q)))
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
    let cmp = |a: usize, b: usize| -> std::cmp::Ordering {
        let (Some(ea), Some(eb), Some(la), Some(lb)) =
            (entries.get(a), entries.get(b), lines.get(a), lines.get(b))
        else {
            return std::cmp::Ordering::Equal;
        };
        match st.sort {
            SortCol::Name => ea
                .name
                .to_lowercase()
                .cmp(&eb.name.to_lowercase())
                .then_with(|| ea.name.cmp(&eb.name)),
            SortCol::Players => la.live.players.cmp(&lb.live.players),
            SortCol::Ping => la.ping_rank().cmp(&lb.ping_rank()),
            SortCol::Map => text(la, &la.map).cmp(&text(lb, &lb.map)),
            SortCol::Time => text(la, &la.time).cmp(&text(lb, &lb.time)),
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
    let entries = entries(cx, kind);
    let lines = lines(cx, &entries);
    let shared = shared(ui.ctx(), kind);
    let mut run = Run::new();
    let order;
    let selected;
    {
        let mut st = shared.lock();
        if st.focus_last && st.open_last(&entries) {
            st.focus_last = false;
        }
        order = shown(&entries, &lines, &st);
        st.shown = order
            .iter()
            .filter_map(|&i| lines.get(i).map(|l| l.key.clone()))
            .collect();
        if cx.nav.take_focus_search() {
            st.focus_search = true;
        }
        keys(ui.ctx(), cx, kind, &mut st, &entries, &lines, &mut run);
        selected = st.sel.as_ref().and_then(|k| {
            order
                .iter()
                .copied()
                .find(|&i| lines.get(i).is_some_and(|l| l.key == *k))
        });
    }
    // The panel's model and its debounced fetches, before the frame borrows.
    let model = selected
        .and_then(|i| entries.get(i))
        .map(|e| detail::prepare(cx, &e.ip, i64::from(e.port), &e.name));
    let now_s = crate::store::now_ms() / 1000;
    let st_figures = stats(&entries, &lines, now_s);
    {
        let mut st = shared.lock();
        let data = Frame {
            kind,
            entries: &entries,
            lines: &lines,
            order: &order,
            selected,
            stats: &st_figures,
        };
        frame(
            ui,
            cx,
            &mut st,
            &data,
            cache,
            model.as_ref(),
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
        || ctx.any_popup_open()
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
        let chip_label = t("favorites", "onlineOnly");
        let chip_w = ph(ui, chip_label, font(Face::SansMedium, size::TEXT_2XS))
            .size()
            .x
            + 16.0;
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
        let chip =
            ui.add(Button::chip(chip_label, st.listed_only).tip(t("favorites", "onlineOnlyTitle")));
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

/// The list pane: nothing matching, or the rows on screen.
fn list(ui: &mut Ui, cx: &Ctx, st: &mut State, d: &Frame<'_>, cache: &mut RowCache, run: &mut Run) {
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
    let friends = &cx.stores.friends;
    let has_friends = !friends.list().is_empty();
    let direct = t("favorites", "openDirect");
    let remove = match kind {
        Kind::Favorites => t("servers", "removeFavorite"),
        Kind::History => t("history", "remove"),
    };
    let buttons = [
        RowButton {
            id: BTN_DIRECT,
            icon: lucide::PLUG_ZAP,
            label: direct,
            danger: false,
            tint: None,
        },
        RowButton {
            id: BTN_REMOVE,
            icon: lucide::TRASH_2,
            label: remove,
            danger: true,
            tint: None,
        },
    ];
    let remove_kbd = commands::shortcut("saved.remove")
        .and_then(commands::Shortcut::key_text)
        .unwrap_or_default();
    let menu_items = [RowMenuItem {
        id: BTN_REMOVE,
        icon: lucide::TRASH_2,
        label: remove,
        danger: true,
        kbd: &remove_kbd,
    }];
    let menu: &[RowMenuItem] = if wide { &menu_items } else { &[] };
    let tag = RowTag {
        text: t("favorites", "notInList"),
        tone: Tone::Warn,
        tip: t("favorites", "serverOfflineHint"),
    };
    let sel = st.sel.clone();
    let scroll_to = st.scroll_to.take();
    let cursor = sel
        .as_ref()
        .and_then(|k| st.shown.iter().position(|s| s == k));
    let mut acts: Vec<(RowAction, usize)> = Vec::new();
    let out = VirtualList::new(("saved-list", kind.id()), d.order.len())
        .row_height(ROW_PX)
        .overscan(OVERSCAN)
        .header(HEAD_H)
        .scroll_to(scroll_to)
        .cursor(cursor)
        .show(
            ui,
            |ui| heading(ui, kind, st, run),
            |ui, i| {
                let Some(&at) = d.order.get(i) else {
                    return;
                };
                let (Some(e), Some(l)) = (d.entries.get(at), d.lines.get(at)) else {
                    return;
                };
                let port = i64::from(e.port);
                let listed = servers.row(&e.ip, port);
                let mut rd = match &listed {
                    Some(r) => RowData::from_server(r),
                    None => RowData {
                        ip: &e.ip,
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
                let (qp, gp) = (
                    u16::try_from(rd.query_port).unwrap_or(0),
                    u16::try_from(rd.game_port).unwrap_or(0),
                );
                rd.favorite = profile.is_favorite(&e.ip, e.port)
                    || profile.is_favorite(&e.ip, qp)
                    || profile.is_favorite(&e.ip, gp);
                rd.excluded = profile.is_excluded(&e.ip);
                let names: Vec<String> = if has_friends {
                    friends.on(&e.ip, qp).map(|f| f.name.clone()).collect()
                } else {
                    Vec::new()
                };
                let (top, bottom) = match e.ts.filter(|_| wide) {
                    Some(ts) => (format::relative(ts), format::date_time(ts)),
                    None => (String::new(), String::new()),
                };
                let w = ui.available_width();
                let cols = ListColumns::new(
                    egui::Rangef::new(ui.max_rect().left(), ui.max_rect().left() + w),
                    wide,
                );
                let mut row = ServerListRow::new(rd)
                    .selected(sel.as_ref() == Some(&l.key))
                    .friends(&names)
                    .buttons(&buttons)
                    .menu(menu);
                if !l.listed {
                    row = row.tag(tag);
                }
                if wide && !top.is_empty() {
                    row = row.extra(RowExtra {
                        top: &top,
                        bottom: &bottom,
                        tip: &bottom,
                    });
                }
                let resp = row.show(ui, &cols, cache);
                if let Some(a) = resp.action {
                    acts.push((a, at));
                }
            },
        );
    st.page_rows = ((out.viewport.height() / ROW_PX).floor() as usize).max(1);
    // The rows on screen, for their pings.
    let on: Vec<String> = out
        .visible
        .filter_map(|i| d.order.get(i).and_then(|&at| d.lines.get(at)))
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
            RowAction::DirectConnect | RowAction::Custom(BTN_DIRECT) => "list.direct",
            RowAction::ToggleExclude => "servers.row.exclude_ip",
            RowAction::Custom(BTN_REMOVE) => kind.cmd(Cmd::Remove),
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

    fn move_down(st: &mut State) {
        cmds::move_by(st, 1);
    }
}
