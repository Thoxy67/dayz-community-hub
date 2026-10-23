//! The cells every server list shares, standalone (a detail panel, a card) and as the
//! painters [`super::server_row`] draws its rows with: the flags (`ServerFlags`), the
//! in-game clock (`TimeOfDay`), the OS mark (`OsIcon`), the mods count (`ModsCount`), the
//! ping (`PingButton`), the players (`PlayersButton`), the star (`FavoriteButton`), the
//! ban (`ExcludeButton`), Join (`JoinButton`) and the shortcuts sheet's key
//! (`ShortcutsButton`). They take plain values and answer with a `Response`; what a click
//! means (ping again, star, join) is the caller's.

use std::sync::Arc;

use egui::{
    Color32, CornerRadius, Galley, Id, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2, Widget, WidgetInfo, WidgetType,
};

use super::text::{draw, ph};
use crate::i18n::{t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha, lerp};
use crate::kit::{
    Brand, Button, IconButton, PING_TIMEOUT_MS, Side, SignalSize, Size, TipExt, Variant, fill_tone,
    signal_color, signal_level,
};
use crate::kit::{Players, Signal, Tone};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

// ---------------------------------------------------------------------------------------
// Flags

/// What a server is and asks of you before you join (`ServerFlags` props).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flags {
    pub password: bool,
    pub first_person: bool,
    pub battleye: bool,
    pub saved_password: bool,
    pub official: bool,
    /// A community server named like an official one.
    pub mimics_official: bool,
}

impl Flags {
    pub fn any(&self) -> bool {
        self.password
            || self.first_person
            || self.battleye
            || self.saved_password
            || self.official
            || self.mimics_official
    }
}

/// The badges' words, laid out once (they change only with the language).
pub(crate) struct FlagWords {
    official: Arc<Galley>,
    mimics: Arc<Galley>,
    fpp: Arc<Galley>,
}

impl FlagWords {
    pub(crate) fn new(ui: &Ui) -> Self {
        Self {
            official: ph(
                ui,
                t("servers", "officialBadge"),
                font(Face::MonoSemibold, size::TEXT_3XS),
            ),
            mimics: ph(
                ui,
                t("servers", "mimicsOfficial"),
                font(Face::Sans, size::TEXT_3XS),
            ),
            fpp: ph(ui, "1PP", font(Face::MonoSemibold, size::TEXT_3XS)),
        }
    }
}

/// `gap-1.5` between a row's title items.
pub(crate) const ITEM_GAP: f32 = 6.0;

