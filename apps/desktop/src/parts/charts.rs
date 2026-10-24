//! Charts, painted with the egui painter: `PlayerChart` (players over time, an area under
//! a line, with the minimum, average and peak and the value under the pointer),
//! `WeekHeat` (a week of hours as a heat map) and `ChartTip` (their tooltip, over the mark,
//! never taking the pointer).

use egui::epaint::{Mesh, PathShape, PathStroke};
use egui::{
    Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget,
};

use super::text::{draw, ph};
use crate::i18n::{self, Locale, t, tf};
use crate::kit::paint::{self, alpha};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// `ChartTip`: the tooltip of a chart, over `anchor` (the mark's box on screen), below it
/// when there is no room above; the same plate as the app's tooltips. Never takes the
/// pointer, so moving across marks does not flicker. Lines are `\n`-separated.
pub fn chart_tip(ctx: &egui::Context, id: Id, anchor: Rect, text: &str) {
    if text.is_empty() || !anchor.is_finite() {
        return;
    }
    let tk = ctx.tokens();
    let job = egui::text::LayoutJob::simple(
        text.to_owned(),
        font(Face::Sans, size::TEXT_2XS),
        tk.fg,
        320.0 - 16.0,
    );
    chart_tip_job(ctx, id, anchor, job);
}

/// [`chart_tip`] with its own text runs (faces and colours per run, as the Svelte tips mix
/// a bold title, an accent figure and muted words). Wrapped at the plate's width.
pub fn chart_tip_job(ctx: &egui::Context, id: Id, anchor: Rect, mut job: egui::text::LayoutJob) {
    if job.text.is_empty() || !anchor.is_finite() {
        return;
    }
    let tk = ctx.tokens();
    job.wrap.max_width = 320.0 - 16.0;
    let galley = ctx.fonts_mut(|f| f.layout_job(job));
    let size = galley.size() + Vec2::new(16.0, 12.0);
    let screen = ctx.content_rect();
    let gap = 6.0;
    let above = anchor.top() - size.y - gap;
    let top = if above >= screen.top() + 4.0 {
        above
    } else {
        anchor.bottom() + gap
    };
    let left = (anchor.center().x - size.x / 2.0)
        .max(screen.left() + 4.0)
        .min(screen.right() - size.x - 4.0);
    egui::Area::new(id)
        .order(egui::Order::Tooltip)
        .fixed_pos(Pos2::new(left, top))
        .interactable(false)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(size, Sense::hover());
            let p = ui.painter();
            p.add(paint::shadow_pop().as_shape(r, CornerRadius::same(size::RADIUS_MD)));
            paint::plate(p, r, size::RADIUS_MD, tk.overlay, tk.border);
            p.galley(r.min + Vec2::new(8.0, 6.0), galley, tk.fg);
        });
}

// ---------------------------------------------------------------------------------------

/// `PlayerChart`: player counts over time (a day unless `since` says otherwise) as an area
/// under a line in a 64px well, the average dashed, the capacity in red, and the value
/// under the pointer read out. Points are `(unix seconds, players)`, oldest first.
#[must_use = "add it with ui.add(…)"]
pub struct PlayerChart<'a> {
    points: &'a [(i64, f64)],
    capacity: Option<f64>,
    label: Option<&'a str>,
    since: Option<&'a str>,
    plot_h: f32,
}

impl<'a> PlayerChart<'a> {
    pub fn new(points: &'a [(i64, f64)]) -> Self {
        Self {
            points,
            capacity: None,
            label: None,
            since: None,
            plot_h: 64.0,
        }
    }

    /// The plot's height (64px by default; the server panel's Stats tab draws it larger).
    pub fn height(mut self, h: f32) -> Self {
        self.plot_h = h.clamp(32.0, 480.0);
        self
    }

    /// The server's capacity: a red line, and the top of the scale.
    pub fn max(mut self, m: Option<f64>) -> Self {
        self.capacity = m.filter(|v| v.is_finite() && *v > 0.0);
        self
    }

