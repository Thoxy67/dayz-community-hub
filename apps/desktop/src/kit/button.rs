//! `ui/button` (Button, IconButton) and `ui/chip`.
//!
//! Heights are the theme's measures: `Lg` 34, `Sm` 28 (the default), `Xs` 22, and the
//! square `Icon` 28 / `IconXs` 22. Colours ease over `--duration-fast`; a press sinks the
//! button a little (`active:scale-[0.98]`).

use egui::{
    Color32, CornerRadius, Rect, Response, Sense, Ui, Vec2, Widget, WidgetInfo, WidgetType,
};

use super::paint::{self, alpha, lerp};
use super::tooltip::{Side, TipExt};
use crate::icons::Icon;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

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
            Size::Lg => size::CONTROL_LG,
            Size::Sm | Size::Icon => size::CONTROL,
            Size::Xs | Size::IconXs => size::CONTROL_SM,
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
        }
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

    /// `ui/chip`: an `Xs` button that says "on" by outline, not by fill.
    pub fn chip(label: &'a str, on: bool) -> Self {
        let mut b = Self::new(label).size(Size::Xs);
        b.chip = true;
        b.active = on;
        b
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
                WidgetInfo::selected(WidgetType::Button, on, true, label)
            } else {
                WidgetInfo::labeled(WidgetType::Button, on, label)
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
            if let Some(icon) = self.icon {
                let ir =
                    Rect::from_min_size(egui::pos2(x, cy - icon_px / 2.0), Vec2::splat(icon_px));
                if self.spinning {
                    super::Spinner::paint(ui, ir, ink);
                    ui.ctx().request_repaint();
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
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, on, label));
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
            resp
        };
        resp.tip_kbd(self.label, self.kbd, self.side)
    }
}
