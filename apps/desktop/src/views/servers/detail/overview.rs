//! The panel's first page (= `OverviewTab.svelte`): how to join, what the server is,
//! what it will cost, who is on, and you.

use egui::{Color32, CornerRadius, Sense, Stroke, Ui, Vec2};
use serde_json::json;

use super::model::{Model, Population, until_text};
use super::{Run, population_warning, target_args, words};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::alpha;
use crate::kit::{Button, Copy, Side, Size, Skeleton, TipExt, Variant};
use crate::parts::{Fact, Facts, Section, draw, map_name, ph, ph_trunc};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// A fact, owned until the grid borrows it.
struct F {
    label: String,
    value: String,
    tone: Option<Color32>,
    tip: String,
}

fn f(label: &str, value: impl Into<String>, tone: Option<Color32>) -> F {
    F {
        label: label.to_string(),
        value: value.into(),
        tone,
        tip: String::new(),
    }
}

fn yes_no(v: Option<bool>) -> String {
    match v {
        None => format::NONE.to_string(),
        Some(true) => t("detail", "yes").to_string(),
        Some(false) => t("detail", "no").to_string(),
    }
}

fn facts(ui: &Ui, m: &Model) -> Vec<F> {
    let tk = ui.tokens();
    let tone = |on: bool, c: Color32| Some(if on { c } else { tk.fg_muted });
    let mut v = Vec::new();
    let mn = map_name(&m.map);
    v.push(f(
        t("detail", "map"),
        if mn.is_empty() {
            format::NONE.to_string()
        } else {
            mn.into_owned()
        },
        Some(tk.map),
    ));
    if let Some(s) = &m.listed {
        let version = if s.version.is_empty() {
            format::NONE.to_string()
        } else {
            s.version.clone()
        };
        v.push(f(t("detail", "version"), version, Some(tk.fg_muted)));
        let platform = if s.environment == "w" {
            t("detail", "windows")
        } else {
            t("detail", "linux")
        };
        v.push(f(t("detail", "platform"), platform, None));
        let be = s.battleye.unwrap_or(false);
        v.push(f(
            t("detail", "battleye"),
            yes_no(s.battleye),
            tone(be, tk.ok),
        ));
        v.push(f(
            t("detail", "firstPerson"),
            yes_no(Some(s.first_person)),
            tone(s.first_person, tk.warn),
        ));
        v.push(f(
            t("detail", "vac"),
            yes_no(Some(s.vac)),
            tone(s.vac, tk.ok),
        ));
    } else if let Some(ver) = m
        .a2s
        .as_ref()
        .map(|a| a.version.as_str())
        .filter(|v| !v.is_empty())
    {
        v.push(f(t("detail", "version"), ver, Some(tk.fg_muted)));
    }
    // What the server states about itself over A2S; the list already says BattlEye and
    // first person for a listed one.
    if let Some(d) = m.a2s.as_ref().and_then(|a| a.dayz.as_ref()) {
        if m.listed.is_none() {
            v.push(f(
                t("detail", "battleye"),
                yes_no(Some(d.battleye)),
                tone(d.battleye, tk.ok),
            ));
            v.push(f(
                t("detail", "firstPerson"),
                yes_no(Some(d.first_person_only)),
                tone(d.first_person_only, tk.warn),
            ));
        }
        let hive = if d.official {
            t("detail", "hiveOfficial")
        } else if d.private_hive {
            t("detail", "hivePrivate")
        } else {
            t("detail", "hivePublic")
        };
        let mut h = f(t("detail", "hive"), hive, tone(d.official, tk.accent));
        h.tip = t("detail", "hiveHint").to_string();
        v.push(h);
        if let Some(q) = d.login_queue {
            let value = if q > 0 {
                tf("detail", "queueWaiting", &[("count", &q)])
            } else {
                t("detail", "queueEmpty").to_string()
            };
            v.push(f(t("detail", "loginQueue"), value, tone(q > 0, tk.warn)));
        }
        if let Some(day) = d.time_accel {
            let x = |v: f32| format::num((f64::from(v) * 10.0).round() / 10.0);
            // `entm` multiplies the day's speed at night (DayZ's
            // `serverNightTimeAcceleration`), as the browser's clock reads it.
            let night = day * d.night_time_accel.unwrap_or(1.0);
            v.push(f(
                t("detail", "dmTimeSpeed"),
                tf(
                    "detail",
                    "dmTimeSpeedValue",
                    &[("day", &x(day)), ("night", &x(night))],
                ),
                None,
            ));
        }
        if d.whitelisted {
            v.push(f(
                t("detail", "whitelist"),
                t("detail", "yes"),
                Some(tk.warn),
            ));
        }
    }
    if let Some(c) = m.count {
        v.push(f(
            t("detail", "maxPlayers"),
            format::num(c.max as f64),
            None,
        ));
        v.push(f(
            t("detail", "realPlayers"),
            format::num((c.players - i64::from(c.bots)).max(0) as f64),
            None,
        ));
        if c.bots > 0 {
            v.push(f(
                t("detail", "bots"),
                format::num(f64::from(c.bots)),
                Some(tk.warn),
            ));
        }
    }
    if let Some(x) = m.metrics_data() {
        if let Some(rank) = x.rank_pos {
            v.push(f(
                t("detail", "rank"),
                format!("#{}", format::num(rank)),
                Some(tk.accent),
            ));
        }
        if let Some(up) = x.uptime_7d {
            let c = if up >= 90.0 {
                tk.ok
            } else if up >= 70.0 {
                tk.warn
            } else {
                tk.err
            };
            v.push(f(t("detail", "uptime"), format!("{up:.1}%"), Some(c)));
        }
        let now = crate::store::now_ms();
        if let Some(r) = x
            .restart
            .as_ref()
            .and_then(|r| r.next_restart.as_deref())
            .and_then(|n| until_text(n, now))
        {
            v.push(f(
                t("detail", "dmNextRestart"),
                tf("detail", "dmIn", &[("time", &r)]),
                None,
            ));
        }
        if let Some(w) = &x.wipe
            && let Some(next) = &w.next
        {
            let value = match w.days_until {
                Some(d) => tf("detail", "dmInDays", &[("days", &(d.round() as i64))]),
                None => next.clone(),
            };
            let src = if w.next_source.as_deref() == Some("announced") {
                t("detail", "dmAnnounced")
            } else {
                t("detail", "dmPredicted")
            };
            let mut it = f(t("detail", "dmNextWipe"), value, None);
            it.tip = format!("{next} · {src}");
            v.push(it);
        }
    }
    v
}

