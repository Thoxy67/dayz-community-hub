//! The status bar (= `StatusBar.svelte` and `status/*.svelte`): the window's instrument
//! panel in two groups. On the left, what is happening: the server list, the ping scan,
//! the mods' downloads and their state, the offline mode's install, the launcher's own
//! download; on the right, the state of things: the game, BattlEye, Steam, an update, the
//! version.
//!
//! Every segment has the same anatomy ([`Segment`]): an icon, then pieces (figures in mono,
//! words in sans, dots, meters) on **one baseline**, with one gap between pieces; running
//! work shows an inline meter with its figures. Segments of one kind sit together; a thin
//! separator marks where the group changes. A bar too narrow for everything drops pieces
//! and then whole segments by rank ([`rank`]): the version first, then words, then the
//! least needed segments; what still does not fit on the left is left out whole, never cut
//! through a figure. Figures that count up keep their widest width, so nothing jumps.
//!
//! Nothing here ticks for nothing: ages and the game's time are re-read every 30 s (one
//! repaint asked for then), the Steam client and BattlEye are asked with that clock while
//! the window has focus (every 5 s for BattlEye while DayZ runs); a sweep repaints at
//! 30 fps only while it shows, and not at all with motion off.

use std::sync::Arc;
use std::time::Duration;

use egui::{
    Color32, CornerRadius, CursorIcon, Galley, Id, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2,
};

use crate::app::{Ctx, reduced_motion};
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::{Align, Brand, MenuItem, Side, TipExt, menu, paint};
use crate::parts::text::{baseline, draw_on};
use crate::rt::{self, Task};
use crate::store::mods::{ModOpPhase, ModOpVia};
use crate::store::pad::InputMode;
use crate::store::servers::RefreshPhase;
use crate::store::updater::UpdateState;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};
use crate::views::ViewId;
use dz_game::battleye::BattlEyeState;
use dz_hub::features::stats::PlayKind;
use dz_hub::features::steamcmd::ModOpType;
use dz_hub::features::steamworks::ModDownloaderDto;

/// The bar's clock: ages, the game's time, Steam's and BattlEye's state.
const CLOCK: f64 = 30.0;
/// BattlEye is asked this often while DayZ runs.
const BE_EVERY: f64 = 5.0;
/// How long a running game may go without BattlEye's service before it is a fault.
const BE_GRACE: f64 = 20.0;
/// No flash while the window opens.
const ARM_AFTER: f64 = 1.5;
/// A flash's length (`animate-flash`).
const FLASH: f64 = 0.9;
/// Frames per second of a sweep.
const ANIM_FPS: f32 = 30.0;

/// A segment's padding on each side, inside its hover.
const PAD: f32 = 6.0;
/// The icons' size.
const ICON: f32 = 12.0;
/// From the icon to the first piece.
const ICON_GAP: f32 = 5.0;
/// Between two pieces (a figure and its word, a word and a meter).
const GAP: f32 = 4.0;
/// Between two segments of one group.
const SEG_GAP: f32 = 2.0;
/// Between two groups (a separator in the middle).
const GROUP_GAP: f32 = 13.0;
/// The bar's ends.
const EDGE: f32 = 6.0;
/// A segment's hover, and a small button's side.
const BODY_H: f32 = 22.0;
const BUTTON: f32 = 20.0;
/// A dot's diameter.
const DOT: f32 = 6.0;
/// An inline meter.
const METER_W: f32 = 36.0;
const METER_H: f32 = 4.0;
/// A separator's height.
const SEP_H: f32 = 14.0;
/// Digits' height as a share of the size, for icons, dots and meters to sit on the
/// middle of the figures rather than on the middle of the line box.
const CAP: f32 = 0.72;

/// When a piece or a segment is dropped from a narrow bar: lower goes first.
mod rank {
    pub const VERSION: u8 = 10;
    pub const LIST_NOTE: u8 = 14;
    pub const PING_AGAIN_WORD: u8 = 18;
    pub const STEAM_WORD: u8 = 20;
    pub const BE_WORD: u8 = 22;
    pub const MOD_NAME: u8 = 24;
    pub const GAME_WORD: u8 = 26;
    pub const LIST_STEAM_WORD: u8 = 28;
    pub const CHECKING_WORD: u8 = 30;
    pub const BEHIND_WORD: u8 = 32;
    pub const SERVERS_WORD: u8 = 34;
    pub const UPDATE_WORD: u8 = 36;
    pub const WORK_WORD: u8 = 40;
    pub const PING_WORD: u8 = 42;
    pub const GAME_NAME: u8 = 44;
    pub const PING_AGAIN: u8 = 50;
    pub const CHECKING: u8 = 52;
    pub const STEAM: u8 = 54;
    pub const BATTLEYE: u8 = 56;
}

/// The longest server name the game segment shows before `…`.
const GAME_NAME_MAX: f32 = 220.0;
/// The longest mod name a download shows before `…`.
const MOD_NAME_MAX: f32 = 160.0;

/// What the bar keeps between frames.
#[derive(Default)]
pub(crate) struct StatusState {
    opened_at: Option<f64>,
    next_clock: f64,
    steam: Option<Task<Option<bool>>>,
    steam_running: Option<bool>,
    be_next: f64,
    /// When the game was first seen running (egui time).
    game_seen: Option<f64>,
    flash_total: Option<(u32, f64)>,
    flash_behind: Option<(usize, f64)>,
}

/// How a segment reads at a glance (= `Segment.svelte`'s `Tone`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Idle,
    Accent,
    Info,
    Ok,
    Warn,
    Err,
}

impl Tone {
    fn icon(self, t: &Tokens) -> Color32 {
        match self {
            Tone::Idle => t.fg_muted,
            Tone::Accent => t.accent,
            Tone::Info => t.info,
            Tone::Ok => t.ok,
            Tone::Warn => t.warn,
            Tone::Err => t.err,
        }
    }

    /// Words' colour; figures are brighter on a calm segment (`fg`), in the tone's colour
    /// on a loud one.
    fn text(self, t: &Tokens) -> Color32 {
        match self {
            Tone::Warn => t.warn,
            Tone::Err => t.err,
            Tone::Ok => t.ok,
            _ => t.fg_muted,
        }
    }

    fn figure(self, t: &Tokens) -> Color32 {
        match self {
            Tone::Warn | Tone::Err | Tone::Ok => self.text(t),
            _ => t.fg,
        }
    }

    /// A meter's fill.
    fn fill(self, t: &Tokens) -> Color32 {
        match self {
            Tone::Idle => t.fg_muted,
            other => other.icon(t),
        }
    }
}