    /// The chart's name; "Player count (24 h)" by default.
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = Some(l);
        self
    }

    /// Under the left edge; "24h ago" by default.
    pub fn since(mut self, s: &'a str) -> Self {
        self.since = Some(s);
        self
    }
}

/// Min, average (rounded), peak and the last value.
fn chart_stats(points: &[(i64, f64)]) -> (f64, f64, f64, f64) {
    let (mut min, mut peak, mut sum) = (f64::INFINITY, f64::NEG_INFINITY, 0.0);
    for (_, v) in points {
        min = min.min(*v);
        peak = peak.max(*v);
        sum += v;
    }
    if points.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let avg = (sum / points.len() as f64).round();
    (min, avg, peak, points.last().map_or(0.0, |p| p.1))
}

/// "17:40", or with the day past a day and a half of span.
fn when(ts: i64, long: bool) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(ts, 0).single() {
        Some(d) if long => {
            let wd = weekday_short(
                i18n::locale(),
                d.format("%u").to_string().parse().unwrap_or(1) - 1,
            );
            format!("{wd} {} {}", d.format("%-d"), d.format("%H:%M"))
        }
        Some(d) => d.format("%H:%M").to_string(),
        None => String::new(),
    }
}

fn num(v: f64) -> String {
    crate::i18n::format::num(v)
}

