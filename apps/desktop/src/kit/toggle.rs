//! `ui/switch` (Switch, MiniSwitch), `ui/toggle` (a checkbox drawn as a dense row, for
//! filter panels) and `ui/checkbox`.
//!
//! Track `h-3.5 w-7 rounded-full border bg-bg`, lit accent (or warn) when on with the
//! soft glow; the thumb (`size-2.5`, faint, then `accent-fg`) slides 13px over
//! `--duration-fast`. Box `size-3.5 rounded-xs border-border-strong bg-bg`, filled accent
//! when ticked, the tick popping in.

use egui::{Color32, CornerRadius, Rect, Response, Role, Sense, Ui, Vec2, Widget, WidgetInfo};

use super::Tone;
use super::paint::{self, fade};
use crate::icons::lucide;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

const TRACK: Vec2 = Vec2::new(28.0, 14.0);
const THUMB: f32 = 10.0;

fn lit(t: &Tokens, tone: Tone) -> Color32 {
    if tone == Tone::Warn { t.warn } else { t.accent }
}

/// Paints a switch's track and thumb in `track`; `on` is the eased 0→1.
fn paint_track(ui: &Ui, track: Rect, on: f32, tone: Tone, en: bool, thumb_from: f32, travel: f32) {
    let t = ui.tokens();
    let p = ui.painter();
    let c = lit(&t, tone);
    if on > 0.5 && en && tone != Tone::Warn {
        p.add(paint::glow_soft(&t).as_shape(track, CornerRadius::same(7)));
    }
    paint::plate(
        p,
        track,
        7,
        fade(paint::lerp(t.bg, c, on), en),
        fade(t.border, en),
    );
    let x = track.left() + thumb_from + travel * on;
    let r = Rect::from_min_size(
        egui::pos2(x, track.center().y - THUMB / 2.0),
        Vec2::splat(THUMB),
    );
    p.rect_filled(
        r,
        CornerRadius::same(5),
        fade(paint::lerp(t.fg_faint, t.accent_fg, on), en),
    );
}

/// A setting whose flipping *does* something: the whole row is the target, the label
/// beside the track, a wrapped hint under it. `Switch::new(&mut on, "Auto-finish")`.
#[must_use = "add it with ui.add(…)"]
pub struct Switch<'a> {
    on: &'a mut bool,
    label: &'a str,
    hint: &'a str,
    tone: Tone,
    enabled: bool,
}

impl<'a> Switch<'a> {
    pub fn new(on: &'a mut bool, label: &'a str) -> Self {
        Self {
            on,
            label,
            hint: "",
            tone: Tone::Accent,
            enabled: true,
        }
    }

    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = h;
        self
    }

    /// `Tone::Warn` for the ones that write to the game.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

impl Widget for Switch<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let w = ui.available_width();
        let text_w = (w - 16.0 - TRACK.x - 8.0).max(10.0);
        let checked = *self.on;
        let label_ink = if checked { t.fg } else { t.fg_muted };
        let lg = paint::galley_trunc(
            ui,
            self.label,
            font(Face::Sans, size::TEXT_2XS),
            label_ink,
            text_w,
        );
        let hg = (!self.hint.is_empty()).then(|| {
            paint::galley_wrap(
                ui,
                self.hint,
                font(Face::Sans, size::TEXT_2XS),
                t.fg_faint,
                text_w,
            )
        });
        let text_h = lg.size().y + hg.as_ref().map_or(0.0, |g| g.size().y);
        let h = text_h.max(TRACK.y) + 12.0;
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), sense);
        if resp.clicked() {
            *self.on = !*self.on;
            resp.mark_changed();
        }
        let now = *self.on;
        let label = self.label;
        resp.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, now, label));
        if ui.is_rect_visible(rect) {
            let hover = paint::ease(ui, resp.id.with("hover"), self.enabled && resp.hovered());
            let on = paint::ease(ui, resp.id.with("on"), now);
            let p = ui.painter();
            p.rect_filled(
                rect,
                CornerRadius::ZERO,
                paint::lerp(Color32::TRANSPARENT, t.raised, hover),
            );
            let top = rect.top() + 6.0 + (text_h.max(TRACK.y) - text_h) / 2.0;
            // With a hint under it, the track sits on the label's line, not the block's.
            let cy = if hg.is_some() {
                top + lg.size().y / 2.0
            } else {
                rect.top() + 6.0 + text_h.max(TRACK.y) / 2.0
            };
            let track =
                Rect::from_min_size(egui::pos2(rect.left() + 8.0, cy - TRACK.y / 2.0), TRACK);
            paint_track(ui, track, on, self.tone, self.enabled, 4.0, 13.0);
            let x = track.right() + 8.0;
            let ink = fade(if now { t.fg } else { t.fg_muted }, self.enabled);
            p.galley_with_override_text_color(egui::pos2(x, top), lg.clone(), ink);
            if let Some(hg) = hg {
                p.galley(
                    egui::pos2(x, top + lg.size().y),
                    hg,
                    fade(t.fg_faint, self.enabled),
                );
            }
            if paint::keyboard_focus(ui, &resp) {
                p.rect_stroke(
                    rect,
                    CornerRadius::ZERO,
                    egui::Stroke::new(1.0, t.accent),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if self.enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }
}

/// The switch alone, for a row that already says what it switches.
/// `MiniSwitch::new(&mut on).label("Enabled")` (the label is the accessible name).
#[must_use = "add it with ui.add(…)"]
pub struct MiniSwitch<'a> {
    on: &'a mut bool,
    label: &'a str,
    tone: Tone,
    enabled: bool,
}

