//! The panel's Players, Mods and Rules tabs (= `PlayersTab.svelte`, `ModsTab.svelte`,
//! `RulesTab.svelte`).

use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use serde_json::json;

use super::model::Model;
use super::{DetailState, ModsFilter, Run, target_args, words};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::alpha;
use crate::kit::{Button, Copy, FieldSize, Input, Size, Skeleton, Variant};
use crate::parts::{Empty, ModRef, ModState, ModStatusList, draw, ph, ph_trunc};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// `px-pad py-2.5 gap-2` over the whole body.
fn padded(ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
    let r = ui.max_rect().shrink2(Vec2::new(size::PAD, 10.0));
    let mut c = ui.new_child(
        UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    c.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
    body(&mut c);
}

fn skeletons(ui: &mut Ui, n: usize, h: f32) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for _ in 0..n {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
            Skeleton::paint(ui, r, Some(size::RADIUS_XS), 0.6);
        }
    });
}

/// A bordered well the rest of the height, for a list (`rounded-sm border bg-bg`).
fn well(ui: &mut Ui) -> Rect {
    let tk = ui.tokens();
    let r = ui.available_rect_before_wrap();
    let r = Rect::from_min_max(r.min, Pos2::new(r.right(), r.bottom().max(r.top() + 40.0)));
    ui.painter().rect(
        r,
        CornerRadius::same(size::RADIUS_SM),
        tk.bg,
        Stroke::new(1.0, tk.border),
        egui::StrokeKind::Inside,
    );
    ui.allocate_rect(r, Sense::hover());
    r
}

