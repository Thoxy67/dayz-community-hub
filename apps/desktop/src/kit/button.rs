//! `ui/button` (Button, IconButton) and `ui/chip`.
//!
//! Heights are the theme's measures: `Lg` 34, `Sm` 28 (the default), `Xs` 22, and the
//! square `Icon` 28 / `IconXs` 22. Colours ease over `--duration-fast`; a press sinks the
//! button a little (`active:scale-[0.98]`).

use egui::{Color32, CornerRadius, Rect, Response, Role, Sense, Ui, Vec2, Widget, WidgetInfo};

use super::paint::{self, alpha, lerp};
use super::tooltip::{Side, TipExt};
use crate::icons::Icon;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// The controller button a button answers to (a dialog's B closes it, X confirms it):
/// its glyph shows before the label while the pad density is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadKey {
    Back,
    Confirm,
}

/// How the window draws a [`PadKey`]'s glyph (`parts::pad`, which knows the pad in
/// hand): its width, and drawing it in a rect.
#[derive(Clone, Copy)]
pub struct PadChips {
    pub width: fn(&Ui, PadKey) -> f32,
    pub paint: fn(&Ui, PadKey, Rect),
}

static PAD_CHIPS: std::sync::OnceLock<PadChips> = std::sync::OnceLock::new();

/// Installs the glyph drawing; the first call wins.
pub fn set_pad_chips(c: PadChips) {
    let _ = PAD_CHIPS.set(c);
}

/// The glyph drawing, while the pad density is worn.
pub fn pad_chips() -> Option<PadChips> {
    if size::pad_density() {
        PAD_CHIPS.get().copied()
    } else {
        None
    }
}

/// A dialog's footer being laid out: how many labelled buttons it had last frame, and
/// how many it has had so far this frame (`kit::dialog` opens and closes it).
#[derive(Clone, Copy)]
struct Footer {
    last: usize,
    seen: usize,
}

thread_local! {
    static FOOTER: std::cell::Cell<Option<Footer>> = const { std::cell::Cell::new(None) };
}

/// A dialog's footer starts: its buttons take their pad keys from their place (the
/// first is the safe one, B; the last acts, X), knowing `last` buttons were there the
/// frame before.
pub(super) fn footer_begin(last: usize) {
    FOOTER.with(|f| f.set(Some(Footer { last, seen: 0 })));
}

/// The footer ends: how many labelled buttons it had.
pub(super) fn footer_end() -> usize {
    FOOTER.with(|f| f.take()).map_or(0, |f| f.seen)
}

fn confirm_chip_id() -> egui::Id {
    egui::Id::unique("kit::button.confirm_chip")
}

/// Whether an enabled button wore the X (confirm) chip this frame or the last: the pad
/// hints' bar, drawn before the dialogs, says what X does then.
pub fn confirm_chip_shown(ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<u64>(confirm_chip_id()))
        .is_some_and(|f| f + 1 >= now)
}

/// The pad key of the `i`-th labelled button of a footer of `n` (the dialog's B closes
/// it, X presses its last button): the first of several is B, the last X; a lone button
/// is B when it only dismisses (ghost or plain), else X.
fn footer_key(i: usize, n: usize, v: Variant) -> Option<PadKey> {
    if n == 0 || i >= n {
        None
    } else if n == 1 {
        Some(match v {
            Variant::Ghost | Variant::Default => PadKey::Back,
            _ => PadKey::Confirm,
        })
    } else if i == 0 {
        Some(PadKey::Back)
    } else if i + 1 == n {
        Some(PadKey::Confirm)
    } else {
        None
    }
}

/// The key a labelled button laid out now takes from its place in a dialog's footer.
fn footer_next(v: Variant) -> Option<PadKey> {
    FOOTER.with(|c| {
        let mut f = c.get()?;
        let k = footer_key(f.seen, f.last, v);
        f.seen += 1;
        c.set(Some(f));
        k
    })
}

/// What a button wears.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    /// Raised plate, border: the ordinary button.
    #[default]
    Default,
    /// Filled accent with a soft glow: the action a panel exists for.
    Accent,
    /// No plate until hovered: toolbars, icon buttons.
    Ghost,
    /// Red outline: something that cannot be taken back.
    Danger,
    /// Stencilled capitals on the accent: joining a server.
    Play,
}

