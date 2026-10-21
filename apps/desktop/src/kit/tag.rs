//! `ui/tag`: a small mono label in a tone (`rounded-sm px-1.5 py-px font-mono text-2xs
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
}

impl<'a> Tag<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            tone: Tone::Neutral,
            filled: false,
            icon: None,
            tip: "",
        }
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
        let g = paint::galley(ui, self.text, font(Face::Mono, size::TEXT_2XS), ink);
        let icon_w = self.icon.map_or(0.0, |_| 12.0 + 4.0);
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
            paint::text_at(p, &g, x, rect.center().y, ink);
        }
        resp.tip_kbd(self.tip, "", super::Side::Top)
    }
}