enum FlagItem<'w> {
    Official(&'w Arc<Galley>),
    Mimics(&'w Arc<Galley>),
    Lock,
    Key,
    Fpp(&'w Arc<Galley>),
    Shield,
}

fn flag_items<'w>(f: &Flags, w: &'w FlagWords, mut each: impl FnMut(FlagItem<'w>)) {
    if f.official {
        each(FlagItem::Official(&w.official));
    } else if f.mimics_official {
        each(FlagItem::Mimics(&w.mimics));
    }
    if f.password {
        each(FlagItem::Lock);
    }
    if f.saved_password {
        each(FlagItem::Key);
    }
    if f.first_person {
        each(FlagItem::Fpp(&w.fpp));
    }
    if f.battleye {
        each(FlagItem::Shield);
    }
}

fn item_width(item: &FlagItem<'_>) -> f32 {
    match item {
        // px-1, a 10px badge-check, gap-0.5, the word.
        FlagItem::Official(g) => 4.0 + 10.0 + 2.0 + g.size().x + 4.0,
        // The same, with a 1px border.
        FlagItem::Mimics(g) => 1.0 + 4.0 + 10.0 + 2.0 + g.size().x + 4.0 + 1.0,
        FlagItem::Lock | FlagItem::Key | FlagItem::Shield => 12.0,
        FlagItem::Fpp(g) => 4.0 + g.size().x + 4.0,
    }
}

/// The width the flags take, gaps between them included (none before or after).
pub(crate) fn flags_width(f: &Flags, w: &FlagWords) -> f32 {
    let mut total = 0.0;
    let mut n = 0;
    flag_items(f, w, |i| {
        total += item_width(&i);
        n += 1;
    });
    if n > 1 {
        total += ITEM_GAP * (n - 1) as f32;
    }
    total
}

/// Draws the flags from `x`, centred on `cy`, each with its hint; returns the end.
pub(crate) fn paint_flags(ui: &Ui, x: f32, cy: f32, f: &Flags, w: &FlagWords, id: Id) -> f32 {
    let tk = ui.tokens();
    let p = ui.painter();
    let mut x = x;
    let mut first = true;
    let mut k = 0u8;
    flag_items(f, w, |item| {
        if !first {
            x += ITEM_GAP;
        }
        first = false;
        let iw = item_width(&item);
        let (rect, tip) = match item {
            FlagItem::Official(g) => {
                let r = Rect::from_min_size(Pos2::new(x, cy - 6.0), Vec2::new(iw, 12.0));
                p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), alpha(tk.ok, 0.15));
                lucide::BADGE_CHECK.paint(
                    p,
                    Rect::from_min_size(Pos2::new(x + 4.0, cy - 5.0), Vec2::splat(10.0)),
                    tk.ok,
                );
                draw(p, g, x + 16.0, cy, tk.ok);
                (r, t("servers", "officialTitle"))
            }
            FlagItem::Mimics(g) => {
                let r = Rect::from_min_size(Pos2::new(x, cy - 7.0), Vec2::new(iw, 14.0));
                p.rect_stroke(
                    r,
                    CornerRadius::same(size::RADIUS_XS),
                    Stroke::new(1.0, alpha(tk.warn, 0.4)),
                    StrokeKind::Inside,
                );
                lucide::TRIANGLE_ALERT.paint(
                    p,
                    Rect::from_min_size(Pos2::new(x + 5.0, cy - 5.0), Vec2::splat(10.0)),
                    tk.warn,
                );
                draw(p, g, x + 17.0, cy, tk.warn);
                (r, t("servers", "mimicsOfficialTitle"))
            }
            FlagItem::Lock => {
                let r = Rect::from_min_size(Pos2::new(x, cy - 6.0), Vec2::splat(12.0));
                lucide::LOCK.paint(p, r, tk.err);
                (r, t("servers", "passwordProtected"))
            }
            FlagItem::Key => {
                let r = Rect::from_min_size(Pos2::new(x, cy - 6.0), Vec2::splat(12.0));
                lucide::KEY_ROUND.paint(p, r, tk.ok);
                (r, t("servers", "savedPassword"))
            }
            FlagItem::Fpp(g) => {
                let r = Rect::from_min_size(Pos2::new(x, cy - 6.0), Vec2::new(iw, 12.0));
                p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), alpha(tk.warn, 0.15));
                draw(p, g, x + 4.0, cy, tk.warn);
                (r, t("servers", "firstPerson"))
            }
            FlagItem::Shield => {
                let r = Rect::from_min_size(Pos2::new(x, cy - 6.0), Vec2::splat(12.0));
                lucide::SHIELD_CHECK.paint(p, r, tk.info);
                (r, t("servers", "battleye"))
            }
        };
        let _ = ui
            .interact(rect, id.with(("flag", k)), Sense::hover())
            .tip_kbd(tip, "", Side::Top);
        k = k.wrapping_add(1);
        x += iw;
    });
    x
}

/// `ServerFlags`: `ui.add(ServerFlags::new(flags))`.
#[must_use = "add it with ui.add(…)"]
pub struct ServerFlags {
    flags: Flags,
}

impl ServerFlags {
    pub fn new(flags: Flags) -> Self {
        Self { flags }
    }
}

impl Widget for ServerFlags {
    fn ui(self, ui: &mut Ui) -> Response {
        let words = FlagWords::new(ui);
        let w = flags_width(&self.flags, &words);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 14.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            paint_flags(
                ui,
                rect.left(),
                rect.center().y,
                &self.flags,
                &words,
                resp.id,
            );
        }
        resp
    }
}

// ---------------------------------------------------------------------------------------
// Time of day

/// The phase of an in-game clock (`"17:40"`): sunrise at dawn and dusk, sun by day, moon
/// by night; an unreadable hour is dawn, as in the Svelte part.
pub fn day_phase(time: &str) -> Icon {
    let digits: String = time
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    match digits.parse::<u32>() {
        Ok(h) if (5..7).contains(&h) || (19..21).contains(&h) => lucide::SUNRISE,
        Ok(h) if (7..19).contains(&h) => lucide::SUN,
        Ok(_) => lucide::MOON,
        Err(_) => lucide::SUNRISE,
    }
}

/// Draws the clock (icon 12px faint, the time in mono `fg-muted`) from `x`; returns the end.
pub(crate) fn paint_time(p: &Painter, x: f32, cy: f32, phase: Icon, g: &Arc<Galley>, t: &Tokens) {
    phase.paint(
        p,
        Rect::from_min_size(Pos2::new(x, cy - 6.0), Vec2::splat(12.0)),
        t.fg_faint,
    );
    draw(p, g, x + 12.0 + 4.0, cy, t.fg_muted);
}

/// `TimeOfDay`: a server's in-game clock with dawn, day, dusk or night beside it.
/// `.small()` is the lists' `text-3xs`.
#[must_use = "add it with ui.add(…)"]
pub struct TimeOfDay<'a> {
    time: &'a str,
    small: bool,
}

