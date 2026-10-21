//! One installed mod, top to bottom in the order a player asks about it (=
//! `ModDetail.svelte`): is it current (and the button that makes it so), where it is,
//! what it requires, which of your servers run it, whether the game loads it, its ids;
//! repairing and deleting last.

use dz_hub::features::mods::{InstalledModDto, ModSourceDto};
use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde_json::{Value, json};

use super::list::{source_tip, state_look};
use super::model::{self, Where};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::alpha;
use crate::kit::{
    Brand, Button, Copy, IconButton, Meter, MiniSwitch, Side, Size, Tag, TipExt, Tone, Variant,
};
use crate::parts::{DetailList, Section, draw, ph, ph_trunc};
use crate::store::mods::Mods;
use crate::theme::{TokensExt, font, fonts::Face, size};

type Run = Vec<(&'static str, Value)>;

/// "3 days ago" says more than a date for a month; past that, a date alone.
fn recent(ts: i64) -> bool {
    crate::store::now_ms() / 1000 - ts < 30 * 86_400
}

pub(super) fn show(ui: &mut Ui, cx: &Ctx, m: &InstalledModDto, run: &mut Run) {
    let tk = ui.tokens();
    let mods = &cx.stores.mods;
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, CornerRadius::ZERO, tk.panel);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let busy = mods.op_state(m.id).is_some();
    let id = json!({ "id": m.id });

    // ── header ──
    let head = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(size::PAD as i8, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            ui.horizontal_top(|ui| {
                let w = ui.available_width() - size::CONTROL_SM - 8.0;
                let g = crate::kit::paint::galley_wrap(
                    ui,
                    &m.name,
                    font(Face::SansSemibold, size::TEXT_BASE),
                    tk.fg,
                    w.max(40.0),
                );
                let (r, _) =
                    ui.allocate_exact_size(Vec2::new(w.max(40.0), g.size().y), Sense::hover());
                ui.painter().galley(r.min, g, tk.fg);
                if ui
                    .add(
                        IconButton::new(lucide::X, t("mods", "closeDetail"))
                            .size(Size::IconXs)
                            .variant(Variant::Ghost)
                            .kbd("Esc"),
                    )
                    .clicked()
                {
                    run.push(("mods.close_detail", Value::Null));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                state_look(mods, m.id, Some(m)).show(ui, false);
                source_tag(ui, mods, m);
                let g = ph(ui, &m.size_human, font(Face::Mono, size::TEXT_2XS));
                let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, 18.0), Sense::hover());
                draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                let (word, v) = if m.update_available {
                    ("update", Variant::Accent)
                } else {
                    ("redownload", Variant::Default)
                };
                if ui
                    .add(
                        Button::new(t("mods", word))
                            .icon(lucide::REFRESH_CW)
                            .variant(v)
                            .enabled(!busy),
                    )
                    .clicked()
                {
                    run.push(("mods.update_one", id.clone()));
                }
                if ui
                    .add(
                        Button::new("Workshop")
                            .icon(lucide::EXTERNAL_LINK)
                            .tip(t("mods", "openWorkshop")),
                    )
                    .clicked()
                {
                    run.push(("mods.open_workshop", id.clone()));
                }
                if ui
                    .add(
                        IconButton::new(lucide::FOLDER_OPEN, t("mods", "openModFolder"))
                            .variant(Variant::Ghost),
                    )
                    .clicked()
                {
                    run.push(("mods.open_mod_dir", id.clone()));
                }
            });
        });
    let hr = head.response.rect;
    ui.painter().hline(
        rect.x_range(),
        hr.bottom() - 0.5,
        Stroke::new(1.0, tk.border),
    );

    egui::ScrollArea::vertical()
        .id_salt(("mods-detail-body", m.id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui, mods, m, busy, run);
        });
}

fn source_tag(ui: &mut Ui, mods: &Mods, m: &InstalledModDto) {
    let item = mods.steam_item(m.id);
    let answered = mods.steam.as_ref().is_some_and(|s| s.available);
    let w = model::where_of(m, item, answered);
    let tip = source_tip(mods, m, w);
    let mut tag = Tag::new(w.label(false))
        .tip(&tip)
        .tone(if w == Where::Subscribed {
            Tone::Accent
        } else {
            Tone::Neutral
        });
    tag = if m.source == ModSourceDto::Steam {
        tag.brand(Brand::Steam)
    } else {
        tag.icon(lucide::HARD_DRIVE_DOWNLOAD)
    };
    ui.add(tag);
}