impl Widget for PlayerChart<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let w = ui.available_width();
        let sorted_copy;
        let pts: &[(i64, f64)] = if self.points.windows(2).all(|p| p[0].0 <= p[1].0) {
            self.points
        } else {
            let mut v = self.points.to_vec();
            v.sort_by_key(|p| p.0);
            sorted_copy = v;
            &sorted_copy
        };
        let (min, avg, peak, now) = chart_stats(pts);
        let top = 1.0_f64.max(self.capacity.unwrap_or(0.0)).max(peak);
        let t0 = pts.first().map_or(0, |p| p.0);
        let t1 = pts.last().map_or(1, |p| p.0);
        let span = (t1 - t0).max(1) as f64;
        let long = t1 - t0 > 36 * 3600;
        let line_h = 12.0;
        let ph_ = self.plot_h;
        let h = line_h + 4.0 + ph_ + 4.0 + line_h;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let plot = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + line_h + 4.0),
            Vec2::new(w, ph_),
        );
        let x = |ts: i64| plot.left() + ((ts - t0) as f64 / span) as f32 * plot.width();
        let y = |v: f64| plot.bottom() - (v / top) as f32 * (ph_ - 4.0) - 2.0;
        let p = ui.painter();
        p.rect_filled(plot, CornerRadius::same(size::RADIUS_XS), tk.plot);
        let pp = p.with_clip_rect(plot.intersect(p.clip_rect()));

        // Average, dashed; capacity.
        let ya = y(avg);
        let mut dx = plot.left();
        while dx < plot.right() {
            pp.hline(
                egui::Rangef::new(dx, (dx + 3.0).min(plot.right())),
                ya,
                Stroke::new(1.0, tk.border_strong),
            );
            dx += 6.0;
        }
        if let Some(c) = self.capacity {
            pp.hline(plot.x_range(), y(c), Stroke::new(1.0, alpha(tk.err, 0.4)));
        }
        // Area and line.
        if !pts.is_empty() {
            let line: Vec<Pos2> = pts.iter().map(|(ts, v)| Pos2::new(x(*ts), y(*v))).collect();
            let fill = alpha(tk.accent, 0.14);
            let mut mesh = Mesh::default();
            for seg in line.windows(2) {
                let (a, b) = (seg[0], seg[1]);
                let i = mesh.vertices.len() as u32;
                mesh.colored_vertex(a, fill);
                mesh.colored_vertex(b, fill);
                mesh.colored_vertex(Pos2::new(b.x, plot.bottom()), fill);
                mesh.colored_vertex(Pos2::new(a.x, plot.bottom()), fill);
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
            }
            pp.add(mesh);
            pp.add(PathShape::line(line, PathStroke::new(1.5, tk.accent)));
        }
        // The point under the pointer.
        let hover_id = resp.id.with("hover");
        let hovered = resp
            .hover_pos()
            .filter(|pos| plot.contains(*pos) && !pts.is_empty())
            .map(|pos| {
                let tt = t0 as f64 + ((pos.x - plot.left()) / plot.width()) as f64 * span;
                let mut best = 0;
                for (i, (ts, _)) in pts.iter().enumerate() {
                    if (*ts as f64 - tt).abs()
                        < (pts.get(best).map_or(0, |p| p.0) as f64 - tt).abs()
                    {
                        best = i;
                    }
                }
                best
            });
        let _ = hover_id;
        let faint = tk.fg_faint;
        let figs = font(Face::Sans, size::TEXT_3XS);
        let ty = rect.top() + line_h / 2.0;
        let label = self.label.unwrap_or_else(|| t("detail", "bmPlayerHistory"));
        draw(p, &ph(ui, label, figs.clone()), rect.left(), ty, faint);
        if let Some(i) = hovered
            && let Some((ts, v)) = pts.get(i)
        {
            let hx = x(*ts);
            pp.vline(hx, plot.y_range(), Stroke::new(1.0, tk.fg_faint));
            pp.circle_filled(Pos2::new(hx, y(*v)), 2.5, tk.accent);
            let s = format!("{} · {}", num(*v), when(*ts, long));
            let g = ph(ui, &s, figs.clone());
            draw(p, &g, rect.right() - g.size().x, ty, tk.fg);
        } else {
            // "Min 3 · avg: 21 · Peak 58", the figures brighter.
            let parts = [
                (format!("{} ", t("detail", "bmMin")), faint),
                (num(min), tk.fg_muted),
                (
                    format!(
                        " · {} · {} ",
                        tf("detail", "bmAvg", &[("count", &num(avg))]),
                        t("detail", "bmPeak")
                    ),
                    faint,
                ),
                (num(peak), tk.fg_muted),
            ];
            let gs: Vec<_> = parts
                .iter()
                .map(|(s, c)| (ph(ui, s, figs.clone()), *c))
                .collect();
            let total: f32 = gs.iter().map(|(g, _)| g.size().x).sum();
            let mut gx = rect.right() - total;
            for (g, c) in &gs {
                draw(p, g, gx, ty, *c);
                gx += g.size().x;
            }
        }
        let by = rect.bottom() - line_h / 2.0;
        let since = self.since.unwrap_or_else(|| t("detail", "bm24hAgo"));
        draw(p, &ph(ui, since, figs.clone()), rect.left(), by, faint);
        let ng = ph(ui, &num(now), figs.clone());
        let lg = ph(ui, &format!("{} · ", t("detail", "bmNow")), figs);
        draw(p, &ng, rect.right() - ng.size().x, by, tk.fg_muted);
        draw(p, &lg, rect.right() - ng.size().x - lg.size().x, by, faint);
        resp
    }
}

// ---------------------------------------------------------------------------------------

/// A weekday's short name (Monday is 0) as `Intl.DateTimeFormat(locale, {weekday:
/// "short"})` writes it.
pub fn weekday_short(l: Locale, day: u32) -> &'static str {
    const EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const FR: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
    const DE: [&str; 7] = ["Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa.", "So."];
    const ES: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
    const RU: [&str; 7] = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];
    let names = match l {
        Locale::En => &EN,
        Locale::Fr => &FR,
        Locale::De => &DE,
        Locale::Es => &ES,
        Locale::Ru => &RU,
    };
    names.get(day as usize).copied().unwrap_or("")
}

/// `WeekHeat`: a week of hours, Monday first, each cell as bright as its value against the
/// week's highest; the hour it is now is ringed. `values[day][hour]` is in the player's own
/// time. The pointer or the keyboard (focus the grid, then the arrows) picks a cell: it is
/// outlined, its day and hour light up on the axes, and its tooltip (`cell(day, hour,
/// value)`) shows. `hint` says what the grid shows.
pub struct WeekHeat<'a, F: Fn(&str, u32, f64) -> String> {
    values: &'a [[f64; 24]; 7],
    cell: F,
    hint: &'a str,
    now: Option<(u32, u32)>,
}

