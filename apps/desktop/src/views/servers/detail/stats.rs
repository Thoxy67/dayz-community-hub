//! The server's long view, from DayZ Metrics (= `StatsTab.svelte`, `BusyHours.svelte`):
//! how it ranks, how reliably it is up, how full it gets, when it restarts and wipes,
//! whether its player count can be trusted, its players drawn large over a day to a
//! month, and the hours of the week it is busiest.

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde_json::{Value, json};

use super::model::{Model, iso_secs, until_text};
use super::overview::error_box;
use super::{DetailState, Run, population_warning, stencil, words};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{
    FieldSize, Flag, IconButton, Segment, Segmented, Side, Size, Skeleton, Spinner, TipExt, Variant,
};
use crate::parts::{Empty, Fact, Facts, PlayerChart, Section, WeekHeat, draw, ph, ph_trunc};
use crate::theme::{TokensExt, font, fonts::Face, size};

const RANGES: [&str; 4] = ["1d", "7d", "2w", "1m"];

pub fn range_of(id: &str) -> Option<&'static str> {
    RANGES.iter().copied().find(|r| *r == id)
}

fn pad(ui: &mut Ui, top: i8, bottom: i8, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: size::PAD as i8,
            right: size::PAD as i8,
            top,
            bottom,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
            body(ui);
        });
}

fn skeleton(ui: &mut Ui, h: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    Skeleton::paint(ui, r, Some(size::RADIUS_SM), 0.6);
}