/// How big a button is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    /// `h-control-lg px-4 text-sm`.
    Lg,
    /// `h-control px-2.5 text-xs`.
    #[default]
    Sm,
    /// `h-control-sm px-1.5 text-2xs`.
    Xs,
    /// `size-control`, a square for one icon.
    Icon,
    /// `size-control-sm`.
    IconXs,
}

impl Size {
    fn height(self) -> f32 {
        match self {
            Size::Lg => size::control_lg(),
            Size::Sm | Size::Icon => size::control(),
            Size::Xs | Size::IconXs => size::control_sm(),
        }
    }

    fn pad(self) -> f32 {
        match self {
            Size::Lg => 16.0,
            Size::Sm => 10.0,
            Size::Xs => 6.0,
            Size::Icon | Size::IconXs => 0.0,
        }
    }

    fn text(self) -> f32 {
        match self {
            Size::Lg => size::TEXT_SM,
            Size::Sm | Size::Icon => size::TEXT_XS,
            Size::Xs | Size::IconXs => size::TEXT_2XS,
        }
    }

    fn icon(self) -> f32 {
        match self {
            Size::Lg | Size::Icon => size::ICON,
            Size::Sm | Size::IconXs => size::ICON_SM,
            Size::Xs => 12.0,
        }
    }

    fn square(self) -> bool {
        matches!(self, Size::Icon | Size::IconXs)
    }
}

/// The colours of a button in one state.
struct Look {
    fill: Color32,
    border: Color32,
    ink: Color32,
    glow: Option<egui::Shadow>,
    ring: Option<Color32>,
}

fn look(
    t: &Tokens,
    v: Variant,
    hover: f32,
    press: bool,
    chip_on: bool,
    danger_hover: bool,
) -> Look {
    let clear = Color32::TRANSPARENT;
    let mut l = match v {
        Variant::Default => Look {
            fill: if press {
                alpha(t.raised, 0.6)
            } else {
                lerp(alpha(t.raised, 0.8), t.raised, hover)
            },
            border: lerp(t.border, t.border_strong, hover),
            ink: t.fg,
            glow: None,
            ring: None,
        },
        Variant::Accent => Look {
            fill: if press {
                alpha(t.accent, 0.8)
            } else {
                lerp(t.accent, alpha(t.accent, 0.9), hover)
            },
            border: alpha(t.accent, 0.5),
            ink: t.accent_fg,
            glow: Some(if hover > 0.5 {
                paint::glow(t)
            } else {
                paint::glow_soft(t)
            }),
            ring: (hover > 0.5).then(|| alpha(t.accent, 0.45)),
        },
        Variant::Ghost => Look {
            fill: if press {
                alpha(t.raised, 0.6)
            } else {
                lerp(clear, t.raised, hover)
            },
            border: clear,
            ink: lerp(t.fg_muted, t.fg, hover),
            glow: None,
            ring: None,
        },
        Variant::Danger => Look {
            fill: if press {
                alpha(t.err, 0.15)
            } else {
                lerp(clear, alpha(t.err, 0.10), hover)
            },
            border: alpha(t.err, 0.4),
            ink: t.err,
            glow: None,
            ring: None,
        },
        Variant::Play => {
            let f = if press { 0.95 } else { 1.0 + 0.10 * hover };
            Look {
                fill: paint::bright(t.accent, f),
                border: paint::bright(t.accent, f),
                ink: t.accent_fg,
                glow: Some(if hover > 0.5 {
                    paint::glow(t)
                } else {
                    paint::glow_soft(t)
                }),
                ring: (hover > 0.5).then(|| alpha(t.accent, 0.45)),
            }
        }
    };
    if chip_on {
        // `border-accent bg-transparent text-accent hover:bg-accent/10`
        l = Look {
            fill: lerp(clear, alpha(t.accent, 0.10), hover),
            border: t.accent,
            ink: t.accent,
            glow: None,
            ring: None,
        };
    } else if danger_hover {
        l.fill = lerp(l.fill, alpha(t.err, 0.10), hover);
        l.ink = lerp(l.ink, t.err, hover);
    }
    l
}

