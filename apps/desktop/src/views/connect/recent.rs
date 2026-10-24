//! Addresses to start from (= `RecentList.svelte`): favourites first (they carry saved
//! passwords), then the servers joined most recently, each group under its own heading
//! with its count. A click fills the form and asks the
//! server. Wide: cards over the right pane with what the list knows of each (map,
//! players); compact: a list under the form.

use std::sync::Arc;

use dz_hub::features::browser::ServerRef;
use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::app::Ctx;
use crate::i18n::{format, t};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{EmptyState, SectionLabel, Signal, SignalSize, TipExt};
use crate::theme::{TokensExt, font, fonts::Face, size};

struct Pick {
    name: String,
    ip: String,
    port: u16,
    ts: Option<i64>,
    password: Option<String>,
    fav: bool,
}

fn picks(cx: &Ctx, wide: bool) -> Vec<Pick> {
    let mut seen = rustc_hash::FxHashSet::default();
    let mut out = Vec::new();
    let p = &cx.stores.profile;
    for f in &p.favorites {
        if seen.insert((f.ip.as_str(), f.port)) {
            out.push(Pick {
                name: f.name.clone(),
                ip: f.ip.clone(),
                port: f.port,
                ts: None,
                password: f.password.clone(),
                fav: true,
            });
        }
    }
    for h in &p.history {
        if seen.insert((h.ip.as_str(), h.port)) {
            out.push(Pick {
                name: h.name.clone(),
                ip: h.ip.clone(),
                port: h.port,
                ts: Some(h.ts),
                password: None,
                fav: false,
            });
        }
    }
    out.truncate(if wide { 24 } else { 14 });
    out
}

/// The icon, the heading and the words over the cards.
pub fn heading(ui: &mut Ui) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let x0 = size::ICON + 10.0;
    let sub = paint::galley_wrap(
        ui,
        t("connect", "startFrom"),
        font(Face::Sans, size::TEXT_2XS),
        tk.fg_muted,
        w - x0,
    );
    let h = 18.0 + 2.0 + sub.size().y;
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    lucide::PLUG_ZAP.paint(
        p,
        Rect::from_min_size(Pos2::new(r.left(), r.top() + 2.0), Vec2::splat(size::ICON)),
        tk.accent,
    );
    let title = paint::galley(
        ui,
        t("connect", "recent"),
        font(Face::SansSemibold, size::TEXT_SM),
        tk.fg,
    );
    p.galley(Pos2::new(r.left() + x0, r.top()), title, tk.fg);
    p.galley(Pos2::new(r.left() + x0, r.top() + 20.0), sub, tk.fg_muted);
    ui.add_space(14.0);
}

/// The recent servers as cards (`wide`) or a list; a click loads one.
pub fn list(ui: &mut Ui, cx: &mut Ctx, wide: bool) {
    let picks = picks(cx, wide);
    if picks.is_empty() {
        EmptyState::new()
            .icon(lucide::HISTORY)
            .title(t("connect", "noRecent"))
            .compact()
            .show(ui, |_| {});
        return;
    }
    // Borrowed from the snapshot (no owned rows); the pings as every list resolves
    // them, the scan's in the snapshot first.
    let snap = Arc::clone(cx.stores.servers.snapshot());
    let rows: Vec<Option<ServerRef<'_>>> = picks
        .iter()
        .map(|p| dz_api::addr(&p.ip, i64::from(p.port)).and_then(|a| snap.lookup(a)))
        .collect();
    let pings: Vec<Option<u32>> = picks
        .iter()
        .map(|p| cx.stores.servers.ping_ms(&p.ip, i64::from(p.port)))
        .collect();
    let mut chosen = None;
    // Favourites come first in `picks`: split there, each group under its own heading.
    let favs = picks.iter().take_while(|p| p.fav).count();
    let groups = [
        (0..favs, lucide::STAR, t("connect", "groupFavorites")),
        (
            favs..picks.len(),
            lucide::HISTORY,
            t("connect", "groupRecent"),
        ),
    ];
    let mut first = true;
    for (range, icon, title) in groups {
        if range.is_empty() {
            continue;
        }
        if !first {
            ui.add_space(if wide { 16.0 } else { 8.0 });
        }
        first = false;
        group_heading(ui, icon, title, range.len(), wide);
        let base = range.start;
        let (Some(gp), Some(gr), Some(gs)) = (
            picks.get(range.clone()),
            rows.get(range.clone()),
            pings.get(range),
        ) else {
            continue;
        };
        if wide {
            cards(ui, base, gp, gr, gs, &mut chosen);
        } else {
            compact(ui, base, gp, gr, gs, &mut chosen);
        }
    }
    if let Some(i) = chosen
        && let Some(p) = picks.get(i)
    {
        let row = rows.get(i).copied().flatten();
        let port = row
            .as_ref()
            .map(|r| r.game_port())
            .filter(|p| *p != 0)
            .unwrap_or(p.port);
        let qp = row.as_ref().map(|r| r.query_port());
        cx.stores.direct_load(&p.ip, port, qp, p.password.clone());
    }
}

