//! One server in any list (the browser, favourites, history, friends): `ServerListRow`,
//! its columns (`server-list.ts`), its right-click menu (`ServerMenu`) and the list's
//! heading (`TableHead`, `SortHead`).
//!
//! This is the hottest code in the window: a 9 500-row list scrolled fast draws a screenful
//! of these every frame. So a row paints its cells itself (no nested layouts) from galleys
//! laid out once per server and kept in a [`RowCache`] the list owns: a frame where nothing
//! changed lays out no text and allocates nothing; a row whose figures changed (a ping
//! answered) is laid out again on its own. Icons come from the texture cache.

use std::hash::{Hash, Hasher};
use std::net::Ipv4Addr;
use std::sync::Arc;

use egui::{
    Color32, CornerRadius, Galley, Id, Pos2, Rangef, Rect, Response, Sense, Stroke, Ui, UiBuilder,
    Vec2,
};
use rustc_hash::{FxHashMap, FxHasher};

use super::cells::{
    BARS_W, FlagWords, Flags, PlayersText, dash_color, day_phase, exclude_label, flags_width,
    mods_extra, mods_tip, mods_tone, mods_width, paint_bars, paint_flags, paint_join_quiet,
    paint_mods, paint_players, paint_star, paint_time, ping_text, ping_timed_out, same_version,
    star_label,
};
use super::text::{draw, ph, ph_trunc};
use crate::i18n::{self, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{IconButton, MenuItem, Side, Size, TipExt, Tone, menu};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, motion, size};

/// Pixels per row: two lines of text and the players bar (`LIST_ROW_PX`).
pub const ROW_PX: f32 = 48.0;

/// A row's height in the density worn (taller while a controller drives the window).
pub fn row_px() -> f32 {
    crate::theme::size::row(ROW_PX)
}
/// Below this width of its own the list drops the map column (`@max-[720px]`).
pub const NARROW_PX: f32 = 720.0;
/// `gap-x-3` between columns.
const COL_GAP: f32 = 12.0;
/// `pl-2 pr-2`.
const ROW_PAD: f32 = 8.0;
/// The map column (map and clock).
const MAP_W: f32 = 136.0;
/// The list's own column (history: when you last played), wide enough for an English
/// date and time in the normal density.
const EXTRA_W: f32 = 140.0;

/// The list's own column in the density worn: its date (mono, `TEXT_3XS`) grows with the
/// pad density's text.
fn extra_w() -> f32 {
    EXTRA_W * size::text(size::TEXT_3XS) / size::TEXT_3XS
}
/// The map column is kept only while the server column keeps this much beside it.
const MAP_MIN_SERVER: f32 = 220.0;
/// The server column's widest: past it, the room goes before the actions.
const SERVER_MAX: f32 = 720.0;
/// The name's narrowest: the tag, then the flags, give way before it is cut further.
const MIN_NAME: f32 = 120.0;

/// A column of the server lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Col {
    Star,
    Ping,
    Players,
    Server,
    /// Map and clock; gone when the list is narrow.
    Map,
    Mods,
    /// The list's own column (history: when you last played).
    Extra,
    Actions,
}

/// Where every column sits, shared by the heading and the rows so they line up
/// (`LIST_GRID` / `LIST_GRID_EXTRA`): star 1.5rem · ping 4.75rem · players 6.25rem ·
/// server 1fr · map 8.5rem · mods 4.5rem · [extra 7.5rem] · actions 7.5rem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListColumns {
    pub star: Rangef,
    pub ping: Rangef,
    pub players: Rangef,
    pub server: Rangef,
    pub map: Option<Rangef>,
    pub mods: Rangef,
    pub extra: Option<Rangef>,
    pub actions: Rangef,
}

impl ListColumns {
    /// The columns across `x` (the list's full width; the row padding is inside).
    ///
    /// The server column gives way last: the map column goes first (when the list is
    /// narrow, or when keeping it would leave the server column under
    /// [`MAP_MIN_SERVER`]); on a wide screen the server column stops at [`SERVER_MAX`]
    /// and the spare room goes before the actions, so the map and the mods stay next to
    /// the name instead of drifting to the far edge.
    pub fn new(x: Rangef, wide: bool) -> Self {
        let inner = x.span() - 2.0 * ROW_PAD;
        let mut fixed: f32 = 24.0 + 76.0 + 100.0 + 72.0 + 120.0;
        let mut n: f32 = 6.0;
        let extra_w = extra_w();
        if wide {
            fixed += extra_w;
            n += 1.0;
        }
        let bare = inner - fixed - (n - 1.0) * COL_GAP;
        let with_map = bare - MAP_W - COL_GAP;
        let map_on = x.span() >= NARROW_PX && with_map >= MAP_MIN_SERVER;
        let room = if map_on { with_map } else { bare };
        let server_w = room.clamp(40.0, SERVER_MAX);
        let spare = (room - server_w).max(0.0);
        let mut at = x.min + ROW_PAD;
        // `lead`: room left empty before the column.
        let mut take = |w: f32, lead: f32| {
            at += lead;
            let r = Rangef::new(at, at + w);
            at += w + COL_GAP;
            r
        };
        let star = take(24.0, 0.0);
        let ping = take(76.0, 0.0);
        let players = take(100.0, 0.0);
        let server = take(server_w, 0.0);
        let map = map_on.then(|| take(MAP_W, 0.0));
        let mods = take(72.0, 0.0);
        let extra = wide.then(|| take(extra_w, 0.0));
        let actions = take(120.0, spare);
        Self {
            star,
            ping,
            players,
            server,
            map,
            mods,
            extra,
            actions,
        }
    }

    pub fn get(&self, col: Col) -> Option<Rangef> {
        match col {
            Col::Star => Some(self.star),
            Col::Ping => Some(self.ping),
            Col::Players => Some(self.players),
            Col::Server => Some(self.server),
            Col::Map => self.map,
            Col::Mods => Some(self.mods),
            Col::Extra => self.extra,
            Col::Actions => Some(self.actions),
        }
    }

    pub fn narrow(&self) -> bool {
        self.map.is_none()
    }
}

/// A server's IP as a row holds it: the list's address (a number), or the text a
/// favourite or a typed address keeps, which may be a host name. Shown, copied and
/// hashed the same either way: text that is an IPv4 address is read as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Host<'a> {
    V4(Ipv4Addr),
    Name(&'a str),
}

impl<'a> Host<'a> {
    /// `text` as a host: an IPv4 address when it is one.
    pub fn of(text: &'a str) -> Self {
        text.trim().parse().map_or(Host::Name(text), Host::V4)
    }
}

impl Default for Host<'_> {
    fn default() -> Self {
        Host::Name("")
    }
}

impl std::fmt::Display for Host<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Host::V4(ip) => ip.fmt(f),
            Host::Name(name) => f.write_str(name),
        }
    }
}