/// The colours of a button that cannot be pressed, whatever its variant: no accent, no
/// glow, a quiet plate and faint ink, so an unavailable primary action never reads as
/// ready (a dimmed accent still looks like gold to press).
fn disabled_look(t: &Tokens, v: Variant) -> Look {
    let clear = Color32::TRANSPARENT;
    let (fill, border) = match v {
        Variant::Ghost => (clear, clear),
        Variant::Danger => (clear, alpha(t.border, 0.8)),
        Variant::Default | Variant::Accent | Variant::Play => {
            (alpha(t.raised, 0.35), alpha(t.border, 0.8))
        }
    };
    Look {
        fill,
        border,
        ink: t.fg_faint,
        glow: None,
        ring: None,
    }
}

/// A button: `Button::new("Refresh").icon(lucide::REFRESH_CW).variant(Variant::Accent)`.
#[must_use = "add it with ui.add(…)"]
pub struct Button<'a> {
    label: &'a str,
    icon: Option<Icon>,
    trailing: Option<Icon>,
    variant: Variant,
    size: Size,
    active: bool,
    enabled: bool,
    chip: bool,
    danger_hover: bool,
    width: Option<f32>,
    icon_size: Option<f32>,
    tip: Option<(&'a str, &'a str, Side)>,
    spinning: bool,
    start: bool,
    tint: Option<Color32>,
    pad_key: Option<PadKey>,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            trailing: None,
            variant: Variant::Default,
            size: Size::Sm,
            active: false,
            enabled: true,
            chip: false,
            danger_hover: false,
            width: None,
            icon_size: None,
            tip: None,
            spinning: false,
            start: false,
            tint: None,
            pad_key: None,
        }
    }

    /// The controller button it answers to, shown as a glyph before the label while a
    /// controller drives the window.
    pub fn pad_key(mut self, k: PadKey) -> Self {
        self.pad_key = Some(k);
        self
    }

    /// The label in its own colour whatever the state (a chip's count in `text-err`
    /// or `text-warn`, as a `<span class>` inside the Svelte chip); `None` keeps the ink.
    pub fn tint(mut self, c: Option<Color32>) -> Self {
        self.tint = c;
        self
    }

    /// Icon and label from the left edge, not centred (`justify-start`, with a width).
    pub fn start(mut self) -> Self {
        self.start = true;
        self
    }

    /// While `on`, a turning spinner stands in for the leading icon (a check under way).
    pub fn spinner(mut self, on: bool) -> Self {
        self.spinning = on;
        self
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// An icon after the label (a chevron).
    pub fn trailing(mut self, icon: Icon) -> Self {
        self.trailing = Some(icon);
        self
    }

    pub fn icon_size(mut self, px: f32) -> Self {
        self.icon_size = Some(px);
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    /// Latched on: drawn as the accent variant (`aria-pressed`).
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// A fixed width (the label is centred, cut if it must be). `f32::INFINITY` fills.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// The hint under the pointer (and its shortcut), as `Tooltip` gives it.
    pub fn tip(mut self, text: &'a str) -> Self {
        self.tip = Some((text, "", Side::Bottom));
        self
    }

    pub fn tip_kbd(mut self, text: &'a str, kbd: &'a str, side: Side) -> Self {
        self.tip = Some((text, kbd, side));
        self
    }

    /// The whole button turns red under the pointer (`hoverTone="danger"`).
    pub fn danger_hover(mut self) -> Self {
        self.danger_hover = true;
        self
    }

    /// `ui/chip`: an `Xs` button that says "on" by outline, not by fill. Beside a
    /// `CONTROL`-high field (a toolbar's search) it takes `.size(Size::Sm)` to match.
    pub fn chip(label: &'a str, on: bool) -> Self {
        let mut b = Self::new(label).size(Size::Xs);
        b.chip = true;
        b.active = on;
        b
    }

    /// The width the button takes at its natural size (its icons, label and padding;
    /// not a pad chip): what a row measures before it shares out the rest.
    pub fn natural_width(&self, ui: &Ui) -> f32 {
        let s = self.size;
        let h = s.height();
        if s.square() {
            return h;
        }
        let icon_px = self.icon_size.unwrap_or_else(|| s.icon());
        let mut parts: Vec<f32> = Vec::with_capacity(3);
        if self.icon.is_some() {
            parts.push(icon_px);
        }
        if !self.label.is_empty() {
            let face = if self.variant == Variant::Play && !self.active {
                Face::DisplayBold
            } else {
                Face::SansMedium
            };
            parts.push(
                paint::galley(ui, self.label, font(face, s.text()), Color32::WHITE)
                    .size()
                    .x,
            );
        }
        if self.trailing.is_some() {
            parts.push(icon_px);
        }
        let gaps = 6.0 * parts.len().saturating_sub(1) as f32;
        parts.iter().sum::<f32>() + gaps + 2.0 * s.pad() + 2.0
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let s = self.size;
        let play = self.variant == Variant::Play && !self.active;
        let text_font = if play {
            // `font-display font-extrabold` at the size's own text size.
            font(Face::DisplayBold, s.text())
        } else {
            font(Face::SansMedium, s.text())
        };
        let icon_px = self.icon_size.unwrap_or_else(|| s.icon());
        let g = (!self.label.is_empty() && !s.square()).then(|| {
            if play {
                paint::tracked(ui, self.label, text_font.clone(), t.fg, 0.08, true, None)
            } else {
                paint::galley(ui, self.label, text_font.clone(), t.fg)
            }
        });
        let gap = 6.0;
        let mut content = 0.0;
        let mut parts = 0;
        // In a dialog's footer its place gives the key, unless it was given one.
        let placed = if g.is_some() {
            footer_next(self.variant)
        } else {
            None
        };
        let key = self.pad_key.or(placed);
        if key == Some(PadKey::Confirm) && self.enabled && pad_chips().is_some() {
            let frame = ui.ctx().cumulative_frame_nr();
            ui.ctx()
                .data_mut(|d| d.insert_temp(confirm_chip_id(), frame));
        }
        let chip = key
            .filter(|_| !s.square())
            .and_then(|k| pad_chips().map(|c| (k, c, (c.width)(ui, k))));
        if let Some((_, _, cw)) = chip {
            content += cw;
            parts += 1;
        }
        if self.icon.is_some() {
            content += icon_px;
            parts += 1;
        }
        if let Some(g) = &g {
            content += g.size().x;
            parts += 1;
        }
        if self.trailing.is_some() {
            content += icon_px;
            parts += 1;
        }
        if parts > 1 {
            content += gap * (parts - 1) as f32;
        }
        let h = s.height();
        let natural = if s.square() {
            h
        } else {
            content + 2.0 * s.pad() + 2.0
        };
        let w = match self.width {
            Some(w) if w.is_infinite() => ui.available_width().max(natural.min(h)),
            Some(w) => w.max(0.0),
            None => natural,
        };
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), sense);
        let label = self.label;
        let checked = self.active;
        let on = self.enabled;
        resp.widget_info(|| {
            if checked {
                WidgetInfo::selected(Role::Button, on, true, label)
            } else {
                WidgetInfo::labeled(Role::Button, on, label)
            }
        });
        if ui.is_rect_visible(rect) {
            let hover = paint::ease(ui, resp.id.with("hover"), self.enabled && resp.hovered());
            let press = self.enabled && resp.is_pointer_button_down_on();
            let v = if self.active && !self.chip {
                Variant::Accent
            } else {
                self.variant
            };
            let l = if self.enabled {
                look(
                    &t,
                    v,
                    hover,
                    press,
                    self.chip && self.active,
                    self.danger_hover,
                )
            } else {
                disabled_look(&t, v)
            };
            let scale = if press {
                match s {
                    Size::Icon => 0.96,
                    Size::IconXs => 1.0,
                    _ => 0.98,
                }
            } else {
                1.0
            };
            let r = Rect::from_center_size(rect.center(), rect.size() * scale);
            let radius = size::RADIUS_MD;
            let painter = ui.painter();
            let en = self.enabled;
            if let Some(glow) = l.glow {
                painter.add(glow.as_shape(r, CornerRadius::same(radius)));
            }
            paint::plate(painter, r, radius, l.fill, l.border);
            if let Some(ring) = l.ring {
                painter.rect_stroke(
                    r.expand(1.0),
                    CornerRadius::same(radius + 1),
                    egui::Stroke::new(1.0, ring),
                    egui::StrokeKind::Outside,
                );
            }
            let ink = l.ink;
            let mut x = if self.start {
                r.left() + s.pad()
            } else {
                r.center().x - content / 2.0
            };
            let cy = r.center().y;
            if let Some((k, c, cw)) = chip {
                let gh = (size::control_sm() - 4.0).min(r.height() - 4.0);
                (c.paint)(
                    ui,
                    k,
                    Rect::from_min_size(egui::pos2(x, cy - gh / 2.0), Vec2::new(cw, gh)),
                );
                x += cw + gap;
            }
            if let Some(icon) = self.icon {
                let ir =
                    Rect::from_min_size(egui::pos2(x, cy - icon_px / 2.0), Vec2::splat(icon_px));
                if self.spinning {
                    super::Spinner::paint(ui, ir, ink);
                } else {
                    icon.paint(painter, ir, ink);
                }
                x += icon_px + gap;
            }
            if let Some(g) = &g {
                let ink = self.tint.filter(|_| en).unwrap_or(ink);
                let room = (r.right() - s.pad() - x).max(0.0);
                if g.size().x > room + 0.5 && self.width.is_some() {
                    // A fixed width narrower than the label: lay it out again, cut.
                    let cut = paint::galley_trunc(ui, label, text_font, t.fg, room);
                    paint::text_at(painter, &cut, r.left() + s.pad(), cy, ink);
                    x = r.left() + s.pad() + cut.size().x + gap;
                } else {
                    paint::text_at(painter, g, x, cy, ink);
                    x += g.size().x + gap;
                }
            }
            if let Some(icon) = self.trailing {
                let ir =
                    Rect::from_min_size(egui::pos2(x, cy - icon_px / 2.0), Vec2::splat(icon_px));
                icon.paint(painter, ir, ink);
            }
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(painter, r, radius, &t);
            }
        }
        let resp = if self.enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            crate::pad::registry::dormant(ui.ctx(), resp.id);
            if self.tip.is_some_and(|(text, _, _)| !text.is_empty()) {
                super::tooltip::reachable(&resp);
            }
            resp
        };
        match self.tip {
            Some((text, kbd, side)) => resp.tip_kbd(text, kbd, side),
            None => resp,
        }
    }
}