/// A small tone tag with a dot (`Tag` with `size-1.5 rounded-full bg-current`).
fn status_tag(ui: &mut Ui, text: &str, c: Color32) {
    let g = ph(ui, text, font(Face::Mono, size::TEXT_2XS));
    let (r, _) = ui.allocate_exact_size(
        Vec2::new(6.0 + 6.0 + 4.0 + g.size().x + 6.0, 18.0),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(r, CornerRadius::same(size::RADIUS_XS), alpha(c, 0.12));
    ui.painter()
        .circle_filled(Pos2::new(r.left() + 9.0, r.center().y), 3.0, c);
    draw(ui.painter(), &g, r.left() + 16.0, r.center().y, c);
}

pub fn show(ui: &mut Ui, cx: &Ctx, st: &mut DetailState, m: &Model, run: &mut Run) {
    let tk = ui.tokens();
    let entry = &m.metrics;
    let x = m.metrics_data();

    // The heading: the source, its status, a refresh.
    pad(ui, 10, 0, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            stencil(ui, t("detail", "dmTitle"), tk.fg_faint);
            if let Some(s) = x.and_then(|x| x.status.as_deref()) {
                let (text, c) = match s {
                    "online" => (t("detail", "bmOnline"), tk.ok),
                    "offline" => (t("detail", "bmOffline"), tk.err),
                    other => (other, tk.fg_muted),
                };
                status_tag(ui, text, c);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if entry.loading {
                    let (r, _) =
                        ui.allocate_exact_size(Vec2::splat(size::CONTROL_SM), Sense::hover());
                    Spinner::paint(
                        ui,
                        Rect::from_center_size(r.center(), Vec2::splat(14.0)),
                        tk.fg_faint,
                    );
                } else if ui
                    .add(
                        IconButton::new(lucide::REFRESH_CW, t("detail", "dmRefresh"))
                            .size(Size::IconXs)
                            .variant(Variant::Ghost),
                    )
                    .clicked()
                {
                    run.push(("servers.detail.refresh_stats", Value::Null));
                }
            });
        });
    });

    let Some(x) = x else {
        if entry.loading {
            pad(ui, 10, 10, |ui| {
                skeleton(ui, 56.0);
                skeleton(ui, 144.0);
                skeleton(ui, 80.0);
            });
        } else if let Some(e) = &entry.error {
            pad(ui, 10, 10, |ui| {
                if e.starts_with("Not listed") {
                    Empty::new()
                        .icon(lucide::CHART_LINE)
                        .title(t("detail", "dmNotListed"))
                        .compact()
                        .show(ui, |_| {});
                } else {
                    error_box(ui, e);
                    if super::overview::link(ui, t("detail", "retry")).clicked() {
                        run.push(("servers.detail.refresh_stats", Value::Null));
                    }
                }
            });
        }
        return;
    };

    pad(ui, 10, 10, |ui| {
        population_warning(ui, m, false);
        figures(ui, cx, x, m);
        // The span.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let labels = [
                t("detail", "dmRange1d"),
                t("detail", "dmRange7d"),
                t("detail", "dmRange2w"),
                t("detail", "dmRange1m"),
            ];
            let segs: Vec<Segment<&'static str>> = RANGES
                .iter()
                .zip(labels)
                .map(|(r, l)| Segment::new(*r, l))
                .collect();
            let mut v = st.range;
            ui.add(Segmented::new(&mut v, &segs).size(FieldSize::Xs));
            if v != st.range {
                run.push(("servers.detail.stats_range", json!({ "range": v })));
            }
        });
        // The chart: the day comes with the figures; longer spans by the site's id.
        let long = (st.range != "1d").then(|| cx.stores.server_data.history(x.id, st.range));
        let day: Vec<(i64, f64)>;
        let points: &[(i64, f64)] = if st.range == "1d" {
            day = x
                .player_history
                .iter()
                .copied()
                .filter(|p| p.1.is_finite())
                .collect();
            &day
        } else {
            long.as_ref()
                .and_then(|e| e.data.as_deref())
                .map_or(&[][..], Vec::as_slice)
        };
        let since = match st.range {
            "7d" => Some(t("detail", "dm7dAgo")),
            "2w" => Some(t("detail", "dm2wAgo")),
            "1m" => Some(t("detail", "dm1mAgo")),
            _ => None,
        };
        if points.len() > 1 {
            let mut c = PlayerChart::new(points).max(x.max_players).height(144.0);
            if st.range != "1d" {
                c = c.label(t("detail", "dmPlayerCount"));
            }
            if let Some(s) = since {
                c = c.since(s);
            }
            ui.add(c);
        } else if long.as_ref().is_some_and(|e| e.loading) {
            skeleton(ui, 144.0);
        } else if let Some(e) = long.as_ref().and_then(|e| e.error.as_ref()) {
            words(ui, e, font(Face::Sans, size::TEXT_2XS), tk.err);
        }
    });

    // When it is full: a week of hours in the player's own time.
    let heat = cx.stores.server_data.heatmap(x.id);
    if let Some(cells) = heat.data.as_deref()
        && cells.iter().any(|c| c.avg > 0.0)
    {
        let values = week(cells);
        Section::new(t("detail", "dmBusyHours"))
            .icon(lucide::FLAME)
            .show(
                ui,
                |_| {},
                |ui| {
                    WeekHeat::new(
                        &values,
                        |day, hour, v| {
                            tf(
                                "detail",
                                "dmBusyCell",
                                &[
                                    ("day", &day),
                                    ("hour", &hour),
                                    ("count", &(v.round() as i64)),
                                ],
                            )
                        },
                        t("detail", "dmBusyHint"),
                    )
                    .show(ui);
                },
            );
    }

    schedule(ui, x);
    details(ui, x);

    if !x.notices.is_empty() {
        Section::new(t("detail", "dmNotices"))
            .icon(lucide::MEGAPHONE)
            .show(
                ui,
                |_| {},
                |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for n in &x.notices {
                        words(
                            ui,
                            &format!("•  {n}"),
                            font(Face::Sans, size::TEXT_2XS),
                            tk.fg_muted,
                        );
                    }
                },
            );
    }

    // Links, deduplicated: Discord, the website, the rest.
    let mut links: Vec<(String, String)> = Vec::new();
    if let Some(d) = &x.discord {
        links.push((t("detail", "dmDiscord").to_string(), d.clone()));
    }
    if let Some(w) = &x.website {
        links.push((t("detail", "dmWebsite").to_string(), w.clone()));
    }
    for k in &x.links {
        if !links.iter().any(|(_, u)| *u == k.url) {
            links.push((k.label.clone(), k.url.clone()));
        }
    }
    if !links.is_empty() {
        Section::new(t("detail", "dmLinks"))
            .icon(lucide::LINK)
            .show(
                ui,
                |_| {},
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                        for (label, url) in &links {
                            if link_button(ui, label, url).clicked() {
                                run.push(("servers.detail.open_link", json!({ "url": url })));
                            }
                        }
                    });
                },
            );
    }

    // The foot: the site's page, and where this comes from.
    pad(ui, 10, 10, |ui| {
        super::line(ui, 16.0, |ui| {
            let label = t("detail", "openDm");
            let g = ph(ui, label, font(Face::Sans, size::TEXT_2XS));
            let (r, resp) =
                ui.allocate_exact_size(Vec2::new(12.0 + 6.0 + g.size().x, 16.0), Sense::click());
            let c = if resp.hovered() {
                tk.accent
            } else {
                tk.fg_faint
            };
            lucide::EXTERNAL_LINK.paint(
                ui.painter(),
                Rect::from_center_size(Pos2::new(r.left() + 6.0, r.center().y), Vec2::splat(12.0)),
                c,
            );
            draw(ui.painter(), &g, r.left() + 18.0, r.center().y, c);
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(ui.painter(), r, size::RADIUS_XS, &tk);
            }
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, label));
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                run.push(("servers.detail.open_link", json!({ "url": x.url })));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let g = ph(
                    ui,
                    t("detail", "dmSource"),
                    font(Face::Sans, size::TEXT_2XS),
                );
                let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
            });
        });
    });
}