impl<'a> TimeOfDay<'a> {
    pub fn new(time: &'a str) -> Self {
        Self { time, small: false }
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
}

impl Widget for TimeOfDay<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let px = if self.small {
            size::TEXT_3XS
        } else {
            size::TEXT_2XS
        };
        let text = if self.time.is_empty() {
            crate::i18n::format::NONE
        } else {
            self.time
        };
        let g = ph(ui, text, font(Face::Mono, px));
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(16.0 + g.size().x, 14.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            paint_time(
                ui.painter(),
                rect.left(),
                rect.center().y,
                day_phase(self.time),
                &g,
                &t,
            );
        }
        resp
    }
}

// ---------------------------------------------------------------------------------------
// OS

/// The OS mark of a server's `environment`: "w" is Windows, anything else Linux.
pub fn os_brand(environment: &str) -> Brand {
    if environment == "w" {
        Brand::Windows
    } else {
        Brand::Linux
    }
}

/// Its name, for the hint.
pub fn os_label(environment: &str) -> &'static str {
    if environment == "w" {
        t("servers", "osWindows")
    } else {
        t("servers", "osLinux")
    }
}

/// `OsIcon`: the server's operating system, `fg-faint`, 14px (`.size(12.0)` in lists).
#[must_use = "add it with ui.add(…)"]
pub struct OsIcon<'a> {
    environment: &'a str,
    size: f32,
}

impl<'a> OsIcon<'a> {
    pub fn new(environment: &'a str) -> Self {
        Self {
            environment,
            size: 14.0,
        }
    }

    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
}

impl Widget for OsIcon<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::hover());
        if ui.is_rect_visible(rect) {
            os_brand(self.environment).paint(ui.painter(), rect, ui.tokens().fg_faint);
        }
        resp.tip_kbd(os_label(self.environment), "", Side::Top)
    }
}

// ---------------------------------------------------------------------------------------
// Mods

/// How a server's mods stand against yours: `mods` colour while yours are not known, red
/// when some are missing, orange when some are behind, green when all are ready.
pub fn mods_tone(t: &Tokens, missing: Option<u32>, stale: u32) -> Color32 {
    match missing {
        None => t.mods,
        Some(m) if m > 0 => t.err,
        Some(_) if stale > 0 => t.warn,
        Some(_) => t.ok,
    }
}

/// The hint: what the colour means.
pub fn mods_tip(count: usize, missing: Option<u32>, stale: u32) -> String {
    let Some(missing) = missing else {
        return t("servers", "showMods").to_owned();
    };
    let mut parts: Vec<String> = Vec::new();
    if missing > 0 {
        parts.push(tf(
            "servers",
            "modsMissingTip",
            &[("missing", &missing), ("count", &count)],
        ));
    }
    if stale > 0 {
        parts.push(tf("servers", "modsStaleTip", &[("stale", &stale)]));
    }
    if parts.is_empty() {
        parts.push(tf("servers", "modsAllReady", &[("count", &count)]));
    }
    parts.join(" · ")
}

/// The "−n" or "↑n" after the count, if any.
pub fn mods_extra(missing: Option<u32>, stale: u32) -> Option<String> {
    match missing {
        Some(m) if m > 0 => Some(format!("−{m}")),
        _ if stale > 0 => Some(format!("↑{stale}")),
        _ => None,
    }
}

/// The width of a mods button with these galleys (`px-1`, puzzle, `gap-1`).
pub(crate) fn mods_width(count: &Galley, extra: Option<&Arc<Galley>>) -> f32 {
    4.0 + 12.0 + 4.0 + count.size().x + extra.map_or(0.0, |e| 4.0 + e.size().x) + 4.0
}

/// Draws a mods button in `rect` (16px tall), lit by `hover` (0–1) when clickable.
pub(crate) fn paint_mods(
    p: &Painter,
    rect: Rect,
    count: &Arc<Galley>,
    extra: Option<&Arc<Galley>>,
    tone: Color32,
    hover: f32,
    t: &Tokens,
) {
    if hover > 0.0 {
        p.rect_filled(
            rect,
            CornerRadius::same(size::RADIUS_XS),
            alpha(t.mods, 0.10 * hover),
        );
    }
    let cy = rect.center().y;
    let mut x = rect.left() + 4.0;
    lucide::PUZZLE.paint(
        p,
        Rect::from_min_size(Pos2::new(x, cy - 6.0), Vec2::splat(12.0)),
        tone,
    );
    x += 16.0;
    draw(p, count, x, cy, tone);
    x += count.size().x + 4.0;
    if let Some(e) = extra {
        draw(p, e, x, cy, tone);
    }
}

/// The dash of an empty cell (`text-fg-faint/60`).
pub(crate) fn dash_color(t: &Tokens) -> Color32 {
    alpha(t.fg_faint, 0.6)
}

