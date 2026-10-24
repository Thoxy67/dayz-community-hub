//! The on-screen keyboard (`kit::osk`) in the window's words, with the buttons of the
//! pad in hand beside its Done button.

use egui::{Ui, Vec2};

use super::pad::{PadButton, PadGlyph, kind};
use crate::app::Ctx;
use crate::i18n::t;
use crate::kit::{osk, paint};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// After the views and the dialogs: the keyboard, when it is up.
pub fn show(cx: &Ctx) {
    if !osk::is_open(cx.egui) {
        return;
    }
    let labels = osk::Labels {
        done: t("pad", "done"),
        shift: t("pad", "oskShift"),
        backspace: t("pad", "oskBackspace"),
        space: t("pad", "oskSpace"),
    };
    let pad = crate::kit::tooltip::pad_mode(cx.egui);
    osk::show(cx.egui, &labels, |ui| {
        if pad {
            hints(ui);
        }
    });
}

/// What the pad's buttons do on the keyboard, as far as there is room.
fn hints(ui: &mut Ui) {
    let tk = ui.tokens();
    let k = kind(ui);
    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
    let items: [(&[PadButton], &str); 5] = [
        (&[PadButton::A], t("pad", "oskType")),
        (&[PadButton::X], t("pad", "oskBackspace")),
        (&[PadButton::Y], t("pad", "oskShift")),
        (&[PadButton::Lb, PadButton::Rb], t("pad", "oskCaret")),
        (&[PadButton::Menu], t("pad", "done")),
    ];
    for (buttons, label) in items {
        let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_XS), tk.fg_muted);
        let glyphs: f32 = buttons
            .iter()
            .map(|b| PadGlyph::new(*b, k).width(ui) + 4.0)
            .sum();
        // Only whole hints: one that does not fit is left out.
        if ui.available_width() < glyphs + g.size().x + 12.0 {
            break;
        }
        for b in buttons {
            ui.add(PadGlyph::new(*b, k));
        }
        let (r, _) = ui.allocate_exact_size(
            Vec2::new(g.size().x + 8.0, size::control()),
            egui::Sense::hover(),
        );
        paint::text_at(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
    }
}