/// `[day][hour]`, Monday first, in the player's time: the site sends UTC and numbers
/// weekdays as JavaScript does (0 is Sunday).
pub fn week(cells: &[dz_hub::features::metrics::HeatCell]) -> [[f64; 24]; 7] {
    let shift = chrono::Local::now().offset().local_minus_utc() / 3600;
    let mut g = [[0.0_f64; 24]; 7];
    for c in cells {
        let h = i32::from(c.hour) + shift;
        let dow = (i32::from(c.dow) + h.div_euclid(24)).rem_euclid(7);
        let day = (dow - 1).rem_euclid(7) as usize;
        let hour = h.rem_euclid(24) as usize;
        if let Some(row) = g.get_mut(day)
            && let Some(v) = row.get_mut(hour)
        {
            *v = if c.avg.is_finite() { c.avg } else { 0.0 };
        }
    }
    g
}

fn tone_up(up: f64, tk: &crate::theme::Tokens) -> Color32 {
    if up >= 90.0 {
        tk.ok
    } else if up >= 70.0 {
        tk.warn
    } else {
        tk.err
    }
}

/// The four figures a player weighs a server by, big where they matter.
fn figures(ui: &mut Ui, cx: &Ctx, x: &dz_hub::features::metrics::ServerMetricsDto, m: &Model) {
    let _ = m;
    let tk = ui.tokens();
    let w = ui.available_width();
    let four = w >= 416.0;
    let cols = if four { 4 } else { 2 };
    let rows = 4 / cols;
    let cell_h = 50.0;
    let h = rows as f32 * cell_h + (rows as f32 - 1.0) + 2.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(size::RADIUS_SM),
        tk.border,
        Stroke::new(1.0, tk.border),
        egui::StrokeKind::Inside,
    );
    let cw = (w - 2.0 - (cols as f32 - 1.0)) / cols as f32;
    let at = |i: usize| {
        let (c, r) = (i % cols, i / cols);
        Rect::from_min_size(
            Pos2::new(
                rect.left() + 1.0 + c as f32 * (cw + 1.0),
                rect.top() + 1.0 + r as f32 * (cell_h + 1.0),
            ),
            Vec2::new(cw, cell_h),
        )
    };
    // Places gained over the month (positive is better: a smaller number).
    let ranks = cx.stores.server_data.rank_history(x.id);
    let gain = ranks
        .data
        .as_deref()
        .filter(|r| r.len() >= 2)
        .and_then(|r| r.first())
        .zip(x.rank_pos)
        .map(|(first, now)| (first.1 - now).round() as i64)
        .filter(|g| *g != 0);
    let none = format::NONE;
    let big = font(Face::DisplayBold, size::TEXT_XL);
    let cell = |i: usize,
                label: &str,
                value: &str,
                tone: Color32,
                tail: Option<(String, Color32, Option<bool>)>,
                tip: &str| {
        let r = at(i);
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::ZERO, tk.panel);
        let lg =
            crate::views::servers::stencil_galley(ui, label, tk.fg_faint, Some(r.width() - 16.0));
        p.galley(Pos2::new(r.left() + 8.0, r.top() + 6.0), lg, tk.fg_faint);
        let vg = paint::tracked(ui, value, big.clone(), tone, 0.02, true, None);
        let vy = r.bottom() - 6.0 - vg.size().y;
        let vw = vg.size().x;
        let vg_h = vg.size().y;
        p.galley(Pos2::new(r.left() + 8.0, vy), vg, tone);
        match tail {
            // "/60" after the peak: smaller, faint, on the value's baseline.
            Some((text, c, None)) => {
                let g = paint::tracked(
                    ui,
                    &text,
                    font(Face::DisplayBold, size::TEXT_SM),
                    c,
                    0.02,
                    true,
                    None,
                );
                let y = vy + vg_h - g.size().y - 1.0;
                ui.painter().galley(Pos2::new(r.left() + 8.0 + vw, y), g, c);
            }
            Some((text, c, Some(arrow))) => {
                let g = ph(ui, &text, font(Face::Mono, size::TEXT_2XS));
                let x0 = r.left() + 8.0 + vw + 6.0;
                let cy = r.bottom() - 13.0;
                let icon = if arrow {
                    lucide::TRENDING_UP
                } else {
                    lucide::TRENDING_DOWN
                };
                let p = ui.painter();
                icon.paint(
                    p,
                    Rect::from_center_size(Pos2::new(x0 + 6.0, cy), Vec2::splat(12.0)),
                    c,
                );
                draw(p, &g, x0 + 14.0, cy, c);
                if !tip.is_empty() {
                    let tr = Rect::from_min_size(
                        Pos2::new(x0, cy - 8.0),
                        Vec2::new(14.0 + g.size().x, 16.0),
                    );
                    let _ = ui
                        .interact(tr, ui.id().with(("stat-tip", i)), Sense::hover())
                        .tip_kbd(tip, "", Side::Top);
                }
            }
            None => {}
        }
    };
    let rank = x
        .rank_pos
        .map_or_else(|| none.to_string(), |r| format!("#{}", format::num(r)));
    let rank_tail = gain.map(|g| {
        let up = g > 0;
        (
            format!("{}{}", if up { "+" } else { "" }, format::num(g as f64)),
            if up { tk.ok } else { tk.err },
            Some(up),
        )
    });
    cell(
        0,
        t("detail", "bmRank"),
        &rank,
        tk.accent,
        rank_tail,
        t("detail", "dmRankTrend"),
    );
    let (up_s, up_c) = match x.uptime_7d {
        Some(u) => (format!("{u:.1}%"), tone_up(u, &tk)),
        None => (none.to_string(), tk.fg_faint),
    };
    cell(1, t("detail", "dmUptime7d"), &up_s, up_c, None, "");
    let peak = x.peak_7d.map_or_else(|| none.to_string(), format::num);
    let cap = format!(
        "/{}",
        x.max_players.map_or_else(|| "?".to_string(), format::num)
    );
    cell(
        2,
        t("detail", "dmPeak7d"),
        &peak,
        tk.fg,
        Some((cap, tk.fg_faint, None)),
        "",
    );
    let avg = x
        .avg_players_7d
        .map_or_else(|| none.to_string(), |a| format::num(a.round()));
    let wow = x.wow_pct.filter(|v| v.is_finite()).map(|w| {
        let up = w >= 0.0;
        (
            format!("{}{}%", if up { "+" } else { "" }, w.round() as i64),
            if up { tk.ok } else { tk.err },
            Some(up),
        )
    });
    cell(
        3,
        t("detail", "dmAvg7d"),
        &avg,
        tk.fg,
        wow,
        t("detail", "dmTrend"),
    );
}

