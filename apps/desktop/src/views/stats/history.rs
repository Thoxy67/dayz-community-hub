//! Every session (the "Full history" section): a search, the day chip, the sessions newest
//! first a page at a time, each with its length, a Forget button on hover and a way back
//! to the server or the mission. Only the rows on screen are laid out.

use egui::{CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde_json::{Value, json};

use super::places::{place_action, put};
use super::{Run, StatsState, dates, name_of};
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, IconButton, Input, Size, Tag, TipExt, Tone};
use crate::parts::{Section, draw, map_name, ph, ph_trunc};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// A session's row: `py-1` around the 28px buttons.
const ROW_H: f32 = 36.0;

pub fn show(ui: &mut Ui, st: &mut StatsState, run: &mut Run) {
    let count = tf(
        "stats",
        "historyCount",
        &[("shown", &st.rows.len()), ("total", &st.total)],
    );
    Section::new(t("stats", "sectionHistory"))
        .icon(lucide::HISTORY)
        .show(
            ui,
            |ui| {
                let tk = ui.tokens();
                let g = ph(ui, &count, font(Face::Mono, size::TEXT_2XS));
                let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
            },
            |ui| {
                toolbar(ui, st, run);
                ui.spacing_mut().item_spacing.y = 0.0;
                rows(ui, st, run);
                let more = st.next_page();
                if more > 0 {
                    ui.add_space(8.0);
                    if ui
                        .add(Button::new(&tf("stats", "showMore", &[("count", &more)])))
                        .clicked()
                    {
                        run.push(("stats.history_more", Value::Null));
                    }
                }
            },
        );
}

/// The search and, when the history is narrowed to a day, its chip.
fn toolbar(ui: &mut Ui, st: &mut StatsState, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(w, size::CONTROL),
        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
        |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            let r = ui.add(
                Input::new(&mut st.search)
                    .search()
                    .placeholder(t("stats", "historySearch"))
                    .clear_label(t("common", "clearSearch"))
                    .width(384.0_f32.min(w))
                    .id_salt("stats-history-search"),
            );
            if r.changed() {
                st.typed(ui.ctx());
            }
            if let Some(day) = st.day {
                let text = tf("stats", "filterDay", &[("date", &dates::long_day(day))]);
                let g = ph(ui, &text, font(Face::Sans, size::TEXT_2XS));
                let h = size::CONTROL_SM;
                let cw = 8.0 + g.size().x + 4.0 + 16.0 + 2.0;
                let (cr, _) = ui.allocate_exact_size(Vec2::new(cw, h), Sense::hover());
                paint::plate(
                    ui.painter(),
                    cr,
                    size::RADIUS_SM,
                    alpha(tk.accent, 0.08),
                    alpha(tk.accent, 0.35),
                );
                draw(ui.painter(), &g, cr.left() + 8.0, cr.center().y, tk.fg);
                let xr = Rect::from_center_size(
                    Pos2::new(cr.right() - 2.0 - 8.0, cr.center().y),
                    Vec2::splat(16.0),
                );
                let x = ui.interact(xr, Id::new("stats-day-chip-x"), Sense::click());
                let hv = x.hovered();
                if hv {
                    ui.painter().rect_filled(
                        xr,
                        CornerRadius::same(size::RADIUS_XS),
                        alpha(tk.accent, 0.15),
                    );
                }
                lucide::X.paint(
                    ui.painter(),
                    Rect::from_center_size(xr.center(), Vec2::splat(12.0)),
                    if hv { tk.fg } else { tk.fg_faint },
                );
                let x = x
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .tip(t("stats", "filterRemove"));
                if x.clicked() {
                    run.push(("stats.clear_day", Value::Null));
                }
            }
        },
    );
    ui.add_space(8.0);
}

