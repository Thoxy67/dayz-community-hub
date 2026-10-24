//! The navigation rail (= `Rail.svelte`, `RejoinCard.svelte`): the places in their groups
//! with their counts, one accent bar that slides to the current one, a way to close a
//! running game, and at the foot the last-played card, the settings, About and the
//! collapse switch.

use std::sync::Arc;

use dz_hub::features::browser::ServerRef;
use egui::{Color32, CornerRadius, Galley, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};

use crate::app::Ctx;
use crate::commands;
use crate::i18n::{format, t};
use crate::icons::lucide;
use crate::kit::{self, Players, Side, Signal, SignalSize, TipExt, paint};
use crate::store::pad::InputMode;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, motion, size};
use crate::views::{GROUPS, Group, PLACES, Place, ViewId};

/// The rail's side padding (`px-1.5`).
const PAD_X: f32 = 6.0;
/// An entry's height (`h-control-lg`) and the gap between entries (`gap-0.5`).
const ENTRY_H: f32 = size::CONTROL_LG;
const ENTRY_GAP: f32 = 2.0;
/// The space above a group and its heading's height (a thin line when compact).
const GROUP_GAP: f32 = 12.0;
const GROUP_GAP_COMPACT: f32 = 6.0;
const HEADING_H: f32 = 15.0;
const DIVIDER_H: f32 = 5.0;
/// The air above and below the places.
const PLACES_PAD: f32 = 10.0;
/// The last-played card's name line (`text-xs`, its line and the space under it).
const NAME_H: f32 = 18.0;
/// The "DayZ is running" block's height, open.
const GAME_BLOCK_H: f32 = 62.0;

/// How the rail fits the window's height: the full card and entries while they fit,
/// then the card on one line, then compact entries under thin dividers (and past that,
/// the places scroll).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fit {
    entry_h: f32,
    compact: bool,
    card_short: bool,
}

impl Fit {
    const FULL: Self = Self {
        entry_h: ENTRY_H,
        compact: false,
        card_short: false,
    };
    const SHORT_CARD: Self = Self {
        card_short: true,
        ..Self::FULL
    };
    const COMPACT: Self = Self {
        entry_h: size::CONTROL,
        compact: true,
        card_short: true,
    };
}

/// The places' natural height: their groups, headings (or lines) and entries.
fn places_height(collapsed: bool, fit: Fit) -> f32 {
    let mut h = 2.0 * PLACES_PAD;
    for (gi, (g, _)) in GROUPS.iter().enumerate() {
        let n = PLACES.iter().filter(|p| p.group == *g).count() as f32;
        h += n * (ENTRY_GAP + fit.entry_h);
        h += match (gi, collapsed || fit.compact) {
            (0, true) => 0.0,
            (0, false) => HEADING_H,
            (_, true) => group_gap(fit) + DIVIDER_H,
            (_, false) => GROUP_GAP + HEADING_H,
        };
    }
    h
}

fn group_gap(fit: Fit) -> f32 {
    if fit.compact {
        GROUP_GAP_COMPACT
    } else {
        GROUP_GAP
    }
}

/// The first fit whose places, the game's block (`extra`) and foot hold in `height`,
/// else the most compact.
fn pick_fit(collapsed: bool, height: f32, extra: f32, mut foot: impl FnMut(Fit) -> f32) -> Fit {
    for fit in [Fit::FULL, Fit::SHORT_CARD] {
        if places_height(collapsed, fit) + extra + foot(fit) <= height {
            return fit;
        }
    }
    Fit::COMPACT
}

/// The height of "DayZ is running" and its button, with the space under it.
fn game_block_height(collapsed: bool) -> f32 {
    (if collapsed { ENTRY_H } else { GAME_BLOCK_H }) + 10.0
}

fn fit_for(cx: &mut Ctx, collapsed: bool, height: f32) -> Fit {
    // The game's block shows at the places' foot while it runs (not with a pad).
    let pad = cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) == InputMode::Gamepad;
    let extra = if cx.stores.game.running && !pad {
        game_block_height(collapsed)
    } else {
        0.0
    };
    pick_fit(collapsed, height, extra, |fit| {
        foot_height(cx, collapsed, fit)
    })
}

/// The figure beside a place, and its tone.
struct Count {
    value: String,
    tone: Option<Color32>,
}