/// What a row shows, borrowed from wherever it lives (a `ServerRow`, a favourite).
#[derive(Clone, Copy, Debug, Default)]
pub struct RowData<'a> {
    pub ip: Host<'a>,
    pub query_port: i64,
    /// The port players connect to: the address shown and copied.
    pub game_port: i64,
    /// The port joining goes through (favourites keep the query port).
    pub join_port: i64,
    pub name: &'a str,
    /// The map id as servers report it; empty when not known.
    pub map: &'a str,
    pub time: &'a str,
    pub version: &'a str,
    /// The player's installed game version; `None` when not known. Known, the row shows
    /// the server's version only when it differs (in the warning tone); unknown, always.
    pub game_version: Option<&'a str>,
    pub environment: &'a str,
    pub mods_count: usize,
    /// Of its mods, how many you have not installed; `None` when not known.
    pub mods_missing: Option<u32>,
    /// Of its mods you have, how many are behind.
    pub mods_stale: u32,
    pub flags: Flags,
    pub players: u32,
    pub max_players: u32,
    pub bots: u32,
    pub players_loading: bool,
    /// `None` when never pinged; at or past `PING_TIMEOUT_MS` a timeout.
    pub ping_ms: Option<u32>,
    pub ping_pending: bool,
    /// Full according to the list, but the last query failed.
    pub unverified_full: bool,
    pub favorite: bool,
    pub excluded: bool,
    /// In the current list; an unlisted server is dimmed but joinable.
    pub listed: bool,
}

impl<'a> RowData<'a> {
    /// A listed server read straight from the hub's snapshot (no copy): its live
    /// state and flags as they stand this frame.
    pub fn from_server(r: &dz_hub::features::browser::ServerRef<'a>) -> Self {
        let s = r.server;
        Self {
            ip: Host::V4(s.ip()),
            query_port: i64::from(s.query_port()),
            game_port: i64::from(s.game_port),
            join_port: i64::from(s.query_port()),
            name: &s.name,
            map: r.map,
            time: &s.time,
            version: &s.version,
            game_version: None,
            environment: &s.environment,
            mods_count: s.mods.len(),
            mods_missing: r.mods_missing(),
            mods_stale: r.mods_stale(),
            flags: Flags {
                password: s.password,
                first_person: s.first_person_only,
                battleye: s.battl_eye.unwrap_or(false),
                saved_password: false,
                official: s.official,
                mimics_official: r.mimics_official,
            },
            players: u32::try_from(r.players().max(0)).unwrap_or(u32::MAX),
            max_players: u32::try_from(r.max_players().max(0)).unwrap_or(u32::MAX),
            bots: r.bots(),
            players_loading: false,
            ping_ms: r.ping_ms(),
            ping_pending: false,
            unverified_full: r.unverified_full(),
            favorite: r.favorite(),
            excluded: r.excluded(),
            listed: true,
        }
    }

    /// A listed server, as the hub's page gives it.
    pub fn from_row(r: &'a dz_hub::features::browser::ServerRow) -> Self {
        Self {
            ip: Host::V4(r.ip),
            query_port: i64::from(r.query_port),
            game_port: i64::from(r.game_port),
            join_port: i64::from(r.query_port),
            name: &r.name,
            map: &r.map,
            time: &r.time,
            version: &r.version,
            game_version: None,
            environment: &r.environment,
            mods_count: r.mods_count,
            mods_missing: r.mods_missing,
            mods_stale: r.mods_stale,
            flags: Flags {
                password: r.password,
                first_person: r.first_person_only,
                battleye: r.battl_eye.unwrap_or(false),
                saved_password: false,
                official: r.official,
                mimics_official: r.mimics_official,
            },
            players: u32::try_from(r.players.max(0)).unwrap_or(u32::MAX),
            max_players: u32::try_from(r.max_players.max(0)).unwrap_or(u32::MAX),
            bots: r.bots,
            players_loading: false,
            ping_ms: r.ping_ms,
            ping_pending: false,
            unverified_full: r.unverified_full,
            favorite: r.favorite,
            excluded: r.excluded,
            listed: true,
        }
    }

    /// The key a list and the cache know the server by.
    pub fn key(&self) -> u64 {
        let mut h = FxHasher::default();
        self.ip.hash(&mut h);
        self.query_port.hash(&mut h);
        self.join_port.hash(&mut h);
        h.finish()
    }
}

/// A tag after the flags ("not in the list").
#[derive(Clone, Copy, Debug)]
pub struct RowTag<'a> {
    pub text: &'a str,
    pub tone: Tone,
    pub tip: &'a str,
}

/// A label the player put on a saved server, drawn as a small tinted pill on the address
/// line.
#[derive(Clone, Copy, Debug)]
pub struct RowLabel<'a> {
    pub text: &'a str,
    pub color: Color32,
}

/// Labels shown on a row at most; the rest are counted ("+2").
const MAX_LABELS_SHOWN: usize = 3;
/// A label pill's text is cut past this width.
const LABEL_MAX_W: f32 = 72.0;

/// The list's own column (`wide`): two lines, the second faint mono, with a hint.
#[derive(Clone, Copy, Debug)]
pub struct RowExtra<'a> {
    pub top: &'a str,
    pub bottom: &'a str,
    pub tip: &'a str,
}

/// One of the list's own hover actions, beside "copy the link" (`actions` snippet).
#[derive(Clone, Copy, Debug)]
pub struct RowButton<'a> {
    /// Answered as [`RowAction::Custom`].
    pub id: u32,
    pub icon: Icon,
    pub label: &'a str,
    /// The label is the row's "exclude / include this address", worked out only when
    /// the buttons show (the hovered or picked row), not for every row each frame.
    pub exclude: bool,
    /// Removes or hides something: red under the pointer.
    pub danger: bool,
    /// Colours the icon (an exclusion in force is red).
    pub tint: Option<Color32>,
}

/// One of the list's own items at the end of the right-click menu (`menu` snippet).
#[derive(Clone, Copy, Debug)]
pub struct RowMenuItem<'a> {
    /// Answered as [`RowAction::Custom`].
    pub id: u32,
    pub icon: Icon,
    pub label: &'a str,
    pub danger: bool,
    /// Its shortcut, written at the right ("Del"); empty for none.
    pub kbd: &'a str,
}

/// What the user did to a row this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAction {
    /// A click on the row, or "Details" in its menu.
    Select,
    /// A double click, Join, or "Connect" in its menu.
    Join,
    /// "Details" in the menu (`ondetails`).
    Details,
    ToggleFavorite,
    /// The signal, or "Ping this server".
    Ping,
    /// The players cell: ask the server directly.
    RefreshPlayers,
    /// The mods count, or "Show mod list".
    ShowMods,
    /// The link button (L), or "Copy dzch:// link".
    CopyLink,
    /// "Copy IP:port" in the menu (the address on the row copies itself).
    CopyAddress,
    /// "Open in Direct Connect".
    DirectConnect,
    /// "Create a desktop shortcut" (a launcher that joins this server).
    DesktopShortcut,
    /// "Exclude {ip}" / "Stop excluding {ip}".
    ToggleExclude,
    /// One of the list's own buttons or menu items.
    Custom(u32),
}

/// What a row answers: its response (for keys, scrolling to it) and the action, if any.
pub struct RowResponse {
    pub response: Response,
    pub action: Option<RowAction>,
}

// ---------------------------------------------------------------------------------------
// The cache

/// The words every row shares (they change with the language).
struct Words {
    flags: FlagWords,
    ms: Arc<Galley>,
    dash: Arc<Galley>,
}

/// One server's text, laid out.
struct Laid {
    stamp: u64,
    used: u64,
    name: Arc<Galley>,
    address: Arc<Galley>,
    version: Option<Arc<Galley>>,
    /// The version shown is not the player's.
    version_other: bool,
    map: Option<Arc<Galley>>,
    time: Option<Arc<Galley>>,
    phase: Icon,
    ping: Arc<Galley>,
    ping_unit: bool,
    players: PlayersText,
    mods: Option<(Arc<Galley>, Option<Arc<Galley>>)>,
    friends: Option<Arc<Galley>>,
    tag: Option<Arc<Galley>>,
    extra: Option<(Arc<Galley>, Arc<Galley>)>,
    /// The player's labels (as many as fit) and how many more there are.
    labels: Vec<Arc<Galley>>,
    labels_more: Option<Arc<Galley>>,
    /// The player's note, cut to the room left on the address line.
    note: Option<Arc<Galley>>,
}

