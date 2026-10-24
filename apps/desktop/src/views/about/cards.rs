//! The three cards under the update card, side by side from `lg` up and as tall as the
//! tallest (a grid row): the machine (and a copy of it for a bug report), where the data
//! comes from, the keys or the controller's buttons.

use egui::{
    Align, CornerRadius, Frame, Layout, Margin, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    Ui, UiBuilder, Vec2, pos2, vec2,
};
use serde_json::Value;

use super::update::{icon, stencil, words};
use super::{AboutState, GAP, Run, SOURCES, gb};
use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Size, Skeleton};
use crate::parts::ShortcutMode;
use crate::store::pad::InputMode;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// Three columns from this window width (`lg:grid-cols-3`).
const LG: f32 = 1024.0;

pub fn show(ui: &mut Ui, cx: &mut Ctx, st: &mut AboutState, run: &mut Run) {
    let w = ui.available_width();
    let wide = ui.ctx().content_rect().width() >= LG;
    let hid = ui.scope_id().with("about.cards.h");
    let row_h: f32 = ui.data(|d| d.get_temp(hid)).unwrap_or(0.0);
    let pad = cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) == InputMode::Gamepad;
    let kind = cx.stores.pad.kind(cx.prefs.get().pad_glyphs);
    let pad_name = cx.stores.pad.pads.first().map(|p| p.name.clone());
    let mode = st.keys_mode.get_or_insert(if pad {
        ShortcutMode::Pad
    } else {
        ShortcutMode::Keys
    });

    let mut draw = |ui: &mut Ui, i: usize, min_h: f32| -> f32 {
        match i {
            0 => card(ui, t("about", "system"), lucide::CPU, min_h, |ui| {
                machine(ui, st_specs(&st.specs), run);
            }),
            1 => card(
                ui,
                t("about", "dataSources"),
                lucide::DATABASE,
                min_h,
                |ui| {
                    sources(ui, run);
                },
            ),
            _ => card(ui, t("keys", "title"), lucide::KEYBOARD, min_h, |ui| {
                crate::shell::shortcut_list(ui, mode, kind);
                if let Some(name) = pad_name.as_deref() {
                    let tk = ui.tokens();
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        icon(ui, lucide::GAMEPAD_2, 14.0, tk.fg_faint);
                        ui.add(
                            egui::Label::new(
                                RichText::new(name)
                                    .font(font(Face::Sans, size::TEXT_2XS))
                                    .color(tk.fg_faint),
                            )
                            .truncate(),
                        );
                    });
                }
            }),
        }
    };

    if wide {
        let cw = ((w - 2.0 * GAP) / 3.0).max(0.0);
        let origin = ui.cursor().min;
        let mut tallest = 0.0_f32;
        for i in 0..3 {
            let rect = Rect::from_min_size(
                pos2(origin.x + i as f32 * (cw + GAP), origin.y),
                vec2(cw, f32::INFINITY),
            );
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(rect)
                    .layout(Layout::top_down(Align::Min))
                    .id_salt(("about.card", i)),
            );
            tallest = tallest.max(draw(&mut child, i, row_h));
        }
        ui.allocate_rect(
            Rect::from_min_size(origin, vec2(w, tallest.max(row_h))),
            Sense::hover(),
        );
        if (tallest - row_h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(hid, tallest));
            ui.ctx().request_repaint();
        }
    } else {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = GAP;
            for i in 0..3 {
                draw(ui, i, 0.0);
            }
        });
    }
}

/// The specs, the error, or nothing yet.
enum Specs<'a> {
    Loading,
    Failed(&'a str),
    Ready(&'a dz_hub::features::system::SystemSpecsDto),
}

fn st_specs(f: &crate::rt::Fetch<dz_hub::features::system::SystemSpecsDto>) -> Specs<'_> {
    match (&f.value, &f.error) {
        (Some(s), _) => Specs::Ready(s),
        (None, Some(e)) => Specs::Failed(e),
        _ => Specs::Loading,
    }
}

