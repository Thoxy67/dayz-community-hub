//! Where you play (the servers and missions, ranked) and the maps: rows with a bar against
//! the first, a tooltip under the pointer or the focus, a click (or Enter) narrows the
//! history to the place or the map.

use dz_hub::features::stats::{MapStatDto, PlaceStatDto, PlayKind};
use egui::text::{LayoutJob, TextFormat};
use egui::{CornerRadius, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use serde_json::{Value, json};

use super::{Loaded, PLACES_SHOWN, Run, StatsState, dur_or_dash, name_of, sessions_text};
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Tag, TipExt, Tone};
use crate::parts::{JoinButton, Section, chart_tip_job, draw, map_name, ph, ph_trunc};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// A place's row: `py-1.5` around a name line and a detail line.
const PLACE_H: f32 = 44.5;
/// A map's row: `py-0.5` around one line.
const MAP_H: f32 = 22.0;

/// "Where you play".
pub fn places(ui: &mut Ui, l: &Loaded, st: &mut StatsState, run: &mut Run) {
    Section::new(t("stats", "sectionPlaces"))
        .icon(lucide::MAP_PIN)
        .show(
            ui,
            |_| {},
            |ui| {
                let all = &l.data.places_played;
                if all.is_empty() {
                    nothing(ui);
                    return;
                }
                let shown = if st.all_places {
                    all.len()
                } else {
                    all.len().min(PLACES_SHOWN)
                };
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, p) in all.iter().take(shown).enumerate() {
                    place_row(ui, l, i, p, i + 1 == shown, run);
                }
                if all.len() > PLACES_SHOWN {
                    ui.add_space(4.0);
                    let label = if st.all_places {
                        t("stats", "showFewerPlaces").to_owned()
                    } else {
                        tf("stats", "showAllPlaces", &[("count", &all.len())])
                    };
                    if link(ui, &label).clicked() {
                        run.push(("stats.toggle_all_places", Value::Null));
                    }
                }
            },
        );
}

/// "Nothing played in this period."
fn nothing(ui: &mut Ui) {
    let tk = ui.tokens();
    let g = ph_trunc(
        ui,
        t("stats", "nothingInRange"),
        font(Face::Sans, size::TEXT_2XS),
        ui.available_width(),
    );
    let (r, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), g.size().y), Sense::hover());
    draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
}

/// A text button in the accent (`text-2xs font-medium text-accent hover:bg-accent/10`).
fn link(ui: &mut Ui, label: &str) -> Response {
    let tk = ui.tokens();
    let g = ph(ui, label, font(Face::SansMedium, size::TEXT_2XS));
    let (r, resp) = ui.allocate_exact_size(g.size() + Vec2::new(8.0, 2.0), Sense::click());
    let h = paint::ease(ui, resp.id.with("h"), resp.hovered());
    let p = ui.painter();
    if h > 0.0 {
        p.rect_filled(
            r,
            CornerRadius::same(size::RADIUS_XS),
            alpha(tk.accent, 0.1 * h),
        );
    }
    draw(p, &g, r.left() + 4.0, r.center().y, tk.accent);
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, r, size::RADIUS_XS, &tk);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A thin bar from the start, rounded at its end (`h-1.5 rounded-full`), against `top`.
fn bar(ui: &Ui, x0: f32, x1: f32, cy: f32, value: i64, top: i64) {
    let tk = ui.tokens();
    let track = Rect::from_min_max(Pos2::new(x0, cy - 3.0), Pos2::new(x1.max(x0), cy + 3.0));
    let p = ui.painter();
    p.rect_filled(track, CornerRadius::same(3), tk.raised);
    let share = (value as f32 / top.max(1) as f32).clamp(0.02, 1.0);
    let fill = Rect::from_min_size(track.min, Vec2::new(track.width() * share, 6.0));
    p.rect_filled(fill, CornerRadius::same(3), tk.accent);
}

/// The share of the period's time: "42 %", or a dash.
fn share(l: &Loaded, secs: i64) -> String {
    let total = l.data.total_secs;
    if total > 0 {
        format!(
            "{} %",
            ((secs as f64 / total as f64) * 100.0).round() as i64
        )
    } else {
        format::NONE.to_owned()
    }
}

/// A row's look while pointed at or focused: `bg-raised/50`, the ring for the keyboard.
fn row_ground(ui: &Ui, rect: Rect, resp: &Response) -> bool {
    let tk = ui.tokens();
    let on = resp.contains_pointer() || resp.has_focus();
    let p = ui.painter();
    if on {
        p.rect_filled(
            rect,
            CornerRadius::same(size::RADIUS_XS),
            alpha(tk.raised, 0.5),
        );
    }
    if paint::keyboard_focus(ui, resp) {
        p.rect_stroke(
            rect,
            CornerRadius::same(size::RADIUS_XS),
            Stroke::new(2.0, alpha(tk.accent, 0.6)),
            egui::StrokeKind::Inside,
        );
    }
    on
}