/// The rows' laid-out text, kept between frames: the list owns one and hands it to every
/// row it draws. Entries not drawn for a couple of frames are dropped once it grows, and
/// everything is laid out again when the language, the scale or the font atlas changes.
#[derive(Default)]
pub struct RowCache {
    rows: FxHashMap<u64, Laid>,
    words: Option<Words>,
    words_stamp: u64,
    pass: u64,
    atlas_fill: f32,
}

impl RowCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many servers are laid out now.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Forget everything (a theme's fonts changed).
    pub fn clear(&mut self) {
        self.rows.clear();
        self.words = None;
    }

    fn begin(&mut self, ui: &Ui) -> u64 {
        let ctx = ui.ctx();
        let pass = ctx.cumulative_pass_nr();
        if pass != self.pass {
            self.pass = pass;
            // A recreated font atlas (full, or new text options) invalidates every galley
            // laid out before: its fill ratio drops when that happens.
            let fill = ctx.fonts(|f| f.font_atlas_fill_ratio());
            if fill + 1e-4 < self.atlas_fill {
                self.clear();
            }
            self.atlas_fill = fill;
            if self.rows.len() > 256 {
                self.rows.retain(|_, l| l.used + 2 >= pass);
            }
            let mut h = FxHasher::default();
            i18n::locale().hash(&mut h);
            ctx.pixels_per_point().to_bits().hash(&mut h);
            // The pad density's text is larger: its galleys are not the mouse's.
            size::pad_density().hash(&mut h);
            let stamp = h.finish();
            if self.words.is_none() || stamp != self.words_stamp {
                self.words_stamp = stamp;
                self.rows.clear();
                self.words = Some(Words {
                    flags: FlagWords::new(ui),
                    ms: ph(ui, "ms", font(Face::Mono, size::TEXT_2XS)),
                    dash: ph(ui, i18n::format::NONE, font(Face::Sans, size::TEXT_XS)),
                });
            }
        }
        pass
    }
}

// ---------------------------------------------------------------------------------------
// The row

/// `ServerListRow`: one server, the same cells in the same places in every list.
/// `ServerListRow::new(RowData::from_row(&row)).selected(sel).show(ui, &cols, &mut cache)`.
#[must_use = "call show(…)"]
pub struct ServerListRow<'a> {
    data: RowData<'a>,
    selected: bool,
    friends: &'a [&'a str],
    tag: Option<RowTag<'a>>,
    extra: Option<RowExtra<'a>>,
    buttons: &'a [RowButton<'a>],
    menu: &'a [RowMenuItem<'a>],
    mods_clickable: bool,
    labels: &'a [RowLabel<'a>],
    note: &'a str,
}

impl<'a> ServerListRow<'a> {
    pub fn new(data: RowData<'a>) -> Self {
        Self {
            data,
            selected: false,
            friends: &[],
            tag: None,
            extra: None,
            buttons: &[],
            menu: &[],
            mods_clickable: true,
            labels: &[],
            note: "",
        }
    }