pub fn show(ui: &mut Ui, cx: &Ctx, m: &Model, run: &mut Run) {
    let tk = ui.tokens();
    if matches!(m.population, Some(Population::Fake | Population::Suspect)) {
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: size::PAD as i8,
                right: size::PAD as i8,
                top: 10,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                population_warning(ui, m, false);
            });
    }

    // ── connection ──
    let address = m.address();
    let share = m.share_link();
    Section::new(t("detail", "connection"))
        .icon(lucide::PLUG)
        .first()
        .show(
            ui,
            |_| {},
            |ui| {
                let ports = format!("{} · {}", m.game_port, m.query_port);
                let ports_label =
                    format!("{} · {}", t("detail", "gamePort"), t("detail", "queryPort"));
                let pw = m.listed.as_ref().is_some_and(|l| l.password);
                let mut labels = vec![t("detail", "address"), ports_label.as_str()];
                if pw {
                    labels.push(t("detail", "password"));
                }
                labels.push(t("detail", "shareLink"));
                let small = font(Face::Sans, size::TEXT_2XS);
                let label_w = labels
                    .iter()
                    .map(|l| ph(ui, l, small.clone()).size().x)
                    .fold(0.0_f32, f32::max)
                    .min(ui.available_width() * 0.45);
                let row = |ui: &mut Ui, label: &str, value: &mut dyn FnMut(&mut Ui)| {
                    super::line(ui, 18.0, |ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        let g = ph_trunc(ui, label, small.clone(), label_w);
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::new(label_w, 18.0), Sense::hover());
                        draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
                        value(ui);
                    });
                };
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    row(ui, t("detail", "address"), &mut |ui| {
                        let _ = ui
                            .add(
                                Copy::new(address)
                                    .labels(t("common", "copy"), t("common", "copied"))
                                    .text_size(size::TEXT_XS)
                                    .color(ui.tokens().fg),
                            )
                            .tip_kbd(t("detail", "copyIp"), "", Side::Top);
                    });
                    row(ui, &ports_label, &mut |ui| {
                        let g = ph(ui, &ports, font(Face::Sans, size::TEXT_2XS));
                        let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                        draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
                    });
                    if pw {
                        row(ui, t("detail", "password"), &mut |ui| {
                            let g = ph(ui, t("detail", "passwordRequired"), small.clone());
                            let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                            draw(ui.painter(), &g, r.left(), r.center().y, tk.err);
                        });
                    }
                    row(ui, t("detail", "shareLink"), &mut |ui| {
                        let _ = ui
                            .add(
                                Copy::new(share).labels(t("common", "copy"), t("common", "copied")),
                            )
                            .tip_kbd(t("detail", "shareLinkHint"), "", Side::Top);
                    });
                });
                if ui
                    .add(
                        Button::new(t("detail", "openDirect"))
                            .icon(lucide::PLUG_ZAP)
                            .size(Size::Xs),
                    )
                    .clicked()
                {
                    run.push(("list.direct", target_args(m)));
                }
            },
        );

    // ── a favourite's notes (`views/favorites`), near the top: the player's own words ──
    let ports: Vec<u16> = [m.game_port, m.query_port]
        .into_iter()
        .filter_map(|p| u16::try_from(p).ok())
        .collect();
    crate::views::favorites::detail_notes(ui, cx, &m.ip, &ports, run);

    // ── details ──
    let items = facts(ui, m);
    Section::new(t("detail", "details"))
        .icon(lucide::INFO)
        .show(
            ui,
            |_| {},
            |ui| {
                let fs: Vec<Fact> = items
                    .iter()
                    .map(|i| {
                        let mut x = Fact::new(&i.label, &i.value);
                        if let Some(c) = i.tone {
                            x = x.tone(c);
                        }
                        if !i.tip.is_empty() {
                            x = x.tip(&i.tip);
                        }
                        x
                    })
                    .collect();
                ui.add(Facts::new(&fs));
                if m.live_error.is_some() && m.a2s.is_none() {
                    error_box(ui, t("detail", "liveFailed"));
                    // The way out beside the failure, not only the header's icon.
                    ui.add_space(6.0);
                    if ui
                        .add(
                            Button::new(t("detail", "retry"))
                                .icon(lucide::REFRESH_CW)
                                .size(Size::Sm),
                        )
                        .clicked()
                    {
                        run.push(("servers.detail.refresh_live", target_args(m)));
                    }
                }
            },
        );

    // ── mods ──
    Section::new(t("detail", "modsState"))
        .icon(lucide::PUZZLE)
        .show(ui, |_| {}, |ui| mods(ui, m, run));

    // ── players now ──
    Section::new(t("detail", "playersNow"))
        .icon(lucide::USERS)
        .show(ui, |_| {}, |ui| players(ui, m, run));

    // ── you ──
    Section::new(t("detail", "you"))
        .icon(lucide::HISTORY)
        .show(ui, |_| {}, |ui| you(ui, cx, m));
}