/// Everyone the server says is on it, longest first, searchable.
pub fn players(ui: &mut Ui, cx: &Ctx, st: &mut DetailState, m: &Model, run: &mut Run) {
    let tk = ui.tokens();
    padded(ui, |ui| {
        // Steam friends on this server, by its query port or (unlisted) the port asked.
        // Friends sit on IPv4 servers only: a host name has none.
        let friends = &cx.stores.friends;
        let on = |port: i64| -> Vec<&str> {
            dz_api::addr(&m.ip, port)
                .map(|a| friends.on(a).map(|f| f.name.as_str()).collect())
                .unwrap_or_default()
        };
        let mut mine = on(m.query_port);
        if mine.is_empty() {
            mine = on(m.game_port);
        }
        if !mine.is_empty() {
            let s = tf("friends", "friendsHere", &[("names", &mine.join(", "))]);
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
            ui.painter()
                .rect_filled(r, CornerRadius::same(size::RADIUS_SM), alpha(tk.ok, 0.1));
            lucide::USERS.paint(
                ui.painter(),
                Rect::from_center_size(Pos2::new(r.left() + 14.0, r.center().y), Vec2::splat(12.0)),
                tk.ok,
            );
            let g = ph_trunc(ui, &s, font(Face::Sans, size::TEXT_2XS), w - 32.0);
            draw(ui.painter(), &g, r.left() + 26.0, r.center().y, tk.ok);
        }
        if m.live_loading && m.a2s.is_none() {
            skeletons(ui, 6, 24.0);
            return;
        }
        if m.live_error.is_some() && m.a2s.is_none() {
            Empty::new()
                .icon(lucide::USERS)
                .title(t("detail", "liveFailed"))
                .compact()
                .show(ui, |ui| {
                    if ui
                        .add(Button::new(t("detail", "retry")).icon(lucide::REFRESH_CW))
                        .clicked()
                    {
                        run.push(("servers.detail.refresh_live", target_args(m)));
                    }
                });
            return;
        }
        if m.players.is_empty() {
            let title = if m.a2s.as_ref().is_some_and(|a| a.players == 0) {
                t("detail", "a2sNoPlayers")
            } else {
                t("detail", "a2sNamesNotReported")
            };
            Empty::new()
                .icon(lucide::USERS)
                .title(title)
                .compact()
                .show(ui, |_| {});
            return;
        }
        let before = st.players_q.clone();
        ui.add(
            Input::new(&mut st.players_q)
                .search()
                .size(FieldSize::Xs)
                .placeholder(t("detail", "searchPlayers"))
                .clear_label(t("common", "clear"))
                .char_limit(100)
                .id_salt("server-players-search"),
        );
        if before != st.players_q {
            ui.ctx().request_repaint();
        }
        let q = st.players_q.trim().to_lowercase();
        let shown: Vec<(usize, &dz_hub::features::a2s::A2sPlayerDto)> = m
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| q.is_empty() || p.name.to_lowercase().contains(&q))
            .collect();
        let longest = m.players.iter().map(|p| p.duration).fold(1.0_f32, f32::max);
        // [1.5rem | 1fr | 3.5rem | 4rem]
        let cols = |r: Rect| {
            let l = r.left() + 8.0;
            let rr = r.right() - 8.0;
            let n = (l, l + 24.0);
            let time = (rr - 64.0, rr);
            let score = (time.0 - 8.0 - 56.0, time.0 - 8.0);
            let name = (n.1 + 8.0, score.0 - 8.0);
            (n, name, score, time)
        };
        let head = |ui: &mut Ui| {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 14.0), Sense::hover());
            let (n, name, score, time) = cols(r);
            let f = font(Face::DisplayBold, size::TEXT_3XS);
            let st = |ui: &Ui, s: &str| {
                crate::kit::paint::tracked(ui, s, f.clone(), tk.fg_faint, 0.1, true, None)
            };
            let p = ui.painter();
            let g = st(ui, "#");
            p.galley(Pos2::new(n.1 - g.size().x, r.top()), g, tk.fg_faint);
            let g = st(ui, t("detail", "players"));
            p.galley(Pos2::new(name.0, r.top()), g, tk.fg_faint);
            let g = st(ui, t("detail", "score"));
            p.galley(Pos2::new(score.1 - g.size().x, r.top()), g, tk.fg_faint);
            let g = st(ui, t("detail", "timeHere"));
            p.galley(Pos2::new(time.1 - g.size().x, r.top()), g, tk.fg_faint);
        };
        head(ui);
        let area = well(ui);
        let mut lui = ui.new_child(UiBuilder::new().max_rect(area.shrink(1.0)));
        lui.spacing_mut().item_spacing = Vec2::ZERO;
        if shown.is_empty() {
            let g = ph(ui, t("detail", "noMatch"), font(Face::Sans, size::TEXT_2XS));
            draw(
                lui.painter(),
                &g,
                area.center().x - g.size().x / 2.0,
                area.top() + 20.0,
                tk.fg_faint,
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("server-players-list")
            .auto_shrink([false, false])
            .show_rows(&mut lui, 28.0, shown.len(), |ui, range| {
                for (i, p) in shown.get(range).unwrap_or_default() {
                    let w = ui.available_width();
                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::hover());
                    let pt = ui.painter();
                    let frac = (p.duration / longest).clamp(0.0, 1.0);
                    pt.rect_filled(
                        Rect::from_min_size(r.min, Vec2::new(r.width() * frac, r.height())),
                        CornerRadius::ZERO,
                        alpha(tk.accent, 0.06),
                    );
                    pt.hline(
                        r.x_range(),
                        r.bottom() - 0.5,
                        Stroke::new(1.0, alpha(tk.border, 0.5)),
                    );
                    let (n, name, score, time) = cols(r);
                    let mono = font(Face::Mono, size::TEXT_2XS);
                    let cy = r.center().y;
                    let g = ph(ui, &(i + 1).to_string(), mono.clone());
                    draw(pt, &g, n.1 - g.size().x, cy, tk.fg_faint);
                    let nm = if p.name.is_empty() {
                        format::NONE
                    } else {
                        p.name.as_str()
                    };
                    let g = ph_trunc(
                        ui,
                        nm,
                        font(Face::Sans, size::TEXT_2XS),
                        (name.1 - name.0).max(10.0),
                    );
                    draw(pt, &g, name.0, cy, tk.fg);
                    let g = ph(ui, &p.score.to_string(), mono.clone());
                    draw(pt, &g, score.1 - g.size().x, cy, tk.fg_muted);
                    let g = ph(ui, &format::duration(p.duration.max(0.0) as i64), mono);
                    draw(pt, &g, time.1 - g.size().x, cy, tk.fg);
                }
            });
    });
}