    /// The player's labels and note (a favourite's), on the address line after the
    /// version: as many pills as fit, then the note's first line, cut.
    pub fn marks(mut self, labels: &'a [RowLabel<'a>], note: &'a str) -> Self {
        self.labels = labels;
        self.note = note;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    /// Steam friends on this server, by name (a green badge with their count; the names
    /// are joined only for the tooltip).
    pub fn friends(mut self, names: &'a [&'a str]) -> Self {
        self.friends = names;
        self
    }

    pub fn tag(mut self, tag: RowTag<'a>) -> Self {
        self.tag = Some(tag);
        self
    }

    /// The list's own column; the columns must have been made `wide`.
    pub fn extra(mut self, extra: RowExtra<'a>) -> Self {
        self.extra = Some(extra);
        self
    }

    /// The list's own hover actions, after "copy the link".
    pub fn buttons(mut self, b: &'a [RowButton<'a>]) -> Self {
        self.buttons = b;
        self
    }

    /// The list's own items at the end of the right-click menu.
    pub fn menu(mut self, m: &'a [RowMenuItem<'a>]) -> Self {
        self.menu = m;
        self
    }

    /// Whether the mods count opens a list (`onmods`); on by default.
    pub fn mods_clickable(mut self, on: bool) -> Self {
        self.mods_clickable = on;
        self
    }

    /// The player's game version: the row's own, else the one the app keeps in the
    /// context ([`set_game_version`]).
    fn game(&self, ctx: &egui::Context) -> Option<Arc<str>> {
        self.data
            .game_version
            .map(Arc::from)
            .or_else(|| installed_version(ctx))
    }

    fn stamp(&self, cols: &ListColumns, name_w: f32, game: Option<&str>) -> u64 {
        let d = &self.data;
        let mut h = FxHasher::default();
        (
            d.name,
            d.ip,
            d.game_port,
            d.map,
            d.time,
            d.version,
            d.mods_count,
        )
            .hash(&mut h);
        (
            d.mods_missing,
            d.mods_stale,
            d.players,
            d.max_players,
            d.bots,
        )
            .hash(&mut h);
        (d.ping_ms, d.ping_pending, game).hash(&mut h);
        self.friends.len().hash(&mut h);
        self.tag.map(|t| t.text).hash(&mut h);
        self.extra.map(|e| (e.top, e.bottom)).hash(&mut h);
        for l in self.labels {
            (l.text, l.color.to_array()).hash(&mut h);
        }
        self.note.hash(&mut h);
        (name_w.round() as i32).hash(&mut h);
        (cols.server.span().round() as i32).hash(&mut h);
        cols.map.map(|m| m.span().round() as i32).hash(&mut h);
        h.finish()
    }

    fn lay_out(&self, ui: &Ui, cols: &ListColumns, name_w: f32, stamp: u64, pass: u64) -> Laid {
        let game = self.game(ui.ctx());
        let d = &self.data;
        let mono_2xs = font(Face::Mono, size::TEXT_2XS);
        let mono_3xs = font(Face::Mono, size::TEXT_3XS);
        let server_w = cols.server.span();
        let address = ph_trunc(
            ui,
            &format!("{}:{}", d.ip, d.game_port),
            mono_2xs.clone(),
            (server_w - 16.0).max(20.0),
        );
        // The version only when it tells something: not the player's, or theirs unknown.
        let version_other = game
            .as_deref()
            .is_some_and(|g| !d.version.is_empty() && !same_version(d.version, g));
        let version_shown = game.is_none() || version_other;
        let version_room = server_w - address.size().x - 16.0 - 8.0;
        let version = (version_shown && !d.version.is_empty() && version_room > 8.0)
            .then(|| ph_trunc(ui, d.version, mono_3xs.clone(), version_room));
        let map = match cols.map {
            Some(m) if !d.map.is_empty() => Some(ph_trunc(
                ui,
                d.map,
                font(Face::Sans, size::TEXT_XS),
                m.span(),
            )),
            _ => None,
        };
        let time =
            (!d.map.is_empty() && !d.time.is_empty()).then(|| ph(ui, d.time, mono_3xs.clone()));
        let (ping, ping_unit) = ping_text(d.ping_ms, d.ping_pending);
        let mods = (d.mods_count > 0).then(|| {
            (
                ph(ui, &d.mods_count.to_string(), mono_2xs.clone()),
                mods_extra(d.mods_missing, d.mods_stale)
                    .map(|e| ph(ui, &e, font(Face::MonoSemibold, size::TEXT_2XS))),
            )
        });
        let friends = (!self.friends.is_empty()).then(|| {
            ph(
                ui,
                &self.friends.len().to_string(),
                font(Face::MonoSemibold, size::TEXT_3XS),
            )
        });
        let tag = self.tag.map(|t| ph(ui, t.text, mono_2xs.clone()));
        let extra = match (self.extra, cols.extra) {
            (Some(e), Some(x)) => Some((
                ph_trunc(ui, e.top, font(Face::Sans, size::TEXT_2XS), x.span()),
                ph_trunc(ui, e.bottom, mono_3xs.clone(), x.span()),
            )),
            _ => None,
        };
        // The address line's room after the address (and its copy icon) and the version.
        let mut room = server_w
            - address.size().x
            - 16.0
            - 4.0
            - version.as_ref().map_or(0.0, |v| v.size().x + 8.0);
        let mut labels = Vec::new();
        let label_font = font(Face::SansMedium, size::TEXT_3XS);
        for l in self.labels.iter().take(MAX_LABELS_SHOWN) {
            let g = ph_trunc(ui, l.text, label_font.clone(), LABEL_MAX_W);
            let w = g.size().x + 8.0 + 4.0;
            if w > room {
                break;
            }
            room -= w;
            labels.push(g);
        }
        let more = self.labels.len().saturating_sub(labels.len());
        let labels_more = (more > 0).then(|| {
            let g = ph(ui, &format!("+{more}"), mono_3xs.clone());
            room -= g.size().x + 4.0;
            g
        });
        let first_line = self.note.lines().find(|l| !l.trim().is_empty());
        // Too little room for its words: the note's icon alone, its words on hover.
        let note_font = font(Face::Sans, size::TEXT_2XS);
        let note = first_line.and_then(|n| {
            if room > 40.0 {
                Some(ph_trunc(ui, n.trim(), note_font, room - 14.0))
            } else if room > 12.0 {
                Some(ph(ui, "", note_font))
            } else {
                None
            }
        });
        Laid {
            stamp,
            used: pass,
            name: ph_trunc(
                ui,
                d.name,
                font(Face::SansSemibold, size::TEXT_XS),
                name_w.max(16.0),
            ),
            address,
            version,
            version_other,
            map,
            time,
            phase: day_phase(d.time),
            ping: ph(ui, &ping, mono_2xs),
            ping_unit,
            players: PlayersText::new(ui, d.players, d.max_players, d.bots),
            mods,
            friends,
            tag,
            extra,
            labels,
            labels_more,
            note,
        }
    }

    /// Draws the row across the full width at the cursor, [`row_px`] tall, with its columns
    /// at `cols` (from [`ListColumns::new`] over the same width).
    pub fn show(self, ui: &mut Ui, cols: &ListColumns, cache: &mut RowCache) -> RowResponse {
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, row_px()), Sense::hover());
        let key = self.data.key();
        let id = ui.id().with(("server-row", key));
        let row = ui.interact(rect, id, Sense::click());
        let mut action = None;
        if row.clicked() {
            action = Some(RowAction::Select);
        }
        if row.double_clicked() {
            action = Some(RowAction::Join);
        }
        if ui.is_rect_visible(rect)
            && let Some(a) = self.paint(ui, rect, cols, cache, id, &row)
        {
            action = Some(a);
        }
        if paint::keyboard_focus(ui, &row) {
            let tk = ui.tokens();
            paint::focus_ring(ui.painter(), rect.shrink(2.0), 0, &tk);
        }
        let d = self.data;
        let menu_items = self.menu;
        let show_mods = d.mods_count > 0 && self.mods_clickable;
        if let Some(inner) = menu::context(&row, |ui| {
            server_menu(ui, d.ip, d.favorite, d.excluded, show_mods, menu_items)
        }) && let Some(a) = inner.inner
        {
            action = Some(a);
        }
        RowResponse {
            response: row,
            action,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(
        &self,
        ui: &mut Ui,
        rect: Rect,
        cols: &ListColumns,
        cache: &mut RowCache,
        id: Id,
        row: &Response,
    ) -> Option<RowAction> {
        let pass = cache.begin(ui);
        let tk = ui.tokens();
        let d = &self.data;
        let mut action = None;

        // Room for the name: the server column less what follows it on its line.
        let words = cache.words.as_ref()?;
        let mut fw = flags_width(&d.flags, &words.flags);
        // Plex Mono advances 0.6em: the badges' widths are known before laying them out.
        let friends_w = if self.friends.is_empty() {
            0.0
        } else {
            let digits = (self.friends.len().checked_ilog10().unwrap_or(0) + 1) as f32;
            6.0 + 4.0 + 10.0 + 2.0 + 0.6 * size::text(size::TEXT_3XS) * digits + 4.0
        };
        let mut tag_w = self.tag.map_or(0.0, |t| {
            6.0 + 12.0 + 0.6 * size::text(size::TEXT_2XS) * t.text.chars().count() as f32
        });
        let room = |fw: f32, tag_w: f32| {
            let flags = if fw > 0.0 { 6.0 + fw } else { 0.0 };
            cols.server.span() - flags - friends_w - tag_w
        };
        // The name keeps its narrowest (or its whole width, when shorter): the tag goes
        // first, then the flags. The name is measured only when the room is short.
        if room(fw, tag_w) < MIN_NAME {
            let whole = ph(ui, d.name, font(Face::SansSemibold, size::TEXT_XS))
                .size()
                .x;
            let keep = MIN_NAME.min(whole);
            if room(fw, tag_w) < keep {
                tag_w = 0.0;
            }
            if room(fw, tag_w) < keep {
                fw = 0.0;
            }
        }
        let show_tag = tag_w > 0.0;
        let key = d.key();
        let name_w = room(fw, tag_w);
        let stamp = self.stamp(cols, name_w, self.game(ui.ctx()).as_deref());
        let fresh = cache.rows.get(&key).is_some_and(|l| l.stamp == stamp);
        if !fresh {
            let laid = self.lay_out(ui, cols, name_w, stamp, pass);
            cache.rows.insert(key, laid);
        }
        let l = cache.rows.get_mut(&key)?;
        l.used = pass;
        let words = cache.words.as_ref()?;

        let mut rui = ui.new_child(UiBuilder::new().max_rect(rect).id_salt(("row-ui", key)));
        if !d.listed || d.excluded {
            rui.multiply_opacity(0.55);
        }
        let ui = &mut rui;

        // Ground: hover, selection, an open menu; the hairline under.
        let hovered = row.contains_pointer();
        let menu_open = menu::is_open(row);
        let hover = paint::ease(ui, id.with("hover"), hovered);
        {
            let p = ui.painter();
            let ground = if self.selected {
                alpha(tk.accent, 0.10)
            } else if menu_open {
                alpha(tk.raised, 0.7)
            } else {
                alpha(tk.raised, 0.5 * hover)
            };
            if ground.a() > 0 {
                p.rect_filled(rect, CornerRadius::ZERO, ground);
            }
            p.hline(
                rect.x_range(),
                rect.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.5)),
            );
            if self.selected {
                let a = paint::ease_for(ui, id.with("sel"), true, motion::BASE);
                p.rect_filled(
                    Rect::from_min_max(rect.min, Pos2::new(rect.left() + 2.0, rect.bottom())),
                    CornerRadius::ZERO,
                    tk.accent.gamma_multiply(a),
                );
            }
        }
        let cy = rect.center().y;
        // The two lines' places, spread over a taller row (the pad density's).
        let k = (rect.height() / ROW_PX).max(1.0);

        // Star.
        let star = Rect::from_center_size(Pos2::new(cols.star.min + 10.0, cy), Vec2::splat(20.0));
        let sr = ui.interact(star, id.with("star"), Sense::click());
        let pop_id = id.with("pop");
        if sr.clicked() {
            if !d.favorite {
                let now = ui.input(|i| i.time);
                ui.data_mut(|m| m.insert_temp(pop_id, now));
            }
            action = Some(RowAction::ToggleFavorite);
        }
        let sh = paint::ease(ui, sr.id.with("hover"), sr.hovered());
        let popped = ui.data(|m| m.get_temp::<f64>(pop_id));
        paint_star(ui, star, 14.0, d.favorite || sr.clicked(), sh, popped);
        let _ = sr.on_hover_cursor(egui::CursorIcon::PointingHand).tip_kbd(
            star_label(d.favorite),
            "",
            Side::Top,
        );

        // Ping.
        let ping_w = 2.0
            + BARS_W
            + 6.0
            + l.ping.size().x
            + if l.ping_unit { words.ms.size().x } else { 0.0 }
            + if d.unverified_full { 4.0 + 12.0 } else { 0.0 }
            + 2.0;
        // The signal sits a pixel above the middle, as the grid's baseline put it.
        let pcy = cy - 1.0;
        let pr = Rect::from_min_size(
            Pos2::new(cols.ping.min, pcy - 8.0),
            Vec2::new(ping_w.min(cols.ping.span()), 16.0),
        );
        let presp = ui.interact(pr, id.with("ping"), Sense::click());
        if presp.clicked() {
            action = Some(RowAction::Ping);
        }
        {
            let ph = paint::ease(ui, presp.id.with("hover"), presp.hovered());
            let p = ui.painter();
            if ph > 0.0 {
                p.rect_filled(
                    pr,
                    CornerRadius::same(size::RADIUS_XS),
                    alpha(tk.raised, ph),
                );
            }
            let time = ui.input(|i| i.time);
            let ink = paint_bars(
                p,
                pr.left() + 2.0,
                pcy,
                d.ping_ms,
                d.ping_pending,
                time,
                &tk,
            );
            let mut x = pr.left() + 2.0 + BARS_W + 6.0;
            draw(p, &l.ping, x, pcy, ink);
            x += l.ping.size().x;
            if l.ping_unit {
                draw(p, &words.ms, x, pcy, tk.fg_faint);
                x += words.ms.size().x;
            }
            if d.unverified_full {
                lucide::TRIANGLE_ALERT.paint(
                    p,
                    Rect::from_min_size(Pos2::new(x + 4.0, pcy - 6.0), Vec2::splat(12.0)),
                    tk.warn,
                );
            }
        }
        if d.ping_pending {
            crate::kit::paint::animate(ui.ctx());
        }
        let ping_tip = if ping_timed_out(d.ping_ms, d.ping_pending) {
            t("servers", "pingNoAnswer")
        } else {
            t("servers", "clickPing")
        };
        let _ = presp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(ping_tip, "", Side::Top);

        // Players.
        let plr = Rect::from_min_size(
            Pos2::new(cols.players.min, cy - 21.0 / 2.0 - 1.0),
            Vec2::new(100.0, 23.0),
        );
        let plresp = ui.interact(plr, id.with("players"), Sense::click());
        if plresp.clicked() {
            action = Some(RowAction::RefreshPlayers);
        }
        {
            let h = paint::ease(ui, plresp.id.with("hover"), plresp.hovered());
            if h > 0.0 {
                ui.painter().rect_filled(
                    plr,
                    CornerRadius::same(size::RADIUS_XS),
                    alpha(tk.raised, h),
                );
            }
            let pct = if d.max_players > 0 {
                (d.players as f32 / d.max_players as f32).min(1.0)
            } else {
                0.0
            };
            let shown = ui
                .ctx()
                .animate_value_with_time(id.with("pct"), pct, motion::SLOW);
            let inner = Rect::from_min_size(
                Pos2::new(plr.left() + 2.0, plr.top() + 1.0),
                Vec2::new(96.0, 21.0),
            );
            let bots = paint_players(
                ui.painter(),
                inner,
                &l.players,
                d.players,
                d.max_players,
                d.bots,
                shown,
                d.players_loading,
                &tk,
            );
            if let Some(br) = bots {
                let r = ui.interact(br, id.with("bots"), Sense::hover());
                if r.hovered() {
                    let _ = r.tip_kbd(&tf("servers", "bots", &[("count", &d.bots)]), "", Side::Top);
                }
            }
        }
        let _ = plresp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(t("servers", "clickRefreshPlayers"), "", Side::Top);

        // Server: the name and what follows it, the address and (when it matters) the
        // version under.
        let top_cy = rect.top() + 15.3 * k;
        let bot_cy = rect.top() + 31.7 * k;
        let mut x = cols.server.min;
        // What does not fit the column is cut, never drawn over the map.
        let row_clip = ui.clip_rect();
        ui.set_clip_rect(row_clip.intersect(Rect::from_x_y_ranges(cols.server, rect.y_range())));
        {
            let nr =
                Rect::from_min_size(Pos2::new(x, top_cy - l.name.size().y / 2.0), l.name.size());
            draw(ui.painter(), &l.name, x, top_cy, tk.fg);
            let r = ui.interact(nr, id.with("name"), Sense::hover());
            let _ = r.tip_kbd(d.name, "", Side::Top);
            x += l.name.size().x;
        }
        if fw > 0.0 {
            x = paint_flags(
                ui,
                x + 6.0,
                top_cy,
                &d.flags,
                &words.flags,
                id.with("flags"),
            );
        }
        if let Some(f) = &l.friends {
            x += 6.0;
            let w = 4.0 + 10.0 + 2.0 + f.size().x + 4.0;
            let r = Rect::from_min_size(Pos2::new(x, top_cy - 6.0), Vec2::new(w, 12.0));
            let p = ui.painter();
            p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), alpha(tk.ok, 0.15));
            lucide::USERS.paint(
                p,
                Rect::from_min_size(Pos2::new(x + 4.0, top_cy - 5.0), Vec2::splat(10.0)),
                tk.ok,
            );
            draw(p, f, x + 16.0, top_cy, tk.ok);
            let resp = ui.interact(r, id.with("friends"), Sense::hover());
            if resp.hovered() {
                let names = self.friends.join(", ");
                let _ = resp.tip_kbd(
                    &tf("friends", "friendsHere", &[("names", &names)]),
                    "",
                    Side::Top,
                );
            }
            x += w;
        }
        if let (true, Some(g), Some(tag)) = (show_tag, &l.tag, self.tag) {
            x += 6.0;
            paint_tag(ui, x, top_cy, g, tag, id.with("tag"), &tk);
        }

        // The address copies itself; a tick says it did.
        let x0 = cols.server.min;
        {
            let ar = Rect::from_min_size(
                Pos2::new(x0, bot_cy - 7.0),
                Vec2::new(l.address.size().x + 16.0, 14.0),
            );
            let resp = ui.interact(ar, id.with("copy"), Sense::click());
            let copied_id = id.with("copied");
            let now = ui.input(|i| i.time);
            if resp.clicked() {
                ui.ctx().copy_text(format!("{}:{}", d.ip, d.game_port));
                ui.data_mut(|m| m.insert_temp(copied_id, now));
            }
            let copied = ui
                .data(|m| m.get_temp::<f64>(copied_id))
                .is_some_and(|at| now - at < 1.4);
            if copied {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            }
            let h = paint::ease(ui, resp.id.with("hover"), resp.hovered());
            let ink = if copied {
                tk.ok
            } else {
                paint::lerp(tk.fg_faint, tk.fg_muted, h)
            };
            let p = ui.painter();
            draw(p, &l.address, x0, bot_cy, ink);
            let ir = Rect::from_min_size(
                Pos2::new(x0 + l.address.size().x + 4.0, bot_cy - 6.0),
                Vec2::splat(12.0),
            );
            if copied {
                lucide::CHECK.paint(p, ir, ink);
            } else if h > 0.0 {
                lucide::COPY.paint(p, ir, ink.gamma_multiply(h));
            }
            if resp.hovered() {
                let tip = if copied {
                    t("common", "copied").to_owned()
                } else {
                    tf(
                        "servers",
                        "copyIp",
                        &[("address", &format_args!("{}:{}", d.ip, d.game_port))],
                    )
                };
                let _ = resp
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .tip_kbd(&tip, "", Side::Top);
            }
            if let Some(v) = &l.version {
                let x = ar.right() + 8.0;
                if l.version_other {
                    draw(ui.painter(), v, x, bot_cy, tk.warn);
                    let vr = Rect::from_min_size(Pos2::new(x, bot_cy - v.size().y / 2.0), v.size());
                    let r = ui.interact(vr, id.with("version"), Sense::hover());
                    if r.hovered() {
                        let yours = self.game(ui.ctx()).unwrap_or_default();
                        let _ = r.tip_kbd(
                            &tf(
                                "servers",
                                "versionOther",
                                &[("version", &d.version), ("yours", &yours)],
                            ),
                            "",
                            Side::Top,
                        );
                    }
                } else {
                    draw(ui.painter(), v, x, bot_cy, tk.fg_faint);
                }
            }
        }

        if !l.labels.is_empty() || l.labels_more.is_some() || l.note.is_some() {
            let mut x = x0
                + l.address.size().x
                + 16.0
                + 4.0
                + l.version.as_ref().map_or(0.0, |v| v.size().x + 8.0);
            for (i, (g, lab)) in l.labels.iter().zip(self.labels).enumerate() {
                let r = Rect::from_min_size(
                    Pos2::new(x, bot_cy - 6.5),
                    Vec2::new(g.size().x + 8.0, 13.0),
                );
                let p = ui.painter();
                p.rect_filled(
                    r,
                    CornerRadius::same(size::RADIUS_XS),
                    alpha(lab.color, 0.16),
                );
                draw(p, g, x + 4.0, bot_cy, lab.color);
                let resp = ui.interact(r, id.with(("label", i)), Sense::hover());
                if resp.hovered() {
                    let _ = resp.tip_kbd(lab.text, "", Side::Top);
                }
                x = r.right() + 4.0;
            }
            if let Some(g) = &l.labels_more {
                draw(ui.painter(), g, x, bot_cy, tk.fg_faint);
                let r =
                    Rect::from_min_size(Pos2::new(x, bot_cy - 6.0), Vec2::new(g.size().x, 12.0));
                let resp = ui.interact(r, id.with("labels-more"), Sense::hover());
                if resp.hovered() {
                    let rest: Vec<&str> = self
                        .labels
                        .iter()
                        .skip(l.labels.len())
                        .map(|l| l.text)
                        .collect();
                    let _ = resp.tip_kbd(&rest.join(", "), "", Side::Top);
                }
                x += g.size().x + 4.0;
            }
            if let Some(g) = &l.note {
                let x = x + 2.0;
                let ir = Rect::from_min_size(Pos2::new(x, bot_cy - 5.0), Vec2::splat(10.0));
                lucide::MESSAGE_SQUARE.paint(ui.painter(), ir, tk.fg_faint);
                draw(ui.painter(), g, x + 13.0, bot_cy, tk.fg_muted);
                let r = Rect::from_min_size(
                    Pos2::new(x, bot_cy - 7.0),
                    Vec2::new(g.size().x + 13.0, 14.0),
                );
                let resp = ui.interact(r, id.with("note"), Sense::hover());
                if resp.hovered() {
                    let _ = resp.tip_kbd(self.note, "", Side::Top);
                }
            }
        }

        ui.set_clip_rect(row_clip);

        // Map and clock.
        if let Some(mc) = cols.map {
            let p = ui.painter();
            match &l.map {
                Some(g) => {
                    let my = rect.top() + 17.0 * k;
                    draw(p, g, mc.min, my, tk.map);
                    let r = Rect::from_min_size(Pos2::new(mc.min, my - 8.0), g.size());
                    let _ = ui.interact(r, id.with("map"), Sense::hover()).tip_kbd(
                        d.map,
                        "",
                        Side::Top,
                    );
                    if let Some(tg) = &l.time {
                        paint_time(
                            ui.painter(),
                            mc.min,
                            rect.top() + 33.0 * k,
                            l.phase,
                            tg,
                            &tk,
                        );
                    }
                }
                None => draw(p, &words.dash, mc.min, cy, dash_color(&tk)),
            }
        }

        // Mods.
        match &l.mods {
            Some((count, extra)) => {
                let w = mods_width(count, extra.as_ref());
                let r = Rect::from_min_size(Pos2::new(cols.mods.min, cy - 8.0), Vec2::new(w, 16.0));
                let sense = if self.mods_clickable {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let resp = ui.interact(r, id.with("mods"), sense);
                if self.mods_clickable && resp.clicked() {
                    action = Some(RowAction::ShowMods);
                }
                let h = paint::ease(
                    ui,
                    resp.id.with("hover"),
                    self.mods_clickable && resp.hovered(),
                );
                paint_mods(
                    ui.painter(),
                    r,
                    count,
                    extra.as_ref(),
                    mods_tone(&tk, d.mods_missing, d.mods_stale),
                    h,
                    &tk,
                );
                if resp.hovered() {
                    let tip = mods_tip(d.mods_count, d.mods_missing, d.mods_stale);
                    let resp = if self.mods_clickable {
                        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
                    } else {
                        resp
                    };
                    let _ = resp.tip_kbd(&tip, "", Side::Top);
                }
            }
            None => draw(
                ui.painter(),
                &words.dash,
                cols.mods.min,
                cy,
                dash_color(&tk),
            ),
        }

        // The list's own column.
        if let (Some((top, bottom)), Some(xc), Some(e)) = (&l.extra, cols.extra, self.extra) {
            let p = ui.painter();
            let ty = rect.top() + 16.0 * k;
            draw(p, top, xc.min, ty, tk.fg);
            draw(p, bottom, xc.min, rect.top() + 32.0 * k, tk.fg_faint);
            if !e.tip.is_empty() {
                let r = Rect::from_x_y_ranges(xc, Rangef::new(ty - 8.0, rect.top() + 39.0 * k));
                let _ =
                    ui.interact(r, id.with("extra"), Sense::hover())
                        .tip_kbd(e.tip, "", Side::Top);
            }
        }

        // Join, and the row's own actions in a framed group left of it.
        let join = Rect::from_min_size(
            Pos2::new(cols.actions.max - size::CONTROL, cy - size::CONTROL / 2.0),
            Vec2::splat(size::CONTROL),
        );
        let jr = ui.interact(join, id.with("join"), Sense::click());
        if jr.clicked() {
            action = Some(RowAction::Join);
        }
        {
            let jh = paint::ease(ui, jr.id.with("hover"), jr.hovered());
            let lit = paint::ease(ui, id.with("lit"), self.selected || hovered || jr.hovered());
            paint_join_quiet(ui, join, lit, jh, jr.is_pointer_button_down_on());
            if paint::keyboard_focus(ui, &jr) {
                paint::focus_ring(ui.painter(), join, size::RADIUS_SM, &tk);
            }
        }
        let _ = jr.on_hover_cursor(egui::CursorIcon::PointingHand).tip_kbd(
            t("servers", "connectTitle"),
            "",
            Side::Top,
        );

        let shown = paint::ease(ui, id.with("actions"), hovered || self.selected);
        if shown > 0.0 {
            let n = 1 + self.buttons.len();
            let bw = size::CONTROL_SM;
            let gw = 2.0 + n as f32 * bw + (n - 1) as f32 + 2.0;
            let g = Rect::from_min_size(
                Pos2::new(join.left() - 6.0 - gw, cy - (bw + 4.0) / 2.0),
                Vec2::new(gw, bw + 4.0),
            );
            let mut gui = ui.new_child(
                UiBuilder::new()
                    .max_rect(g.shrink(2.0))
                    .layout(egui::Layout::left_to_right(egui::Align::Center))
                    .id_salt("actions"),
            );
            gui.multiply_opacity(shown);
            gui.spacing_mut().item_spacing.x = 1.0;
            paint::plate(gui.painter(), g, size::RADIUS_SM, tk.panel, tk.border);
            let link = gui.add(
                IconButton::new(lucide::LINK, t("servers", "copyLink"))
                    .size(Size::IconXs)
                    .kbd("L"),
            );
            if link.clicked() {
                action = Some(RowAction::CopyLink);
            }
            for b in self.buttons {
                let words;
                let label = if b.exclude {
                    words = exclude_label(d.ip, d.excluded);
                    words.as_str()
                } else {
                    b.label
                };
                let mut ib = IconButton::new(b.icon, label).size(Size::IconXs);
                if b.danger {
                    ib = ib.danger_hover();
                }
                if let Some(c) = b.tint {
                    ib = ib.tint(c);
                }
                if gui.add(ib).clicked() {
                    action = Some(RowAction::Custom(b.id));
                }
            }
        }
        action
    }
}