pub(super) fn error_box(ui: &mut Ui, text: &str) {
    let tk = ui.tokens();
    egui::Frame::new()
        .fill(alpha(tk.err, 0.1))
        .stroke(Stroke::new(1.0, alpha(tk.err, 0.3)))
        .corner_radius(CornerRadius::same(size::RADIUS_SM))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            words(ui, text, font(Face::Sans, size::TEXT_2XS), tk.err);
        });
}

/// A small tinted count (`rounded-xs bg-ok/12 px-1.5 py-0.5 text-ok`).
fn pill(ui: &mut Ui, text: &str, c: Color32) {
    let g = ph(ui, text, font(Face::Sans, size::TEXT_2XS));
    let (r, _) = ui.allocate_exact_size(g.size() + Vec2::new(12.0, 4.0), Sense::hover());
    ui.painter()
        .rect_filled(r, CornerRadius::same(size::RADIUS_XS), alpha(c, 0.12));
    draw(ui.painter(), &g, r.left() + 6.0, r.center().y, c);
}

/// A faint link that underlines under the pointer (`See all N`).
pub(super) fn link(ui: &mut Ui, text: &str) -> egui::Response {
    let tk = ui.tokens();
    let g = ph(ui, text, font(Face::Sans, size::TEXT_2XS));
    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::click());
    let c = if resp.hovered() { tk.fg } else { tk.fg_faint };
    draw(ui.painter(), &g, r.left(), r.center().y, c);
    if resp.hovered() {
        ui.painter()
            .hline(r.x_range(), r.bottom() + 1.0, Stroke::new(1.0, c));
    }
    if crate::kit::paint::keyboard_focus(ui, &resp) {
        crate::kit::paint::focus_ring(ui.painter(), r, size::RADIUS_XS, &tk);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, text));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn faint(ui: &mut Ui, s: &str) {
    let tk = ui.tokens();
    words(ui, s, font(Face::Sans, size::TEXT_2XS), tk.fg_faint);
}

