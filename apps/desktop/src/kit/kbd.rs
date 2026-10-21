//! `ui/kbd`: a key, as a menu or a hint shows a shortcut.
//! `rounded-xs border bg-bg px-1 font-mono text-3xs leading-3.5 text-fg-faint`, with a
//! one-pixel drop under it.

use egui::{Color32, CornerRadius, Rect, Response, Sense, Ui, Vec2, Widget};

use super::paint;
use crate::theme::{TokensExt, font, fonts::Face, size};

const H: f32 = 16.0;

/// `Kbd::new("Ctrl K")`.
#[must_use = "add it with ui.add(…)"]
pub struct Kbd<'a> {
    keys: &'a str,
}

impl<'a> Kbd<'a> {
    pub fn new(keys: &'a str) -> Self {
        Self { keys }
    }

    /// The width it will take, for layouts that reserve room for it.
    pub fn width(ui: &Ui, keys: &str) -> f32 {
        let g = paint::galley(ui, keys, font(Face::Mono, size::TEXT_3XS), Color32::WHITE);
        g.size().x + 10.0
    }

    /// Paints it with its left edge at `x`, centred on `cy`; returns its rect.
    pub fn paint_at(ui: &Ui, keys: &str, x: f32, cy: f32) -> Rect {
        let t = ui.tokens();
        let g = paint::galley(ui, keys, font(Face::Mono, size::TEXT_3XS), t.fg_faint);
        let r = Rect::from_min_size(egui::pos2(x, cy - H / 2.0), Vec2::new(g.size().x + 10.0, H));
        let p = ui.painter();
        p.rect_filled(
            r.translate(egui::vec2(0.0, 1.0)),
            CornerRadius::same(size::RADIUS_XS),
            Color32::from_black_alpha(89),
        );
        paint::plate(p, r, size::RADIUS_XS, t.bg, t.border);
        paint::text_center(p, &g, r, t.fg_faint);
        r
    }
}

impl Widget for Kbd<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = Self::width(ui, self.keys);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, H + 1.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            Self::paint_at(ui, self.keys, rect.left(), rect.top() + H / 2.0);
        }
        resp
    }
}