/// A kit `Tag` drawn from a laid-out galley at `x` (18px tall).
fn paint_tag(ui: &Ui, x: f32, cy: f32, g: &Arc<Galley>, tag: RowTag<'_>, id: Id, t: &Tokens) {
    let c = match tag.tone {
        Tone::Ok => Some(t.ok),
        Tone::Warn => Some(t.warn),
        Tone::Err => Some(t.err),
        Tone::Accent => Some(t.accent),
        _ => None,
    };
    let (fill, ink, ring) = match c {
        Some(c) => (alpha(c, 0.12), c, alpha(c, 0.25)),
        None => (alpha(t.raised, 0.7), t.fg_muted, alpha(t.border, 0.6)),
    };
    let r = Rect::from_min_size(Pos2::new(x, cy - 9.0), Vec2::new(g.size().x + 12.0, 18.0));
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(size::RADIUS_SM), fill);
    p.rect_stroke(
        r,
        CornerRadius::same(size::RADIUS_SM),
        Stroke::new(1.0, ring),
        egui::StrokeKind::Outside,
    );
    draw(p, g, x + 6.0, cy, ink);
    if !tag.tip.is_empty() {
        let _ = ui
            .interact(r, id, Sense::hover())
            .tip_kbd(tag.tip, "", Side::Top);
    }
}