fn count_of(cx: &Ctx, id: ViewId, tk: &Tokens) -> Option<Count> {
    let s = &cx.stores;
    let plain = |n: usize| {
        (n > 0).then(|| Count {
            value: n.to_string(),
            tone: None,
        })
    };
    match id {
        ViewId::Servers => (s.servers.total > 0).then(|| Count {
            value: format::compact(f64::from(s.servers.total)),
            tone: None,
        }),
        ViewId::Favorites => plain(s.profile.favorites.len()),
        ViewId::History => plain(s.profile.history.len()),
        ViewId::Mods => {
            let stale = s.mods.stale_count();
            if stale > 0 {
                Some(Count {
                    value: format!("{stale}↑"),
                    tone: Some(tk.warn),
                })
            } else {
                plain(s.mods.installed.len())
            }
        }
        ViewId::Friends => {
            let n = s.friends.in_dayz();
            (n > 0).then(|| Count {
                value: n.to_string(),
                tone: Some(tk.ok),
            })
        }
        _ => None,
    }
}

pub(super) fn show(ui: &mut Ui, cx: &mut Ctx) {
    let tk = ui.tokens();
    let rect = ui.max_rect();
    let collapsed = crate::pad::rail_collapsed(ui.ctx(), cx.stores, cx.prefs);
    // (The right border is the panel's separator line.)
    let inner = rect;
    let fit = fit_for(cx, collapsed, inner.height());
    let foot_h = foot_height(cx, collapsed, fit);
    let split = (inner.bottom() - foot_h).max(inner.top());
    let top = Rect::from_min_max(inner.min, pos2(inner.right(), split));
    let foot = Rect::from_min_max(pos2(inner.left(), split), inner.max);

    let mut active: Option<Rect> = None;
    let mut active_in_foot = false;
    // ── the places, in their groups ──
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(top).id_salt("rail-top"));
    child.set_clip_rect(top.intersect(ui.clip_rect()));
    // A short window cannot hold every place: the handle shows without a hover, so the
    // places below the fold are known to be there.
    child.spacing_mut().scroll.dormant_handle_opacity = 0.6;
    // Inset from the rail's border, so the handle does not sit on the separator.
    child.spacing_mut().scroll.bar_outer_margin = 3.0;
    // The current place is scrolled into sight when it changes (a shortcut, the palette).
    let seen_id = ui.scope_id().with("rail-seen-view");
    let changed = ui.data(|d| d.get_temp::<ViewId>(seen_id)) != Some(cx.nav.view);
    if changed {
        ui.data_mut(|d| d.insert_temp(seen_id, cx.nav.view));
    }
    egui::ScrollArea::vertical()
        .id_salt("rail-scroll")
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let w = ui.available_width();
            ui.add_space(PLACES_PAD);
            for (gi, (g, label)) in GROUPS.iter().enumerate() {
                if collapsed || fit.compact {
                    // A thin line between groups; none above the first.
                    if gi > 0 {
                        ui.add_space(group_gap(fit));
                        let (r, _) = ui.allocate_exact_size(vec2(w, DIVIDER_H), Sense::hover());
                        let x = if collapsed {
                            (r.center().x - 10.0)..=(r.center().x + 10.0)
                        } else {
                            (r.left() + PAD_X + 10.0)..=(r.right() - PAD_X - 10.0)
                        };
                        ui.painter()
                            .hline(x, r.top() + 0.5, Stroke::new(1.0, tk.border));
                    }
                } else {
                    if gi > 0 {
                        ui.add_space(GROUP_GAP);
                    }
                    let (r, _) = ui.allocate_exact_size(vec2(w, HEADING_H), Sense::hover());
                    let gl = super::stencil(ui, t("nav", label), tk.fg_faint, Some(w - 32.0));
                    ui.painter().galley_with_override_text_color(
                        pos2(r.left() + PAD_X + 10.0, r.top() + 5.5 - gl.size().y / 2.0),
                        gl,
                        tk.fg_faint,
                    );
                }
                for p in PLACES.iter().filter(|p| p.group == *g) {
                    ui.add_space(ENTRY_GAP);
                    let r = entry(ui, cx, p, collapsed, w, fit.entry_h);
                    if cx.nav.view == p.id {
                        active = Some(r);
                        if changed {
                            ui.scroll_to_rect(r, None);
                        }
                    }
                }
            }
            ui.add_space(PLACES_PAD);
            // The game is open: a way to close it, pushed to the foot of the places.
            let pad = cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) == InputMode::Gamepad;
            if cx.stores.game.running && !pad {
                let room = ui.available_height() - game_block_height(collapsed);
                if room > 0.0 {
                    ui.add_space(room);
                }
                game_block(ui, cx, collapsed, w);
                ui.add_space(10.0);
            }
        });

    // ── the foot ──
    let p = ui.painter_at(foot);
    p.hline(
        foot.x_range(),
        foot.top() + 0.5,
        Stroke::new(1.0, tk.border),
    );
    kit::topo::paint(
        &p,
        foot.shrink2(vec2(0.0, 0.5)).translate(vec2(0.0, 0.5)),
        &tk,
        0.4,
    );
    let mut y = foot.top() + 1.0 + 8.0;
    y = rejoin(ui, cx, collapsed, fit, foot, y);
    let mut fui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(pos2(foot.left(), y), foot.max))
            .id_salt("rail-foot"),
    );
    fui.spacing_mut().item_spacing = Vec2::ZERO;
    let w = fui.available_width();
    for p in PLACES.iter().filter(|p| p.group == Group::Foot) {
        let r = entry(&mut fui, cx, p, collapsed, w, fit.entry_h);
        if cx.nav.view == p.id {
            active = Some(r);
            active_in_foot = true;
        }
        fui.add_space(ENTRY_GAP);
    }
    collapse_button(&mut fui, cx, collapsed, w);

    // ── the accent bar beside the current place, sliding to the next ──
    if let Some(r) = active {
        let id = ui.scope_id().with("rail-bar");
        let dur = if crate::app::reduced_motion(ui.ctx()) {
            0.0
        } else {
            motion::BASE
        };
        let y = ui.ctx().animate_value_with_time(id, r.top() + 6.0, dur);
        let bar = Rect::from_min_size(
            pos2(rect.left() + PAD_X, y),
            vec2(2.0, (r.height() - 12.0).max(0.0)),
        );
        // A place scrolled out of the places' area takes its bar along (not over the foot).
        let clip = if active_in_foot { foot } else { top };
        ui.painter_at(clip)
            .rect_filled(bar, CornerRadius::same(1), tk.accent);
    }
}