fn phase_word(p: Option<&str>) -> Option<&'static str> {
    match p? {
        "fresh" => Some(t("detail", "dmPhaseFresh")),
        "mid" => Some(t("detail", "dmPhaseMid")),
        "late" => Some(t("detail", "dmPhaseLate")),
        _ => None,
    }
}

fn source_word(s: Option<&str>) -> String {
    match s {
        Some("announced") => t("detail", "dmAnnounced").to_string(),
        Some("predicted") => t("detail", "dmPredicted").to_string(),
        Some(o) => o.to_string(),
        None => String::new(),
    }
}

/// "Restarts and wipes".
fn schedule(ui: &mut Ui, x: &dz_hub::features::metrics::ServerMetricsDto) {
    let tk = ui.tokens();
    let restart = x.restart.as_ref();
    let wipe = x.wipe.as_ref();
    let past: Vec<String> = wipe
        .map(|w| {
            w.events
                .iter()
                .filter_map(|e| {
                    let on = e.on.as_ref()?;
                    Some(if e.source.as_deref() == Some("surge") {
                        format!("{on} ({})", t("detail", "dmWipeSurge"))
                    } else {
                        on.clone()
                    })
                })
                .take(4)
                .collect()
        })
        .unwrap_or_default();
    let next_restart = restart.and_then(|r| r.next_restart.as_deref());
    let any = next_restart.is_some()
        || wipe.is_some_and(|w| w.next.is_some() || w.last.is_some())
        || !past.is_empty();
    if !any {
        return;
    }
    let small = font(Face::Sans, size::TEXT_2XS);
    let tiny = font(Face::Sans, size::TEXT_3XS);
    Section::new(t("detail", "dmSchedule"))
        .icon(lucide::CALENDAR_CLOCK)
        .show(
            ui,
            |_| {},
            |ui| {
                let label_w = 110.0_f32.min(ui.available_width() * 0.4);
                let row = |ui: &mut Ui, label: &str, body: &mut dyn FnMut(&mut Ui)| {
                    super::line_top(ui, 16.0, |ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        let g = ph_trunc(ui, label, small.clone(), label_w);
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::new(label_w, g.size().y), Sense::hover());
                        ui.painter().galley(r.min, g, tk.fg_faint);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            body(ui);
                        });
                    });
                };
                let now = crate::store::now_ms();
                if let (Some(r), Some(next)) = (restart, next_restart) {
                    row(ui, t("detail", "dmNextRestart"), &mut |ui| {
                        let when = until_text(next, now).map_or_else(
                            || iso_secs(next).map(format::date_time).unwrap_or_default(),
                            |u| tf("detail", "dmIn", &[("time", &u)]),
                        );
                        let mut s = when;
                        if let Some(h) = r.period_hours {
                            s.push_str(" · ");
                            s.push_str(&tf("detail", "dmEvery", &[("hours", &format::num(h))]));
                        }
                        let resp = words(ui, &s, small.clone(), tk.fg);
                        if let Some(ts) = iso_secs(next) {
                            let _ = resp.tip_kbd(&format::date_time(ts), "", Side::Top);
                        }
                        if !r.slots_utc.is_empty() {
                            words(
                                ui,
                                &format!("{} UTC", r.slots_utc.join(" · ")),
                                font(Face::Mono, size::TEXT_3XS),
                                tk.fg_faint,
                            );
                        }
                        if let Some(c) = r.coverage {
                            let pct = (c * 100.0).round() as i64;
                            words(
                                ui,
                                &tf("detail", "dmOnSchedule", &[("pct", &pct)]),
                                tiny.clone(),
                                tk.fg_faint,
                            );
                        }
                        if let Some(n) = r.unscheduled_7d.filter(|n| *n > 0.0) {
                            words(
                                ui,
                                &tf("detail", "dmUnscheduled", &[("count", &format::num(n))]),
                                tiny.clone(),
                                tk.warn,
                            );
                        }
                        if let Some(n) = r.restart_loops_7d.filter(|n| *n > 0.0) {
                            words(
                                ui,
                                &tf("detail", "dmRestartLoops", &[("count", &format::num(n))]),
                                tiny.clone(),
                                tk.err,
                            );
                        }
                    });
                }
                if let Some(w) = wipe {
                    if let Some(next) = &w.next {
                        row(ui, t("detail", "dmNextWipe"), &mut |ui| {
                            let mut s = next.clone();
                            if let Some(d) = w.days_until {
                                s.push_str(" · ");
                                s.push_str(&tf(
                                    "detail",
                                    "dmInDays",
                                    &[("days", &(d.round() as i64))],
                                ));
                            }
                            super::line(ui, 18.0, |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                words(ui, &s, small.clone(), tk.fg);
                                let src = source_word(w.next_source.as_deref());
                                if !src.is_empty() {
                                    let tone = if w.next_source.as_deref() == Some("announced") {
                                        crate::kit::Tone::Ok
                                    } else {
                                        crate::kit::Tone::Neutral
                                    };
                                    ui.add(crate::kit::Tag::new(&src).tone(tone));
                                }
                            });
                        });
                    }
                    if let Some(last) = &w.last {
                        row(ui, t("detail", "dmLastWipe"), &mut |ui| {
                            let mut s = last.clone();
                            if let Some(d) = w.days_since {
                                s.push_str(" · ");
                                s.push_str(&tf(
                                    "detail",
                                    "dmDaysAgo",
                                    &[("days", &(d.round() as i64))],
                                ));
                            }
                            super::line(ui, 18.0, |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                words(ui, &s, small.clone(), tk.fg_muted);
                                if let Some(ph) = phase_word(w.phase.as_deref()) {
                                    let tone = if w.phase.as_deref() == Some("fresh") {
                                        crate::kit::Tone::Ok
                                    } else {
                                        crate::kit::Tone::Neutral
                                    };
                                    ui.add(crate::kit::Tag::new(ph).tone(tone));
                                }
                            });
                        });
                    }
                    if !past.is_empty() && w.last.is_none() {
                        row(ui, t("detail", "dmPastWipes"), &mut |ui| {
                            words(
                                ui,
                                &past.join(" · "),
                                font(Face::Mono, size::TEXT_3XS),
                                tk.fg_muted,
                            );
                        });
                    }
                }
            },
        );
}