// ---------------------------------------------------------------------------------------
// The menu

/// `ServerMenu`: what a right-click on a server offers in every list, then the list's own
/// items. Call inside `menu::context`; answers the item chosen.
pub fn server_menu(
    ui: &mut Ui,
    ip: impl std::fmt::Display,
    favorite: bool,
    excluded: bool,
    show_mods: bool,
    more: &[RowMenuItem<'_>],
) -> Option<RowAction> {
    let mut out = None;
    let mut item = |ui: &mut Ui, it: MenuItem<'_>, a: RowAction| {
        if ui.add(it).clicked() {
            out = Some(a);
        }
    };
    item(
        ui,
        MenuItem::new(t("servers", "connect"))
            .icon(lucide::PLAY)
            .kbd("Enter"),
        RowAction::Join,
    );
    item(
        ui,
        MenuItem::new(t("servers", "details"))
            .icon(lucide::PANEL_RIGHT)
            .kbd("I"),
        RowAction::Details,
    );
    if show_mods {
        item(
            ui,
            MenuItem::new(t("servers", "showMods")).icon(lucide::PUZZLE),
            RowAction::ShowMods,
        );
    }
    menu::separator(ui);
    item(
        ui,
        MenuItem::new(star_label(favorite))
            .icon(lucide::STAR)
            .kbd("F"),
        RowAction::ToggleFavorite,
    );
    item(
        ui,
        MenuItem::new(t("servers", "pingNow"))
            .icon(lucide::ACTIVITY)
            .kbd("P"),
        RowAction::Ping,
    );
    menu::separator(ui);
    item(
        ui,
        MenuItem::new(t("servers", "copyIpPort")).icon(lucide::COPY),
        RowAction::CopyAddress,
    );
    item(
        ui,
        MenuItem::new(t("servers", "copyLinkShort"))
            .icon(lucide::LINK)
            .kbd("L"),
        RowAction::CopyLink,
    );
    item(
        ui,
        MenuItem::new(t("servers", "directConnect"))
            .icon(lucide::PLUG_ZAP)
            .kbd("D"),
        RowAction::DirectConnect,
    );
    item(
        ui,
        MenuItem::new(t("servers", "desktopShortcut")).icon(lucide::MONITOR),
        RowAction::DesktopShortcut,
    );
    menu::separator(ui);
    let ex_label = if excluded {
        tf("servers", "includeIp", &[("ip", &ip)])
    } else {
        tf("servers", "excludeIp", &[("ip", &ip)])
    };
    let mut ex = MenuItem::new(&ex_label).icon(lucide::BAN);
    if !excluded {
        ex = ex.danger();
    }
    item(ui, ex, RowAction::ToggleExclude);
    if !more.is_empty() {
        menu::separator(ui);
        for m in more {
            let mut it = MenuItem::new(m.label).icon(m.icon);
            if !m.kbd.is_empty() {
                it = it.kbd(m.kbd);
            }
            if m.danger {
                it = it.danger();
            }
            item(ui, it, RowAction::Custom(m.id));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// The heading

/// The face of a list's heading: display, bold, `text-2xs`, tracked 0.1em, capitals.
fn head_galley(ui: &Ui, text: &str) -> Arc<Galley> {
    paint::tracked(
        ui,
        text,
        font(Face::DisplayBold, size::TEXT_2XS),
        Color32::PLACEHOLDER,
        0.1,
        true,
        None,
    )
}

/// `TableHead`: the sticky heading row of a list (`h-7 border-b bg-panel/95 px-2`). The
/// closure gets the heading's `Ui` and the columns over its width; place cells with
/// [`head_cell`].
pub struct TableHead {
    wide: bool,
}

impl TableHead {
    /// Over the server lists' columns; `wide` with the list's own column.
    pub fn new(wide: bool) -> Self {
        Self { wide }
    }

    pub fn show(self, ui: &mut Ui, cells: impl FnOnce(&mut Ui, &ListColumns)) -> Response {
        let tk = ui.tokens();
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::ZERO, alpha(tk.panel, 0.95));
        p.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, tk.border),
        );
        let cols = ListColumns::new(rect.x_range(), self.wide);
        let mut hui = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("table-head"));
        cells(&mut hui, &cols);
        resp
    }
}

