//! Under the header: each source of the server list, what it has brought and how far it
//! has got; the breakdown opens on "Details" (= `SourcesBar.svelte`).
//!
//! The clock that counts down Steam's throttle and the elapsed time ticks only while one
//! of those is on screen: an idle bar asks for no frame.

use std::sync::Arc;

use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Align, Meter, MeterSize, Side, Spinner, TipExt, menu};
use crate::parts::{draw, ph, ph_trunc};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};
use dz_hub::events::{SourcePhase as Phase, SourcesChanged};
use egui::{Color32, CornerRadius, Galley, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};

/// The bar's height: `py-1` around a 2xs line.
pub const BAR_H: f32 = 24.0;

fn phase_word(p: Phase) -> &'static str {
    t(
        "sources",
        match p {
            Phase::Idle => "idle",
            Phase::Running => "running",
            Phase::Done => "done",
            Phase::Failed => "failed",
            Phase::Off => "off",
        },
    )
}

fn dot(t: &Tokens, p: Phase) -> Color32 {
    match p {
        Phase::Idle => alpha(t.fg_faint, 0.5),
        Phase::Running => t.accent,
        Phase::Done => t.ok,
        Phase::Failed => t.err,
        Phase::Off => alpha(t.fg_faint, 0.3),
    }
}

fn secs(n: i64) -> String {
    if n < 60 {
        format!("{n} s")
    } else {
        format!("{} min {} s", n / 60, n % 60)
    }
}

fn now_s() -> i64 {
    crate::store::now_ms() / 1000
}

/// Lays text out in one go: (galley, colour) pieces drawn left to right.
struct Line {
    parts: Vec<(Arc<Galley>, Color32, f32)>,
}

impl Line {
    fn new() -> Self {
        Self { parts: Vec::new() }
    }

    fn push(&mut self, g: Arc<Galley>, c: Color32, gap: f32) {
        self.parts.push((g, c, gap));
    }

    fn width(&self) -> f32 {
        self.parts.iter().map(|(g, _, gap)| g.size().x + gap).sum()
    }
}