/// `ModsCount`: how many mods a server runs and whether yours are ready for it; a button
/// when `.clickable(true)` (there is a list to show). A dash when it runs none.
#[must_use = "add it with ui.add(…)"]
pub struct ModsCount {
    count: usize,
    missing: Option<u32>,
    stale: u32,
    clickable: bool,
}

impl ModsCount {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            missing: None,
            stale: 0,
            clickable: false,
        }
    }

    /// How many are not installed; `None` while the mods on disk are not known.
    pub fn missing(mut self, m: Option<u32>) -> Self {
        self.missing = m;
        self
    }

    pub fn stale(mut self, n: u32) -> Self {
        self.stale = n;
        self
    }

    pub fn clickable(mut self, on: bool) -> Self {
        self.clickable = on;
        self
    }
}

impl Widget for ModsCount {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        if self.count == 0 {
            let g = ph(
                ui,
                crate::i18n::format::NONE,
                font(Face::Sans, size::TEXT_XS),
            );
            let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
            draw(
                ui.painter(),
                &g,
                rect.left(),
                rect.center().y,
                dash_color(&t),
            );
            return resp;
        }
        let f = font(Face::Mono, size::TEXT_2XS);
        let count = ph(ui, &self.count.to_string(), f);
        let extra = mods_extra(self.missing, self.stale)
            .map(|e| ph(ui, &e, font(Face::MonoSemibold, size::TEXT_2XS)));
        let w = mods_width(&count, extra.as_ref());
        let sense = if self.clickable {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 16.0), sense);
        if ui.is_rect_visible(rect) {
            let hover = paint::ease(ui, resp.id.with("hover"), self.clickable && resp.hovered());
            paint_mods(
                ui.painter(),
                rect,
                &count,
                extra.as_ref(),
                mods_tone(&t, self.missing, self.stale),
                hover,
                &t,
            );
        }
        let tip = mods_tip(self.count, self.missing, self.stale);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &tip));
        let resp = if self.clickable {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        };
        resp.tip_kbd(&tip, "", Side::Top)
    }
}

// ---------------------------------------------------------------------------------------
// Ping

/// The signal's geometry at `Sm` (as `kit::Signal`): bars 3px, 2px apart, 12px tall.
pub(crate) const BARS_W: f32 = 4.0 * 3.0 + 3.0 * 2.0;

/// Draws a `Sm` signal's bars (and its timeout stroke) from `x`, centred on `cy`; returns
/// the tone of its figure. `time` drives the pending pulse.
pub(crate) fn paint_bars(
    p: &Painter,
    x: f32,
    cy: f32,
    ms: Option<u32>,
    pending: bool,
    time: f64,
    t: &Tokens,
) -> Color32 {
    let (h, bw, gap) = (12.0, 3.0, 2.0);
    let level = signal_level(ms);
    let timed_out = ping_timed_out(ms, pending);
    let tone = if pending {
        t.fg_faint
    } else {
        signal_color(t, level)
    };
    let bottom = cy + h / 2.0;
    for b in 1..=4u8 {
        let bh = h * f32::from(b) * 0.25;
        let bx = x + f32::from(b - 1) * (bw + gap);
        let r = Rect::from_min_max(Pos2::new(bx, bottom - bh), Pos2::new(bx + bw, bottom));
        let a = if pending {
            let ph = ((time - f64::from(b) * 0.12) / 2.0).rem_euclid(1.0) as f32;
            0.4 * (0.75 + 0.25 * (ph * std::f32::consts::TAU).cos())
        } else if b <= level {
            1.0
        } else {
            0.2
        };
        p.rect_filled(r, CornerRadius::same(1), tone.gamma_multiply(a));
    }
    if timed_out {
        // No answer is common (a server down, a firewall) and says nothing bad about the
        // player's connection: a faint stroke and a faint dash, not an alarm. Red stays
        // for slow answers.
        let c = Pos2::new(x + BARS_W / 2.0, bottom - h / 2.0);
        let d = Vec2::angled((-25.0_f32).to_radians()) * (BARS_W / 2.0 + 1.0);
        p.line_segment([c - d, c + d], Stroke::new(1.0, t.fg_faint));
        return dash_color(t);
    }
    tone
}

/// The server did not answer its last ping (at or past `PING_TIMEOUT_MS`, the backend's
/// sentinel for a failure), and none is under way.
pub fn ping_timed_out(ms: Option<u32>, pending: bool) -> bool {
    !pending && ms.is_some_and(|m| m >= PING_TIMEOUT_MS)
}

