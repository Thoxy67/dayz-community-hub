//! `ui/tooltip`: a hint beside a control, with its shortcut.
//!
//! The delays are the provider's (`delayDuration 350`, `skipDelayDuration 400`), set once
//! by [`super::install`]; the first hint waits, and for a moment after one was up the
//! next comes at once. The plate: overlay, border, `rounded-md`, `px-2 py-1`, `text-2xs`,
//! wrapped at 20rem, 6px from the control.

use egui::{RectAlign, Response};

use super::kbd::Kbd;
use super::paint;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// Which side of the control a floating thing opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Top,
    #[default]
    Right,
    Bottom,
    Left,
}

impl Side {
    pub(crate) fn align(self) -> RectAlign {
        match self {
            Side::Top => RectAlign::TOP,
            Side::Right => RectAlign::RIGHT,
            Side::Bottom => RectAlign::BOTTOM,
            Side::Left => RectAlign::LEFT,
        }
    }

    fn alternatives(self) -> &'static [RectAlign] {
        match self {
            Side::Top | Side::Bottom => &[
                RectAlign::BOTTOM,
                RectAlign::TOP,
                RectAlign::RIGHT,
                RectAlign::LEFT,
            ],
            Side::Right | Side::Left => &[
                RectAlign::RIGHT,
                RectAlign::LEFT,
                RectAlign::BOTTOM,
                RectAlign::TOP,
            ],
        }
    }
}

/// The widest a hint grows before it wraps (`max-w-[20rem]`).
const MAX_WIDTH: f32 = 320.0;

/// Hints on any [`Response`]: `ui.add(…).tip("Refresh")`.
pub trait TipExt: Sized {
    /// A hint to the right (`side="right"`, the Svelte default).
    fn tip(self, text: &str) -> Self;
    /// A hint with a shortcut beside it, on `side`.
    fn tip_kbd(self, text: &str, kbd: &str, side: Side) -> Self;
}

impl TipExt for Response {
    fn tip(self, text: &str) -> Self {
        self.tip_kbd(text, "", Side::Right)
    }

    fn tip_kbd(self, text: &str, kbd: &str, side: Side) -> Self {
        if text.is_empty() {
            return self;
        }
        show(&self, text, kbd, side);
        self
    }
}

fn show(resp: &Response, text: &str, kbd: &str, side: Side) {
    let t = resp.ctx.tokens();
    let mut tip = egui::Tooltip::for_enabled(resp);
    if !tip.popup.is_open() {
        return;
    }
    tip.popup = tip
        .popup
        .frame(
            egui::Frame::new()
                .fill(t.overlay)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::same(size::RADIUS_MD))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .shadow(paint::shadow_pop()),
        )
        .align(side.align())
        .align_alternatives(side.alternatives())
        .gap(6.0)
        .width(MAX_WIDTH);
    tip.show(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let f = font(Face::Sans, size::TEXT_2XS);
        let kbd_w = if kbd.is_empty() {
            0.0
        } else {
            Kbd::width(ui, kbd) + 6.0
        };
        let g = paint::galley_wrap(ui, text, f, t.fg, MAX_WIDTH - 18.0 - kbd_w);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
            ui.painter().galley(r.min, g, t.fg);
            if !kbd.is_empty() {
                ui.add(Kbd::new(kbd));
            }
        });
    });
}
