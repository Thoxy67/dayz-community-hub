//! What the buttons do, while a controller drives the window (= `PadHints.svelte`): always
//! in the same order (A, B, a toast's action, X, Y, then Menu, the bumpers, the triggers,
//! R3, View as room allows), only those that apply. A dialog or a popup shows its own; the
//! palette its own. The words are the registry's hints (`commands::SHORTCUTS`).
//!
//! The bar is a panel of its own ([`super::PAD_HINTS_HEIGHT`] high) above the status bar,
//! beside the rail: the main area ends above it, so it never covers the last row.

use egui::{Context, Id, LayerId, Order, Rect, Stroke, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::app::Ctx;
use crate::commands;
use crate::i18n::t;
use crate::icons::lucide;
use crate::parts::{PadButton, PadGlyph};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// Between a glyph and the next, and between a glyph and its words.
const GLYPH_GAP: f32 = 4.0;
/// Between two hints.
const HINT_GAP: f32 = 16.0;

/// One hint: its buttons and what they do.
struct Hint {
    buttons: Vec<PadButton>,
    text: String,
}

fn hint(buttons: &[PadButton], text: &str) -> Hint {
    Hint {
        buttons: buttons.to_vec(),
        text: text.to_string(),
    }
}

/// The words a shortcut gives the bar.
fn hint_of(id: &str) -> &'static str {
    commands::shortcut(id)
        .and_then(|s| s.hint)
        .map_or("", |h| t("pad", h))
}

/// The hints as they stand, the most needed first (the bar drops from the end).
fn hints(ctx: &Context, cx: &Ctx) -> Vec<Hint> {
    let mut out: Vec<Hint> = Vec::with_capacity(10);
    // The on-screen keyboard takes every button while it is up.
    if crate::kit::osk::is_open(ctx) {
        out.push(hint(&[PadButton::A], t("pad", "oskType")));
        out.push(hint(&[PadButton::B], t("pad", "close")));
        out.push(hint(&[PadButton::X], t("pad", "oskBackspace")));
        out.push(hint(&[PadButton::Y], t("pad", "oskShift")));
        out.push(hint(&[PadButton::Lb, PadButton::Rb], t("pad", "oskCaret")));
        out.push(hint(&[PadButton::Menu], t("pad", "done")));
        return out;
    }
    let d = &cx.stores.dialogs;
    if d.palette {
        out.push(hint(&[PadButton::A], hint_of("palette.run")));
        out.push(hint(&[PadButton::B], hint_of("palette.close")));
        out.push(hint(
            &[PadButton::Lb, PadButton::Rb],
            hint_of("palette.section"),
        ));
        if crate::pad::nav::has_keyboard(cx) {
            out.push(hint(&[PadButton::Y], hint_of("palette.keyboard")));
        }
        return out;
    }
    // A text field, not any focused widget: the controller's focus is egui's focus (or
    // the pad rests on a text field, where A types).
    let typing = ctx.text_edit_focused();
    let on_text = typing || crate::pad::on_text(cx);
    let in_dialog = d.open() || ctx.memory(|m| m.top_modal_layer().is_some());
    // A menu or a list open over the window: A picks, B closes it.
    let in_popup = !in_dialog && egui::Popup::is_any_open(ctx);
    let view = if in_dialog || in_popup {
        crate::pad::ViewHints::default()
    } else {
        crate::pad::view_hints(cx)
    };
    let a = if on_text {
        "type"
    } else if view.join {
        "join"
    } else {
        "select"
    };
    out.push(hint(&[PadButton::A], t("pad", a)));
    // Done typing first: B leaves a dialog's field, the dialog stays.
    let b = if typing {
        "done"
    } else if in_dialog || in_popup {
        "close"
    } else {
        "back"
    };
    out.push(hint(&[PadButton::B], t("pad", b)));
    if in_dialog {
        // X presses a dialog's last footer button (the shell's confirm, or any other's).
        if d.pending().is_some() || crate::kit::button::confirm_chip_shown(ctx) {
            out.push(hint(&[PadButton::X], hint_of("dialog.confirm")));
        }
        return out;
    }
    if in_popup {
        return out;
    }
    // A toast's button, while it is up (L3 rejoins otherwise).
    if let Some(text) = crate::kit::toast::action_hint(ctx) {
        out.push(Hint {
            buttons: vec![PadButton::L3],
            text,
        });
    }
    for (id, pb) in [(view.x, PadButton::X), (view.y, PadButton::Y)] {
        if let Some(id) = id {
            out.push(hint(&[pb], hint_of(id)));
        }
    }
    // Menu opens the actions of what has the focus (else the palette, as View does).
    if view.menu {
        out.push(hint(&[PadButton::Menu], hint_of("pad.menu")));
    }
    out.push(hint(&[PadButton::Lb, PadButton::Rb], hint_of("view.prev")));
    let lt = if view.tabs { "tabs" } else { "page" };
    out.push(hint(&[PadButton::Lt, PadButton::Rt], t("pad", lt)));
    if let Some(id) = view.r3 {
        out.push(hint(&[PadButton::R3], hint_of(id)));
    }
    out.push(hint(&[PadButton::View], hint_of("palette.toggle")));
    out.retain(|h| !h.text.is_empty());
    out
}