fn mods(ui: &mut Ui, m: &Model, run: &mut Run) {
    let tk = ui.tokens();
    match &m.listed {
        Some(l) if l.mods_count > 0 => {
            if m.mod_rows.is_empty() {
                words(
                    ui,
                    &tf("detail", "modsFailed", &[("count", &l.mods_count)]),
                    font(Face::Sans, size::TEXT_2XS),
                    tk.warn,
                );
                return;
            }
            super::line(ui, 20.0, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                pill(ui, &format!("{} ✓", m.installed), tk.ok);
                if m.missing > 0 {
                    pill(
                        ui,
                        &format!("{} {}", m.missing, t("detail", "modMissing")),
                        tk.err,
                    );
                }
                if m.stale > 0 {
                    pill(
                        ui,
                        &format!("{} {}", m.stale, t("detail", "modStale")),
                        tk.warn,
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let s = tf("detail", "seeAll", &[("count", &m.mod_rows.len())]);
                    if link(ui, &s).clicked() {
                        run.push(("servers.detail.tab", json!({ "tab": "mods" })));
                    }
                });
            });
            if m.missing > 0 || m.stale > 0 {
                let label = if m.missing > 0 {
                    tf("detail", "downloadAndJoin", &[("count", &m.missing)])
                } else {
                    tf("detail", "updateAndJoin", &[("count", &m.stale)])
                };
                if ui
                    .add(
                        Button::new(&label)
                            .icon(lucide::DOWNLOAD)
                            .variant(Variant::Accent),
                    )
                    .clicked()
                {
                    run.push(("servers.detail.download_and_join", target_args(m)));
                }
            } else {
                super::line(ui, 16.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    lucide::CIRCLE_CHECK.paint(ui.painter(), r, tk.ok);
                    words(
                        ui,
                        t("detail", "allInstalled"),
                        font(Face::Sans, size::TEXT_2XS),
                        tk.ok,
                    );
                });
            }
        }
        _ if m.mods_count > 0 => {
            words(
                ui,
                t("detail", "modsFromA2s"),
                font(Face::Sans, size::TEXT_2XS),
                tk.fg_muted,
            );
        }
        _ => faint(ui, t("detail", "noMods")),
    }
}