/// One place: icon, name, count (`h-control-lg rounded-sm gap-2.5 text-sm`).
fn entry(ui: &mut Ui, cx: &mut Ctx, p: &Place, collapsed: bool, w: f32, h: f32) -> Rect {
    let tk = ui.tokens();
    let (row, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let r = Rect::from_min_max(
        pos2(row.left() + PAD_X, row.top()),
        pos2(row.right() - PAD_X, row.bottom()),
    );
    let resp = ui.interact(r, ui.scope_id().with(("rail", p.id.id())), Sense::click());
    let label = t("nav", p.label);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let on = cx.nav.view == p.id;
    let h = paint::ease(ui, resp.id, resp.hovered());
    let painter = ui.painter();
    let fill = if on {
        tk.raised
    } else {
        paint::lerp(Color32::TRANSPARENT, paint::alpha(tk.raised, 0.6), h)
    };
    painter.rect_filled(r, CornerRadius::same(size::RADIUS_SM), fill);
    let ink = if on {
        tk.fg
    } else {
        paint::lerp(tk.fg_muted, tk.fg, h)
    };
    let icon_ink = if on {
        tk.accent
    } else {
        paint::lerp(tk.fg_faint, tk.fg_muted, h)
    };
    let cy = r.center().y;
    let count = count_of(cx, p.id, &tk);
    if collapsed {
        p.icon.paint(
            painter,
            Rect::from_center_size(r.center(), Vec2::splat(size::ICON)),
            icon_ink,
        );
        // The figure's own tone: amber for mod updates, green for friends in game.
        if let Some(tone) = count.as_ref().and_then(|c| c.tone) {
            painter.circle_filled(pos2(r.right() - 8.0 - 3.0, r.top() + 6.0 + 3.0), 3.0, tone);
        }
    } else {
        let mut x = r.left() + 10.0;
        p.icon.paint(
            painter,
            Rect::from_min_size(pos2(x, cy - size::ICON / 2.0), Vec2::splat(size::ICON)),
            icon_ink,
        );
        x += size::ICON + 10.0;
        let mut right = r.right() - 10.0;
        if let Some(c) = &count {
            // Pops in once when the figure changes (a new mod update, a favourite added).
            let k = pop(ui, egui::Id::unique(("rail-count", p.id.id())), &c.value);
            let tone = c.tone.unwrap_or(tk.fg_faint).gamma_multiply(k);
            let g = paint::galley(ui, &c.value, font(Face::Mono, size::TEXT_2XS), tone);
            right -= g.size().x;
            paint::text_at(painter, &g, right, cy + 2.0 * (1.0 - k), tone);
            right -= 10.0;
        }
        let g = paint::galley_trunc(
            ui,
            label,
            font(Face::Sans, size::TEXT_SM),
            ink,
            (right - x).max(0.0),
        );
        paint::text_at(painter, &g, x, cy, ink);
    }
    super::ring(ui, &resp, r, size::RADIUS_SM);
    // The hint (and its shortcut, written out) is the collapsed rail's only.
    let resp = if collapsed {
        let kbd = commands::view_key(p.id).unwrap_or_default();
        resp.tip_kbd(label, &kbd, Side::Right)
    } else {
        resp
    };
    if resp.clicked() {
        cx.go(p.id);
    }
    r
}

/// `animate-pop-in` (140 ms) for a figure that changed: 0 → 1. The first value shows
/// at once; motion off, every value does.
fn pop(ui: &Ui, id: egui::Id, value: &str) -> f32 {
    const POP: f64 = 0.14;
    let now = ui.input(|i| i.time);
    let hash = egui::Id::unique(value);
    let prev: Option<(egui::Id, f64)> = ui.data(|d| d.get_temp(id));
    let at = match prev {
        Some((h, at)) if h == hash => at,
        Some(_) if !crate::app::reduced_motion(ui.ctx()) => {
            ui.data_mut(|d| d.insert_temp(id, (hash, now)));
            now
        }
        _ => {
            ui.data_mut(|d| d.insert_temp(id, (hash, f64::NEG_INFINITY)));
            f64::NEG_INFINITY
        }
    };
    let t = ((now - at) / POP).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    1.0 - (1.0 - t).powi(3)
}

/// "DayZ is running" and the button that closes it.
fn game_block(ui: &mut Ui, cx: &mut Ctx, collapsed: bool, w: f32) {
    let tk = ui.tokens();
    let closing = cx.stores.game.closing;
    let label = t("nav", "killGame");
    let h = if collapsed { ENTRY_H } else { GAME_BLOCK_H };
    let (row, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let r = row.shrink2(vec2(PAD_X, 0.0));
    let p = ui.painter();
    let btn = if collapsed {
        r
    } else {
        paint::plate(
            p,
            r,
            size::RADIUS_SM,
            paint::alpha(tk.err, 0.08),
            paint::alpha(tk.err, 0.3),
        );
        let line_y = r.top() + 8.0 + 7.0;
        p.circle_filled(pos2(r.left() + 10.0 + 3.0, line_y), 3.0, tk.ok);
        let g = paint::galley(
            ui,
            t("nav", "gameRunning"),
            font(Face::Sans, size::TEXT_2XS),
            tk.fg_muted,
        );
        paint::text_at(p, &g, r.left() + 10.0 + 6.0 + 6.0, line_y, tk.fg_muted);
        Rect::from_min_max(
            pos2(r.left() + 10.0, r.bottom() - 8.0 - size::CONTROL),
            pos2(r.right() - 10.0, r.bottom() - 8.0),
        )
    };
    let resp = ui.interact(
        btn,
        ui.scope_id().with("kill-game"),
        if !closing {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hov = paint::ease(ui, resp.id, resp.hovered() && !closing);
    let p = ui.painter();
    let ink = paint::fade(tk.err, !closing);
    if collapsed {
        paint::plate(
            p,
            btn,
            size::RADIUS_SM,
            paint::lerp(paint::alpha(tk.err, 0.08), paint::alpha(tk.err, 0.15), hov),
            paint::lerp(paint::alpha(tk.err, 0.3), paint::alpha(tk.err, 0.5), hov),
        );
        lucide::SQUARE_X.paint(
            p,
            Rect::from_center_size(btn.center(), Vec2::splat(size::ICON)),
            ink,
        );
        let _ = resp.clone().tip_kbd(label, "", Side::Right);
    } else {
        paint::plate(
            p,
            btn,
            size::RADIUS_SM,
            paint::lerp(Color32::TRANSPARENT, paint::alpha(tk.err, 0.15), hov),
            paint::alpha(tk.err, 0.4),
        );
        let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_XS), ink);
        let total = size::ICON_SM + 6.0 + g.size().x;
        let x = btn.center().x - total / 2.0;
        lucide::SQUARE_X.paint(
            p,
            Rect::from_min_size(pos2(x, btn.center().y - 7.0), Vec2::splat(size::ICON_SM)),
            ink,
        );
        paint::text_at(p, &g, x + size::ICON_SM + 6.0, btn.center().y, ink);
    }
    super::ring(ui, &resp, btn, size::RADIUS_SM);
    if resp.clicked() {
        let _ = cx.run("game.close", &serde_json::Value::Null);
    }
}

/// The last-played entry, or `None` when there is none or it was put away.
fn last_played(cx: &Ctx) -> Option<(String, u16, String, i64)> {
    let h = cx.stores.profile.history.first()?;
    let addr = format!("{}:{}", h.ip, h.port);
    if cx.prefs.get().ui.dismissed_rejoin.as_deref() == Some(addr.as_str()) {
        return None;
    }
    Some((h.ip.clone(), h.port, h.name.clone(), h.ts))
}

/// The foot's height: border, `pt-2`, the card and its gap, two entries, the collapse
/// button, `pb-1.5`.
fn foot_height(cx: &mut Ctx, collapsed: bool, fit: Fit) -> f32 {
    let entries = 2.0 * fit.entry_h + 2.0 * ENTRY_GAP + size::CONTROL;
    let card = match last_played(cx) {
        None => 0.0,
        Some(_) if collapsed || fit.card_short => fit.entry_h + 8.0,
        Some((ip, port, ..)) => {
            let listed = cx.stores.servers.row_of(&ip, i64::from(port)).is_some();
            card_height(listed) + 8.0
        }
    };
    1.0 + 8.0 + card + entries + 6.0
}

fn card_height(listed: bool) -> f32 {
    let head = 24.0 + 1.0;
    let body = if listed {
        6.0 + NAME_H + 6.0 + 21.0 + 6.0 + 15.0 + 6.0 + size::CONTROL + 6.0
    } else {
        6.0 + NAME_H + 6.0 + 15.0 + 6.0 + size::CONTROL + 6.0
    };
    head + body + 2.0
}

/// The last-played card (or its one-line and collapsed forms) from `y`; returns the
/// next free y.
fn rejoin(ui: &mut Ui, cx: &mut Ctx, collapsed: bool, fit: Fit, foot: Rect, y: f32) -> f32 {
    let Some((ip, port, name, ts)) = last_played(cx) else {
        return y;
    };
    let tk = ui.tokens();
    let rejoin_label = t("nav", "rejoin");
    if collapsed || fit.card_short {
        let r = if collapsed {
            Rect::from_min_size(
                pos2(foot.left() + PAD_X, y),
                vec2(foot.width() - 2.0 * PAD_X, fit.entry_h),
            )
        } else {
            Rect::from_min_size(
                pos2(foot.left() + 8.0, y),
                vec2(foot.width() - 16.0, fit.entry_h),
            )
        };
        let resp = ui.interact(r, ui.scope_id().with("rejoin-line"), Sense::click());
        let h = paint::ease(ui, resp.id, resp.hovered());
        let p = ui.painter();
        p.rect_filled(
            r,
            CornerRadius::same(size::RADIUS_SM),
            paint::lerp(
                paint::alpha(tk.accent, 0.15),
                paint::alpha(tk.accent, 0.25),
                h,
            ),
        );
        if collapsed {
            lucide::PLAY.paint(
                p,
                Rect::from_center_size(r.center(), Vec2::splat(size::ICON)),
                tk.accent,
            );
        } else {
            let x = r.left() + 8.0;
            lucide::PLAY.paint(
                p,
                Rect::from_min_size(pos2(x, r.center().y - 8.0), Vec2::splat(size::ICON)),
                tk.accent,
            );
            let tx = x + size::ICON + 8.0;
            let g = paint::galley_trunc(
                ui,
                &name,
                font(Face::SansSemibold, size::TEXT_XS),
                tk.accent,
                (r.right() - 8.0 - tx).max(0.0),
            );
            paint::text_at(p, &g, tx, r.center().y, tk.accent);
        }
        super::ring(ui, &resp, r, size::RADIUS_SM);
        let tip = format!("{rejoin_label}: {name}");
        if resp.tip_kbd(&tip, "", Side::Right).clicked() {
            let _ = cx.run("rejoin", &serde_json::Value::Null);
        }
        return r.bottom() + 8.0;
    }

    // The snapshot is held here (an `Arc`), so the row borrows it and not the stores.
    let snap = Arc::clone(cx.stores.servers.snapshot());
    let listed = dz_api::addr(&ip, i64::from(port)).and_then(|a| snap.lookup(a));
    let card = Rect::from_min_size(
        pos2(foot.left() + 8.0, y),
        vec2(foot.width() - 16.0, card_height(listed.is_some())),
    );
    let p = ui.painter();
    p.rect_filled(card, CornerRadius::same(size::RADIUS_MD), tk.panel);
    // Header: "LAST PLAYED", how long ago, put away.
    let head = Rect::from_min_size(card.min + vec2(1.0, 1.0), vec2(card.width() - 2.0, 24.0));
    p.hline(
        head.x_range(),
        head.bottom() + 0.5,
        Stroke::new(1.0, paint::alpha(tk.border, 0.7)),
    );
    let cy = head.center().y;
    let sg = super::stencil(ui, t("nav", "lastPlayed"), tk.fg_faint, None);
    paint::text_at(p, &sg, head.left() + 8.0, cy + 0.5, tk.fg_faint);
    let close_r = Rect::from_min_size(pos2(head.right() - 8.0 - 16.0, cy - 8.0), Vec2::splat(16.0));
    let close = ui.interact(close_r, ui.scope_id().with("rejoin-close"), Sense::click());
    let ch = paint::ease(ui, close.id, close.hovered());
    let p = ui.painter();
    p.rect_filled(
        close_r,
        CornerRadius::same(size::RADIUS_XS),
        paint::lerp(Color32::TRANSPARENT, tk.raised, ch),
    );
    lucide::X.paint(
        p,
        Rect::from_center_size(close_r.center(), Vec2::splat(12.0)),
        paint::lerp(tk.fg_faint, tk.fg, ch),
    );
    super::ring(ui, &close, close_r, size::RADIUS_XS);
    let close_label = t("common", "close");
    if close.tip_kbd(close_label, "", Side::Top).clicked() {
        let _ = cx.run("rejoin.dismiss", &serde_json::Value::Null);
    }
    let ago_x = head.left() + 8.0 + sg.size().x + 6.0;
    // Short: the heading already says it is the last game, and the room is narrow.
    let ago = format::ago_short(ts);
    let ag = paint::galley_trunc(
        ui,
        &ago,
        font(Face::Mono, size::TEXT_3XS),
        tk.fg_faint,
        (close_r.left() - 6.0 - ago_x).max(0.0),
    );
    let ag_r = Rect::from_min_size(
        pos2(close_r.left() - 6.0 - ag.size().x, cy - ag.size().y / 2.0),
        ag.size(),
    );
    paint::text_at(ui.painter(), &ag, ag_r.left(), cy, tk.fg_faint);
    let _ = ui
        .interact(ag_r, ui.scope_id().with("rejoin-ago"), Sense::hover())
        .tip_kbd(&format::date_time(ts), "", Side::Top);

    // Body.
    let left = card.left() + 1.0 + 8.0;
    let right = card.right() - 1.0 - 8.0;
    let mut y = head.bottom() + 1.0 + 6.0;
    let name_r = Rect::from_min_size(pos2(left, y), vec2(right - left, 16.0));
    let nresp = ui.interact(name_r, ui.scope_id().with("rejoin-name"), Sense::click());
    let nh = paint::ease(ui, nresp.id, nresp.hovered());
    let ink = paint::lerp(tk.fg, tk.accent, nh);
    let ng = paint::galley_trunc(
        ui,
        &name,
        font(Face::SansSemibold, size::TEXT_XS),
        ink,
        name_r.width(),
    );
    paint::text_at(ui.painter(), &ng, left, name_r.center().y, ink);
    super::ring(ui, &nresp, name_r, size::RADIUS_XS);
    if nresp.tip_kbd(&name, "", Side::Top).clicked() {
        cx.go_focus(ViewId::History, "last");
    }
    y = name_r.top() + NAME_H + 6.0;
    match &listed {
        Some(row) => {
            y = listed_lines(ui, cx, *row, left, right, y);
        }
        None => {
            let g = paint::galley_trunc(
                ui,
                t("nav", "notListed"),
                font(Face::Sans, size::TEXT_2XS),
                tk.warn,
                right - left,
            );
            paint::text_at(ui.painter(), &g, left, y + 7.5, tk.warn);
            y += 15.0 + 6.0;
        }
    }
    // REJOIN.
    let br = Rect::from_min_size(pos2(left, y), vec2(right - left, size::CONTROL));
    let bresp = ui.interact(br, ui.scope_id().with("rejoin-btn"), Sense::click());
    let bh = paint::ease(ui, bresp.id, bresp.hovered());
    let p = ui.painter();
    p.rect_filled(
        br,
        CornerRadius::same(size::RADIUS_SM),
        paint::bright(tk.accent, 1.0 + 0.1 * bh),
    );
    let lg = paint::tracked(
        ui,
        rejoin_label,
        font(Face::DisplayBold, size::TEXT_SM),
        tk.accent_fg,
        0.08,
        true,
        None,
    );
    let total = size::ICON_SM + 6.0 + lg.size().x;
    let x = br.center().x - total / 2.0;
    lucide::PLAY.paint(
        p,
        Rect::from_min_size(pos2(x, br.center().y - 7.0), Vec2::splat(size::ICON_SM)),
        tk.accent_fg,
    );
    paint::text_at(
        p,
        &lg,
        x + size::ICON_SM + 6.0,
        br.center().y + 0.5,
        tk.accent_fg,
    );
    super::ring(ui, &bresp, br, size::RADIUS_SM);
    if bresp.clicked() {
        let _ = cx.run("rejoin", &serde_json::Value::Null);
    }
    // The card's border, over its insides.
    ui.painter().rect_stroke(
        card,
        CornerRadius::same(size::RADIUS_MD),
        Stroke::new(1.0, tk.border),
        egui::StrokeKind::Inside,
    );
    card.bottom() + 8.0
}

/// The listed server's live head-count, ping, map and time.
fn listed_lines(
    ui: &mut Ui,
    cx: &mut Ctx,
    row: ServerRef<'_>,
    left: f32,
    right: f32,
    y: f32,
) -> f32 {
    let tk = ui.tokens();
    let pr = Rect::from_min_size(pos2(left, y), vec2(64.0, 21.0));
    let players = u32::try_from(row.players().max(0)).unwrap_or(0);
    let max = u32::try_from(row.max_players().max(0)).unwrap_or(0);
    ui.place(pr, Players::new(players, max).bots(row.bots()).compact());
    let key = row.addr().to_string();
    let ms = crate::store::servers::Servers::listed_ping(&row)
        .or_else(|| cx.stores.servers.ping.get(&key).copied());
    let pending = cx.stores.servers.pending.contains(&key);
    let sr = Rect::from_min_max(pos2(right - 56.0, y), pos2(right, y + 21.0));
    let mut sui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(sr)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let sresp = sui.add(Signal::new(ms).pending(pending).size(SignalSize::Xs));
    let click = ui.interact(
        sresp.rect,
        ui.scope_id().with("rejoin-ping"),
        Sense::click(),
    );
    super::ring(ui, &click, sresp.rect, size::RADIUS_XS);
    if click.clicked() {
        let timeout = cx.stores.profile.ping_settings().1;
        cx.stores
            .servers
            .ping_one(&row.ip().to_string(), i64::from(row.query_port()), timeout);
    }
    let y = y + 21.0 + 6.0;
    // Map and time (`text-2xs text-map`).
    let cy = y + 7.5;
    lucide::MAP.paint(
        ui.painter(),
        Rect::from_min_size(pos2(left, cy - 6.0), Vec2::splat(12.0)),
        tk.map,
    );
    let mut map_right = right;
    if !row.server.time.is_empty() {
        let tg = paint::galley(
            ui,
            &row.server.time,
            font(Face::Mono, size::TEXT_2XS),
            tk.fg_faint,
        );
        map_right -= tg.size().x;
        paint::text_at(ui.painter(), &tg, map_right, cy, tk.fg_faint);
        map_right -= 6.0;
    }
    let mg: Arc<Galley> = paint::galley_trunc(
        ui,
        row.map,
        font(Face::Sans, size::TEXT_2XS),
        tk.map,
        (map_right - left - 16.0).max(0.0),
    );
    paint::text_at(ui.painter(), &mg, left + 16.0, cy, tk.map);
    y + 15.0 + 6.0
}

/// "Collapse the menu" (`h-control text-2xs text-fg-faint`).
fn collapse_button(ui: &mut Ui, cx: &mut Ctx, collapsed: bool, w: f32) {
    let tk = ui.tokens();
    let (row, _) = ui.allocate_exact_size(vec2(w, size::CONTROL), Sense::hover());
    let r = row.shrink2(vec2(PAD_X, 0.0));
    let resp = ui.interact(r, ui.scope_id().with("rail-collapse"), Sense::click());
    let label = if collapsed {
        t("nav", "expand")
    } else {
        t("nav", "collapse")
    };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let h = paint::ease(ui, resp.id, resp.hovered());
    let p = ui.painter();
    p.rect_filled(
        r,
        CornerRadius::same(size::RADIUS_SM),
        paint::lerp(Color32::TRANSPARENT, paint::alpha(tk.raised, 0.6), h),
    );
    let ink = paint::lerp(tk.fg_faint, tk.fg_muted, h);
    if collapsed {
        lucide::PANEL_LEFT_OPEN.paint(
            p,
            Rect::from_center_size(r.center(), Vec2::splat(size::ICON)),
            ink,
        );
        let _ = resp.clone().tip_kbd(label, "", Side::Right);
    } else {
        let x = r.left() + 10.0;
        lucide::PANEL_LEFT_CLOSE.paint(
            p,
            Rect::from_min_size(pos2(x, r.center().y - 8.0), Vec2::splat(size::ICON)),
            ink,
        );
        let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_2XS), ink);
        paint::text_at(p, &g, x + size::ICON + 10.0, r.center().y, ink);
    }
    super::ring(ui, &resp, r, size::RADIUS_SM);
    if resp.clicked() {
        let _ = cx.run("rail.toggle_collapse", &serde_json::Value::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::{Fit, game_block_height, pick_fit, places_height};

    #[test]
    fn compact_places_are_shorter_and_collapsed_has_no_headings() {
        let full = places_height(false, Fit::FULL);
        assert_eq!(full, places_height(false, Fit::SHORT_CARD));
        let compact = places_height(false, Fit::COMPACT);
        assert!(compact < full, "{compact} < {full}");
        // Collapsed: thin lines instead of headings, so shorter than the full rail.
        assert!(places_height(true, Fit::FULL) < full);
    }

    #[test]
    fn the_fit_steps_down_as_the_rail_shortens() {
        // A foot whose card is 150 tall in full and 40 on one line.
        let foot = |f: Fit| if f.card_short { 40.0 } else { 150.0 };
        let full = places_height(false, Fit::FULL) + 150.0;
        let short = places_height(false, Fit::SHORT_CARD) + 40.0;
        assert_eq!(pick_fit(false, full, 0.0, foot), Fit::FULL);
        assert_eq!(pick_fit(false, full - 1.0, 0.0, foot), Fit::SHORT_CARD);
        assert_eq!(pick_fit(false, short, 0.0, foot), Fit::SHORT_CARD);
        assert_eq!(pick_fit(false, short - 1.0, 0.0, foot), Fit::COMPACT);
        // The game's block counts: the height that held the full rail no longer does.
        assert_eq!(
            pick_fit(false, full, game_block_height(false), foot),
            Fit::SHORT_CARD
        );
        // The 900x600 window's rail (600 less the title and status bars) holds the
        // compact places with a one-line card.
        use crate::theme::size;
        let rail = 600.0 - size::TITLEBAR - crate::shell::STATUSBAR_HEIGHT;
        let c = Fit::COMPACT.entry_h;
        let foot = 1.0 + 8.0 + (c + 8.0) + 2.0 * c + 2.0 * super::ENTRY_GAP + size::CONTROL + 6.0;
        let need = places_height(false, Fit::COMPACT) + foot;
        assert!(need <= rail, "{need} <= {rail}");
    }
}
