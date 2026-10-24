//! `ui/tag`: a small label in a tone (`rounded-sm px-1.5 py-px font-sans text-2xs
//! leading-4`): tinted (`bg-<tone>/12 text-<tone> ring-1 ring-<tone>/25`) or filled.

use egui::{CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::Tone;
use super::paint::{self, alpha};
use super::tooltip::TipExt;
use crate::icons::Icon;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// `Tag::new("1PP").tone(Tone::Warn)`, `Tag::new("OFFICIAL").tone(Tone::Ok).icon(lucide::BADGE_CHECK)`.
#[must_use = "add it with ui.add(…)"]
pub struct Tag<'a> {
    text: &'a str,
    tone: Tone,
    filled: bool,
    icon: Option<Icon>,
    tip: &'a str,
    spinner: bool,
    brand: Option<super::Brand>,
    trailing: Option<Icon>,
}

impl<'a> Tag<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            tone: Tone::Neutral,
            filled: false,
            icon: None,
            tip: "",
            spinner: false,
            brand: None,
            trailing: None,
        }
    }

    /// A turning spinner first (`<Spinner class="size-2.5 text-current" />`).
    pub fn spinner(mut self) -> Self {
        self.spinner = true;
        self
    }

    /// A brand mark (Steam) before the text, after the icon or spinner.
    pub fn brand(mut self, b: super::Brand) -> Self {
        self.brand = Some(b);
        self
    }

    /// An icon after the text (a retry glyph).
    pub fn trailing(mut self, icon: Icon) -> Self {
        self.trailing = Some(icon);
        self
    }

    /// Neutral, Ok, Warn, Err or Accent (others read as neutral).
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Fill the tone rather than tinting it: told apart without relying on hue.
    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn tip(mut self, tip: &'a str) -> Self {
        self.tip = tip;
        self
    }
}

impl Widget for Tag<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let (fill, ink, ring) = if self.filled {
            match self.tone {
                Tone::Ok => (t.ok, t.bg, None),
                Tone::Warn => (t.warn, t.bg, None),
                Tone::Err => (t.err, t.bg, None),
                Tone::Accent => (t.accent, t.accent_fg, None),
                _ => (t.fg_muted, t.bg, None),
            }
        } else {
            let c = match self.tone {
                Tone::Ok => Some(t.ok),
                Tone::Warn => Some(t.warn),
                Tone::Err => Some(t.err),
                Tone::Accent => Some(t.accent),
                _ => None,
            };
            match c {
                Some(c) => (alpha(c, 0.12), c, Some(alpha(c, 0.25))),
                None => (alpha(t.raised, 0.7), t.fg_muted, Some(alpha(t.border, 0.6))),
            }
        };
        let g = paint::galley(ui, self.text, font(Face::Sans, size::TEXT_2XS), ink);
        let icon_w = self.icon.map_or(0.0, |_| 12.0 + 4.0)
            + if self.spinner { 10.0 + 4.0 } else { 0.0 }
            + self.brand.map_or(0.0, |_| 10.0 + 4.0)
            + self.trailing.map_or(0.0, |_| 10.0 + 4.0);
        let sz = Vec2::new(g.size().x + icon_w + 12.0, 18.0);
        let (rect, resp) = ui.allocate_exact_size(sz, Sense::hover());
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            p.rect_filled(rect, CornerRadius::same(size::RADIUS_SM), fill);
            if let Some(ring) = ring {
                p.rect_stroke(
                    rect,
                    CornerRadius::same(size::RADIUS_SM),
                    Stroke::new(1.0, ring),
                    StrokeKind::Outside,
                );
            }
            let mut x = rect.left() + 6.0;
            if let Some(icon) = self.icon {
                icon.paint(
                    p,
                    Rect::from_min_size(egui::pos2(x, rect.center().y - 6.0), Vec2::splat(12.0)),
                    ink,
                );
                x += 16.0;
            }
            let small = |x: f32| {
                Rect::from_min_size(egui::pos2(x, rect.center().y - 5.0), Vec2::splat(10.0))
            };
            if self.spinner {
                super::Spinner::paint(ui, small(x), ink);
                x += 14.0;
            }
            if let Some(b) = self.brand {
                b.paint(p, small(x), ink);
                x += 14.0;
            }
            paint::text_at(p, &g, x, rect.center().y, ink);
            if let Some(icon) = self.trailing {
                icon.paint(p, small(x + g.size().x + 4.0), ink);
            }
        }
        resp.tip_kbd(self.tip, "", super::Side::Top)
    }
}