/// The figure a signal shows: `…` pending, `—` never asked or no answer, else the ms.
pub fn ping_text(ms: Option<u32>, pending: bool) -> (String, bool) {
    if pending {
        ("…".to_owned(), false)
    } else {
        match ms {
            Some(m) if m < PING_TIMEOUT_MS => (m.to_string(), true),
            _ => (crate::i18n::format::NONE.to_owned(), false),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Version

/// Whether a server's version is the player's: "1.28.162391" against "1.28.162391", or
/// against a file version with a zero between ("1.28.0.162391"). Compares the numbers
/// (major, minor, build) and falls back on the trimmed text when they are not numbers.
pub fn same_version(a: &str, b: &str) -> bool {
    fn parts(v: &str) -> Option<Vec<u64>> {
        let n: Option<Vec<u64>> = v.trim().split('.').map(|p| p.parse().ok()).collect();
        n.map(|mut n| {
            // A file version's third field is a zero the servers' version leaves out.
            if n.len() == 4 && n.get(2) == Some(&0) {
                n.remove(2);
            }
            n
        })
    }
    match (parts(a), parts(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a.trim() == b.trim(),
    }
}

/// `PingButton`: the server's signal; a click asks again (the caller pings). `warn` marks a
/// "full" the last query could not confirm.
#[must_use = "add it with ui.add(…)"]
pub struct PingCell {
    ms: Option<u32>,
    pending: bool,
    warn: bool,
    size: SignalSize,
    label: bool,
}

impl PingCell {
    pub fn new(ms: Option<u32>) -> Self {
        Self {
            ms,
            pending: false,
            warn: false,
            size: SignalSize::Sm,
            label: true,
        }
    }

    pub fn pending(mut self, p: bool) -> Self {
        self.pending = p;
        self
    }

    pub fn warn(mut self, w: bool) -> Self {
        self.warn = w;
        self
    }

    pub fn size(mut self, s: SignalSize) -> Self {
        self.size = s;
        self
    }

    /// Without the figure.
    pub fn bars_only(mut self) -> Self {
        self.label = false;
        self
    }
}

impl Widget for PingCell {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let slot = ui.painter().add(egui::Shape::Noop);
        let rect = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(2.0);
                let mut s = Signal::new(self.ms).pending(self.pending).size(self.size);
                if !self.label {
                    s = s.bars_only();
                }
                ui.add(s);
                if self.warn {
                    ui.add_space(4.0);
                    let r = lucide::TRIANGLE_ALERT.show(ui, 12.0, tk.warn);
                    let _ = r.tip_kbd(t("servers", "playerCountUnverified"), "", Side::Top);
                }
                ui.add_space(2.0);
            })
            .response
            .rect;
        let resp = ui.interact(rect, ui.next_auto_id().with("ping"), Sense::click());
        hover_plate(ui, &resp, rect, slot);
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(t("servers", "clickPing"), "", Side::Top)
    }
}

/// `hover:bg-raised` under a cell button, into the shape slot reserved before its content.
fn hover_plate(ui: &Ui, resp: &Response, rect: Rect, slot: egui::layers::ShapeIdx) {
    let tk = ui.tokens();
    let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
    if hover > 0.0 {
        let shape = egui::Shape::rect_filled(
            rect,
            CornerRadius::same(size::RADIUS_XS),
            alpha(tk.raised, hover),
        );
        ui.painter().set(slot, shape);
    }
}

// ---------------------------------------------------------------------------------------
// Players

/// The players' galleys for [`paint_players`].
pub(crate) struct PlayersText {
    pub count: Arc<Galley>,
    pub max: Arc<Galley>,
    pub bots: Option<Arc<Galley>>,
}

impl PlayersText {
    pub(crate) fn new(ui: &Ui, players: u32, max: u32, bots: u32) -> Self {
        Self {
            count: ph(
                ui,
                &players.to_string(),
                font(Face::MonoSemibold, size::TEXT_2XS),
            ),
            max: ph(ui, &format!("/{max}"), font(Face::Mono, size::TEXT_2XS)),
            bots: (bots > 0).then(|| ph(ui, &bots.to_string(), font(Face::Mono, size::TEXT_2XS))),
        }
    }
}

/// Draws `kit::Players` (96px, the count, `/max`, the bots, the 4px bar) in `rect`, from
/// laid-out galleys; `shown` is the bar's (animated) fill. Returns the bots' rect, if any.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_players(
    p: &Painter,
    rect: Rect,
    txt: &PlayersText,
    players: u32,
    max: u32,
    bots: u32,
    shown: f32,
    loading: bool,
    t: &Tokens,
) -> Option<Rect> {
    let (ink, fill) = match fill_tone(players, max) {
        Tone::Ok => (t.ok, t.ok),
        Tone::Warn => (t.warn, t.warn),
        Tone::Err => (t.err, t.err),
        _ => (t.fg_faint, alpha(t.fg_faint, 0.4)),
    };
    let line_y = rect.top() + 7.5;
    let mut x = rect.left();
    let ink_n = if loading {
        ink.gamma_multiply(0.5)
    } else {
        ink
    };
    draw(p, &txt.count, x, line_y, ink_n);
    x += txt.count.size().x + 4.0;
    draw(p, &txt.max, x, line_y, t.fg_faint);
    let mut bots_rect = None;
    if let Some(b) = &txt.bots {
        let bx = rect.right() - b.size().x;
        draw(p, b, bx, line_y, t.fg_faint);
        let ir = Rect::from_min_size(Pos2::new(bx - 14.0, line_y - 6.0), Vec2::splat(12.0));
        lucide::BOT.paint(p, ir, t.fg_faint);
        bots_rect = Some(ir.union(Rect::from_min_size(Pos2::new(bx, line_y - 7.0), b.size())));
    }
    let w = rect.width();
    let bar = Rect::from_min_size(
        Pos2::new(rect.left(), rect.bottom() - 4.0),
        Vec2::new(w, 4.0),
    );
    p.rect_filled(bar, CornerRadius::same(2), t.raised);
    if shown > 0.0 {
        p.rect_filled(
            Rect::from_min_size(bar.min, Vec2::new(w * shown.min(1.0), 4.0)),
            CornerRadius::same(2),
            fill,
        );
    }
    let pct = if max > 0 {
        (players as f32 / max as f32).min(1.0)
    } else {
        0.0
    };
    let bot_pct = if max > 0 {
        (bots as f32 / max as f32).min(pct)
    } else {
        0.0
    };
    if bot_pct > 0.0 {
        let hatch = Rect::from_min_max(
            Pos2::new(bar.left() + w * (pct - bot_pct), bar.top()),
            Pos2::new(bar.left() + w * pct, bar.bottom()),
        );
        let hp = p.with_clip_rect(hatch.intersect(p.clip_rect()));
        let mut hx = hatch.left() - 4.0;
        while hx < hatch.right() + 4.0 {
            hp.line_segment(
                [
                    Pos2::new(hx, hatch.bottom()),
                    Pos2::new(hx + 4.0, hatch.top()),
                ],
                Stroke::new(1.4, t.fg_faint),
            );
            hx += 4.0 * std::f32::consts::SQRT_2;
        }
    }
    bots_rect
}