/// A heading cell over `x`: a left-to-right `Ui` centred in the heading's height.
pub fn head_cell<R>(ui: &mut Ui, x: Rangef, add: impl FnOnce(&mut Ui) -> R) -> R {
    // The heading's face sits a pixel high in its strip.
    let r = Rect::from_x_y_ranges(x, ui.max_rect().y_range()).translate(Vec2::new(0.0, -1.0));
    let mut c = ui.new_child(
        UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt(("head-cell", x.min as i32)),
    );
    c.spacing_mut().item_spacing.x = 8.0;
    add(&mut c)
}

/// A heading that does not sort (`MODS` in some lists): the heading's face.
pub fn head_label(ui: &mut Ui, text: &str) -> Response {
    let g = head_galley(ui, text);
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    draw(
        ui.painter(),
        &g,
        rect.left(),
        rect.center().y,
        ui.tokens().fg_faint,
    );
    resp
}

/// `SortHead`: a column heading that sorts by it; the arrow says which way.
#[must_use = "add it with ui.add(…)"]
pub struct SortHead<'a> {
    label: &'a str,
    active: bool,
    asc: bool,
    tip: &'a str,
}

impl<'a> SortHead<'a> {
    pub fn new(label: &'a str, active: bool, asc: bool) -> Self {
        Self {
            label,
            active,
            asc,
            tip: "",
        }
    }