#[derive(Clone, Copy)]
enum SegIcon {
    None,
    Icon(Icon),
    Steam,
}

/// Work measured or not: a sweep (nothing to measure), or a fraction 0–1.
#[derive(Clone, Copy, PartialEq)]
enum Progress {
    Sweep,
    At(f32),
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Dot(Color32),
    Meter(Progress),
    /// A faint `·` between the main figure and a note.
    Sep,
}

/// One piece of a segment.
struct Piece {
    kind: Kind,
    text: String,
    mono: bool,
    /// Its own colour (else the segment's).
    color: Option<Color32>,
    /// Dropped from a narrow bar at this rank.
    drop: Option<u8>,
    /// Cut with `…` past this width.
    max_w: Option<f32>,
    /// Never narrower than this text (a figure that counts up does not jump).
    room: Option<String>,
    /// Flashes since (egui time).
    flash: Option<f64>,
}

impl Piece {
    fn text(s: impl Into<String>) -> Self {
        Self {
            kind: Kind::Text,
            text: s.into(),
            mono: false,
            color: None,
            drop: None,
            max_w: None,
            room: None,
            flash: None,
        }
    }
    fn mono(s: impl Into<String>) -> Self {
        Self {
            mono: true,
            ..Self::text(s)
        }
    }
    fn dot(c: Color32) -> Self {
        Self {
            kind: Kind::Dot(c),
            ..Self::text("")
        }
    }
    fn meter(p: Progress) -> Self {
        Self {
            kind: Kind::Meter(p),
            ..Self::text("")
        }
    }
    fn sep() -> Self {
        Self {
            kind: Kind::Sep,
            ..Self::text("·")
        }
    }
    fn drop(mut self, rank: u8) -> Self {
        self.drop = Some(rank);
        self
    }
    fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }
    fn max(mut self, w: f32) -> Self {
        self.max_w = Some(w);
        self
    }
    fn room(mut self, widest: impl Into<String>) -> Self {
        self.room = Some(widest.into());
        self
    }
    fn flash(mut self, at: Option<f64>) -> Self {
        self.flash = at;
        self
    }
    fn shown(&self, level: u8) -> bool {
        self.drop.is_none_or(|r| r > level)
    }
}

/// What a click on a segment (or one of its small buttons) does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Act {
    Refresh,
    GoServers,
    Scan,
    ScanPause,
    ReopenOp,
    Go(ViewId, Option<&'static str>),
    RepairBe,
    SteamMenu,
    Command(&'static str),
}

struct Segment {
    /// Its id among the bar's segments.
    key: &'static str,
    icon: SegIcon,
    tone: Tone,
    tip: String,
    act: Option<Act>,
    pieces: Vec<Piece>,
    /// Dropped from a narrow bar at this rank.
    drop: Option<u8>,
    /// Segments of one group sit together; a separator marks a change of group.
    group: u8,
    /// Small icon buttons beside it: icon, label, action.
    actions: Vec<(Icon, &'static str, Act)>,
}

impl Segment {
    fn new(key: &'static str, icon: SegIcon, tone: Tone, tip: String) -> Self {
        Self {
            key,
            icon,
            tone,
            tip,
            act: None,
            pieces: Vec::new(),
            drop: None,
            group: 0,
            actions: Vec::new(),
        }
    }
    fn act(mut self, a: Act) -> Self {
        self.act = Some(a);
        self
    }
    fn act_if(mut self, a: Option<Act>) -> Self {
        self.act = a;
        self
    }
    fn piece(mut self, p: Piece) -> Self {
        self.pieces.push(p);
        self
    }
    fn drop(mut self, rank: u8) -> Self {
        self.drop = Some(rank);
        self
    }
    fn group(mut self, g: u8) -> Self {
        self.group = g;
        self
    }
    fn shown(&self, level: u8) -> bool {
        self.drop.is_none_or(|r| r > level)
    }
    fn moving(&self) -> bool {
        self.pieces
            .iter()
            .any(|p| p.kind == Kind::Meter(Progress::Sweep))
    }
}

/// Where the line's pieces sit: the words' baseline, and the middle of the figures (for
/// icons, dots and meters).
#[derive(Clone, Copy)]
struct Line {
    base: f32,
    mid: f32,
}

impl Line {
    fn new(ui: &Ui, cy: f32) -> Self {
        let g = ui.painter().layout_no_wrap(
            "0".to_owned(),
            font(Face::Sans, size::TEXT_2XS),
            Color32::PLACEHOLDER,
        );
        let base = cy - g.size().y / 2.0 + baseline(&g);
        Self {
            base,
            mid: (base - size::TEXT_2XS * CAP / 2.0).round(),
        }
    }
}

pub(super) fn show(ui: &mut Ui, cx: &mut Ctx, st: &mut StatusState) {
    let tk = ui.tokens();
    let rect = ui.max_rect();
    let now = ui.input(|i| i.time);
    // (The top border is the panel's separator line.)
    clock(ui, cx, st, now);

    let line = Line::new(ui, rect.center().y);
    let left = left_segments(cx, st, now);
    let right = right_segments(cx, st, now, &tk);

    // The level: how much a narrow bar drops (0: nothing).
    let level = fit(ui, &left, &right, rect.width());

    let mut acts: Vec<Act> = Vec::new();
    let mut seps: Vec<f32> = Vec::new();
    // Whether a drawn segment sweeps (a dropped one asks for no frame).
    let mut sweeping = false;

    // Right group, from the right edge inwards.
    let shown_right = shown_widths(ui, &right, level);
    let mut x = rect.right() - EDGE;
    let mut placed = Vec::with_capacity(shown_right.len());
    let mut prev_group: Option<u8> = None;
    for &(i, w) in shown_right.iter().rev() {
        let Some(s) = right.get(i) else { continue };
        if let Some(g) = prev_group {
            if g != s.group {
                x -= GROUP_GAP;
                seps.push(x + GROUP_GAP / 2.0);
            } else {
                x -= SEG_GAP;
            }
        }
        prev_group = Some(s.group);
        x -= w;
        placed.push((s, x));
    }
    let right_start = x;
    for (s, sx) in placed.into_iter().rev() {
        sweeping |= s.moving();
        let (a, resp) = draw(ui, s, sx, line, level, now, &tk);
        acts.extend(a);
        if s.act == Some(Act::SteamMenu) {
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    Id::unique(("dzch.shell.menu", "steam")),
                    egui::Popup::default_response_id(&resp),
                )
            });
            steam_menu(ui, cx, &resp);
        }
    }
    // The divider between the two halves.
    let div_x = right_start - GROUP_GAP / 2.0;
    seps.push(div_x);

    // Left group, from the left edge: a segment that does not fit whole is left out
    // (with the ones after it), never cut through.
    let limit = div_x - GROUP_GAP / 2.0;
    let mut x = rect.left() + EDGE;
    let mut prev_group: Option<u8> = None;
    for (i, w) in shown_widths(ui, &left, level) {
        let Some(s) = left.get(i) else { continue };
        let mut sx = x;
        let mut sep = None;
        if let Some(g) = prev_group {
            if g != s.group {
                sep = Some(sx + GROUP_GAP / 2.0);
                sx += GROUP_GAP;
            } else {
                sx += SEG_GAP;
            }
        }
        if sx + w > limit {
            break;
        }
        seps.extend(sep);
        prev_group = Some(s.group);
        sweeping |= s.moving();
        let (a, _) = draw(ui, s, sx, line, level, now, &tk);
        acts.extend(a);
        x = sx + w;
    }

    let p = ui.painter();
    for sx in seps {
        p.vline(
            sx.round() + 0.5,
            (line.mid - SEP_H / 2.0)..=(line.mid + SEP_H / 2.0),
            Stroke::new(1.0, tk.border),
        );
    }

    // Animations: a sweep asks for its next frame; a flash until it fades.
    let moving = sweeping && !reduced_motion(ui.ctx());
    if moving {
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f32(1.0 / ANIM_FPS));
    }
    for at in [st.flash_total.map(|f| f.1), st.flash_behind.map(|f| f.1)]
        .into_iter()
        .flatten()
    {
        if now - at < FLASH {
            ui.ctx().request_repaint();
        }
    }

    for a in acts {
        run(cx, a);
    }
}