/// `PlayersButton`: who is on a server now; a click asks it directly (the caller queries).
#[must_use = "add it with ui.add(…)"]
pub struct PlayersCell {
    players: u32,
    max: u32,
    bots: u32,
    loading: bool,
    compact: bool,
}

impl PlayersCell {
    pub fn new(players: u32, max: u32) -> Self {
        Self {
            players,
            max,
            bots: 0,
            loading: false,
            compact: false,
        }
    }

    pub fn bots(mut self, n: u32) -> Self {
        self.bots = n;
        self
    }

    pub fn loading(mut self, l: bool) -> Self {
        self.loading = l;
        self
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl Widget for PlayersCell {
    fn ui(self, ui: &mut Ui) -> Response {
        let bots_label = tf("servers", "bots", &[("count", &self.bots)]);
        let slot = ui.painter().add(egui::Shape::Noop);
        let rect = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(2.0);
                let mut w = Players::new(self.players, self.max)
                    .bots(self.bots)
                    .loading(self.loading)
                    .bots_label(&bots_label);
                if self.compact {
                    w = w.compact();
                }
                ui.add(w);
                ui.add_space(2.0);
            })
            .response
            .rect;
        let resp = ui.interact(rect, ui.next_auto_id().with("players"), Sense::click());
        hover_plate(ui, &resp, rect, slot);
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(t("servers", "clickRefreshPlayers"), "", Side::Top)
    }
}

// ---------------------------------------------------------------------------------------
// Favourite

/// How big a star is: `Sm` 20px (lists), `Md` a control (28px, panels).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StarSize {
    #[default]
    Sm,
    Md,
}

/// `animate-pop-in` (140ms): the star pops once when starred by a click.
const POP: f64 = 0.14;

/// Draws the star button in `rect`: `hover:bg-warn/15`, the star `warn` when on.
/// `popped_at` is when it was starred by a click (it pops once).
pub(crate) fn paint_star(
    ui: &Ui,
    rect: Rect,
    icon_px: f32,
    on: bool,
    hover: f32,
    popped_at: Option<f64>,
) {
    let t = ui.tokens();
    let p = ui.painter();
    if hover > 0.0 {
        p.rect_filled(
            rect,
            CornerRadius::same(size::RADIUS_XS),
            alpha(t.warn, 0.15 * hover),
        );
    }
    let (mut scale, mut a) = (1.0, 1.0);
    if let Some(at) = popped_at.filter(|_| on) {
        let k = ((ui.input(|i| i.time) - at) / POP).clamp(0.0, 1.0) as f32;
        if k < 1.0 {
            let e = egui::emath::easing::cubic_out(k);
            scale = 0.97 + 0.03 * e;
            a = e;
            ui.ctx().request_repaint();
        }
    }
    let ink = if on { t.warn } else { t.fg_faint };
    let ir = Rect::from_center_size(
        rect.center() + Vec2::new(0.0, 2.0 * (1.0 - a)),
        Vec2::splat(icon_px * scale),
    );
    lucide::STAR.paint(p, ir, ink.gamma_multiply(a));
}