    pub fn tip(mut self, t: &'a str) -> Self {
        self.tip = t;
        self
    }
}

impl egui::Widget for SortHead<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let g = head_galley(ui, self.label);
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(g.size().x + 4.0 + 12.0, 16.0), Sense::click());
        let h = paint::ease(ui, resp.id.with("hover"), resp.hovered());
        let ink = if self.active {
            tk.accent
        } else {
            paint::lerp(tk.fg_faint, tk.fg, h)
        };
        let p = ui.painter();
        draw(p, &g, rect.left(), rect.center().y, ink);
        let ir = Rect::from_min_size(
            Pos2::new(rect.left() + g.size().x + 4.0, rect.center().y - 6.0),
            Vec2::splat(12.0),
        );
        match (self.active, self.asc) {
            (false, _) => lucide::ARROW_UP_DOWN.paint(p, ir, ink.gamma_multiply(0.4)),
            (true, true) => lucide::ARROW_UP.paint(p, ir, ink),
            (true, false) => lucide::ARROW_DOWN.paint(p, ir, ink),
        }
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(ui.painter(), rect, size::RADIUS_XS, &tk);
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if self.tip.is_empty() {
            resp
        } else {
            resp.tip_kbd(self.tip, "", Side::Top)
        }
    }
}

fn game_version_id() -> Id {
    Id::new("dzch.game_version")
}

/// Keep the installed game's version where every server row finds it (the app sets it
/// when the game store learns it).
pub fn set_game_version(ctx: &egui::Context, v: Option<&str>) {
    let v: Option<Arc<str>> = v.map(Arc::from);
    ctx.data_mut(|d| d.insert_temp(game_version_id(), v));
}

fn installed_version(ctx: &egui::Context) -> Option<Arc<str>> {
    ctx.data(|d| d.get_temp::<Option<Arc<str>>>(game_version_id()))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_line_up() {
        let c = ListColumns::new(Rangef::new(208.0, 1400.0), false);
        assert_eq!(c.star.min, 216.0);
        assert_eq!(c.ping.min, 252.0);
        assert_eq!(c.players.min, 340.0);
        assert_eq!(c.server.min, 452.0);
        assert_eq!(c.map.map(|m| m.min), Some(1040.0));
        assert_eq!(c.mods.min, 1188.0);
        assert_eq!(c.actions.max, 1392.0);
        let n = ListColumns::new(Rangef::new(0.0, 600.0), true);
        assert!(n.narrow() && n.extra.is_some());
        let w = ListColumns::new(Rangef::new(0.0, 1000.0), true);
        assert!((w.actions.max - 992.0).abs() < 0.01);
    }

    #[test]
    fn the_server_column_gives_way_last() {
        // Too tight for the map beside a readable name: the map goes, not the name.
        let c = ListColumns::new(Rangef::new(0.0, 820.0), false);
        assert!(c.narrow());
        assert!(c.server.span() >= MAP_MIN_SERVER);
        let h = ListColumns::new(Rangef::new(0.0, 820.0), true);
        assert!(h.narrow() && h.server.span() >= 200.0);
        // Room enough: the map stays.
        let m = ListColumns::new(Rangef::new(0.0, 1000.0), false);
        assert!(m.map.is_some() && m.server.span() >= MAP_MIN_SERVER);
        // A very wide list caps the name's column; the actions keep the right edge.
        let w = ListColumns::new(Rangef::new(0.0, 1900.0), false);
        assert!((w.server.span() - SERVER_MAX).abs() < 0.01);
        assert_eq!(w.map.map(|m| m.min), Some(w.server.max + COL_GAP));
        assert!((w.actions.max - 1892.0).abs() < 0.01);
    }

    fn frame(
        ctx: &egui::Context,
        row: &dz_hub::features::browser::ServerRow,
        cache: &mut RowCache,
    ) {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let cols = ListColumns::new(Rangef::new(0.0, 1200.0), false);
            let _ = ServerListRow::new(RowData::from_row(row)).show(ui, &cols, cache);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn a_steady_row_is_laid_out_once() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::theme::fonts::definitions());
        let mut row = dz_hub::features::browser::ServerRow {
            game_port: 2302,
            ip: dz_api::Ipv4Addr::new(45, 0, 0, 0),
            query_port: 2303,
            name: "4100 | EUROPE - DE".into(),
            map: "chernarusplus".into(),
            players: 41,
            max_players: 60,
            environment: "w".into(),
            password: false,
            version: "1.28".into(),
            first_person_only: false,
            time: "17:40".into(),
            mods_count: 3,
            mods_missing: Some(0),
            mods_stale: 0,
            vac: true,
            battl_eye: Some(true),
            bots: 0,
            ping_ms: Some(92),
            ping_failed: false,
            favorite: false,
            excluded: false,
            unverified_full: false,
            official: true,
            mimics_official: false,
        };
        let mut cache = RowCache::new();
        frame(&ctx, &row, &mut cache);
        frame(&ctx, &row, &mut cache);
        let key = RowData::from_row(&row).key();
        let first = cache.rows.get(&key).map(|l| Arc::clone(&l.name));
        frame(&ctx, &row, &mut cache);
        let again = cache.rows.get(&key).map(|l| Arc::clone(&l.name));
        assert_eq!(cache.len(), 1);
        assert!(matches!((&first, &again), (Some(a), Some(b)) if Arc::ptr_eq(a, b)));
        // A new ping lays that row out again.
        row.ping_ms = Some(40);
        frame(&ctx, &row, &mut cache);
        let after = cache.rows.get(&key).map(|l| Arc::clone(&l.ping));
        assert!(after.is_some_and(|g| g.text() == "40"));
    }
}
