//! The year of days (= `Calendar.svelte`): a column per week (Monday on top), each day as
//! dark as the time played on it, in four steps of one hue. ‹ › move the year shown, back
//! to the first session. The pointer or the keyboard (focus the grid; ←/→ a week, ↑/↓ a
//! day) picks a day and shows what was played; a click or Enter narrows the history to it.
//! The days still to come in the last week are outlines, apart from days not played.

use egui::text::{LayoutJob, TextFormat};
use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use serde_json::Value;

use super::{Loaded, Run, StatsState, dates, dur_or_dash, now, sessions_text};
use crate::i18n::{self, t};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{IconButton, Size};
use crate::parts::{chart_tip_job, draw, ph, ph_trunc};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// Columns: a year and a week.
pub const WEEKS: i64 = 53;
/// The accent's opacity at each step.
const OPACITY: [f32; 5] = [0.0, 0.3, 0.5, 0.75, 1.0];
/// `gap-[3px]`.
const GAP: f32 = 3.0;

/// Draw the calendar; the day clicked (or Enter on the picked one), if any. The arrows
/// queue `stats.calendar_prev` / `_next` into `run`.
pub fn show(ui: &mut Ui, l: &Loaded, st: &mut StatsState, run: &mut Run) -> Option<i64> {
    let tk = ui.tokens();
    let loc = i18n::locale();
    let offset = dates::utc_offset();
    let today = dates::day_of(now(), offset);
    let end = st.cal.end.unwrap_or(today).min(today);
    let start = dates::first_monday(end, WEEKS);
    let first_day = l.derived.first_day.min(today);
    let w = ui.available_width();
    ui.spacing_mut().item_spacing.y = 6.0;

    // ‹ oct. 2025 – oct. 2026 ›
    let span = match (dates::date_of(start), dates::date_of(end)) {
        (Some(a), Some(b)) => format!(
            "{} – {}",
            dates::month_year_in(loc, a),
            dates::month_year_in(loc, b)
        ),
        _ => String::new(),
    };
    ui.allocate_ui_with_layout(
        Vec2::new(w, size::CONTROL_SM),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_height(size::CONTROL_SM);
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui
                .add(
                    IconButton::new(lucide::CHEVRON_LEFT, t("stats", "prevYear"))
                        .size(Size::IconXs)
                        .enabled(start > first_day),
                )
                .clicked()
            {
                run.push(("stats.calendar_prev", Value::Null));
            }
            let g = ph(ui, &span, font(Face::Mono, size::TEXT_2XS));
            let (r, _) = ui.allocate_exact_size(
                Vec2::new(g.size().x.max(128.0), size::CONTROL_SM),
                Sense::hover(),
            );
            draw(
                ui.painter(),
                &g,
                r.center().x - g.size().x / 2.0,
                r.center().y,
                tk.fg_muted,
            );
            if ui
                .add(
                    IconButton::new(lucide::CHEVRON_RIGHT, t("stats", "nextYear"))
                        .size(Size::IconXs)
                        .enabled(end < today),
                )
                .clicked()
            {
                run.push(("stats.calendar_next", Value::Null));
            }
        },
    );

    let cell = ((w - (WEEKS - 1) as f32 * GAP) / WEEKS as f32).max(3.0);
    let col_x = |left: f32, c: i64| left + c as f32 * (cell + GAP);
    let (mr, _) = ui.allocate_exact_size(Vec2::new(w, 12.0), Sense::hover());
    let grid_h = 7.0 * cell + 6.0 * GAP;
    let (grid, _) = ui.allocate_exact_size(Vec2::new(w, grid_h), Sense::hover());
    let id = ui.id().with("stats-calendar");
    let resp = ui.interact(grid, id, Sense::click());

    // The keyboard first: it may move the year shown.
    if resp.gained_focus() && st.cal.active.is_none() {
        st.cal.active = Some(today.min(end));
    }
    if resp.has_focus() {
        let mv = ui.input(|i| {
            let k = |key| i.key_pressed(key);
            if k(egui::Key::ArrowLeft) {
                Some(-7)
            } else if k(egui::Key::ArrowRight) {
                Some(7)
            } else if k(egui::Key::ArrowUp) {
                Some(-1)
            } else if k(egui::Key::ArrowDown) {
                Some(1)
            } else {
                None
            }
        });
        if let Some(by) = mv {
            st.move_day(by);
        }
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
    } else if resp.lost_focus() {
        st.cal.active = None;
    }
    let end = st.cal.end.unwrap_or(today).min(today);
    let start = dates::first_monday(end, WEEKS);

    let day_at = |pos: Pos2| -> Option<i64> {
        if !grid.contains(pos) {
            return None;
        }
        let c = ((pos.x - grid.left()) / (cell + GAP)).floor() as i64;
        let r = ((pos.y - grid.top()) / (cell + GAP)).floor() as i64;
        ((0..WEEKS).contains(&c) && (0..7).contains(&r))
            .then_some(start + c * 7 + r)
            .filter(|d| *d <= today)
    };
    let cell_rect = |day: i64| -> Rect {
        let i = day - start;
        Rect::from_min_size(
            Pos2::new(
                col_x(grid.left(), i.div_euclid(7)),
                grid.top() + i.rem_euclid(7) as f32 * (cell + GAP),
            ),
            Vec2::splat(cell),
        )
    };

    // The pointer: the day under it is the picked one; leaving clears it.
    match resp.hover_pos().and_then(day_at) {
        Some(d) => {
            st.cal.active = Some(d);
            st.cal.hovering = true;
        }
        None if st.cal.hovering => {
            st.cal.hovering = false;
            st.cal.active = None;
        }
        None => {}
    }
    let mut picked = None;
    if resp.clicked() {
        picked = resp
            .interact_pointer_pos()
            .and_then(day_at)
            .or(st.cal.active);
    }
    if resp.hover_pos().and_then(day_at).is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let active = st.cal.active.filter(|d| (start..=end).contains(d));

    // The months over the columns where they start.
    let mono3 = font(Face::Mono, size::TEXT_3XS);
    for c in 0..WEEKS {
        if let Some(d) = dates::date_of(start + c * 7)
            && chrono::Datelike::day(&d) <= 7
        {
            let g = ph(ui, dates::month_short_in(loc, d), mono3.clone());
            draw(
                ui.painter(),
                &g,
                col_x(mr.left(), c),
                mr.top() + 5.0,
                tk.fg_faint,
            );
        }
    }

    // The days.
    if ui.is_rect_visible(grid) {
        let p = ui.painter();
        let radius = CornerRadius::same(2);
        for i in 0..WEEKS * 7 {
            let day = start + i;
            let r = cell_rect(day);
            if day > today {
                // Not yet: an outline only, so it never reads as a day without play.
                p.rect_stroke(
                    r.shrink(0.5),
                    radius,
                    Stroke::new(1.0, alpha(tk.border, 0.6)),
                    StrokeKind::Inside,
                );
                continue;
            }
            p.rect_filled(r, radius, tk.raised);
            let secs = l.derived.by_day.get(&day).map_or(0, |x| x.0);
            let o = OPACITY.get(l.derived.level(secs)).copied().unwrap_or(0.0);
            if o > 0.0 {
                p.rect_filled(r, radius, alpha(tk.accent, o));
            }
            if day == today {
                p.rect_stroke(r, radius, Stroke::new(1.0, tk.fg), StrokeKind::Outside);
            }
        }
        if let Some(a) = active {
            p.rect_stroke(
                cell_rect(a),
                radius,
                Stroke::new(2.0, tk.fg),
                StrokeKind::Outside,
            );
        }
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(p, grid.expand(2.0), size::RADIUS_XS, &tk);
        }
    }
    if let Some(a) = active {
        tip(ui, l, id, cell_rect(a), a);
    }
    let hint = t("stats", "activityHint");
    let keys = t("common", "chartKeys");
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, format!("{hint} {keys}"))
    });

    // The hint and the legend.
    let f3 = font(Face::Sans, size::TEXT_3XS);
    let less = ph(ui, t("stats", "legendLess"), f3.clone());
    let more = ph(ui, t("stats", "legendMore"), f3.clone());
    let legend_w = less.size().x + more.size().x + 8.0 + 5.0 * 10.0 + 4.0 * 4.0;
    let hint_g = ph_trunc(ui, hint, f3, (w - legend_w - 12.0).max(0.0));
    let (lr, _) = ui.allocate_exact_size(Vec2::new(w, hint_g.size().y.max(12.0)), Sense::hover());
    let p = ui.painter();
    let cy = lr.center().y;
    draw(p, &hint_g, lr.left(), cy, tk.fg_faint);
    let mut x = lr.right() - legend_w;
    draw(p, &less, x, cy, tk.fg_faint);
    x += less.size().x + 4.0;
    for o in OPACITY {
        let r = Rect::from_min_size(Pos2::new(x, cy - 5.0), Vec2::splat(10.0));
        p.rect_filled(r, CornerRadius::same(2), tk.raised);
        if o > 0.0 {
            p.rect_filled(r, CornerRadius::same(2), alpha(tk.accent, o));
        }
        x += 14.0;
    }
    draw(p, &more, x, cy, tk.fg_faint);
    picked
}

/// The picked day's tooltip: its date, the time played, its sessions.
fn tip(ui: &Ui, l: &Loaded, id: egui::Id, anchor: Rect, day: i64) {
    let tk = ui.tokens();
    let Some(date) = dates::date_of(day) else {
        return;
    };
    let (secs, sessions) = l.derived.by_day.get(&day).copied().unwrap_or((0, 0));
    let fmt = |f: egui::FontId, c| TextFormat::simple(f, c);
    let mut job = LayoutJob::default();
    job.append(
        &dates::full_date_in(i18n::locale(), date),
        0.0,
        fmt(font(Face::SansSemibold, size::TEXT_2XS), tk.fg),
    );
    job.append("\n", 0.0, fmt(font(Face::Sans, size::TEXT_2XS), tk.fg));
    job.append(
        &dur_or_dash(secs),
        0.0,
        fmt(font(Face::Mono, size::TEXT_2XS), tk.accent),
    );
    if sessions > 0 {
        job.append(
            &sessions_text(sessions),
            8.0,
            fmt(font(Face::Sans, size::TEXT_2XS), tk.fg_muted),
        );
    }
    chart_tip_job(ui.ctx(), id.with("tip"), anchor, job);
}