/// The star's label: add or remove.
pub fn star_label(on: bool) -> &'static str {
    if on {
        t("servers", "removeFavorite")
    } else {
        t("servers", "addFavorite")
    }
}

/// `FavoriteButton`: add a server to the favourites or take it out (the caller does).
#[must_use = "add it with ui.add(…)"]
pub struct FavoriteButton {
    on: bool,
    size: StarSize,
}

impl FavoriteButton {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            size: StarSize::Sm,
        }
    }

    pub fn size(mut self, s: StarSize) -> Self {
        self.size = s;
        self
    }
}

impl Widget for FavoriteButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let (side, icon) = match self.size {
            StarSize::Sm => (20.0, 14.0),
            StarSize::Md => (size::CONTROL, size::ICON),
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
        let pop_id = resp.id.with("pop");
        if resp.clicked() && !self.on {
            let now = ui.input(|i| i.time);
            ui.data_mut(|d| d.insert_temp(pop_id, now));
        }
        if ui.is_rect_visible(rect) {
            let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
            let popped = ui.data(|d| d.get_temp::<f64>(pop_id));
            // The click flips the state the caller passes next frame; pop from then on.
            let on = self.on || resp.clicked();
            paint_star(ui, rect, icon, on, hover, popped);
        }
        let label = star_label(self.on);
        let on = self.on;
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, label));
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(label, "", Side::Top)
    }
}

// ---------------------------------------------------------------------------------------
// Exclude

/// `ExcludeButton`: hide every server at this IP from the browser, or bring them back:
/// red while hidden. The caller shows it on row hover unless it is always shown.
#[must_use = "add it with ui.add(…)"]
pub struct ExcludeButton<'a> {
    ip: &'a str,
    on: bool,
    size: Size,
    variant: Variant,
}

impl<'a> ExcludeButton<'a> {
    pub fn new(ip: &'a str, on: bool) -> Self {
        Self {
            ip,
            on,
            size: Size::IconXs,
            variant: Variant::Ghost,
        }
    }

    /// `Size::Icon` or `Size::IconXs` (the default).
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
}

/// The ban's label for `ip`.
pub fn exclude_label(ip: impl std::fmt::Display, on: bool) -> String {
    if on {
        tf("servers", "ipExcludedClick", &[("ip", &ip)])
    } else {
        tf("servers", "excludeIp", &[("ip", &ip)])
    }
}

impl Widget for ExcludeButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let label = exclude_label(self.ip, self.on);
        let mut b = IconButton::new(lucide::BAN, &label)
            .size(self.size)
            .variant(self.variant)
            .danger_hover();
        if self.on {
            b = b.tint(ui.tokens().err);
        }
        ui.add(b)
    }
}

// ---------------------------------------------------------------------------------------
// Join

/// Draws the lists' quiet Join (`size-control`, the play icon): an accent outline that
/// fills with the accent as `lit` goes to 1 (the row's hover, or the selected row).
pub(crate) fn paint_join_quiet(ui: &Ui, rect: Rect, lit: f32, hover: f32, press: bool) {
    let t = ui.tokens();
    let p = ui.painter();
    let r = if press {
        Rect::from_center_size(rect.center(), rect.size() * 0.96)
    } else {
        rect
    };
    let radius = CornerRadius::same(size::RADIUS_MD);
    let fill_on = paint::bright(t.accent, 1.0 + 0.10 * hover);
    if lit > 0.0 {
        let glow = if hover > 0.5 {
            paint::glow(&t)
        } else {
            paint::glow_soft(&t)
        };
        let mut glow = glow;
        glow.color = glow.color.gamma_multiply(lit);
        p.add(glow.as_shape(r, radius));
    }
    let fill = lerp(Color32::TRANSPARENT, fill_on, lit);
    let border = lerp(alpha(t.accent, 0.45), fill_on, lit);
    paint::plate(p, r, size::RADIUS_MD, fill, border);
    if hover > 0.5 && lit > 0.5 {
        p.rect_stroke(
            r.expand(1.0),
            CornerRadius::same(size::RADIUS_MD + 1),
            Stroke::new(1.0, alpha(t.accent, 0.45)),
            StrokeKind::Outside,
        );
    }
    let ink = lerp(t.accent, t.accent_fg, lit);
    lucide::PLAY.paint(
        p,
        Rect::from_center_size(r.center(), Vec2::splat(size::ICON_SM)),
        ink,
    );
}