impl<'a> MiniSwitch<'a> {
    pub fn new(on: &'a mut bool) -> Self {
        Self {
            on,
            label: "",
            tone: Tone::Accent,
            enabled: true,
        }
    }

    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

impl Widget for MiniSwitch<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, mut resp) = ui.allocate_exact_size(TRACK, sense);
        if resp.clicked() {
            *self.on = !*self.on;
            resp.mark_changed();
        }
        let now = *self.on;
        let label = self.label;
        resp.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, now, label));
        if ui.is_rect_visible(rect) {
            let on = paint::ease(ui, resp.id.with("on"), now);
            let t = ui.tokens();
            // No glow on the mini one: it sits in table rows.
            let p = ui.painter();
            let c = lit(&t, self.tone);
            let en = self.enabled;
            paint::plate(
                p,
                rect,
                7,
                fade(paint::lerp(t.bg, c, on), en),
                fade(t.border, en),
            );
            let x = rect.left() + 4.0 + 12.0 * on;
            let r = Rect::from_min_size(
                egui::pos2(x, rect.center().y - THUMB / 2.0),
                Vec2::splat(THUMB),
            );
            p.rect_filled(
                r,
                CornerRadius::same(5),
                fade(paint::lerp(t.fg_faint, t.accent_fg, on), en),
            );
            if paint::keyboard_focus(ui, &resp) {
                p.rect_stroke(
                    rect.expand(1.0),
                    CornerRadius::same(8),
                    egui::Stroke::new(1.0, t.accent),
                    egui::StrokeKind::Outside,
                );
            }
        }
        if self.enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }
}

/// Paints the 14px box (`ui/checkbox`); `tick` is the eased 0→1 of the mark.
pub(crate) fn paint_box(
    ui: &Ui,
    r: Rect,
    checked: bool,
    mixed: bool,
    tick: f32,
    hover: f32,
    en: bool,
) {
    let t = ui.tokens();
    let p = ui.painter();
    let on = checked || mixed;
    let (fill, border) = if on {
        (t.accent, t.accent)
    } else {
        (t.bg, paint::lerp(t.border_strong, t.fg_faint, hover))
    };
    paint::plate(p, r, size::RADIUS_XS, fade(fill, en), fade(border, en));
    if mixed {
        lucide::MINUS.paint(
            p,
            Rect::from_center_size(r.center(), Vec2::splat(12.0)),
            fade(t.accent_fg, en),
        );
    } else if checked {
        // pop-in: from 97% and transparent.
        let s = 12.0 * (0.97 + 0.03 * tick);
        let c = fade(t.accent_fg, en).gamma_multiply(tick.max(0.01));
        lucide::CHECK.paint(p, Rect::from_center_size(r.center(), Vec2::splat(s)), c);
    }
}

/// A box that is ticked or not; `indeterminate` for a "select all" over a part.
/// `Checkbox::new(&mut on).label("Select all")` (the label is the accessible name).
#[must_use = "add it with ui.add(…)"]
pub struct Checkbox<'a> {
    on: &'a mut bool,
    mixed: bool,
    label: &'a str,
    enabled: bool,
}