/// The segments shown at `level`, by index, with their widths.
fn shown_widths(ui: &Ui, segs: &[Segment], level: u8) -> Vec<(usize, f32)> {
    segs.iter()
        .enumerate()
        .filter(|(_, s)| s.shown(level))
        .map(|(i, s)| (i, width(ui, s, level)))
        .collect()
}

/// The lowest level at which both halves fit `bar_w` (the highest there is when nothing
/// does: the left half is then cut by whole segments).
fn fit(ui: &Ui, left: &[Segment], right: &[Segment], bar_w: f32) -> u8 {
    let mut ranks: Vec<u8> = left
        .iter()
        .chain(right.iter())
        .flat_map(|s| {
            s.drop
                .into_iter()
                .chain(s.pieces.iter().filter_map(|p| p.drop))
        })
        .collect();
    ranks.sort_unstable();
    ranks.dedup();
    let need = |level: u8| -> f32 {
        let half = |segs: &[Segment]| {
            let mut w = 0.0;
            let mut prev: Option<u8> = None;
            for s in segs.iter().filter(|s| s.shown(level)) {
                if let Some(g) = prev {
                    w += if g == s.group { SEG_GAP } else { GROUP_GAP };
                }
                prev = Some(s.group);
                w += width(ui, s, level);
            }
            w
        };
        EDGE * 2.0 + half(left) + GROUP_GAP + half(right)
    };
    let mut level = 0;
    for r in ranks {
        if need(level) <= bar_w {
            break;
        }
        level = r;
    }
    level
}

fn piece_font(p: &Piece, tone: Tone) -> egui::FontId {
    // Mono for figures, medium on good news (`text-ok font-medium`).
    let face = match (p.mono, tone == Tone::Ok) {
        (true, true) => Face::MonoMedium,
        (true, false) => Face::Mono,
        (false, true) => Face::SansMedium,
        (false, false) => Face::Sans,
    };
    font(face, size::TEXT_2XS)
}

/// A piece's text, laid out once in the placeholder colour (coloured when drawn).
fn piece_galley(ui: &Ui, p: &Piece, tone: Tone) -> Arc<Galley> {
    let f = piece_font(p, tone);
    match p.max_w {
        Some(w) => paint::galley_trunc(ui, &p.text, f, Color32::PLACEHOLDER, w.max(1.0)),
        None => paint::galley(ui, &p.text, f, Color32::PLACEHOLDER),
    }
}

fn piece_width(ui: &Ui, p: &Piece, tone: Tone) -> f32 {
    match p.kind {
        Kind::Dot(_) => DOT,
        Kind::Meter(_) => METER_W,
        Kind::Text | Kind::Sep => {
            let w = piece_galley(ui, p, tone).size().x;
            match &p.room {
                Some(r) => w.max(
                    paint::galley(ui, r, piece_font(p, tone), Color32::PLACEHOLDER)
                        .size()
                        .x,
                ),
                None => w,
            }
        }
    }
}

fn actions_width(s: &Segment) -> f32 {
    if s.actions.is_empty() {
        0.0
    } else {
        BUTTON * s.actions.len() as f32
    }
}

/// The segment's own width at `level` (its small buttons included).
fn width(ui: &Ui, s: &Segment, level: u8) -> f32 {
    let icon = !matches!(s.icon, SegIcon::None);
    let mut w = PAD + if icon { ICON } else { 0.0 };
    for (n, p) in s.pieces.iter().filter(|p| p.shown(level)).enumerate() {
        w += match (n, icon) {
            (0, true) => ICON_GAP,
            (0, false) => 0.0,
            _ => GAP,
        };
        w += piece_width(ui, p, s.tone);
    }
    w + PAD + actions_width(s)
}