/// The bar. `open` (a command asked) opens the details.
pub fn bar(ui: &mut Ui, s: &SourcesChanged, open: bool) {
    let tk = ui.tokens();
    let full = ui.available_rect_before_wrap();
    let rect = Rect::from_min_size(full.min, Vec2::new(full.width(), BAR_H));
    ui.allocate_rect(rect, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, alpha(tk.bg, 0.3));
    p.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, tk.border),
    );
    let inner = rect.shrink2(Vec2::new(size::PAD, 0.0));
    let cy = rect.center().y;
    let small = font(Face::Sans, size::TEXT_2XS);
    let mono = font(Face::Mono, size::TEXT_2XS);
    let now = now_s();
    let steam = &s.steam;
    let wait_left = steam.waiting_until.map(|w| (w - now).max(0));
    if wait_left.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
    }

    // ── the right end: in the list · official · duplicates · Details ──
    let narrow = rect.width() < 760.0;
    let details_label = t("sources", "details");
    let dg = ph(ui, details_label, small.clone());
    let details_w = 4.0 + 12.0 + 4.0 + dg.size().x + 4.0;
    let mut right = Line::new();
    right.push(
        ph(ui, t("sources", "inList"), small.clone()),
        tk.fg_muted,
        8.0,
    );
    right.push(
        ph(ui, &format::num(f64::from(s.total)), mono.clone()),
        tk.fg,
        8.0,
    );
    if !narrow {
        let off = tf(
            "sources",
            "official",
            &[("count", &format::num(f64::from(s.official)))],
        );
        right.push(ph(ui, &format!("· {off}"), small.clone()), tk.fg_faint, 8.0);
        if s.duplicates > 0 {
            let d = tf(
                "sources",
                "duplicates",
                &[("count", &format::num(f64::from(s.duplicates)))],
            );
            right.push(ph(ui, &format!("· {d}"), small.clone()), tk.fg_faint, 8.0);
        }
    }
    let right_w = right.width() + details_w;
    let right_x = inner.right() - right_w;
    let mut x = right_x;
    for (g, c, gap) in &right.parts {
        draw(ui.painter(), g, x, cy, *c);
        x += g.size().x + gap;
    }
    if !narrow && s.duplicates > 0 {
        // The duplicates' hint, over their words.
        if let Some((g, _, _)) = right.parts.last() {
            let r = Rect::from_min_size(
                Pos2::new(x - g.size().x - 8.0, rect.top()),
                Vec2::new(g.size().x, BAR_H),
            );
            let _ = ui
                .interact(r, ui.id().with("dup-hint"), Sense::hover())
                .tip_kbd(t("sources", "duplicatesHint"), "", Side::Bottom);
        }
    }
    let dr = Rect::from_min_size(Pos2::new(x, cy - 9.0), Vec2::new(details_w, 18.0));
    let dresp = ui.interact(dr, ui.id().with("sources-details"), Sense::click());
    let open_now = menu::is_open(&dresp);
    let h = paint::ease(ui, dresp.id.with("hover"), dresp.hovered() || open_now);
    {
        let p = ui.painter();
        p.rect_filled(dr, CornerRadius::same(size::RADIUS_XS), alpha(tk.raised, h));
        let ink = paint::lerp(tk.fg_muted, tk.fg, h);
        lucide::INFO.paint(
            p,
            Rect::from_center_size(Pos2::new(dr.left() + 4.0 + 6.0, cy), Vec2::splat(12.0)),
            ink,
        );
        draw(p, &dg, dr.left() + 20.0, cy, ink);
        if paint::keyboard_focus(ui, &dresp) {
            paint::focus_ring(p, dr, size::RADIUS_XS, &tk);
        }
    }
    dresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, details_label));
    let dresp = dresp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if open && !open_now {
        egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&dresp));
    }
    menu::popover(&dresp, Side::Bottom, Align::End, |ui| details(ui, s, now));

    // ── the left: each source ──
    let left = Rect::from_min_max(inner.min, Pos2::new(right_x - 20.0, inner.bottom()));
    let mut lui = ui.new_child(
        UiBuilder::new()
            .max_rect(left)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    lui.set_clip_rect(left.intersect(ui.clip_rect()));
    lui.spacing_mut().item_spacing.x = 20.0;
    let ui = &mut lui;

    // The public list.
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        cell(
            ui,
            t("sources", "dzsa"),
            s.dzsa.phase,
            t("sources", "dzsaHint"),
        );
        if s.dzsa.phase == Phase::Running {
            ui.add(Spinner::new().size(12.0));
        }
        text(ui, &format::num(f64::from(s.dzsa.count)), &mono, tk.fg);
    });

    // Kept: the servers earlier lists had, checked again; its room is kept from Steam's
    // figures, and it is left out whole when it does not fit (a word cut by the right
    // end reads as a different one).
    let kept_n = if s.kept.alive > 0 {
        s.kept.alive
    } else {
        s.kept.total
    };
    let kept_w = {
        let name =
            crate::views::servers::stencil_galley(ui, t("sources", "kept"), tk.fg_muted, None);
        let num = ph(ui, &format::num(f64::from(kept_n)), mono.clone());
        12.0 + name.size().x + 8.0 + num.size().x
    };
    let kept_room = if s.kept.total > 0 { kept_w + 20.0 } else { 0.0 };

    // Steam: via, the requests' meter or the phase, then its figures (first to go).
    let via = if steam.needs_key {
        t("sources", "needsKey")
    } else if steam.phase != Phase::Off {
        t("sources", "viaWeb")
    } else {
        ""
    };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        cell(
            ui,
            t("sources", "steam"),
            steam.phase,
            t("sources", "steamHint"),
        );
        if !via.is_empty() {
            // A missing key is the reason Steam brings nothing: said in the warning
            // tone, not as faint as a detail.
            let tone = if steam.needs_key {
                tk.warn
            } else {
                tk.fg_faint
            };
            text(ui, via, &small, tone);
        }
        if steam.phase == Phase::Running && steam.requests_total > 0 {
            let frac = steam.requests_done as f32 / steam.requests_total.max(1) as f32;
            let label = tf(
                "sources",
                "requests",
                &[
                    ("done", &steam.requests_done),
                    ("total", &steam.requests_total),
                ],
            );
            ui.add(Meter::new(frac).size(MeterSize::Xs).width(80.0))
                .tip_kbd(&label, "", Side::Bottom);
            text(
                ui,
                &format!("{}/{}", steam.requests_done, steam.requests_total),
                &mono,
                tk.fg_muted,
            );
        } else if steam.phase != Phase::Running {
            // A listing skipped because the last one is recent says when that was.
            let word = match steam.finished.filter(|_| steam.recent) {
                Some(at) => tf("sources", "listedAgo", &[("ago", &format::relative(at))]),
                None => phase_word(steam.phase).to_owned(),
            };
            text(ui, &word, &small, tk.fg_faint);
        }
        if let Some(w) = wait_left {
            let s = tf("sources", "waiting", &[("secs", &w)]);
            text(ui, &s, &small, tk.warn);
        } else if steam.phase != Phase::Off && rect.width() >= 900.0 {
            // What Steam brought, in words; the counts behind it in the tip (and in
            // Details).
            let room = (ui.available_width() - kept_room).max(0.0);
            let added = format!("+{}", format::num(f64::from(steam.added)));
            let word = format!(" {}", t("sources", "added").to_lowercase());
            let tip = format!(
                "{} {} · {} {}/{} · {} {}",
                t("sources", "added"),
                format::num(f64::from(steam.added)),
                t("sources", "answered"),
                format::num(f64::from(steam.answered)),
                format::num(f64::from(steam.queried)),
                t("sources", "listed"),
                format::num(f64::from(steam.listed)),
            );
            let ga = ph(ui, &added, mono.clone());
            // The figure whole or nothing; its word may shorten.
            if room >= ga.size().x {
                let gr = ph_trunc(ui, &word, small.clone(), room - ga.size().x);
                let (r, resp) = ui.allocate_exact_size(
                    Vec2::new((ga.size().x + gr.size().x).min(room), BAR_H),
                    Sense::hover(),
                );
                draw(ui.painter(), &ga, r.left(), cy, tk.ok);
                draw(ui.painter(), &gr, r.left() + ga.size().x, cy, tk.fg_muted);
                let _ = resp.tip_kbd(&tip, "", Side::Bottom);
            }
        }
    });

    // Kept: the servers earlier lists had, checked again.
    if s.kept.total > 0 && ui.available_width() >= kept_room {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            cell(
                ui,
                t("sources", "kept"),
                s.kept.phase,
                t("sources", "keptHint"),
            );
            text(ui, &format::num(f64::from(kept_n)), &mono, tk.fg);
            if s.kept.removed > 0 {
                text(
                    ui,
                    &format!("−{}", format::num(f64::from(s.kept.removed))),
                    &mono,
                    tk.err,
                );
            }
        });
    }
}