impl<'a> Checkbox<'a> {
    pub fn new(on: &'a mut bool) -> Self {
        Self {
            on,
            mixed: false,
            label: "",
            enabled: true,
        }
    }

    pub fn indeterminate(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }

    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::splat(14.0), sense);
        if resp.clicked() {
            *self.on = !*self.on;
            resp.mark_changed();
        }
        let now = *self.on;
        let label = self.label;
        resp.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, now, label));
        if ui.is_rect_visible(rect) {
            let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
            let tick = paint::ease_for(ui, resp.id.with("tick"), now, 0.14);
            paint_box(ui, rect, now, self.mixed, tick, hover, self.enabled);
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(ui.painter(), rect, size::RADIUS_XS, &ui.tokens());
            }
        }
        if self.enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }
}

/// A checkbox drawn as a dense row, the height of a list row, for filter panels: the
/// label and the box are one target. `Toggle::new(&mut on, "Official").count(412)`.
#[must_use = "add it with ui.add(…)"]
pub struct Toggle<'a> {
    on: &'a mut bool,
    label: &'a str,
    count: Option<u64>,
    swatch: Option<Color32>,
    tip: &'a str,
    compact: bool,
}

impl<'a> Toggle<'a> {
    pub fn new(on: &'a mut bool, label: &'a str) -> Self {
        Self {
            on,
            label,
            count: None,
            swatch: None,
            tip: "",
            compact: false,
        }
    }

    /// The height of a small control rather than a list row: for long checklists.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// A figure at the end: how many rows it lets through.
    pub fn count(mut self, n: u64) -> Self {
        self.count = Some(n);
        self
    }

    /// A swatch matching how the thing is drawn elsewhere.
    pub fn swatch(mut self, c: Color32) -> Self {
        self.swatch = Some(c);
        self
    }

    /// Hover text when the label does not say what it means (defaults to the label).
    pub fn tip(mut self, tip: &'a str) -> Self {
        self.tip = tip;
        self
    }
}

impl Widget for Toggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let w = ui.available_width();
        let h = if self.compact {
            size::control_sm()
        } else {
            size::ROW
        };
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        if resp.clicked() {
            *self.on = !*self.on;
            resp.mark_changed();
        }
        let now = *self.on;
        let label = self.label;
        resp.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, now, label));
        if ui.is_rect_visible(rect) {
            let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
            let tick = paint::ease_for(ui, resp.id.with("tick"), now, 0.14);
            let p = ui.painter();
            p.rect_filled(
                rect,
                CornerRadius::same(size::RADIUS_XS),
                paint::lerp(Color32::TRANSPARENT, t.raised, hover),
            );
            let cy = rect.center().y;
            let mut x = rect.left() + 4.0;
            paint_box(
                ui,
                Rect::from_min_size(egui::pos2(x, cy - 7.0), Vec2::splat(14.0)),
                now,
                false,
                tick,
                0.0,
                true,
            );
            x += 14.0 + 6.0;
            if let Some(c) = self.swatch {
                p.rect_filled(
                    Rect::from_min_size(egui::pos2(x, cy - 4.0), Vec2::splat(8.0)),
                    CornerRadius::same(size::RADIUS_XS),
                    c,
                );
                x += 8.0 + 6.0;
            }
            let ink = if now { t.fg } else { t.fg_faint };
            let mut right = rect.right() - 4.0;
            if let Some(n) = self.count {
                let g = paint::galley(
                    ui,
                    &n.to_string(),
                    font(Face::Mono, size::TEXT_2XS),
                    t.fg_faint,
                );
                right -= g.size().x;
                paint::text_at(p, &g, right, cy, t.fg_faint);
                right -= 6.0;
            }
            let g = paint::galley_trunc(
                ui,
                self.label,
                font(Face::Sans, size::TEXT_2XS),
                ink,
                (right - x).max(0.0),
            );
            paint::text_at(p, &g, x, cy, ink);
        }
        let tip = if self.tip.is_empty() {
            self.label
        } else {
            self.tip
        };
        super::TipExt::tip(resp.on_hover_cursor(egui::CursorIcon::PointingHand), tip)
    }
}