/// Draws a segment with its left edge at `x`; returns the action clicked, with the
/// segment's response (for a menu to open from).
fn draw(
    ui: &mut Ui,
    s: &Segment,
    x: f32,
    line: Line,
    level: u8,
    now: f64,
    tk: &Tokens,
) -> (Option<Act>, egui::Response) {
    let total_w = width(ui, s, level);
    let body = Rect::from_min_size(
        pos2(x, line.mid - BODY_H / 2.0),
        vec2(total_w - actions_width(s), BODY_H),
    );
    let id = Id::unique(("sb-seg", s.key));
    let sense = if s.act.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(body, id, sense);
    let lit = s.act.is_some() && (resp.hovered() || menu::is_open(&resp));
    let h = paint::ease(ui, id.with("h"), lit);
    let still = reduced_motion(ui.ctx());
    let p = ui.painter();
    p.rect_filled(
        body,
        CornerRadius::same(size::RADIUS_XS),
        paint::lerp(Color32::TRANSPARENT, tk.raised, h),
    );
    // Under the pointer, a clickable segment's words brighten.
    let lift = |c: Color32| {
        if s.act.is_some() {
            paint::lerp(c, tk.fg, h)
        } else {
            c
        }
    };
    let mut px = body.left() + PAD;
    let mut first = true;
    match s.icon {
        SegIcon::None => {}
        SegIcon::Icon(i) => {
            i.paint(p, icon_rect(px, line), s.tone.icon(tk));
            px += ICON;
            first = false;
        }
        SegIcon::Steam => {
            Brand::Steam.paint(p, icon_rect(px, line), s.tone.icon(tk));
            px += ICON;
            first = false;
        }
    }
    let after_icon = !first;
    for (n, piece) in s.pieces.iter().filter(|p| p.shown(level)).enumerate() {
        if !first {
            px += if after_icon && n == 0 { ICON_GAP } else { GAP };
        }
        first = false;
        let pw = piece_width(ui, piece, s.tone);
        match piece.kind {
            Kind::Dot(c) => {
                p.circle_filled(pos2(px + DOT / 2.0, line.mid), DOT / 2.0, c);
            }
            Kind::Meter(m) => meter(ui, id, m, px, line, s.tone, now, still, tk),
            Kind::Text | Kind::Sep => {
                let ink = match (piece.kind, piece.color) {
                    (Kind::Sep, _) => tk.fg_faint,
                    (_, Some(c)) => lift(c),
                    (_, None) if piece.mono => lift(s.tone.figure(tk)),
                    (_, None) => lift(s.tone.text(tk)),
                };
                let g = piece_galley(ui, piece, s.tone);
                // A figure with room to spare keeps it after itself: it stays beside
                // its meter or word, and what follows does not move as it counts.
                let gx = px;
                if let Some(at) = piece.flash {
                    let k = ((now - at) / FLASH) as f32;
                    if (0.0..1.0).contains(&k) && !still {
                        let fr = Rect::from_center_size(
                            pos2(gx + g.size().x / 2.0, line.mid),
                            vec2(g.size().x + 4.0, BODY_H - 6.0),
                        );
                        p.rect_filled(
                            fr,
                            CornerRadius::same(size::RADIUS_XS),
                            paint::alpha(tk.accent, 0.35 * (1.0 - k)),
                        );
                    }
                }
                draw_on(p, &g, gx, line.base, ink);
            }
        }
        px += pw;
    }
    let mut clicked = None;
    // The small buttons beside it.
    let mut bx = body.right();
    for (i, (icon, label, act)) in s.actions.iter().enumerate() {
        let r = Rect::from_min_size(pos2(bx, line.mid - BUTTON / 2.0), Vec2::splat(BUTTON));
        let b = ui
            .interact(r, id.with(("act", i)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let bh = paint::ease(ui, b.id, b.hovered());
        let p = ui.painter();
        p.rect_filled(
            r,
            CornerRadius::same(size::RADIUS_XS),
            paint::lerp(Color32::TRANSPARENT, tk.raised, bh),
        );
        icon.paint(
            p,
            Rect::from_center_size(r.center(), Vec2::splat(ICON)),
            paint::lerp(tk.fg_muted, tk.fg, bh),
        );
        super::ring(ui, &b, r, size::RADIUS_XS);
        if b.tip_kbd(label, "", Side::Top).clicked() {
            clicked = Some(*act);
        }
        bx += BUTTON;
    }
    super::ring(ui, &resp, body, size::RADIUS_XS);
    let resp = if s.act.is_some() {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    };
    let resp = if menu::is_open(&resp) {
        resp
    } else {
        resp.tip_kbd(&s.tip, "", Side::Top)
    };
    if let (Some(a), true) = (s.act, resp.clicked())
        && a != Act::SteamMenu
    {
        clicked = Some(a);
    }
    (clicked, resp)
}

/// An icon's square, on the line's middle.
fn icon_rect(x: f32, line: Line) -> Rect {
    Rect::from_min_size(pos2(x, line.mid - ICON / 2.0), Vec2::splat(ICON))
}

/// An inline meter: a track in the tone's colour, filled to the fraction, or a band
/// sweeping across it when nothing can be measured.
#[allow(clippy::too_many_arguments)]
fn meter(
    ui: &Ui,
    id: Id,
    m: Progress,
    x: f32,
    line: Line,
    tone: Tone,
    now: f64,
    still: bool,
    tk: &Tokens,
) {
    let track = Rect::from_min_size(pos2(x, line.mid - METER_H / 2.0), vec2(METER_W, METER_H));
    let fill = tone.fill(tk);
    let r = CornerRadius::same(size::RADIUS_XS);
    let p = ui.painter();
    p.rect_filled(track, r, paint::alpha(fill, 0.22));
    match m {
        Progress::At(f) => {
            let f = ui.ctx().animate_value_with_time(
                id.with("meter"),
                f.clamp(0.0, 1.0),
                crate::theme::motion::BASE,
            );
            let w = (track.width() * f).max(if f > 0.0 { METER_H } else { 0.0 });
            if w > 0.0 {
                p.rect_filled(Rect::from_min_size(track.min, vec2(w, METER_H)), r, fill);
            }
        }
        Progress::Sweep => {
            let band = track.width() / 3.0;
            // `animate-sweep`: from -100% to 300% of the band in 1.6 s; still, a band
            // in the middle.
            let k = if still {
                0.5
            } else {
                ((now % 1.6) / 1.6) as f32
            };
            let left = track.left() - band + k * (track.width() + band);
            let band_r = Rect::from_min_max(
                pos2(left.max(track.left()), track.top()),
                pos2((left + band).min(track.right()), track.bottom()),
            );
            if band_r.width() > 0.0 {
                p.rect_filled(band_r, r, fill);
            }
        }
    }
}

fn run(cx: &mut Ctx, a: Act) {
    match a {
        Act::Refresh => {
            let _ = cx.run("servers.refresh", &serde_json::Value::Null);
        }
        Act::GoServers => cx.go(ViewId::Servers),
        Act::Scan => {
            let _ = cx.run("servers.scan", &serde_json::Value::Null);
        }
        Act::ScanPause => {
            let _ = cx.run("servers.scan_pause", &serde_json::Value::Null);
        }
        Act::ReopenOp => {
            let _ = cx.run("mods.reopen_op", &serde_json::Value::Null);
        }
        Act::Go(v, Some(f)) => cx.go_focus(v, f),
        Act::Go(v, None) => cx.go(v),
        Act::RepairBe => {
            let _ = cx.run("battleye.repair", &serde_json::Value::Null);
        }
        Act::Command(id) => {
            let _ = cx.run(id, &serde_json::Value::Null);
        }
        Act::SteamMenu => {}
    }
}

/// The bar's clock, and what it asks for.
fn clock(ui: &Ui, cx: &mut Ctx, st: &mut StatusState, now: f64) {
    let ctx = ui.ctx();
    let opened = *st.opened_at.get_or_insert(now);
    if now - opened < ARM_AFTER {
        ctx.request_repaint_after(Duration::from_secs_f64(ARM_AFTER - (now - opened) + 0.01));
    }
    if let Some(task) = &mut st.steam
        && let Some(r) = task.poll()
    {
        st.steam = None;
        st.steam_running = r;
    }
    let focused = cx.stores.app.focused;
    let running = cx.stores.game.running;
    if now >= st.next_clock {
        st.next_clock = now + CLOCK;
        if focused && cx.stores.app.initialized && st.steam.is_none() {
            let hub = cx.hub.clone();
            st.steam = Some(rt::spawn(async move {
                hub.steamworks_status().await.ok().map(|s| s.steam_running)
            }));
        }
    }
    ctx.request_repaint_after(Duration::from_secs_f64((st.next_clock - now).max(0.05)));
    // BattlEye: with the clock while focused, every few seconds while DayZ runs.
    if running {
        if st.game_seen.is_none() {
            st.game_seen = Some(now);
            st.be_next = now;
        }
    } else {
        st.game_seen = None;
    }
    let every = if running { BE_EVERY } else { CLOCK };
    if cx.stores.app.initialized && (focused || running) && now >= st.be_next {
        st.be_next = now + every;
        cx.stores.battleye.refresh();
        // An update of the game while the launcher is open changes its version.
        cx.stores.game.read_version();
    }
    if running || focused {
        ctx.request_repaint_after(Duration::from_secs_f64((st.be_next - now).max(0.05)));
    }
    // Flashes: a figure that changes after the window opened.
    let armed = now - opened >= ARM_AFTER;
    let total = cx.stores.servers.total;
    match st.flash_total {
        Some((v, _)) if v == total => {}
        _ => st.flash_total = Some((total, if armed { now } else { f64::NEG_INFINITY })),
    }
    let behind = cx.stores.mods.stale_count();
    match st.flash_behind {
        Some((v, _)) if v == behind => {}
        _ => st.flash_behind = Some((behind, if armed { now } else { f64::NEG_INFINITY })),
    }
}

/// `done/total` as the bar writes it, with the room its widest value takes.
fn count(done: impl Into<f64>, total: impl Into<f64>) -> Piece {
    let total = format::num(total.into());
    Piece::mono(format!("{}/{total}", format::num(done.into()))).room(format!("{total}/{total}"))
}

/// A whole percent, with the room `100%` takes.
fn percent(p: u32) -> Piece {
    Piece::mono(format!("{p}%")).room("100%")
}

fn left_segments(cx: &Ctx, st: &StatusState, now: f64) -> Vec<Segment> {
    let mut out = Vec::with_capacity(4);
    out.push(list_segment(cx, st));
    if let Some(s) = scan_segment(cx) {
        out.push(s);
    }
    out.extend(mod_segments(cx, st, now));
    if crate::views::offline::installing(cx.egui) {
        let tip = t("shell", "sbOfflineTip").to_string();
        out.push(
            Segment::new("offline", SegIcon::Icon(lucide::MAP), Tone::Info, tip)
                .act(Act::Go(ViewId::Offline, None))
                .group(2)
                .piece(Piece::text(t("shell", "sbOffline")).drop(rank::WORK_WORD))
                .piece(Piece::meter(Progress::Sweep))
                .piece(Piece::text(t("offline", "installing")).drop(rank::WORK_WORD)),
        );
    }
    let u = &cx.stores.updater;
    if u.state == UpdateState::Downloading {
        let v = u.info.as_ref().map_or("", |i| i.version.as_str());
        let tip = tf(
            "shell",
            "sbUpdatingTip",
            &[("version", &v), ("percent", &u.percent())],
        );
        let mut s = Segment::new("updating", SegIcon::Icon(lucide::DOWNLOAD), Tone::Info, tip)
            .act(Act::Command("about.open_update"))
            .group(2)
            .piece(Piece::text(t("shell", "sbUpdating")).drop(rank::WORK_WORD));
        if u.total > 0 {
            s = s
                .piece(Piece::meter(Progress::At(u.percent() as f32 / 100.0)))
                .piece(percent(u.percent()));
        } else {
            s = s.piece(Piece::meter(Progress::Sweep));
        }
        out.push(s);
    }
    out
}

/// The server list, always there (= `ListSegment.svelte`): the count, then what the list
/// is doing (fetching, the Steam listing with its meter) or how old it is.
fn list_segment(cx: &Ctx, st: &StatusState) -> Segment {
    let s = &cx.stores.servers;
    let down = s.list_error.is_some() && s.total == 0;
    let age = if s.last_refreshed > 0 {
        format::relative(s.last_refreshed / 1000)
    } else {
        String::new()
    };
    let failed = s
        .last_refresh
        .as_ref()
        .filter(|_| !s.refreshing)
        .filter(|r| r.phase == RefreshPhase::Failed)
        .map(|r| r.error.clone().unwrap_or_default());
    let steam = cx
        .stores
        .sources
        .now
        .as_ref()
        .filter(|n| n.steam.phase == dz_hub::events::SourcePhase::Running)
        .map(|n| (n.steam.requests_done, n.steam.requests_total, n.steam.added));
    let icon = SegIcon::Icon(if down {
        lucide::CLOUD_OFF
    } else {
        lucide::SERVER
    });
    let count = |seg: Segment| {
        seg.piece(Piece::mono(format::num(f64::from(s.total))).flash(st.flash_total.map(|f| f.1)))
            .piece(Piece::text(t("shell", "sbServersWord")).drop(rank::SERVERS_WORD))
    };
    if down {
        let e = s.list_error.clone().unwrap_or_default();
        return Segment::new(
            "list",
            icon,
            Tone::Err,
            tf("shell", "sbListFailedTip", &[("error", &e)]),
        )
        .act(Act::Refresh)
        .piece(Piece::text(t("nav", "sbListDown")));
    }
    if s.refreshing {
        return count(Segment::new(
            "list",
            icon,
            Tone::Accent,
            t("shell", "sbFetchingTip").to_string(),
        ))
        .piece(Piece::meter(Progress::Sweep))
        .piece(Piece::text(t("shell", "sbFetching")).drop(rank::LIST_NOTE));
    }
    if let Some(e) = failed {
        return count(
            Segment::new(
                "list",
                icon,
                Tone::Err,
                tf("shell", "sbListFailedTip", &[("error", &e)]),
            )
            .act(Act::Refresh),
        )
        .piece(Piece::sep().drop(rank::LIST_NOTE))
        .piece(Piece::text(t("shell", "sbListFailed")).drop(rank::LIST_NOTE));
    }
    if let Some((done, total, added)) = steam {
        let tip = tf(
            "shell",
            "sbSteamListingTip",
            &[
                ("done", &format::num(f64::from(done))),
                ("total", &format::num(f64::from(total))),
                ("added", &format::num(f64::from(added))),
            ],
        );
        let seg = count(Segment::new("list", icon, Tone::Accent, tip).act(Act::GoServers))
            .piece(Piece::sep().drop(rank::LIST_STEAM_WORD))
            .piece(Piece::text("Steam").drop(rank::LIST_STEAM_WORD));
        return if total > 0 {
            let f = (done as f32 / total as f32).clamp(0.0, 1.0);
            seg.piece(Piece::meter(Progress::At(f)))
                .piece(percent((f * 100.0).floor() as u32))
        } else {
            seg.piece(Piece::meter(Progress::Sweep))
        };
    }
    let stale = s.stale();
    let key = if stale { "sbListStaleTip" } else { "sbListTip" };
    let tip = tf(
        "shell",
        key,
        &[("count", &format::num(f64::from(s.total))), ("age", &age)],
    );
    let tone = if stale { Tone::Warn } else { Tone::Idle };
    let mut seg = count(Segment::new("list", icon, tone, tip).act(Act::Refresh));
    if !age.is_empty() {
        seg = seg
            .piece(Piece::sep().drop(rank::LIST_NOTE))
            .piece(Piece::text(age).drop(rank::LIST_NOTE));
    }
    seg
}

/// The ping scan (= `ScanSegment.svelte`).
fn scan_segment(cx: &Ctx) -> Option<Segment> {
    let s = &cx.stores.servers;
    if let Some((done, total)) = s.scan {
        let paused = s.scan_paused;
        let args: [(&str, &dyn std::fmt::Display); 2] = [
            ("done", &format::num(f64::from(done))),
            ("total", &format::num(f64::from(total))),
        ];
        let tip = tf(
            "shell",
            if paused {
                "sbPingPausedTip"
            } else {
                "sbPingTip"
            },
            &args,
        );
        let mut seg = Segment::new(
            "scan",
            SegIcon::Icon(lucide::RADAR),
            if paused { Tone::Warn } else { Tone::Accent },
            tip,
        )
        .piece(
            Piece::text(t("nav", if paused { "scanPaused" } else { "sbPing" }))
                .drop(rank::PING_WORD),
        );
        if total > 0 {
            seg = seg.piece(Piece::meter(Progress::At(done as f32 / total as f32)));
        }
        seg = seg.piece(count(done, total));
        seg.actions = vec![
            if paused {
                (lucide::PLAY, t("nav", "resumeScan"), Act::ScanPause)
            } else {
                (lucide::PAUSE, t("nav", "pauseScan"), Act::ScanPause)
            },
            (lucide::ROTATE_CCW, t("nav", "sbRescan"), Act::Scan),
        ];
        Some(seg)
    } else if s.total > 0 && !s.refreshing {
        Some(
            Segment::new(
                "scan",
                SegIcon::Icon(lucide::RADAR),
                Tone::Idle,
                t("nav", "sbRescan").to_string(),
            )
            .act(Act::Scan)
            .drop(rank::PING_AGAIN)
            .piece(Piece::text(t("nav", "sbPingAgain")).drop(rank::PING_AGAIN_WORD)),
        )
    } else {
        None
    }
}

/// The mods' side of the work (= `ModSegments.svelte`).
fn mod_segments(cx: &Ctx, st: &StatusState, _now: f64) -> Vec<Segment> {
    let mut out = Vec::new();
    let mods = &cx.stores.mods;
    if mods.busy() {
        let op = &mods.op;
        let waiting = matches!(
            op.phase,
            ModOpPhase::SteamGuardMobile | ModOpPhase::PasswordRequired
        );
        let kind = match op.kind {
            ModOpType::Login => t("progress", "kindLogin"),
            ModOpType::Repair => t("progress", "kindRepair"),
            ModOpType::InstallServer | ModOpType::InstallManual => t("progress", "kindInstall"),
            _ => t("progress", "kindUpdate"),
        };
        let via = if op.via == ModOpVia::Steamworks {
            "Steam"
        } else {
            "SteamCMD"
        };
        let tip = if waiting {
            t("shell", "sbModWaitingTip").to_string()
        } else {
            tf(
                "shell",
                "sbModOpTip",
                &[
                    ("kind", &kind),
                    ("via", &via),
                    ("name", &op.current_name),
                    ("current", &op.current),
                    ("total", &op.total),
                ],
            )
        };
        let mut seg = Segment::new(
            "modop",
            SegIcon::Icon(if waiting {
                lucide::KEY_ROUND
            } else {
                lucide::HARD_DRIVE_DOWNLOAD
            }),
            if waiting { Tone::Warn } else { Tone::Info },
            tip,
        )
        .act(Act::ReopenOp)
        .group(1);
        if waiting {
            seg = seg.piece(Piece::text(t("shell", "sbModWaiting")));
        } else {
            seg = seg.piece(Piece::text(kind).drop(rank::WORK_WORD));
            if op.total > 0 {
                let f = (op.ok + op.failed) as f32 / op.total as f32;
                seg = seg
                    .piece(Piece::meter(Progress::At(f)))
                    .piece(count(op.current as f64, op.total as f64));
            } else {
                seg = seg.piece(Piece::meter(Progress::Sweep));
            }
            if !op.current_name.is_empty() {
                seg = seg.piece(
                    Piece::text(op.current_name.clone())
                        .max(MOD_NAME_MAX)
                        .drop(rank::MOD_NAME),
                );
            }
        }
        out.push(seg);
    } else {
        let missing = !cx.stores.profile.via_steam()
            && cx
                .stores
                .servers
                .stats
                .as_ref()
                .is_some_and(|s| !s.has_steamcmd);
        if missing {
            out.push(
                Segment::new(
                    "steamcmd",
                    SegIcon::Icon(lucide::TRIANGLE_ALERT),
                    Tone::Warn,
                    t("shell", "sbSteamcmdMissingTip").to_string(),
                )
                .act(Act::Command("settings.open_steam"))
                .group(1)
                .piece(Piece::text(t("nav", "sbSteamcmdMissing")).drop(rank::WORK_WORD)),
            );
        }
    }
    let behind = mods.stale_count();
    if mods.checking {
        out.push(
            Segment::new(
                "mods",
                SegIcon::Icon(lucide::PUZZLE),
                Tone::Info,
                t("shell", "sbModsCheckingTip").to_string(),
            )
            .group(1)
            .drop(rank::CHECKING)
            .piece(Piece::meter(Progress::Sweep))
            .piece(Piece::text(t("nav", "sbModsChecking")).drop(rank::CHECKING_WORD)),
        );
    } else if behind > 0 {
        let count = i64::try_from(behind).unwrap_or(i64::MAX);
        out.push(
            Segment::new(
                "mods",
                SegIcon::Icon(lucide::PUZZLE),
                Tone::Warn,
                tf("shell", "sbModsBehindTip", &[("count", &count)]),
            )
            .act(Act::Go(ViewId::Mods, None))
            .group(1)
            .piece(Piece::mono(behind.to_string()).flash(st.flash_behind.map(|f| f.1)))
            .piece(Piece::text(t("shell", "sbModsBehind")).drop(rank::BEHIND_WORD)),
        );
    }
    out
}

fn right_segments(cx: &Ctx, st: &StatusState, now: f64, tk: &Tokens) -> Vec<Segment> {
    let mut out = Vec::with_capacity(5);
    if let Some(s) = game_segment(cx, tk) {
        out.push(s);
    }
    if let Some(s) = battleye_segment(cx, st, now, tk) {
        out.push(s);
    }
    out.push(steam_segment(cx, st, tk));
    let u = &cx.stores.updater;
    match (u.state, &u.info) {
        (UpdateState::Available, Some(info)) => {
            // A copy that cannot update itself (a package, a dev build) is only told.
            let in_place = u.support.as_ref().is_none_or(|s| s.supported);
            let key = if in_place {
                "sbUpdateTip"
            } else {
                "sbUpdateElsewhereTip"
            };
            let tip = tf("shell", key, &[("version", &info.version)]);
            let mut seg = Segment::new("update", SegIcon::Icon(lucide::DOWNLOAD), Tone::Ok, tip)
                .act(Act::Command("about.open_update"))
                .group(2);
            // `v{version} available`: the version is a figure, the rest words (in the
            // language's order).
            let tpl = t("nav", "sbUpdate");
            let figure = format!("v{}", info.version);
            match tpl.split_once("v{version}") {
                Some((pre, post)) => {
                    if !pre.trim().is_empty() {
                        seg = seg.piece(Piece::text(pre.trim()).drop(rank::UPDATE_WORD));
                    }
                    seg = seg.piece(Piece::mono(figure));
                    if !post.trim().is_empty() {
                        seg = seg.piece(Piece::text(post.trim()).drop(rank::UPDATE_WORD));
                    }
                }
                None => {
                    seg = seg.piece(Piece::mono(tf(
                        "nav",
                        "sbUpdate",
                        &[("version", &info.version)],
                    )));
                }
            }
            out.push(seg);
        }
        (UpdateState::Done, _) => out.push(
            Segment::new(
                "update",
                SegIcon::Icon(lucide::REFRESH_CW),
                Tone::Ok,
                t("shell", "sbUpdateReadyTip").to_string(),
            )
            .act(Act::Command("about.open_update"))
            .group(2)
            .piece(Piece::text(t("shell", "sbUpdateReady")).drop(rank::UPDATE_WORD)),
        ),
        _ => {}
    }
    // The version, last: a door to About.
    let v = tf("nav", "version", &[("version", &dz_hub::VERSION)]);
    out.push(
        Segment::new(
            "version",
            SegIcon::None,
            Tone::Idle,
            tf("shell", "sbVersionTip", &[("version", &v)]),
        )
        .act(Act::Command("view.about"))
        .group(2)
        .drop(rank::VERSION)
        .piece(Piece::mono(v).color(tk.fg_muted)),
    );
    out
}

/// The game while it runs: what is played (the server's name, cut with `…`) and for how
/// long; a click shows that server.
fn game_segment(cx: &Ctx, tk: &Tokens) -> Option<Segment> {
    let g = &cx.stores.game;
    if !g.running {
        return None;
    }
    let pad = cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) == InputMode::Gamepad;
    let played = g.played_secs().map(format::duration);
    let session = g.session.as_ref();
    let name = session
        .filter(|s| s.kind != PlayKind::Unknown && !s.name.trim().is_empty())
        .map(|s| s.name.trim().to_owned());
    let mut tip = Vec::with_capacity(4);
    match &name {
        Some(n) => tip.push(tf("shell", "sbGameOnServer", &[("name", n)])),
        None if played.is_none() => tip.push(t("shell", "sbGameOn").to_string()),
        None => {}
    }
    if let Some(p) = &played {
        tip.push(tf("shell", "sbGameFor", &[("time", p)]));
    }
    tip.push(
        t(
            "shell",
            match session.map(|s| s.kind) {
                Some(PlayKind::Server) if session.is_some_and(|s| s.ip.is_some()) => {
                    "sbGameDetailsAction"
                }
                Some(PlayKind::Offline) => "sbGameOfflineAction",
                _ => "sbGameStatsAction",
            },
        )
        .to_string(),
    );
    tip.push(t("shell", if pad { "sbGameQuitPad" } else { "sbGameQuit" }).to_string());
    let mut s = Segment::new(
        "game",
        SegIcon::Icon(lucide::GAMEPAD_2),
        Tone::Ok,
        tip.join(" "),
    )
    .act(Act::Command("game.details"));
    s = match name {
        Some(n) => s.piece(
            Piece::text(n)
                .max(GAME_NAME_MAX)
                .color(tk.fg)
                .drop(rank::GAME_NAME),
        ),
        None => s.piece(Piece::text("DayZ").drop(rank::GAME_WORD)),
    };
    if let Some(p) = played {
        s = s.piece(Piece::mono(p));
    }
    Some(s)
}

/// BattlEye (= `BattlEyeSegment.svelte`).
fn battleye_segment(cx: &Ctx, st: &StatusState, now: f64, tk: &Tokens) -> Option<Segment> {
    let b = &cx.stores.battleye;
    let status = b.status.as_ref()?;
    let be = status.state;
    if be == BattlEyeState::NoGame {
        return None;
    }
    let service = status.service;
    let repairing = b.repairing;
    let not_started = service
        && be != BattlEyeState::Running
        && st.game_seen.is_some_and(|seen| now - seen > BE_GRACE);
    let broken = matches!(be, BattlEyeState::Missing | BattlEyeState::Disabled);
    let tone = if repairing {
        Tone::Info
    } else if not_started {
        Tone::Err
    } else if broken {
        Tone::Warn
    } else {
        Tone::Idle
    };
    let icon = if not_started {
        lucide::SHIELD_X
    } else if broken {
        lucide::SHIELD_ALERT
    } else {
        lucide::SHIELD_CHECK
    };
    let tip = if repairing {
        t("shell", "sbBeRepairing").to_string()
    } else {
        let first = if not_started {
            t("shell", "sbBeNotStarted")
        } else {
            match be {
                BattlEyeState::Running => t("shell", "sbBeRunning"),
                BattlEyeState::Disabled => t("shell", "sbBeDisabled"),
                BattlEyeState::Missing if service => t("shell", "sbBeMissing"),
                BattlEyeState::Missing => t("shell", "sbBeMissingProton"),
                _ if service => t("shell", "sbBeReady"),
                _ => t("shell", "sbBeReadyProton"),
            }
        };
        let action = if be == BattlEyeState::Running && !not_started {
            ""
        } else if service {
            t("shell", "sbBeRepairAction")
        } else if broken {
            t("shell", "sbBeInstallAction")
        } else {
            ""
        };
        if action.is_empty() {
            first.to_string()
        } else {
            format!("{first} {action}")
        }
    };
    let acts = !repairing && be != BattlEyeState::Running && (service || broken);
    let loud = not_started || broken;
    let mut seg = Segment::new("battleye", SegIcon::Icon(icon), tone, tip)
        .act_if(acts.then_some(Act::RepairBe))
        .group(1)
        .piece(Piece::text("BattlEye").drop(if loud { rank::WORK_WORD } else { rank::BE_WORD }));
    if !loud && !repairing {
        seg = seg.drop(rank::BATTLEYE);
    }
    seg = if repairing {
        seg.piece(Piece::meter(Progress::Sweep))
    } else if loud {
        seg.piece(Piece::text(t("shell", "sbBeOff")))
    } else if !service {
        // Under Proton the runtime being there is all that can be seen (not whether
        // BattlEye started with the game): installed is the good state.
        seg.piece(Piece::text(t("shell", "sbBeRuntime")).drop(rank::BE_WORD))
            .piece(Piece::dot(tk.ok))
    } else {
        seg.piece(Piece::dot(if be == BattlEyeState::Running {
            tk.ok
        } else {
            tk.fg_faint
        }))
    };
    Some(seg)
}

/// Steam (= `SteamSegment.svelte`): what downloads the mods, whether the client runs.
fn steam_segment(cx: &Ctx, st: &StatusState, tk: &Tokens) -> Segment {
    let via = cx.stores.profile.via_steam();
    let login = cx
        .stores
        .profile
        .data
        .as_ref()
        .and_then(|p| p.steamcmd_logged_in.clone());
    let running = st.steam_running;
    let mut tip = if via {
        t("shell", "sbSteamClient").to_string()
    } else if let Some(l) = &login {
        tf("shell", "sbSteamCmdAs", &[("login", l)])
    } else {
        t("shell", "sbSteamCmdNoLogin").to_string()
    };
    match running {
        Some(true) => {
            tip.push(' ');
            tip.push_str(t("shell", "sbSteamUp"));
        }
        Some(false) => {
            tip.push(' ');
            tip.push_str(t("shell", "sbSteamDown"));
        }
        None => {}
    }
    tip.push(' ');
    tip.push_str(t("shell", "sbSteamAction"));
    let down = running == Some(false);
    let mut seg = Segment::new(
        "steam",
        SegIcon::Steam,
        if down { Tone::Warn } else { Tone::Idle },
        tip,
    )
    .act(Act::SteamMenu)
    .group(1)
    .piece(Piece::text(if via { "Steam" } else { "SteamCMD" }).drop(rank::STEAM_WORD))
    .piece(Piece::dot(match running {
        Some(true) => tk.ok,
        Some(false) => tk.warn,
        None => tk.fg_faint,
    }));
    if !down {
        seg = seg.drop(rank::STEAM);
    }
    seg
}

/// The Steam segment's menu: what downloads the mods, and the Steam settings.
fn steam_menu(ui: &Ui, cx: &mut Ctx, resp: &egui::Response) {
    let tk = ui.tokens();
    let current = cx
        .stores
        .profile
        .data
        .as_ref()
        .map_or(ModDownloaderDto::Steamcmd, |p| p.mod_downloader);
    let busy = cx.stores.mods.busy();
    let mut pick: Option<ModDownloaderDto> = None;
    let mut settings = false;
    menu::dropdown(resp, Side::Top, Align::End, |ui| {
        ui.set_min_width(240.0 - 8.0);
        menu::label(ui, t("nav", "sbDownloader"));
        for (value, label, hint) in [
            (
                ModDownloaderDto::Steamcmd,
                "SteamCMD",
                t("nav", "sbSteamcmdHint"),
            ),
            (
                ModDownloaderDto::Steamworks,
                t("settings", "downloaderSteam"),
                t("nav", "sbSteamHint"),
            ),
        ] {
            let on = current == value;
            let enabled = !(busy && !on);
            // A fixed width: the popup's room is the window's, and the hint truncates.
            let (r, item) = ui.allocate_exact_size(
                vec2(240.0 - 8.0, 36.0),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            let lit = enabled && (item.hovered() || item.has_focus());
            let p = ui.painter();
            if lit {
                p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.raised);
            }
            let a = if enabled { 1.0 } else { paint::DISABLED };
            if on {
                lucide::CHECK.paint(
                    p,
                    Rect::from_center_size(
                        pos2(r.left() + 6.0 + 7.0, r.center().y),
                        Vec2::splat(12.0),
                    ),
                    tk.accent.gamma_multiply(a),
                );
            }
            let tx = r.left() + 6.0 + 14.0 + 8.0;
            let ink = if on || lit { tk.fg } else { tk.fg_muted };
            let g = paint::galley(
                ui,
                label,
                font(Face::Sans, size::TEXT_XS),
                ink.gamma_multiply(a),
            );
            paint::text_at(p, &g, tx, r.top() + 4.0 + 8.0, ink.gamma_multiply(a));
            let hint = if busy && !on {
                t("nav", "sbDownloaderBusy")
            } else {
                hint
            };
            let hg = paint::galley_trunc(
                ui,
                hint,
                font(Face::Sans, size::TEXT_3XS),
                tk.fg_faint,
                (r.right() - 6.0 - tx).max(0.0),
            );
            paint::text_at(
                p,
                &hg,
                tx,
                r.bottom() - 4.0 - 7.0,
                tk.fg_faint.gamma_multiply(a),
            );
            if item.clicked() && !on {
                pick = Some(value);
            }
        }
        menu::separator(ui);
        if ui
            .add(MenuItem::new(t("nav", "sbSteamSettings")).icon(lucide::SETTINGS))
            .clicked()
        {
            settings = true;
        }
    });
    if let Some(v) = pick {
        cx.stores.profile.set_mod_downloader(v);
    }
    if settings {
        cx.go_focus(ViewId::Settings, "steam");
    }
}