/// `JoinButton`: join a server (the caller connects). `compact` is the icon alone;
/// `quiet` is for lists: an outline that fills on the row's hover (`group_hover`) or when
/// `lit` (the selected row).
#[must_use = "add it with ui.add(…)"]
pub struct JoinButton {
    size: Size,
    compact: bool,
    quiet: bool,
    lit: bool,
    group_hover: bool,
    width: Option<f32>,
    /// Another hint and name than "Connect" (an offline mission's map).
    tip: Option<String>,
}

impl Default for JoinButton {
    fn default() -> Self {
        Self::new()
    }
}

impl JoinButton {
    pub fn new() -> Self {
        Self {
            size: Size::Sm,
            compact: false,
            quiet: false,
            lit: false,
            group_hover: false,
            width: None,
            tip: None,
        }
    }

    /// Its hint and accessible name, when it launches something else than a server (the
    /// stats' offline missions: the map's name).
    pub fn tip(mut self, t: impl Into<String>) -> Self {
        self.tip = Some(t.into());
        self
    }

    /// As wide as this (`[&_button]:w-full`: the server panel's foot).
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// `Size::Sm` (the default) or `Size::Lg`.
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    pub fn lit(mut self, on: bool) -> Self {
        self.lit = on;
        self
    }

    /// The row it sits in is hovered.
    pub fn group_hover(mut self, on: bool) -> Self {
        self.group_hover = on;
        self
    }
}

impl Widget for JoinButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let (label, tip) = match self.tip.as_deref() {
            Some(s) => (s, s),
            None => (t("servers", "connect"), t("servers", "connectTitle")),
        };
        if self.quiet && self.compact {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size::CONTROL), Sense::click());
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
            if ui.is_rect_visible(rect) {
                let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
                let lit = paint::ease(
                    ui,
                    resp.id.with("lit"),
                    self.lit || self.group_hover || resp.hovered(),
                );
                paint_join_quiet(ui, rect, lit, hover, resp.is_pointer_button_down_on());
            }
            return resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip_kbd(tip, "", Side::Top);
        }
        let mut b = if self.compact {
            Button::new(label).size(Size::Icon)
        } else {
            Button::new(label).size(self.size)
        };
        if let Some(w) = self.width {
            b = b.width(w);
        }
        ui.add(
            b.variant(Variant::Play)
                .icon(lucide::PLAY)
                .icon_size(size::ICON_SM)
                .tip_kbd(tip, "", Side::Top),
        )
    }
}

// ---------------------------------------------------------------------------------------
// Shortcuts

/// `ShortcutsButton`: opens the sheet of shortcuts (the caller opens it); `kbd` is the
/// sheet's own key as the registry writes it.
#[must_use = "add it with ui.add(…)"]
pub struct ShortcutsButton<'a> {
    kbd: &'a str,
}

impl<'a> ShortcutsButton<'a> {
    pub fn new(kbd: &'a str) -> Self {
        Self { kbd }
    }
}

impl Widget for ShortcutsButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.add(IconButton::new(lucide::KEYBOARD, t("keys", "sheet")).kbd(self.kbd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases() {
        assert_eq!(day_phase("17:40"), lucide::SUN);
        assert_eq!(day_phase("05:50"), lucide::SUNRISE);
        assert_eq!(day_phase("20:10"), lucide::SUNRISE);
        assert_eq!(day_phase("00:08"), lucide::MOON);
        assert_eq!(day_phase(""), lucide::SUNRISE);
    }

    #[test]
    fn mods_marks() {
        let t = Tokens::default();
        assert_eq!(mods_tone(&t, None, 3), t.mods);
        assert_eq!(mods_tone(&t, Some(2), 3), t.err);
        assert_eq!(mods_tone(&t, Some(0), 3), t.warn);
        assert_eq!(mods_tone(&t, Some(0), 0), t.ok);
        assert_eq!(mods_extra(Some(22), 0).as_deref(), Some("−22"));
        assert_eq!(mods_extra(Some(0), 1).as_deref(), Some("↑1"));
        assert_eq!(mods_extra(None, 0), None);
    }

    #[test]
    fn ping_texts() {
        assert_eq!(ping_text(Some(92), false), ("92".to_owned(), true));
        assert_eq!(ping_text(Some(9999), false).0, crate::i18n::format::NONE);
        assert_eq!(ping_text(None, true).0, "…");
        assert!(ping_timed_out(Some(9999), false));
        assert!(!ping_timed_out(Some(9999), true));
        assert!(!ping_timed_out(Some(92), false));
        assert!(!ping_timed_out(None, false));
    }

    #[test]
    fn versions_compare() {
        assert!(same_version("1.28.162391", "1.28.162391"));
        assert!(same_version("1.28.162391", " 1.28.0.162391 "));
        assert!(!same_version("1.28.162391", "1.27.160000"));
        assert!(!same_version("1.28.162391", "1.28.162392"));
        assert!(same_version("exp", "exp"));
        assert!(!same_version("1.28", "1.28.162391"));
    }
}