/// A small text paragraph in a tone.
fn note(ui: &mut Ui, text: &str, size_px: f32, color: egui::Color32) {
    let w = ui.available_width();
    let g = crate::kit::paint::galley_wrap(ui, text, font(Face::Sans, size_px), color, w);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, color);
}

fn body(ui: &mut Ui, mods: &Mods, m: &InstalledModDto, busy: bool, run: &mut Run) {
    let tk = ui.tokens();
    let id = json!({ "id": m.id });
    let item = mods.steam_item(m.id);
    let answered = mods.steam.as_ref().is_some_and(|s| s.available);
    let w = model::where_of(m, item, answered);

    // ── version ──
    Section::new(t("mods", "sectionVersion"))
        .icon(lucide::HISTORY)
        .first()
        .show(
            ui,
            |_| {},
            |ui| {
                DetailList::new(("mods-version", m.id)).show(ui, |rows| {
                    rows.custom(t("mods", "yourCopy"), |ui| {
                        let d = format::date(m.local_updated);
                        mono(ui, &d, tk.fg, size::TEXT_XS);
                        if recent(m.local_updated) {
                            ui.add_space(6.0);
                            mono(
                                ui,
                                &format::relative(m.local_updated),
                                tk.fg_faint,
                                size::TEXT_2XS,
                            );
                        }
                    });
                    rows.custom(t("mods", "workshopCopy"), |ui| match m.remote_updated {
                        Some(r) => {
                            let ink = if m.update_available { tk.warn } else { tk.fg };
                            mono(ui, &format::date(r), ink, size::TEXT_XS);
                            ui.add_space(6.0);
                            if m.update_available {
                                let days = model::days_behind(m.local_updated, Some(r));
                                let s = if days > 0 {
                                    tf("mods", "daysBehind", &[("days", &days)])
                                } else {
                                    t("mods", "behindToday").to_string()
                                };
                                mono(ui, &s, alpha(tk.warn, 0.8), size::TEXT_2XS);
                            } else if recent(r) {
                                let s = format::relative(r);
                                let g = ph(ui, &s, font(Face::Sans, size::TEXT_2XS));
                                let (rr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                                draw(ui.painter(), &g, rr.left(), rr.center().y, tk.fg_faint);
                            }
                        }
                        None => {
                            let g = ph(
                                ui,
                                t("mods", "detailsUnknown"),
                                font(Face::Sans, size::TEXT_XS),
                            );
                            let (rr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                            draw(ui.painter(), &g, rr.left(), rr.center().y, tk.fg_faint);
                        }
                    });
                });
                if w == Where::Subscribed {
                    note(
                        ui,
                        t("mods", "steamKeepsUpToDate"),
                        size::TEXT_2XS,
                        tk.fg_faint,
                    );
                }
            },
        );

    // ── where it is ──
    Section::new(t("mods", "sectionWhere"))
        .icon(lucide::MAP_PIN)
        .show(
            ui,
            |_| {},
            |ui| {
                note(ui, w.hint(), size::TEXT_XS, tk.fg_muted);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let room = ui.available_width() - size::CONTROL_SM - 4.0;
                    ui.add(
                        Copy::new(&m.path)
                            .labels(t("common", "copy"), t("common", "copied"))
                            .max_width(room.max(40.0)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                IconButton::new(lucide::FOLDER_OPEN, t("mods", "openModFolder"))
                                    .size(Size::IconXs)
                                    .variant(Variant::Ghost),
                            )
                            .clicked()
                        {
                            run.push(("mods.open_mod_dir", id.clone()));
                        }
                    });
                });
                if w == Where::Launcher && item.is_some_and(|i| i.subscribed) {
                    note(ui, t("mods", "subscribedHint"), size::TEXT_2XS, tk.fg_faint);
                } else if w == Where::Steam {
                    let start = ui.cursor().min;
                    note(ui, t("mods", "steamUnknown"), size::TEXT_2XS, tk.fg_faint);
                    if let Some(reason) = mods.steam.as_ref().and_then(|s| s.reason.as_deref()) {
                        let r = Rect::from_min_max(
                            start,
                            Pos2::new(ui.max_rect().right(), ui.cursor().top()),
                        );
                        let _ = ui
                            .interact(r, ui.id().with("steam-reason"), Sense::hover())
                            .tip(reason);
                    }
                }
                if m.other_copy {
                    note(ui, t("mods", "otherCopy"), size::TEXT_2XS, tk.fg_faint);
                }
                let total = mods.total_size();
                let share = if total > 0 {
                    m.size as f64 / total as f64
                } else {
                    0.0
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let s = format!("{} · {:.1}%", format::bytes(m.size as f64), share * 100.0);
                    let g = ph(ui, &s, font(Face::Mono, size::TEXT_2XS));
                    let mw = (ui.available_width() - g.size().x - 8.0).max(8.0);
                    ui.add(Meter::new(share as f32).tone(Tone::Muted).width(mw))
                        .tip(t("mods", "detailsShare"));
                    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                    draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
                });
            },
        );

    // ── dependencies ──
    let requires: &[u64] = mods.workshop(m.id).map_or(&[], |w| w.requires.as_slice());
    let required_by = mods.required_by(m.id);
    if !requires.is_empty() || !required_by.is_empty() {
        Section::new(t("mods", "sectionDependencies"))
            .icon(lucide::NETWORK)
            .show(
                ui,
                |_| {},
                |ui| {
                    for (label, ids) in [
                        ("requiresLabel", requires),
                        ("requiredByLabel", required_by),
                    ] {
                        if ids.is_empty() {
                            continue;
                        }
                        let g = crate::kit::panel::stencil(
                            ui,
                            t("mods", label),
                            egui::Color32::PLACEHOLDER,
                            None,
                        );
                        let (r, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 14.0),
                            Sense::hover(),
                        );
                        draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
                        for dep in ids {
                            dep_line(ui, mods, *dep);
                        }
                    }
                },
            );
    }

    // ── your servers ──
    Section::new(t("mods", "yourServers"))
        .icon(lucide::STAR)
        .show(
            ui,
            |_| {},
            |ui| match mods.usage.get(&m.id) {
                Some(u) if !u.favorites.is_empty() || !u.history.is_empty() => {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for name in &u.favorites {
                        server_line(ui, name, true);
                    }
                    for name in u.history.iter().filter(|n| !u.favorites.contains(n)) {
                        server_line(ui, name, false);
                    }
                }
                _ => note(
                    ui,
                    &tf("mods", "unusedHint", &[("size", &m.size_human)]),
                    size::TEXT_2XS,
                    tk.fg_faint,
                ),
            },
        );

    // ── in game ──
    Section::new(t("mods", "colInGame"))
        .icon(lucide::GAMEPAD_2)
        .show(
            ui,
            |_| {},
            |ui| {
                ui.horizontal(|ui| {
                    let word = if m.managed {
                        "inGameLinked"
                    } else {
                        "inGameNotLinked"
                    };
                    let g = ph(ui, t("mods", word), font(Face::SansMedium, size::TEXT_XS));
                    let (r, _) =
                        ui.allocate_exact_size(Vec2::new(g.size().x, 18.0), Sense::hover());
                    draw(ui.painter(), &g, r.left(), r.center().y, tk.fg);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut on = m.managed;
                        if ui
                            .add(MiniSwitch::new(&mut on).label(t("mods", "detailsLink")))
                            .changed()
                        {
                            run.push(("mods.toggle_managed", id.clone()));
                        }
                    });
                });
                note(
                    ui,
                    t("mods", "detailsLinkHint"),
                    size::TEXT_2XS,
                    tk.fg_faint,
                );
            },
        );

    // ── identifiers ──
    Section::new(t("mods", "sectionIds"))
        .icon(lucide::HASH)
        .show(
            ui,
            |_| {},
            |ui| {
                let sid = m.id.to_string();
                let param = format!("@{}", m.id);
                DetailList::new(("mods-ids", m.id)).show(ui, |rows| {
                    rows.custom(t("mods", "colWorkshopId"), |ui| {
                        ui.add(Copy::new(&sid).labels(t("common", "copy"), t("common", "copied")));
                    });
                    rows.custom(t("mods", "detailsLaunchParam"), |ui| {
                        ui.add(
                            Copy::new(&param).labels(t("common", "copy"), t("common", "copied")),
                        );
                    });
                });
            },
        );

    // ── repair or remove ──
    Section::new(t("mods", "sectionCare"))
        .icon(lucide::WRENCH)
        .show(
            ui,
            |_| {},
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    if ui
                        .add(
                            Button::new(t("mods", "repair"))
                                .icon(lucide::WRENCH)
                                .enabled(!busy)
                                .tip_kbd(t("mods", "repairHint"), "", Side::Top),
                        )
                        .clicked()
                    {
                        run.push(("mods.repair", id.clone()));
                    }
                    if ui
                        .add(
                            Button::new(t("mods", "delete"))
                                .icon(lucide::TRASH_2)
                                .variant(Variant::Danger),
                        )
                        .clicked()
                    {
                        run.push(("mods.delete", id.clone()));
                    }
                });
                note(ui, t("mods", "deleteHint"), size::TEXT_2XS, tk.fg_faint);
            },
        );
}