/// "Favourites 3" / "Recent 12": a section label with the group's count at its end.
fn group_heading(ui: &mut Ui, icon: crate::icons::Icon, title: &str, n: usize, wide: bool) {
    let tk = ui.tokens();
    let r = ui.available_rect_before_wrap();
    // Under the form, inset like the rows' contents.
    let (inset, top) = if wide { (0.0, 0.0) } else { (12.0, 6.0) };
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(
        Pos2::new(r.left() + inset, r.top() + top),
        Pos2::new(r.right() - inset, r.bottom()),
    )));
    let count = n.to_string();
    SectionLabel::new(title)
        .icon(icon)
        .show_with(&mut lui, |ui| {
            let g = paint::galley(ui, &count, font(Face::Mono, size::TEXT_2XS), tk.fg_faint);
            let (rr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
            ui.painter().galley(rr.min, g, tk.fg_faint);
        });
    let h = (lui.min_rect().bottom() - r.top()).max(0.0);
    ui.allocate_exact_size(Vec2::new(r.width(), h), Sense::hover());
    ui.add_space(if wide { 8.0 } else { 4.0 });
}

fn star_or_clock(
    p: &egui::Painter,
    rect: Rect,
    fav: bool,
    filled: bool,
    tk: &crate::theme::Tokens,
) {
    if fav {
        if filled {
            super::filled_star(p, rect, tk.warn);
        } else {
            lucide::STAR.paint(p, rect, tk.warn);
        }
    } else {
        lucide::HISTORY.paint(p, rect, tk.fg_faint);
    }
}

/// `grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-2`.
fn cards(
    ui: &mut Ui,
    base: usize,
    picks: &[Pick],
    rows: &[Option<ServerRef<'_>>],
    pings: &[Option<u32>],
    chosen: &mut Option<usize>,
) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let gap = 8.0;
    let cols = (((w + gap) / (240.0 + gap)).floor() as usize).max(1);
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    let inner = cw - 20.0;
    let name_f = font(Face::SansSemibold, size::TEXT_XS);
    let mono = font(Face::Mono, size::TEXT_3XS);
    let small = font(Face::Sans, size::TEXT_2XS);
    let start = ui.cursor().min;
    let mut y = start.y;
    for (ri, chunk) in picks.chunks(cols).enumerate() {
        // Lay every card of the row out first: the row is as tall as its tallest.
        let laid: Vec<_> = chunk
            .iter()
            .enumerate()
            .map(|(ci, p)| {
                let i = ri * cols + ci;
                let row = rows.get(i).copied().flatten();
                let name_w = inner - 14.0 - 8.0 - if row.is_some() { 14.0 + 10.0 } else { 0.0 };
                let mut job = egui::text::LayoutJob::single_section(
                    p.name.clone(),
                    egui::TextFormat {
                        font_id: name_f.clone(),
                        color: tk.fg,
                        line_height: Some(17.5),
                        ..Default::default()
                    },
                );
                job.wrap.max_width = name_w.max(20.0);
                job.wrap.max_rows = 2;
                job.wrap.break_anywhere = false;
                job.wrap.overflow_character = Some('…');
                let name = ui.painter().layout_job(job);
                let h = 10.0
                    + name.size().y.max(17.5)
                    + 5.0
                    + 14.0
                    + if row.is_some() { 5.0 + 16.0 } else { 0.0 }
                    + 11.0;
                (i, row, name, h)
            })
            .collect();
        let row_h = laid.iter().map(|l| l.3).fold(0.0_f32, f32::max);
        for (ci, (i, row, name, _)) in laid.into_iter().enumerate() {
            let Some(p) = picks.get(i) else { continue };
            let x = start.x + ci as f32 * (cw + gap);
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(cw, row_h));
            let resp = ui.interact(
                rect,
                ui.id().with(("recent-card", base + i)),
                Sense::click(),
            );
            let label = p.name.clone();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            let hover = paint::ease(ui, resp.id.with("h"), resp.hovered() || resp.has_focus());
            let painter = ui.painter();
            painter.rect(
                rect,
                CornerRadius::same(size::RADIUS_MD),
                alpha(tk.raised, 0.3 + 0.4 * hover),
                Stroke::new(1.0, paint::lerp(tk.border, alpha(tk.accent, 0.5), hover)),
                StrokeKind::Inside,
            );
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(painter, rect, size::RADIUS_MD, &tk);
            }
            let l = rect.left() + 10.0;
            let r = rect.right() - 10.0;
            let mut cy = rect.top() + 10.0;
            star_or_clock(
                painter,
                Rect::from_min_size(Pos2::new(l, cy + 2.0), Vec2::splat(14.0)),
                p.fav,
                false,
                &tk,
            );
            painter.galley(Pos2::new(l + 22.0, cy), name.clone(), tk.fg);
            if row.is_some() {
                let ms = pings.get(i).copied().flatten();
                let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
                    Pos2::new(r - 14.0, cy + 2.0),
                    Vec2::new(14.0, 14.0),
                )));
                sui.add(Signal::new(ms).bars_only().size(SignalSize::Xs));
            }
            cy += name.size().y.max(17.5) + 5.0;
            let port = row
                .as_ref()
                .map(|s| s.game_port())
                .filter(|p| *p != 0)
                .unwrap_or(p.port);
            let addr = paint::galley_trunc(
                ui,
                &format!("{}:{port}", p.ip),
                mono.clone(),
                tk.fg_faint,
                inner * 0.6,
            );
            let painter = ui.painter();
            painter.galley(Pos2::new(l, cy), addr, tk.fg_faint);
            if let Some(ts) = p.ts {
                let g = paint::galley(ui, &format::relative(ts), mono.clone(), tk.fg_faint);
                ui.painter()
                    .galley(Pos2::new(r - g.size().x, cy), g, tk.fg_faint);
            }
            cy += 14.0 + 5.0 + 1.0;
            if let Some(s) = &row {
                let pl = paint::galley(ui, &s.players().to_string(), mono.clone(), tk.fg_muted);
                let mx = paint::galley(
                    ui,
                    &format!("/{}", s.max_players()),
                    mono.clone(),
                    tk.fg_faint,
                );
                let px = r - mx.size().x;
                let p2 = ui.painter();
                p2.galley(Pos2::new(px, cy + 2.0), mx, tk.fg_faint);
                p2.galley(
                    Pos2::new(px - pl.size().x, cy + 2.0),
                    pl.clone(),
                    tk.fg_muted,
                );
                let map = paint::galley_trunc(
                    ui,
                    s.map,
                    small.clone(),
                    tk.map,
                    (inner - pl.size().x - 40.0).max(20.0),
                );
                ui.painter().galley(Pos2::new(l, cy), map, tk.map);
            }
            let resp = if p.fav {
                resp.tip(t("connect", "fromFavorites"))
            } else {
                resp
            };
            if resp.clicked() {
                *chosen = Some(base + i);
            }
            let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        y += row_h + gap;
    }
    let h = (y - gap - start.y).max(0.0);
    ui.allocate_rect(Rect::from_min_size(start, Vec2::new(w, h)), Sense::hover());
}