/// The sessions: the whole list's height is taken, the rows on screen are drawn.
fn rows(ui: &mut Ui, st: &StatsState, run: &mut Run) {
    let tk = ui.tokens();
    let n = st.rows.len();
    if n == 0 {
        return;
    }
    let w = ui.available_width();
    let (all, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H * n as f32), Sense::hover());
    let clip = ui.clip_rect();
    let first = (((clip.top() - all.top()) / ROW_H).floor().max(0.0)) as usize;
    let last = ((((clip.bottom() - all.top()) / ROW_H).ceil()).max(0.0) as usize).min(n);
    for i in first..last {
        let Some(r) = st.rows.get(i) else {
            break;
        };
        let rect = Rect::from_min_size(
            Pos2::new(all.left(), all.top() + i as f32 * ROW_H),
            Vec2::new(w, ROW_H),
        );
        let id = Id::new(("stats-session", r.start, r.name.as_str()));
        let hovered = ui.rect_contains_pointer(rect);
        let p = ui.painter().clone();
        if i + 1 < n {
            p.hline(
                rect.x_range(),
                rect.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.4)),
            );
        }
        // `grid-cols-[9.5rem_minmax(0,1fr)_7rem_8.5rem_4.5rem] gap-x-3`
        let name_w = (w - 152.0 - 112.0 - 136.0 - 72.0 - 48.0).max(0.0);
        let x_name = rect.left() + 152.0 + 12.0;
        let x_map = x_name + name_w + 12.0;
        let x_dur = x_map + 112.0 + 12.0;
        let x_act = x_dur + 136.0 + 12.0;
        let cy = rect.center().y;
        let f2 = font(Face::Mono, size::TEXT_2XS);

        let when = ph_trunc(ui, &format::date_time(r.start), f2.clone(), 152.0);
        draw(&p, &when, rect.left(), cy, tk.fg_muted);
        ui.interact(
            Rect::from_min_size(
                Pos2::new(rect.left(), cy - when.size().y / 2.0),
                when.size(),
            ),
            id.with("when"),
            Sense::hover(),
        )
        .tip(&format::relative(r.start));

        let offline = r.kind == dz_hub::features::stats::PlayKind::Offline;
        let tag_w = if offline { 64.0 } else { 0.0 };
        let ng = ph_trunc(
            ui,
            &name_of(r.kind, &r.name),
            font(Face::Sans, size::TEXT_XS),
            (name_w - tag_w).max(0.0),
        );
        draw(&p, &ng, x_name, cy, tk.fg);
        let nr = Rect::from_min_size(Pos2::new(x_name, cy - ng.size().y / 2.0), ng.size());
        if nr.width() > 0.0 {
            ui.interact(nr, id.with("name"), Sense::hover())
                .tip(&r.name);
        }
        if offline {
            let tr = Rect::from_min_size(
                Pos2::new(nr.right() + 6.0, cy - 9.0),
                Vec2::new(tag_w, 18.0),
            );
            let mut tu = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(tr)
                    .id_salt(id.with("tag"))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            tu.add(Tag::new(t("stats", "offlineTag")).tone(Tone::Accent));
        }

        if let Some(m) = r.map.as_deref().filter(|m| !m.is_empty()) {
            let mg = ph_trunc(ui, &map_name(m), font(Face::Sans, size::TEXT_2XS), 112.0);
            draw(&p, &mg, x_map, cy, tk.map);
        }

        let (dur, color) = if r.open {
            (
                format!("{} · {}", format::duration(r.secs), t("stats", "running")),
                tk.ok,
            )
        } else if r.measured {
            (format::duration(r.secs), tk.fg)
        } else {
            (t("stats", "unmeasured").to_owned(), tk.fg_faint)
        };
        let dg = ph_trunc(ui, &dur, f2, 136.0);
        let dr = Rect::from_min_size(
            Pos2::new(x_dur + 136.0 - dg.size().x, cy - dg.size().y / 2.0),
            dg.size(),
        );
        draw(&p, &dg, dr.left(), cy, color);
        if !r.measured {
            ui.interact(dr, id.with("dur"), Sense::hover())
                .tip(t("stats", "unmeasuredHint"));
        }

        // Forget (on hover, finished sessions), then join or launch.
        let right = x_act + 72.0;
        let act = Rect::from_center_size(
            Pos2::new(right - size::CONTROL / 2.0, cy),
            Vec2::splat(size::CONTROL),
        );
        let has_act = place_action_present(r);
        if !r.open && hovered {
            let bx = if has_act { act.left() - 4.0 } else { right };
            let br = Rect::from_center_size(
                Pos2::new(bx - size::CONTROL_SM / 2.0, cy),
                Vec2::splat(size::CONTROL_SM),
            );
            if put(
                ui,
                br,
                IconButton::new(lucide::TRASH_2, t("stats", "deleteSession"))
                    .size(Size::IconXs)
                    .danger_hover(),
            )
            .clicked()
            {
                run.push((
                    "stats.forget_session",
                    json!({ "start": r.start, "name": r.name }),
                ));
            }
        }
        place_action(
            ui,
            act,
            r.kind,
            &r.name,
            r.ip.as_deref(),
            r.port,
            hovered,
            false,
            run,
        );
    }
}

/// Whether a session has a Join or Play button.
fn place_action_present(r: &dz_hub::features::stats::SessionDto) -> bool {
    use dz_hub::features::stats::PlayKind;
    match r.kind {
        PlayKind::Offline => true,
        PlayKind::Server => {
            r.ip.as_deref().is_some_and(|i| !i.is_empty()) && r.port.unwrap_or(0) > 0
        }
        PlayKind::Unknown => false,
    }
}