fn mono(ui: &mut Ui, text: &str, color: egui::Color32, px: f32) {
    let g = ph(ui, text, font(Face::Mono, px));
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    draw(ui.painter(), &g, r.left(), r.center().y, color);
}

/// One required (or requiring) mod: its name, and "not installed" or "Subscribed".
fn dep_line(ui: &mut Ui, mods: &Mods, id: u64) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 18.0), Sense::hover());
    let (tag, ink) = if mods.by_id(id).is_none() {
        (Some(t("mods", "depMissing")), tk.err)
    } else if mods.steam_item(id).is_some_and(|i| i.subscribed) {
        (Some(t("mods", "sourceSubscribed")), tk.fg_faint)
    } else {
        (None, tk.fg_faint)
    };
    let tg = tag.map(|s| ph(ui, s, font(Face::Sans, size::TEXT_2XS)));
    let tw = tg.as_ref().map_or(0.0, |g| g.size().x + 8.0);
    let name = mods.name_of(id);
    let g = ph_trunc(
        ui,
        &name,
        font(Face::Sans, size::TEXT_XS),
        (w - tw).max(8.0),
    );
    draw(ui.painter(), &g, r.left(), r.center().y, tk.fg);
    if let Some(tg) = tg {
        draw(
            ui.painter(),
            &tg,
            r.right() - tg.size().x,
            r.center().y,
            ink,
        );
    }
}