fn players(ui: &mut Ui, m: &Model, run: &mut Run) {
    let tk = ui.tokens();
    if m.live_loading && m.a2s.is_none() {
        for _ in 0..3 {
            let (r, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
            Skeleton::paint(ui, r, Some(size::RADIUS_XS), 0.6);
            ui.add_space(4.0);
        }
        return;
    }
    if !m.players.is_empty() {
        super::stencil(ui, t("detail", "longestHere"), tk.fg_faint);
        ui.add_space(-4.0);
        for (i, p) in m.players.iter().take(5).enumerate() {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::hover());
            let figs = font(Face::Sans, size::TEXT_2XS);
            let n = ph(ui, &(i + 1).to_string(), figs.clone());
            let d = ph(ui, &format::duration(p.duration.max(0.0) as i64), figs);
            let name = if p.name.is_empty() {
                format::NONE
            } else {
                p.name.as_str()
            };
            let room = w - 16.0 - 8.0 - d.size().x - 8.0;
            let ng = ph_trunc(ui, name, font(Face::Sans, size::TEXT_2XS), room.max(10.0));
            let pt = ui.painter();
            draw(
                pt,
                &n,
                r.left() + 16.0 - n.size().x,
                r.center().y,
                tk.fg_faint,
            );
            draw(pt, &ng, r.left() + 24.0, r.center().y, tk.fg);
            draw(pt, &d, r.right() - d.size().x, r.center().y, tk.fg_muted);
        }
        if m.players.len() > 5 {
            let s = tf("detail", "seeAll", &[("count", &m.players.len())]);
            if link(ui, &s).clicked() {
                run.push(("servers.detail.tab", json!({ "tab": "players" })));
            }
        }
    } else if m.live_error.is_some() && m.a2s.is_none() {
        words(
            ui,
            t("detail", "liveFailed"),
            font(Face::Sans, size::TEXT_2XS),
            tk.err,
        );
    } else if m.a2s.as_ref().is_some_and(|a| a.players == 0) {
        faint(ui, t("detail", "a2sNoPlayers"));
    } else if m.a2s.is_some() {
        faint(ui, t("detail", "a2sNamesNotReported"));
    } else {
        faint(ui, t("detail", "a2sClickRefresh"));
    }
}

fn you(ui: &mut Ui, _cx: &Ctx, m: &Model) {
    let tk = ui.tokens();
    let small = font(Face::Sans, size::TEXT_2XS);
    super::line(ui, 18.0, |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        lucide::HISTORY.paint(ui.painter(), r, tk.fg_faint);
        match m.last_played {
            Some(ts) => {
                let s = tf(
                    "detail",
                    "lastPlayedHere",
                    &[("when", &format::relative(ts))],
                );
                let g = ph(ui, &s, small.clone());
                let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                draw(ui.painter(), &g, r.left(), r.center().y, tk.fg);
                let _ = resp.tip_kbd(&format::date_time(ts), "", Side::Top);
                let dot = ph(ui, "·", small.clone());
                let (r, _) = ui.allocate_exact_size(dot.size(), Sense::hover());
                draw(ui.painter(), &dot, r.left(), r.center().y, tk.fg_faint);
                let n = if m.sessions == 1 {
                    t("detail", "sessionsOne").to_string()
                } else {
                    tf("detail", "sessions", &[("count", &m.sessions)])
                };
                let g = ph(ui, &n, small.clone());
                let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
            }
            None => {
                let g = ph(ui, t("detail", "neverPlayed"), small.clone());
                let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
            }
        }
    });
    ui.add_space(-4.0);
    super::line(ui, 18.0, |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        crate::parts::cells::paint_star(ui, r, 14.0, m.favorite, 0.0, None);
        let s = if m.favorite {
            t("detail", "inFavorites")
        } else {
            t("detail", "notInFavorites")
        };
        let g = ph(ui, s, small.clone());
        let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
        draw(
            ui.painter(),
            &g,
            r.left(),
            r.center().y,
            if m.favorite { tk.fg } else { tk.fg_faint },
        );
    });
}