/// The server's configuration as it reports it over A2S, searchable, each value
/// copyable.
pub fn rules(ui: &mut Ui, st: &mut DetailState, m: &Model) {
    let tk = ui.tokens();
    padded(ui, |ui| {
        if m.live_loading && m.a2s.is_none() {
            skeletons(ui, 4, 24.0);
            return;
        }
        let rules = m.a2s.as_ref().map_or(&[][..], |a| a.rules.as_slice());
        if rules.is_empty() {
            Empty::new()
                .icon(lucide::SCROLL_TEXT)
                .title(t("detail", "noRules"))
                .compact()
                .show(ui, |_| {});
            return;
        }
        ui.add(
            Input::new(&mut st.rules_q)
                .search()
                .size(FieldSize::Xs)
                .placeholder(t("detail", "searchRules"))
                .clear_label(t("common", "clear"))
                .char_limit(100)
                .id_salt("server-rules-search"),
        );
        let q = st.rules_q.trim().to_lowercase();
        let shown: Vec<&dz_hub::features::a2s::A2sRuleDto> = rules
            .iter()
            .filter(|r| {
                q.is_empty()
                    || r.name.to_lowercase().contains(&q)
                    || r.value.to_lowercase().contains(&q)
            })
            .collect();
        let area = well(ui);
        let mut lui = ui.new_child(UiBuilder::new().max_rect(area.shrink(1.0)));
        lui.spacing_mut().item_spacing = Vec2::ZERO;
        if shown.is_empty() {
            let g = ph(ui, t("detail", "noMatch"), font(Face::Sans, size::TEXT_2XS));
            draw(
                lui.painter(),
                &g,
                area.center().x - g.size().x / 2.0,
                area.top() + 20.0,
                tk.fg_faint,
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("server-rules-list")
            .auto_shrink([false, false])
            .show_rows(&mut lui, 22.0, shown.len(), |ui, range| {
                for r in shown.get(range).unwrap_or_default() {
                    let w = ui.available_width();
                    let (row, _) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
                    ui.painter().hline(
                        row.x_range(),
                        row.bottom() - 0.5,
                        Stroke::new(1.0, alpha(tk.border, 0.5)),
                    );
                    // [2fr | 3fr]
                    let split = row.left() + row.width() * 0.4;
                    let mono = font(Face::Mono, size::TEXT_3XS);
                    let g = ph_trunc(ui, &r.name, mono, split - row.left() - 16.0);
                    let name_r = Rect::from_min_max(row.min, Pos2::new(split, row.bottom()));
                    draw(
                        ui.painter(),
                        &g,
                        row.left() + 8.0,
                        row.center().y,
                        tk.fg_faint,
                    );
                    if crate::parts::is_cut(&g, &r.name) {
                        let _ = ui
                            .interact(name_r, ui.id().with(("rule", &r.name)), Sense::hover())
                            .on_hover_text(&r.name);
                    }
                    let vr = Rect::from_min_max(
                        Pos2::new(split + 8.0, row.top() + 3.0),
                        Pos2::new(row.right() - 8.0, row.bottom() - 3.0),
                    );
                    let mut c = ui.new_child(
                        UiBuilder::new()
                            .max_rect(vr)
                            .id_salt(("rule-v", &r.name))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    c.add(
                        Copy::new(&r.value)
                            .labels(t("detail", "copyValue"), t("common", "copied"))
                            .max_width(vr.width())
                            .text_size(size::TEXT_3XS)
                            .color(c.tokens().fg_muted),
                    );
                }
            });
    });
}

/// What the server runs against what is installed, filterable, the fix one click away.
pub fn mods(ui: &mut Ui, st: &mut DetailState, m: &Model, run: &mut Run) {
    let tk = ui.tokens();
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(size::PAD as i8, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            let small = font(Face::Sans, size::TEXT_2XS);
            match &m.listed {
                Some(l) if l.mods_count > 0 => {
                    if m.mod_rows.is_empty() {
                        words(
                            ui,
                            &tf("detail", "modsNotReported", &[("count", &l.mods_count)]),
                            small,
                            tk.fg_faint,
                        );
                    } else {
                        rows(ui, st, m, run);
                    }
                }
                _ if !m.mod_rows.is_empty() => {
                    words(ui, t("detail", "modsFromServer"), small, tk.fg_muted);
                    rows(ui, st, m, run);
                }
                _ => {
                    let names = m
                        .a2s
                        .as_ref()
                        .map_or(&[][..], |a| a.mods_from_a2s.as_slice());
                    if names.is_empty() {
                        Empty::new()
                            .icon(lucide::PUZZLE)
                            .title(t("detail", "noMods"))
                            .compact()
                            .show(ui, |_| {});
                        return;
                    }
                    words(ui, t("detail", "modsFromA2s"), small, tk.fg_muted);
                    let w = ui.available_width();
                    let h = 28.0 * names.len() as f32;
                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                    ui.painter().rect(
                        r,
                        CornerRadius::same(size::RADIUS_SM),
                        tk.bg,
                        Stroke::new(1.0, tk.border),
                        egui::StrokeKind::Inside,
                    );
                    for (i, name) in names.iter().enumerate() {
                        let row = Rect::from_min_size(
                            Pos2::new(r.left(), r.top() + 28.0 * i as f32),
                            Vec2::new(w, 28.0),
                        );
                        if i + 1 < names.len() {
                            ui.painter().hline(
                                row.x_range(),
                                row.bottom() - 0.5,
                                Stroke::new(1.0, alpha(tk.border, 0.5)),
                            );
                        }
                        ui.painter().circle_filled(
                            Pos2::new(row.left() + 10.0, row.center().y),
                            2.0,
                            tk.mods,
                        );
                        let g = ph_trunc(ui, name, font(Face::Sans, size::TEXT_2XS), w - 28.0);
                        draw(ui.painter(), &g, row.left() + 18.0, row.center().y, tk.fg);
                    }
                }
            }
        });
}

fn rows(ui: &mut Ui, st: &mut DetailState, m: &Model, run: &mut Run) {
    let tk = ui.tokens();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let all = format!("{} · {}", t("detail", "filterAll"), m.mod_rows.len());
        if ui
            .add(Button::chip(&all, st.mods_filter == ModsFilter::All))
            .clicked()
        {
            st.mods_filter = ModsFilter::All;
        }
        let miss = format!("{} · {}", t("detail", "filterMissing"), m.missing);
        if ui
            .add(
                Button::chip(&miss, st.mods_filter == ModsFilter::Missing)
                    .tint((m.missing > 0).then_some(tk.err)),
            )
            .clicked()
        {
            st.mods_filter = ModsFilter::Missing;
        }
        let stale = format!("{} · {}", t("detail", "filterStale"), m.stale);
        if ui
            .add(
                Button::chip(&stale, st.mods_filter == ModsFilter::Stale)
                    .tint((m.stale > 0).then_some(tk.warn)),
            )
            .clicked()
        {
            st.mods_filter = ModsFilter::Stale;
        }
        if m.listed.is_some() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        crate::kit::IconButton::new(lucide::REFRESH_CW, t("detail", "retry"))
                            .size(Size::IconXs)
                            .variant(Variant::Ghost),
                    )
                    .clicked()
                {
                    run.push(("servers.detail.refresh_live", target_args(m)));
                }
            });
        }
    });
    // On disk: what "Remove this server's mods" could delete (a listed server only).
    let on_disk = m.listed.is_some() && m.mod_rows.iter().any(|r| r.state != ModState::Missing);
    if m.missing > 0 || m.stale > 0 || on_disk {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            if m.missing > 0 {
                let s = tf("detail", "installMissing", &[("count", &m.missing)]);
                if ui
                    .add(
                        Button::new(&s)
                            .icon(lucide::DOWNLOAD)
                            .variant(Variant::Accent)
                            .size(Size::Xs),
                    )
                    .clicked()
                {
                    run.push((
                        "servers.detail.install_missing",
                        json!({ "ids": m.missing_ids() }),
                    ));
                }
            }
            if m.stale > 0 {
                let s = tf("detail", "updateStale", &[("count", &m.stale)]);
                if ui
                    .add(Button::new(&s).icon(lucide::CIRCLE_ARROW_UP).size(Size::Xs))
                    .clicked()
                {
                    run.push((
                        "servers.detail.update_stale",
                        json!({ "ids": m.stale_ids() }),
                    ));
                }
            }
            if on_disk
                && ui
                    .add(
                        Button::new(t("mods", "removeServerMods"))
                            .icon(lucide::TRASH_2)
                            .variant(Variant::Ghost)
                            .size(Size::Xs)
                            .tip(t("mods", "removeServerModsTip")),
                    )
                    .clicked()
            {
                run.push(("servers.row.remove_mods", target_args(m)));
            }
        });
    }
    let items: Vec<ModRef> = match st.mods_filter {
        ModsFilter::All => m.mod_rows.clone(),
        ModsFilter::Missing => m
            .mod_rows
            .iter()
            .filter(|r| r.state == ModState::Missing)
            .cloned()
            .collect(),
        ModsFilter::Stale => m
            .mod_rows
            .iter()
            .filter(|r| r.state == ModState::Stale)
            .cloned()
            .collect(),
    };
    if items.is_empty() {
        let g = ph(
            ui,
            t("detail", "noneInFilter"),
            font(Face::Sans, size::TEXT_2XS),
        );
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::hover());
        draw(
            ui.painter(),
            &g,
            r.center().x - g.size().x / 2.0,
            r.center().y,
            tk.fg_faint,
        );
        return;
    }
    if let Some(id) = ModStatusList::new(&items)
        .summary(st.mods_filter == ModsFilter::All)
        .show(ui)
    {
        run.push((
            "servers.detail.open_link",
            json!({ "url": format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}") }),
        ));
    }
}