/// How many of `widths` (each hint's) fit in `room`, in order.
fn fitting(widths: &[f32], room: f32) -> usize {
    let mut x = 0.0;
    let mut n = 0;
    for w in widths {
        let next = if n == 0 { *w } else { x + HINT_GAP + w };
        if next > room {
            break;
        }
        x = next;
        n += 1;
    }
    n
}

/// Whether something veils the window (a dialog, the palette) with no popup over it: the
/// bar is then drawn again over the veil, as what the buttons do there is the point. Not
/// while the on-screen keyboard is up: it shows its own buttons and stays above the bar.
fn over_veil(ctx: &Context, cx: &Ctx) -> bool {
    let d = &cx.stores.dialogs;
    (d.palette || d.open() || ctx.memory(|m| m.top_modal_layer().is_some()))
        && !egui::Popup::is_any_open(ctx)
        && !crate::kit::osk::is_open(ctx)
}

pub(super) fn show(ui: &mut Ui, cx: &mut Ctx) {
    let ctx = ui.ctx().clone();
    if over_veil(&ctx, cx) {
        let r = ui.max_rect();
        // Foreground: over the dialogs' veil and plates (dialogs keep clear of the bar,
        // `kit::dialog::bottom_inset`), under the keyboard and tooltips (`Order::Tooltip`).
        let layer = LayerId::new(Order::Foreground, Id::unique("shell.padhints.over"));
        let mut over = ui.new_child(UiBuilder::new().layer_id(layer).max_rect(r));
        over.set_clip_rect(r);
        draw(&mut over, cx);
    } else {
        draw(ui, cx);
    }
}

fn draw(ui: &mut Ui, cx: &mut Ctx) {
    let ctx = ui.ctx().clone();
    let list = hints(&ctx, cx);
    let kind = cx.stores.pad.kind(cx.prefs.get().pad_glyphs);
    let name = cx.stores.pad.pads.first().map(|p| p.name.clone());
    let tk = ui.tokens();
    let r = ui.max_rect();
    let p = ui.painter().clone();
    p.rect_filled(r, 0.0, tk.panel);
    p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, tk.border));
    let cy = r.center().y + 0.5;
    let text_font = font(Face::SansMedium, size::TEXT_XS);
    let laid: Vec<(Vec<f32>, std::sync::Arc<egui::Galley>)> = list
        .iter()
        .map(|h| {
            let gw = h
                .buttons
                .iter()
                .map(|b| PadGlyph::new(*b, kind).width(ui))
                .collect();
            let g = crate::kit::paint::galley(ui, &h.text, text_font.clone(), tk.fg_muted);
            (gw, g)
        })
        .collect();
    let widths: Vec<f32> = laid
        .iter()
        .map(|(gw, g)| gw.iter().map(|w| w + GLYPH_GAP).sum::<f32>() + g.size().x)
        .collect();
    let room = (r.width() - 2.0 * size::PAD).max(0.0);
    let n = fitting(&widths, room);
    let gh = crate::parts::pad::glyph_h();
    let mut x = r.left() + size::PAD;
    for ((widths, galley), h) in laid.iter().take(n).zip(list.iter()) {
        for (b, w) in h.buttons.iter().zip(widths) {
            let gr = Rect::from_min_size(pos2(x, cy - gh / 2.0), vec2(*w, gh));
            ui.put(gr, PadGlyph::new(*b, kind));
            x += w + GLYPH_GAP;
        }
        crate::kit::paint::text_at(&p, galley, x, cy, tk.fg_muted);
        x += galley.size().x + HINT_GAP;
    }
    // The controller's name at the end, when there is room for it.
    if let Some(n) = name {
        let icon = size::ICON_SM;
        let max = r.right() - size::PAD - x - icon - 6.0;
        if max > 48.0 {
            let g = crate::kit::paint::galley_trunc(
                ui,
                &n,
                font(Face::Sans, size::TEXT_2XS),
                tk.fg_faint,
                max,
            );
            let gx = r.right() - size::PAD - g.size().x;
            crate::kit::paint::text_at(&p, &g, gx, cy, tk.fg_faint);
            lucide::GAMEPAD_2.paint(
                &p,
                Rect::from_min_size(pos2(gx - 6.0 - icon, cy - icon / 2.0), Vec2::splat(icon)),
                tk.fg_faint,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_give_way_from_the_end() {
        assert_eq!(fitting(&[50.0, 50.0, 50.0], 1000.0), 3);
        assert_eq!(fitting(&[50.0, 50.0, 50.0], 50.0 + HINT_GAP + 50.0), 2);
        assert_eq!(fitting(&[50.0, 50.0], 40.0), 0);
        assert_eq!(fitting(&[], 100.0), 0);
    }
}