/// "Details": the country, time speed, style, playstyle, mods to download, tracked
/// since, the site's pings.
fn details(ui: &mut Ui, x: &dz_hub::features::metrics::ServerMetricsDto) {
    let tk = ui.tokens();
    struct F {
        label: &'static str,
        value: String,
        tip: String,
        tone: Option<Color32>,
    }
    let mut v: Vec<F> = Vec::new();
    if let Some(day) = x.time_accel {
        let night = x.night_time_accel.unwrap_or(day);
        v.push(F {
            label: t("detail", "dmTimeSpeed"),
            value: tf(
                "detail",
                "dmTimeSpeedValue",
                &[("day", &format::num(day)), ("night", &format::num(night))],
            ),
            tip: String::new(),
            tone: None,
        });
    }
    if let Some(b) = &x.vanilla_band {
        v.push(F {
            label: t("detail", "dmStyle"),
            value: b.clone(),
            tip: x
                .vanilla_score
                .map(|s| format!("{} / 100", s.round() as i64))
                .unwrap_or_default(),
            tone: Some(tk.mods),
        });
    }
    if let Some(p) = &x.playstyle {
        v.push(F {
            label: t("detail", "dmPlaystyle"),
            value: p.clone(),
            tip: String::new(),
            tone: None,
        });
    }
    if let Some(b) = x.mod_total_bytes.filter(|b| *b > 0.0) {
        v.push(F {
            label: t("detail", "dmModsSize"),
            value: format::bytes(b),
            tip: String::new(),
            tone: None,
        });
    }
    if let Some(fs) = &x.first_seen {
        v.push(F {
            label: t("detail", "dmTrackedSince"),
            value: iso_secs(fs).map_or_else(|| fs.clone(), format::date),
            tip: iso_secs(fs).map(format::date_time).unwrap_or_default(),
            tone: None,
        });
    }
    if let (Some(lo), Some(hi)) = (x.ping_lo, x.ping_hi) {
        let jitter = x
            .ping_jitter
            .map(|j| format!(" ±{}", j.round() as i64))
            .unwrap_or_default();
        v.push(F {
            label: t("detail", "dmPingRange"),
            value: format!("{}–{} ms{jitter}", lo.round() as i64, hi.round() as i64),
            tip: String::new(),
            tone: Some(tk.fg_muted),
        });
    }
    let country = x.country.as_deref();
    if v.is_empty() && country.is_none() {
        return;
    }
    Section::new(t("detail", "details"))
        .icon(lucide::INFO)
        .show(
            ui,
            |_| {},
            |ui| {
                if let Some(code) = country {
                    let name = super::country::name(code);
                    super::line(ui, 16.0, |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.add(Flag::new(code).size(14.0).tip(&name));
                        let g = ph(ui, &name, font(Face::Sans, size::TEXT_2XS));
                        let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                        draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
                    });
                }
                let fs: Vec<Fact> = v
                    .iter()
                    .map(|f| {
                        let mut x = Fact::new(f.label, &f.value);
                        if let Some(c) = f.tone {
                            x = x.tone(c);
                        }
                        if !f.tip.is_empty() {
                            x = x.tip(&f.tip);
                        }
                        x
                    })
                    .collect();
                if !fs.is_empty() {
                    ui.add(Facts::new(&fs));
                }
            },
        );
}