/// A server that runs the mod: a favourite (filled star) or one played on (history).
fn server_line(ui: &mut Ui, name: &str, favorite: bool) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 18.0), Sense::hover());
    let ir = Rect::from_center_size(Pos2::new(r.left() + 6.0, r.center().y), Vec2::splat(12.0));
    if favorite {
        filled_star(ui.painter(), ir, tk.warn);
    } else {
        lucide::HISTORY.paint(ui.painter(), ir, tk.fg_faint);
    }
    let g = ph_trunc(
        ui,
        name,
        font(Face::Sans, size::TEXT_XS),
        (w - 20.0).max(8.0),
    );
    draw(
        ui.painter(),
        &g,
        r.left() + 20.0,
        r.center().y,
        if favorite { tk.fg } else { tk.fg_muted },
    );
}

/// lucide's star, filled (`fill-warn text-warn`): a fan of triangles from its centre.
fn filled_star(p: &egui::Painter, r: Rect, color: egui::Color32) {
    let c = r.center();
    let (outer, inner) = (r.width() * 0.48, r.width() * 0.22);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color);
    for i in 0..10u32 {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let rad = if i % 2 == 0 { outer } else { inner };
        mesh.colored_vertex(c + Vec2::angled(a) * rad, color);
    }
    for i in 0..10u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
    }
    p.add(egui::Shape::mesh(mesh));
}