fn place_row(ui: &mut Ui, l: &Loaded, i: usize, p: &PlaceStatDto, last: bool, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, PLACE_H), Sense::click());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let on = row_ground(ui, rect, &resp);
    if !last {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.4)),
        );
    }
    // `grid-cols-[1.5rem_minmax(0,1fr)_minmax(6rem,14rem)_5rem_2rem] gap-x-3 px-1`
    let inner = rect.shrink2(Vec2::new(4.0, 0.0));
    let free = (inner.width() - 24.0 - 80.0 - 32.0 - 48.0).max(0.0);
    // The bar gives way before the name on a narrow window: at most 45 % of the room
    // (the full 14rem on a wide one).
    let bar_w = (free * 0.45).clamp(64.0, 224.0).min(free);
    let name_w = (free - bar_w).max(0.0);
    let x_name = inner.left() + 36.0;
    let x_bar = x_name + name_w + 12.0;
    let x_dur = x_bar + bar_w + 12.0;
    let x_act = x_dur + 80.0 + 12.0;
    let cy = rect.center().y;
    let painter = ui.painter().clone();

    // Rank.
    let rank = ph(ui, &(i + 1).to_string(), font(Face::Sans, size::TEXT_2XS));
    draw(
        &painter,
        &rank,
        inner.left() + 24.0 - rank.size().x,
        cy,
        tk.fg_faint,
    );

    // Name (+ Offline), then map · sessions · last played.
    let shown = name_of(p.kind, &p.name);
    let offline = p.kind == PlayKind::Offline;
    let tag_w = if offline { 64.0 } else { 0.0 };
    let name_g = ph_trunc(
        ui,
        &shown,
        font(Face::SansSemibold, size::TEXT_XS),
        (name_w - tag_w).max(0.0),
    );
    let line1 = cy - 8.5;
    let line2 = cy + 9.0;
    draw(&painter, &name_g, x_name, line1, tk.fg);
    let name_rect = Rect::from_center_size(
        Pos2::new(x_name + name_g.size().x / 2.0, line1),
        name_g.size(),
    );
    if name_rect.width() > 0.0 {
        ui.interact(name_rect, resp.id.with("name"), Sense::hover())
            .tip(&p.name);
    }
    if offline {
        let tr = Rect::from_min_size(
            Pos2::new(name_rect.right() + 6.0, line1 - 9.0),
            Vec2::new(tag_w, 18.0),
        );
        let mut tu = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(tr)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        tu.add(Tag::new(t("stats", "offlineTag")).tone(Tone::Accent));
    }
    let f3 = font(Face::Sans, size::TEXT_3XS);
    let sess = ph(ui, &sessions_text(p.sessions), f3.clone());
    let when = ph(ui, &format!("· {}", format::relative(p.last)), f3.clone());
    let fixed = sess.size().x + 8.0 + when.size().x;
    let mut x = x_name;
    if let Some(m) = p.map.as_deref().filter(|m| !m.is_empty()) {
        let mg = ph_trunc(ui, &map_name(m), f3, (name_w - fixed - 8.0).max(0.0));
        draw(&painter, &mg, x, line2, tk.map);
        x += mg.size().x + 8.0;
    }
    draw(&painter, &sess, x, line2, tk.fg_faint);
    draw(&painter, &when, x + sess.size().x + 8.0, line2, tk.fg_faint);

    // Bar, time.
    bar(ui, x_bar, x_bar + bar_w, cy, p.secs, l.derived.top_place);
    let dur = ph(ui, &dur_or_dash(p.secs), font(Face::Sans, size::TEXT_2XS));
    let dur_rect =
        Rect::from_center_size(Pos2::new(x_dur + 80.0 - dur.size().x / 2.0, cy), dur.size());
    draw(&painter, &dur, dur_rect.left(), cy, tk.fg);
    if p.unmeasured > 0 {
        ui.interact(dur_rect, resp.id.with("dur"), Sense::hover())
            .tip(t("stats", "unmeasuredHint"));
    }

    // Join, or launch the mission.
    let act = Rect::from_center_size(
        Pos2::new(x_act + 32.0 - size::CONTROL / 2.0, cy),
        Vec2::splat(size::CONTROL),
    );
    place_action(
        ui,
        act,
        p.kind,
        &p.name,
        p.ip.as_deref(),
        p.port,
        on,
        true,
        run,
    );

    if resp.clicked() {
        run.push(("stats.filter_text", json!({ "text": p.name })));
    }
    if on {
        resp.ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        place_tip(ui, l, resp.id, rect, p);
    }
}

/// The Join button of a server, the Play button of an offline mission (whose click in a
/// places row, as in the Svelte view, also narrows the history to it: `narrow`).
#[allow(clippy::too_many_arguments)]
pub fn place_action(
    ui: &mut Ui,
    rect: Rect,
    kind: PlayKind,
    name: &str,
    ip: Option<&str>,
    port: Option<u16>,
    row_hover: bool,
    narrow: bool,
    run: &mut Run,
) {
    match (kind, ip, port) {
        (PlayKind::Server, Some(ip), Some(port)) if !ip.is_empty() && port > 0 => {
            if put(
                ui,
                rect,
                JoinButton::new().compact().quiet().group_hover(row_hover),
            )
            .clicked()
            {
                run.push(("connect.address", json!({ "ip": ip, "port": port })));
            }
        }
        (PlayKind::Offline, ..) => {
            let map = name_of(kind, name);
            if put(
                ui,
                rect,
                JoinButton::new()
                    .compact()
                    .quiet()
                    .group_hover(row_hover)
                    .tip(map),
            )
            .clicked()
            {
                run.push(("offline.launch", json!({ "mission": name })));
                if narrow {
                    run.push(("stats.filter_text", json!({ "text": name })));
                }
            }
        }
        _ => {}
    }
}