/// A card (`rounded-md border bg-bg/60`): the accent icon and stencilled title over a
/// rule, then the body (`px-pad py-2.5`), at least `min_h` tall. Returns its own height.
fn card(ui: &mut Ui, title: &str, ic: Icon, min_h: f32, body: impl FnOnce(&mut Ui)) -> f32 {
    let tk = ui.tokens();
    let slot = ui.painter().add(Shape::Noop);
    let top = ui.cursor().min;
    let w = ui.available_width();
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let inner = (w - 2.0 * size::PAD).max(0.0);
    let head = Frame::new()
        .inner_margin(Margin::symmetric(size::PAD as i8, 8))
        .show(ui, |ui| {
            ui.set_width(inner);
            ui.spacing_mut().interact_size.y = 0.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                icon(ui, ic, size::ICON_SM, tk.accent);
                stencil(ui, title);
            });
        })
        .response
        .rect;
    Frame::new()
        .inner_margin(Margin::symmetric(size::PAD as i8, 10))
        .show(ui, |ui| {
            ui.set_width(inner);
            body(ui);
        });
    let natural = ui.min_rect().bottom().max(ui.cursor().top()) - top.y;
    let h = natural.max(min_h);
    let rect = Rect::from_min_size(top, vec2(w, h));
    let radius = CornerRadius::same(size::RADIUS_MD);
    ui.painter().set(
        slot,
        Shape::Vec(vec![
            Shape::rect_filled(rect, radius, alpha(tk.bg, 0.6)),
            Shape::hline(
                rect.x_range(),
                head.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.6)),
            ),
            Shape::rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, tk.border),
                StrokeKind::Inside,
            ),
        ]),
    );
    natural
}

/// "This machine": a list of what it is, a copy of it for a bug report, and what for.
fn machine(ui: &mut Ui, specs: Specs<'_>, run: &mut Run) {
    let tk = ui.tokens();
    let s = match specs {
        Specs::Ready(s) => s,
        Specs::Failed(e) => {
            ui.add(
                egui::Label::new(
                    RichText::new(e)
                        .font(font(Face::Mono, size::TEXT_2XS))
                        .color(tk.err),
                )
                .wrap()
                .selectable(true),
            );
            return;
        }
        Specs::Loading => {
            ui.spacing_mut().item_spacing.y = 8.0;
            for _ in 0..3 {
                ui.add(
                    Skeleton::new(ui.available_width(), 16.0)
                        .radius(size::RADIUS_XS)
                        .opacity(0.6),
                );
            }
            return;
        }
    };
    ui.spacing_mut().item_spacing.y = 8.0;
    if let Some(os) = s.os.as_deref() {
        fact(ui, lucide::MONITOR, t("about", "systemOs"), |ui| {
            value(ui, os, Face::Sans, size::TEXT_XS, tk.fg);
        });
    }
    fact(ui, lucide::CPU, t("about", "systemCpu"), |ui| {
        if let Some(name) = s.cpu_name.as_deref() {
            value(ui, name, Face::Sans, size::TEXT_XS, tk.fg);
        }
        let cores = tf(
            "about",
            "systemCores",
            &[
                ("physical", &s.physical_cores),
                ("logical", &s.logical_cores),
            ],
        );
        value(ui, &cores, Face::Mono, size::TEXT_2XS, tk.fg_muted);
    });
    fact(ui, lucide::MEMORY_STICK, t("about", "systemMemory"), |ui| {
        value(ui, &gb(s.total_memory_mb), Face::Mono, size::TEXT_XS, tk.fg);
    });
    fact(ui, lucide::GPU, t("about", "systemGpu"), |ui| {
        if s.gpus.is_empty() {
            words(
                ui,
                t("about", "systemNoGpu"),
                Face::Sans,
                size::TEXT_XS,
                tk.fg_faint,
            );
        }
        for (i, g) in s.gpus.iter().take(16).enumerate() {
            if i > 0 {
                ui.add_space(4.0);
            }
            value(ui, &g.name, Face::Sans, size::TEXT_XS, tk.fg);
            if let Some(v) = g.vram_mb {
                let vram = format!("{} VRAM", gb(v));
                value(ui, &vram, Face::Mono, size::TEXT_2XS, tk.fg_muted);
            }
        }
    });
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        r.x_range(),
        r.center().y,
        Stroke::new(1.0, alpha(tk.border, 0.5)),
    );
    ui.add_space(2.0);
    let b = Button::new(t("about", "copyInfo"))
        .icon(lucide::CLIPBOARD_COPY)
        .size(Size::Xs);
    if ui.add(b).clicked() {
        run.push(("about.copy_system_info", Value::Null));
    }
    words(
        ui,
        t("about", "systemHint"),
        Face::Sans,
        size::TEXT_2XS,
        tk.fg_faint,
    );
}