/// One line each, under the form.
fn compact(
    ui: &mut Ui,
    base: usize,
    picks: &[Pick],
    rows: &[Option<ServerRef<'_>>],
    pings: &[Option<u32>],
    chosen: &mut Option<usize>,
) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let mono = font(Face::Mono, size::TEXT_3XS);
    let n = picks.len();
    for (i, p) in picks.iter().enumerate() {
        let row = rows.get(i).copied().flatten();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 42.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &p.name));
        let hover = paint::ease(ui, resp.id.with("h"), resp.hovered() || resp.has_focus());
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, alpha(tk.raised, 0.6 * hover));
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(painter, rect.shrink(3.0), size::RADIUS_XS, &tk);
        }
        if i + 1 < n {
            painter.hline(
                rect.x_range(),
                rect.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.4)),
            );
        }
        let l = rect.left() + 12.0;
        star_or_clock(
            painter,
            Rect::from_center_size(Pos2::new(l + 6.0, rect.center().y), Vec2::splat(12.0)),
            p.fav,
            false,
            &tk,
        );
        let tx = l + 12.0 + 8.0;
        let sig_w = if row.is_some() { 14.0 + 8.0 } else { 0.0 };
        let tw = (rect.right() - 12.0 - sig_w - tx).max(20.0);
        let name = paint::galley_trunc(ui, &p.name, font(Face::Sans, size::TEXT_2XS), tk.fg, tw);
        let port = row
            .as_ref()
            .map(|s| s.game_port())
            .filter(|p| *p != 0)
            .unwrap_or(p.port);
        let addr = paint::galley(ui, &format!("{}:{port}", p.ip), mono.clone(), tk.fg_faint);
        let painter = ui.painter();
        painter.galley(Pos2::new(tx, rect.top() + 6.0), name, tk.fg);
        let ay = rect.top() + 6.0 + 15.0;
        let aw = addr.size().x;
        painter.galley(Pos2::new(tx, ay), addr, tk.fg_faint);
        if let Some(ts) = p.ts {
            let g = paint::galley_trunc(
                ui,
                &format::relative(ts),
                mono.clone(),
                tk.fg_faint,
                (tw - aw - 8.0).max(10.0),
            );
            ui.painter()
                .galley(Pos2::new(tx + aw + 8.0, ay), g, tk.fg_faint);
        }
        if row.is_some() {
            let ms = pings.get(i).copied().flatten();
            let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(
                Pos2::new(rect.right() - 12.0 - 7.0, rect.center().y),
                Vec2::splat(14.0),
            )));
            sui.add(Signal::new(ms).bars_only().size(SignalSize::Xs));
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            *chosen = Some(base + i);
        }
    }
}