/// A widget at `rect`, without moving the layout's cursor (`Ui::put` would).
pub fn put(ui: &mut Ui, rect: Rect, w: impl egui::Widget) -> Response {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add(w)
}

fn tf_fmt(f: egui::FontId, c: egui::Color32) -> TextFormat {
    TextFormat::simple(f, c)
}

fn place_tip(ui: &Ui, l: &Loaded, id: egui::Id, anchor: Rect, p: &PlaceStatDto) {
    let tk = ui.tokens();
    let sans = font(Face::Sans, size::TEXT_2XS);
    let mut job = LayoutJob::default();
    job.append(
        &name_of(p.kind, &p.name),
        0.0,
        tf_fmt(font(Face::SansSemibold, size::TEXT_2XS), tk.fg),
    );
    job.append("\n", 0.0, tf_fmt(sans.clone(), tk.fg));
    job.append(
        &dur_or_dash(p.secs),
        0.0,
        tf_fmt(font(Face::Sans, size::TEXT_2XS), tk.accent),
    );
    job.append(
        &tf("stats", "tipShare", &[("share", &share(l, p.secs))]),
        8.0,
        tf_fmt(sans.clone(), tk.fg_muted),
    );
    let mut line = sessions_text(p.sessions);
    let measured = p.sessions.saturating_sub(p.unmeasured);
    if p.secs > 0 && measured > 0 {
        let each = format::duration(p.secs / i64::from(measured));
        line.push_str(" · ");
        line.push_str(&tf("stats", "tipPerSession", &[("duration", &each)]));
    }
    job.append(&format!("\n{line}"), 0.0, tf_fmt(sans.clone(), tk.fg_muted));
    job.append(
        &format!(
            "\n{}",
            tf("stats", "tipSince", &[("date", &format::date(p.first))])
        ),
        0.0,
        tf_fmt(sans, tk.fg_faint),
    );
    chart_tip_job(ui.ctx(), id.with("tip"), anchor, job);
}

/// "Maps".
pub fn maps(ui: &mut Ui, l: &Loaded, run: &mut Run) {
    Section::new(t("stats", "sectionMaps"))
        .icon(lucide::MAP)
        .show(
            ui,
            |_| {},
            |ui| {
                if l.data.maps.is_empty() {
                    nothing(ui);
                    return;
                }
                ui.spacing_mut().item_spacing.y = 6.0;
                for m in &l.data.maps {
                    map_row(ui, l, m, run);
                }
            },
        );
}

fn map_row(ui: &mut Ui, l: &Loaded, m: &MapStatDto, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, MAP_H), Sense::click());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let on = row_ground(ui, rect, &resp);
    // `grid-cols-[7rem_minmax(0,1fr)_4.5rem] gap-2 px-1`
    let inner = rect.shrink2(Vec2::new(4.0, 0.0));
    let cy = rect.center().y;
    let name = map_name(&m.map);
    let g = ph_trunc(ui, &name, font(Face::Sans, size::TEXT_XS), 112.0);
    let painter = ui.painter().clone();
    draw(&painter, &g, inner.left(), cy, tk.map);
    let x_bar = inner.left() + 112.0 + 8.0;
    let x_dur = (inner.right() - 72.0).max(x_bar);
    bar(ui, x_bar, x_dur - 8.0, cy, m.secs, l.derived.top_map);
    let dur = ph(ui, &dur_or_dash(m.secs), font(Face::Sans, size::TEXT_2XS));
    draw(&painter, &dur, inner.right() - dur.size().x, cy, tk.fg);
    if resp.clicked() {
        run.push(("stats.filter_text", json!({ "text": m.map })));
    }
    if on {
        resp.ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        let sans = font(Face::Sans, size::TEXT_2XS);
        let mut job = LayoutJob::default();
        job.append(
            &name,
            0.0,
            tf_fmt(font(Face::SansSemibold, size::TEXT_2XS), tk.map),
        );
        job.append("\n", 0.0, tf_fmt(sans.clone(), tk.fg));
        job.append(
            &dur_or_dash(m.secs),
            0.0,
            tf_fmt(font(Face::Sans, size::TEXT_2XS), tk.accent),
        );
        job.append(
            &tf("stats", "tipShare", &[("share", &share(l, m.secs))]),
            8.0,
            tf_fmt(sans.clone(), tk.fg_muted),
        );
        job.append(
            &format!("\n{}", sessions_text(m.sessions)),
            0.0,
            tf_fmt(sans, tk.fg_muted),
        );
        chart_tip_job(ui.ctx(), resp.id.with("tip"), rect, job);
    }
}