/// A button that is only an icon; its label is the tooltip and the accessible name.
/// `IconButton::new(lucide::X, "Close").kbd("Esc")`. Ghost, `Size::Icon` by default.
#[must_use = "add it with ui.add(…)"]
pub struct IconButton<'a> {
    icon: Icon,
    label: &'a str,
    kbd: &'a str,
    side: Side,
    size: Size,
    variant: Variant,
    active: bool,
    enabled: bool,
    danger_hover: bool,
    tint: Option<Color32>,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: Icon, label: &'a str) -> Self {
        Self {
            icon,
            label,
            kbd: "",
            side: Side::Bottom,
            size: Size::Icon,
            variant: Variant::Ghost,
            active: false,
            enabled: true,
            danger_hover: false,
            tint: None,
        }
    }

    /// The shortcut shown beside the hint.
    pub fn kbd(mut self, kbd: &'a str) -> Self {
        self.kbd = kbd;
        self
    }

    /// Where the hint opens (default below).
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// `Size::Icon` (28) or `Size::IconXs` (22).
    pub fn size(mut self, s: Size) -> Self {
        self.size = if s == Size::IconXs {
            Size::IconXs
        } else {
            Size::Icon
        };
        self
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }

    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// `hoverTone="danger"`: for one that removes or hides something.
    pub fn danger_hover(mut self) -> Self {
        self.danger_hover = true;
        self
    }

    /// Colours the icon (`iconClass="text-…"`), whatever the variant says.
    pub fn tint(mut self, c: Color32) -> Self {
        self.tint = Some(c);
        self
    }
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let h = self.size.height();
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(h), sense);
        let label = self.label;
        let on = self.enabled;
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, on, label));
        if ui.is_rect_visible(rect) {
            let hover = paint::ease(ui, resp.id.with("hover"), self.enabled && resp.hovered());
            let press = self.enabled && resp.is_pointer_button_down_on();
            let v = if self.active {
                Variant::Accent
            } else {
                self.variant
            };
            let l = if self.enabled {
                look(&t, v, hover, press, false, self.danger_hover)
            } else {
                disabled_look(&t, v)
            };
            let scale = if press && self.size == Size::Icon {
                0.96
            } else {
                1.0
            };
            let r = Rect::from_center_size(rect.center(), rect.size() * scale);
            let painter = ui.painter();
            if let Some(glow) = l.glow {
                painter.add(glow.as_shape(r, CornerRadius::same(size::RADIUS_MD)));
            }
            paint::plate(painter, r, size::RADIUS_MD, l.fill, l.border);
            let ink = match self.tint {
                Some(c) if self.enabled && !self.active && hover < 0.5 => c,
                _ => l.ink,
            };
            let px = self.size.icon();
            self.icon.paint(
                painter,
                Rect::from_center_size(r.center(), Vec2::splat(px)),
                ink,
            );
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(painter, r, size::RADIUS_MD, &t);
            }
        }
        let resp = if self.enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            crate::pad::registry::dormant(ui.ctx(), resp.id);
            if !self.label.is_empty() {
                super::tooltip::reachable(&resp);
            }
            resp
        };
        resp.tip_kbd(self.label, self.kbd, self.side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_footer_gives_its_buttons_their_pad_keys() {
        assert_eq!(footer_key(0, 2, Variant::Ghost), Some(PadKey::Back));
        assert_eq!(footer_key(1, 2, Variant::Accent), Some(PadKey::Confirm));
        assert_eq!(footer_key(1, 3, Variant::Default), None);
        assert_eq!(footer_key(0, 1, Variant::Ghost), Some(PadKey::Back));
        assert_eq!(footer_key(0, 1, Variant::Danger), Some(PadKey::Confirm));
        // A button more than last frame's count waits a frame.
        assert_eq!(footer_key(2, 2, Variant::Accent), None);
        assert_eq!(footer_key(0, 0, Variant::Accent), None);
        footer_begin(2);
        assert_eq!(footer_next(Variant::Ghost), Some(PadKey::Back));
        assert_eq!(footer_next(Variant::Accent), Some(PadKey::Confirm));
        assert_eq!(footer_end(), 2);
        assert_eq!(footer_next(Variant::Accent), None);
    }

    /// Puts the normal density back when dropped, even when an assert fails first.
    struct NormalDensity;
    impl Drop for NormalDensity {
        fn drop(&mut self) {
            size::set_pad_density(false);
        }
    }

    #[test]
    fn a_footer_of_three_gives_b_and_x_to_its_ends() {
        assert_eq!(footer_key(0, 3, Variant::Ghost), Some(PadKey::Back));
        assert_eq!(footer_key(1, 3, Variant::Default), None);
        assert_eq!(footer_key(2, 3, Variant::Accent), Some(PadKey::Confirm));
        assert_eq!(footer_key(3, 3, Variant::Accent), None);
    }

    #[test]
    fn the_pad_density_grows_controls_and_small_text() {
        let _normal = NormalDensity;
        size::set_pad_density(true);
        assert_eq!(Size::Sm.height(), size::CONTROL + size::PAD_GROW);
        assert_eq!(size::text(size::TEXT_3XS), size::PAD_MIN_TEXT);
        assert_eq!(size::text(size::TEXT_LG), size::TEXT_LG);
        assert!(size::ring() > size::RING);
        assert!(size::set_pad_density(false));
        assert_eq!(Size::Sm.height(), size::CONTROL);
        assert_eq!(size::text(size::TEXT_3XS), size::TEXT_3XS);
        assert!(pad_chips().is_none());
    }
}