/// One line of the machine's list: the term (icon and words, `w-24`), then its value.
fn fact(ui: &mut Ui, ic: Icon, term: &str, value: impl FnOnce(&mut Ui)) {
    let tk = ui.tokens();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
        ui.allocate_ui_with_layout(
            vec2(96.0, 0.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_width(96.0);
                ui.spacing_mut().item_spacing.x = 6.0;
                icon(ui, ic, 14.0, tk.fg_faint);
                words(ui, term, Face::Sans, size::TEXT_XS, tk.fg_faint);
            },
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            value(ui);
        });
    });
}

/// Words a player may copy (`data-selectable`).
fn value(ui: &mut Ui, s: &str, face: Face, px: f32, color: egui::Color32) {
    ui.add(
        egui::Label::new(RichText::new(s).font(font(face, px)).color(color))
            .wrap()
            .selectable(true),
    );
}

/// "Where the data comes from": each source, its site a click away.
fn sources(ui: &mut Ui, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    for (i, src) in SOURCES.iter().enumerate() {
        let room = w - 12.0 - 14.0 - 10.0 - if src.url.is_some() { 22.0 } else { 0.0 };
        let title = wrap(
            ui,
            t("about", src.title),
            Face::Sans,
            size::TEXT_XS,
            1.45,
            tk.fg,
            room,
        );
        let desc = wrap(
            ui,
            t("about", src.desc),
            Face::Sans,
            size::TEXT_2XS,
            1.375,
            tk.fg_faint,
            room,
        );
        let h = 6.0 + title.size().y + desc.size().y + 6.0;
        let sense = if src.url.is_some() {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
        if src.url.is_some() {
            resp.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Link, true, t("about", src.title))
            });
        }
        let hover = if src.url.is_some() {
            paint::ease(ui, resp.id.with("h"), resp.hovered() || resp.has_focus())
        } else {
            0.0
        };
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(ui.painter(), rect, size::RADIUS_SM, &tk);
        }
        let p = ui.painter();
        if hover > 0.0 {
            p.rect_filled(
                rect,
                CornerRadius::same(size::RADIUS_SM),
                alpha(tk.raised, 0.5 * hover),
            );
        }
        let x = rect.left() + 6.0;
        let ir = Rect::from_min_size(pos2(x, rect.top() + 6.0 + 2.0), Vec2::splat(14.0));
        src.icon.paint(p, ir, tk.accent);
        let tx = ir.right() + 10.0;
        p.galley(pos2(tx, rect.top() + 6.0), title.clone(), tk.fg);
        p.galley(
            pos2(tx, rect.top() + 6.0 + title.size().y),
            desc,
            tk.fg_faint,
        );
        if hover > 0.0 {
            let er = Rect::from_min_size(
                pos2(rect.right() - 6.0 - 12.0, rect.top() + 6.0 + 2.0),
                Vec2::splat(12.0),
            );
            lucide::EXTERNAL_LINK.paint(p, er, alpha(tk.fg_faint, hover));
        }
        if src.url.is_some() {
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                run.push(("about.open_source_link", serde_json::json!({ "index": i })));
            }
        }
        ui.add_space(2.0);
    }
}

/// Words wrapped at `width` on lines `lh` times the size apart (the CSS line box).
pub fn wrap(
    ui: &Ui,
    text: &str,
    face: Face,
    px: f32,
    lh: f32,
    color: egui::Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: font(face, px),
            color,
            line_height: Some(px * lh),
            ..Default::default()
        },
    );
    job.wrap.max_width = width.max(40.0);
    paint::layout_top(ui, job)
}
