//! What the buttons do, while a controller drives the window (= `PadHints.svelte`): at
//! most five, always in the same order (A, B, X, Y, then Menu, the bumpers, R3, View as
//! room allows), only those that apply. A dialog shows its own; the palette its own. The
//! words are the registry's hints (`commands::SHORTCUTS`).
//!
//! The bar sits just above the status bar, over the foot of the main area.

use egui::{Area, Context, Id, Order, Rect, Sense, Stroke, Vec2, pos2, vec2};

use super::{PAD_HINTS_HEIGHT, STATUSBAR_HEIGHT};
use crate::app::Ctx;
use crate::commands;
use crate::i18n::t;
use crate::icons::lucide;
use crate::parts::{PadButton, PadGlyph};
use crate::store::pad::InputMode;
use crate::theme::{TokensExt, font, fonts::Face, size};

const MAX: usize = 5;

/// The words a shortcut gives the bar.
fn hint_of(id: &str) -> &'static str {
    commands::shortcut(id)
        .and_then(|s| s.hint)
        .map_or("", |h| t("pad", h))
}

/// The hints as they stand: buttons and words.
fn hints(ctx: &Context, cx: &Ctx) -> Vec<(Vec<PadButton>, &'static str)> {
    let mut out: Vec<(Vec<PadButton>, &'static str)> = Vec::with_capacity(MAX + 3);
    let d = &cx.stores.dialogs;
    if d.palette {
        out.push((vec![PadButton::A], hint_of("palette.run")));
        out.push((vec![PadButton::B], hint_of("palette.close")));
        out.push((
            vec![PadButton::Lb, PadButton::Rb],
            hint_of("palette.section"),
        ));
        if cx.stores.pad.steam_ui {
            out.push((vec![PadButton::Y], hint_of("palette.keyboard")));
        }
        return out;
    }
    // A text field, not any focused widget: the controller's focus is egui's focus.
    let typing = ctx.text_edit_focused();
    let confirm = d.pending().is_some();
    let in_dialog = d.open() || ctx.memory(|m| m.top_modal_layer().is_some());
    let view = if in_dialog {
        crate::pad::ViewHints::default()
    } else {
        crate::pad::view_hints(cx)
    };
    out.push((
        vec![PadButton::A],
        t(
            "pad",
            if typing {
                "type"
            } else if view.join {
                "join"
            } else {
                "select"
            },
        ),
    ));
    out.push((
        vec![PadButton::B],
        t(
            "pad",
            if in_dialog {
                "close"
            } else if typing {
                "done"
            } else {
                "back"
            },
        ),
    ));
    if in_dialog {
        if confirm {
            out.push((vec![PadButton::X], hint_of("dialog.confirm")));
        }
        return out;
    }
    for (id, pb) in [(view.x, PadButton::X), (view.y, PadButton::Y)] {
        if let Some(id) = id {
            out.push((vec![pb], hint_of(id)));
        }
    }
    out.push((vec![PadButton::Menu], hint_of("palette.toggle")));
    out.push((vec![PadButton::Lb, PadButton::Rb], hint_of("view.prev")));
    if let Some(id) = view.r3 {
        out.push((vec![PadButton::R3], hint_of(id)));
    }
    out.push((vec![PadButton::View], hint_of("sheet.toggle")));
    out.truncate(MAX);
    out
}

pub(super) fn show(ctx: &Context, cx: &mut Ctx) {
    if cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) != InputMode::Gamepad {
        return;
    }
    let screen = ctx.content_rect();
    let rect = Rect::from_min_max(
        pos2(
            screen.left(),
            screen.bottom() - STATUSBAR_HEIGHT - PAD_HINTS_HEIGHT,
        ),
        pos2(screen.right(), screen.bottom() - STATUSBAR_HEIGHT),
    );
    let list = hints(ctx, cx);
    let kind = cx.stores.pad.kind(cx.prefs.get().pad_glyphs);
    let name = cx.stores.pad.pads.first().map(|p| p.name.clone());
    Area::new(Id::new("dzch.pad-hints"))
        .order(Order::Middle)
        .fixed_pos(rect.min)
        .interactable(false)
        .show(ctx, |ui| {
            let tk = ui.tokens();
            let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, 0.0, tk.panel);
            p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, tk.border));
            let cy = r.center().y + 0.5;
            let mut x = r.left() + size::PAD;
            for (buttons, text) in &list {
                for b in buttons {
                    let gr = Rect::from_min_size(pos2(x, cy - 9.0), vec2(48.0, 18.0));
                    let mut gui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(gr)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    let used = gui.add(PadGlyph::new(*b, kind)).rect;
                    x = used.right() + 4.0;
                }
                let g = crate::kit::paint::galley(
                    ui,
                    text,
                    font(Face::Sans, size::TEXT_2XS),
                    tk.fg_muted,
                );
                crate::kit::paint::text_at(ui.painter(), &g, x, cy, tk.fg_muted);
                x += g.size().x + 14.0;
            }
            if let Some(n) = name {
                let g = crate::kit::paint::galley_trunc(
                    ui,
                    &n,
                    font(Face::Sans, size::TEXT_2XS),
                    tk.fg_faint,
                    (r.right() - size::PAD - x - 20.0).max(0.0),
                );
                let gx = r.right() - size::PAD - g.size().x;
                crate::kit::paint::text_at(ui.painter(), &g, gx, cy, tk.fg_faint);
                lucide::GAMEPAD_2.paint(
                    ui.painter(),
                    Rect::from_min_size(pos2(gx - 6.0 - 14.0, cy - 7.0), Vec2::splat(14.0)),
                    tk.fg_faint,
                );
            }
        });
}