impl<'a, F: Fn(&str, u32, f64) -> String> WeekHeat<'a, F> {
    pub fn new(values: &'a [[f64; 24]; 7], cell: F, hint: &'a str) -> Self {
        Self {
            values,
            cell,
            hint,
            now: None,
        }
    }

    /// The ringed cell (day, hour); the local clock's by default.
    pub fn now(mut self, day: u32, hour: u32) -> Self {
        self.now = Some((day.min(6), hour.min(23)));
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        use chrono::{Datelike, Timelike};
        let tk = ui.tokens();
        let loc = i18n::locale();
        let (now_day, now_hour) = self.now.unwrap_or_else(|| {
            let n = chrono::Local::now();
            (n.weekday().num_days_from_monday(), n.hour())
        });
        let top_v = self
            .values
            .iter()
            .flatten()
            .fold(1.0_f64, |a, v| if v.is_finite() { a.max(*v) } else { a });
        let figs = font(Face::Sans, size::TEXT_3XS);
        let figs_b = font(Face::SansSemibold, size::TEXT_3XS);
        let label_w = (0..7)
            .map(|d| ph(ui, weekday_short(loc, d), figs_b.clone()).size().x)
            .fold(0.0_f32, f32::max)
            + 6.0;
        let w = ui.available_width();
        let cell_w = ((w - label_w - 24.0) / 24.0).max(2.0);
        let (cell_h, gap, axis_h) = (12.0, 1.0, 10.0);
        let grid_h = 7.0 * (cell_h + gap) + axis_h;
        let hint_g = super::text::ph_trunc(ui, self.hint, font(Face::Sans, size::TEXT_3XS), w);
        let h = grid_h + 4.0 + hint_g.size().y;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        let id = resp.id;
        let grid = Rect::from_min_size(rect.min, Vec2::new(w, grid_h));
        let cells_x0 = rect.left() + label_w;
        let cell_rect = |d: u32, hr: u32| {
            Rect::from_min_size(
                Pos2::new(
                    cells_x0 + hr as f32 * (cell_w + gap),
                    rect.top() + d as f32 * (cell_h + gap),
                ),
                Vec2::new(cell_w, cell_h),
            )
        };

        // Which cell is picked: under the pointer, else the keyboard's.
        let key_id = id.with("key-cell");
        let mut keyed: Option<(u32, u32)> = ui.data(|d| d.get_temp(key_id));
        if resp.gained_focus() && keyed.is_none() {
            keyed = Some((now_day, now_hour));
        }
        if resp.has_focus() {
            let a = keyed.unwrap_or((now_day, now_hour));
            let mv = ui.input(|i| {
                let k = |key| i.key_pressed(key);
                if k(egui::Key::ArrowUp) {
                    Some((-1, 0))
                } else if k(egui::Key::ArrowDown) {
                    Some((1, 0))
                } else if k(egui::Key::ArrowLeft) {
                    Some((0, -1))
                } else if k(egui::Key::ArrowRight) {
                    Some((0, 1))
                } else if k(egui::Key::Home) {
                    Some((0, -24))
                } else if k(egui::Key::End) {
                    Some((0, 24))
                } else {
                    None
                }
            });
            if let Some((dd, dh)) = mv {
                keyed = Some((
                    (a.0 as i32 + dd).clamp(0, 6) as u32,
                    (a.1 as i32 + dh).clamp(0, 23) as u32,
                ));
            }
            // The arrows stay on the grid.
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
            keyed = None;
        }
        ui.data_mut(|d| match keyed {
            Some(k) => {
                d.insert_temp(key_id, k);
            }
            None => d.remove::<(u32, u32)>(key_id),
        });
        let pointed = resp.hover_pos().and_then(|pos| {
            if pos.x < cells_x0 || pos.y >= rect.top() + 7.0 * (cell_h + gap) {
                return None;
            }
            let hr = ((pos.x - cells_x0) / (cell_w + gap)).floor();
            let d = ((pos.y - rect.top()) / (cell_h + gap)).floor();
            ((0.0..24.0).contains(&hr) && (0.0..7.0).contains(&d)).then_some((d as u32, hr as u32))
        });
        let active = pointed.or(keyed);

        let p = ui.painter();
        for d in 0..7u32 {
            let on_day = active.is_some_and(|a| a.0 == d);
            let g = ph(
                ui,
                weekday_short(loc, d),
                if on_day { figs_b.clone() } else { figs.clone() },
            );
            let r0 = cell_rect(d, 0);
            draw(
                p,
                &g,
                rect.left(),
                r0.center().y,
                if on_day { tk.fg } else { tk.fg_faint },
            );
            for hr in 0..24u32 {
                let v = self
                    .values
                    .get(d as usize)
                    .and_then(|row| row.get(hr as usize))
                    .copied()
                    .unwrap_or(0.0);
                let r = cell_rect(d, hr);
                let dim = active.is_some_and(|a| a != (d, hr) && (a.0 == d || a.1 == hr));
                let base = if dim {
                    paint::bright(tk.raised, 0.95)
                } else {
                    tk.raised
                };
                p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), base);
                if v > 0.0 && v.is_finite() {
                    let a = (0.12 + 0.88 * (v / top_v)) as f32;
                    let c = if dim {
                        paint::bright(tk.accent, 0.95)
                    } else {
                        tk.accent
                    };
                    p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), alpha(c, a));
                }
                if (d, hr) == (now_day, now_hour) {
                    p.rect_stroke(
                        r,
                        CornerRadius::same(size::RADIUS_XS),
                        Stroke::new(1.0, tk.fg),
                        StrokeKind::Outside,
                    );
                }
            }
        }
        if let Some((d, hr)) = active {
            p.rect_stroke(
                cell_rect(d, hr),
                CornerRadius::same(size::RADIUS_XS),
                Stroke::new(2.0, tk.fg),
                StrokeKind::Outside,
            );
        }
        // The hours under: every sixth, and the picked one.
        let ay = rect.top() + 7.0 * (cell_h + gap) + axis_h / 2.0;
        for hr in 0..24u32 {
            let on = active.is_some_and(|a| a.1 == hr);
            if hr % 6 == 0 || on {
                let g = ph(
                    ui,
                    &hr.to_string(),
                    if on { figs_b.clone() } else { figs.clone() },
                );
                draw(
                    p,
                    &g,
                    cell_rect(0, hr).left(),
                    ay,
                    if on { tk.fg } else { tk.fg_faint },
                );
            }
        }
        draw(
            p,
            &hint_g,
            rect.left(),
            grid.bottom() + 4.0 + hint_g.size().y / 2.0,
            tk.fg_faint,
        );
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(p, grid, size::RADIUS_XS, &tk);
        }
        if let Some((d, hr)) = active {
            let v = self
                .values
                .get(d as usize)
                .and_then(|row| row.get(hr as usize))
                .copied()
                .unwrap_or(0.0);
            let text = (self.cell)(weekday_short(loc, d), hr, v);
            chart_tip(ui.ctx(), id.with("tip"), cell_rect(d, hr), &text);
        }
        let hint = self.hint;
        let chart_keys = t("common", "chartKeys");
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Unknown, true, format!("{hint} {chart_keys}"))
        });
        let _ = Color32::TRANSPARENT;
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_of_a_day() {
        let pts = [(0, 3.0), (10, 58.0), (20, 2.0), (30, 21.0)];
        assert_eq!(chart_stats(&pts), (2.0, 21.0, 58.0, 21.0));
        assert_eq!(chart_stats(&[]), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn weekdays() {
        assert_eq!(weekday_short(Locale::En, 0), "Mon");
        assert_eq!(weekday_short(Locale::Fr, 6), "dim.");
        assert_eq!(weekday_short(Locale::Ru, 9), "");
    }
}