/// A link to a page the server lists (`border rounded-sm px-2 py-1 text-2xs`).
fn link_button(ui: &mut Ui, label: &str, url: &str) -> egui::Response {
    let tk = ui.tokens();
    let room = ui.available_width().max(60.0);
    let g = ph_trunc(ui, label, font(Face::Sans, size::TEXT_2XS), room - 30.0);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 30.0, 24.0), Sense::click());
    let h = paint::ease(ui, resp.id.with("hover"), resp.hovered());
    let ink = paint::lerp(tk.fg_muted, tk.fg, h);
    let p = ui.painter();
    p.rect_stroke(
        r,
        CornerRadius::same(size::RADIUS_SM),
        Stroke::new(1.0, paint::lerp(tk.border, tk.border_strong, h)),
        egui::StrokeKind::Inside,
    );
    lucide::EXTERNAL_LINK.paint(
        p,
        Rect::from_center_size(Pos2::new(r.left() + 14.0, r.center().y), Vec2::splat(12.0)),
        ink,
    );
    draw(p, &g, r.left() + 24.0, r.center().y, ink);
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, r, size::RADIUS_SM, &tk);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, label));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .tip_kbd(url, "", Side::Top)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_is_monday_first() {
        let cells = [dz_hub::features::metrics::HeatCell {
            dow: 1,
            hour: 12,
            avg: 7.0,
        }];
        let g = week(&cells);
        let total: f64 = g.iter().flatten().sum();
        assert!((total - 7.0).abs() < 1e-9);
        assert_eq!(range_of("2w"), Some("2w"));
        assert_eq!(range_of("3y"), None);
    }
}