fn text(ui: &mut Ui, s: &str, f: &egui::FontId, c: Color32) {
    let g = ph(ui, s, f.clone());
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, BAR_H), Sense::hover());
    draw(ui.painter(), &g, r.left(), r.center().y, c);
}

/// A source's dot and stencilled name, with its hint.
/// A running source's dot is lit, not pulsed: a scan runs for minutes, and an idle
/// window draws nothing.
fn cell(ui: &mut Ui, label: &str, phase: Phase, hint: &str) {
    let tk = ui.tokens();
    let g = crate::views::servers::stencil_galley(ui, label, tk.fg_muted, None);
    let (r, resp) =
        ui.allocate_exact_size(Vec2::new(6.0 + 6.0 + g.size().x, BAR_H), Sense::hover());
    let p = ui.painter();
    p.circle_filled(
        Pos2::new(r.left() + 3.0, r.center().y),
        3.0,
        dot(&tk, phase),
    );
    p.galley(
        Pos2::new(r.left() + 12.0, r.center().y - g.size().y / 2.0),
        g,
        tk.fg_muted,
    );
    let _ = resp.tip_kbd(hint, "", Side::Bottom);
}

/// The details popover: per source, its phase, its hint and its figures.
fn details(ui: &mut Ui, s: &SourcesChanged, now: i64) {
    let tk = ui.tokens();
    egui::Frame::new()
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(320.0 - 24.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
            let w = ui.available_width();
            para(
                ui,
                t("sources", "title"),
                font(Face::Display, size::TEXT_SM),
                tk.fg,
                w,
            );
            ui.add_space(8.0);

            head(
                ui,
                &format!("{} · {}", t("sources", "dzsa"), phase_word(s.dzsa.phase)),
            );
            hint(ui, t("sources", "dzsaHint"), w);
            line(
                ui,
                t("sources", "inList"),
                &format::num(f64::from(s.dzsa.count)),
                tk.fg,
            );
            if let Some(at) = s.dzsa.at {
                line(ui, t("sources", "fetched"), &format::relative(at), tk.fg);
            }
            if let Some(e) = &s.dzsa.error {
                para(ui, e, font(Face::Sans, size::TEXT_2XS), tk.err, w);
            }

            let st = &s.steam;
            let via = if st.needs_key {
                t("sources", "needsKey")
            } else if st.phase != Phase::Off {
                t("sources", "viaWeb")
            } else {
                ""
            };
            ui.add_space(12.0);
            let via_s = if via.is_empty() {
                String::new()
            } else {
                format!(" · {via}")
            };
            head(
                ui,
                &format!(
                    "{}{} · {}",
                    t("sources", "steam"),
                    via_s,
                    phase_word(st.phase)
                ),
            );
            hint(ui, t("sources", "steamHint"), w);
            if let Some(n) = &st.note {
                para(ui, n, font(Face::Sans, size::TEXT_2XS), tk.warn, w);
                ui.add_space(4.0);
            }
            let n = |v: u32| format::num(f64::from(v));
            line(
                ui,
                t("sources", "requestsLabel"),
                &format!("{}/{}", st.requests_done, st.requests_total),
                tk.fg,
            );
            if let Some(c) = st.current.as_deref().filter(|_| st.phase == Phase::Running) {
                line(ui, t("sources", "now"), c, tk.fg_muted);
            }
            line(ui, t("sources", "listed"), &n(st.listed), tk.fg);
            line(ui, t("sources", "farms"), &n(st.farms), tk.fg_faint);
            line(ui, t("sources", "known"), &n(st.known), tk.fg_faint);
            line(ui, t("sources", "queried"), &n(st.queried), tk.fg);
            line(ui, t("sources", "answered"), &n(st.answered), tk.fg);
            line(ui, t("sources", "retried"), &n(st.retried), tk.fg_muted);
            line(ui, t("sources", "noMods"), &n(st.no_mods), tk.fg_faint);
            line(ui, t("sources", "fakes"), &n(st.fakes), tk.fg_faint);
            if st.remembered > 0 {
                line(
                    ui,
                    t("sources", "remembered"),
                    &n(st.remembered),
                    tk.fg_faint,
                );
            }
            line(
                ui,
                t("sources", "added"),
                &format!("+{}", n(st.added)),
                tk.ok,
            );
            if let Some(started) = st.started {
                if st.finished.is_none() {
                    // The elapsed time counts while the details are open.
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs(1));
                }
                let took = (st.finished.unwrap_or(now) - started).max(0);
                line(ui, t("sources", "elapsed"), &secs(took), tk.fg);
            }

            if s.duplicates > 0 {
                ui.add_space(12.0);
                let d = tf(
                    "sources",
                    "duplicates",
                    &[("count", &format::num(f64::from(s.duplicates)))],
                );
                para(ui, &d, font(Face::Sans, size::TEXT_2XS), tk.fg_muted, w);
                hint(ui, t("sources", "duplicatesHint"), w);
            }

            let k = &s.kept;
            if k.total > 0 {
                ui.add_space(12.0);
                head(
                    ui,
                    &format!("{} · {}", t("sources", "kept"), phase_word(k.phase)),
                );
                hint(ui, t("sources", "keptHint"), w);
                line(
                    ui,
                    t("sources", "checked"),
                    &format!("{}/{}", n(k.checked), n(k.total)),
                    tk.fg,
                );
                line(ui, t("sources", "alive"), &n(k.alive), tk.ok);
                line(ui, t("sources", "removed"), &n(k.removed), tk.err);
                line(ui, t("sources", "fakes"), &n(k.fakes), tk.err);
            }
        });
}

fn para(ui: &mut Ui, s: &str, f: egui::FontId, c: Color32, w: f32) {
    let g = paint::galley_wrap(ui, s, f, c, w);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, c);
}

fn head(ui: &mut Ui, s: &str) {
    let tk = ui.tokens();
    let g = crate::views::servers::stencil_galley(ui, s, tk.fg_muted, Some(ui.available_width()));
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, tk.fg_muted);
}

fn hint(ui: &mut Ui, s: &str, w: f32) {
    let tk = ui.tokens();
    para(ui, s, font(Face::Sans, size::TEXT_2XS), tk.fg_faint, w);
    ui.add_space(4.0);
}

/// A label and its figure, baseline to baseline.
fn line(ui: &mut Ui, label: &str, value: &str, tone: Color32) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let gv = ph(ui, value, font(Face::Mono, size::TEXT_2XS));
    let gl = ph_trunc(
        ui,
        label,
        font(Face::Sans, size::TEXT_2XS),
        (w - gv.size().x - 16.0).max(20.0),
    );
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 19.0), Sense::hover());
    let p = ui.painter();
    draw(p, &gl, r.left(), r.center().y, tk.fg_muted);
    draw(p, &gv, r.right() - gv.size().x, r.center().y, tone);
}
