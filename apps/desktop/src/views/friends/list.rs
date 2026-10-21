//! The list pane (= `listPane` and its snippets in `View.svelte`): sticky group heads,
//! friend rows (avatar, status dot, name, what they are doing, Steam buttons on hover)
//! and, under each DayZ block, the shared server row. Only the rows on screen are drawn;
//! the model gives every row's place.

use std::sync::Arc;

use dz_hub::features::friends::{FriendDto, FriendStatus};
use egui::text::{LayoutJob, TextFormat};
use egui::{CornerRadius, Pos2, Rangef, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};

use super::model::{Block, Group, HEAD_H, Item, Model};
use super::{Act, now_unix};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{IconButton, SectionLabel, Size, TipExt, Tone};
use crate::parts::{
    Empty, Flags, ListColumns, RowAction, RowButton, RowCache, RowData, RowTag, ServerListRow,
    image,
};
use crate::store::friends::Selected;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// `pl-6`: the server row sits under its friends, indented.
const SERVER_INDENT: f32 = 24.0;
/// Button ids of the server row's own actions.
const BTN_SHOW: u32 = 1;
const BTN_DETAILS: u32 = 2;
const BTN_DIRECT: u32 = 3;

/// The status word (`statusWord`).
pub fn status_word(s: FriendStatus) -> &'static str {
    t(
        "friends",
        match s {
            FriendStatus::Online => "online",
            FriendStatus::Busy => "busy",
            FriendStatus::Away => "away",
            FriendStatus::Snooze => "snooze",
            FriendStatus::Offline => "offline",
        },
    )
}

/// The status dot's colour: online ok, away and snoozing warn, busy err, offline faint.
fn dot(t: &Tokens, s: FriendStatus) -> egui::Color32 {
    match s {
        FriendStatus::Online => t.ok,
        FriendStatus::Away | FriendStatus::Snooze => t.warn,
        FriendStatus::Busy => t.err,
        FriendStatus::Offline => alpha(t.fg_faint, 0.4),
    }
}

/// The list, into `ui` (the pane), with what was clicked into `acts`.
pub fn show(ui: &mut Ui, cx: &Ctx, model: &Model, cache: &mut RowCache, acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, tk.panel);
    let mut pane = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    pane.set_clip_rect(rect.intersect(ui.clip_rect()));
    ui.allocate_rect(rect, Sense::hover());
    let ui = &mut pane;
    if model.shown == 0 {
        Empty::new()
            .icon(lucide::SEARCH_X)
            .title(t("friends", "noMatch"))
            .show(ui, |_| {});
        return;
    }
    let (tops, total) = model.offsets();
    let list = cx.stores.friends.list();
    egui::ScrollArea::vertical()
        .id_salt("friends-list")
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, vp| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let w = ui.available_width();
            let origin = ui.max_rect().min;
            ui.set_min_size(Vec2::new(w, total));
            for (n, item) in model.items.iter().enumerate() {
                let top = tops.get(n).copied().unwrap_or(0.0);
                let h = item.height();
                if top + h < vp.min.y || top > vp.max.y {
                    continue;
                }
                let r = Rect::from_min_size(origin + Vec2::new(0.0, top), Vec2::new(w, h));
                let mut rui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(r)
                        .id_salt(("friends-item", n))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                match *item {
                    Item::Head(g, count) => head(&mut rui, r, g, count),
                    Item::Friend(i) => {
                        if let Some(f) = list.get(i) {
                            friend_row(&mut rui, r, cx, f, acts);
                        }
                    }
                    Item::Server(b) => {
                        if let Some(b) = model.blocks.get(b) {
                            server_row(&mut rui, r, cx, b, cache, acts);
                        }
                    }
                }
            }
            // The head of the section under the top edge sticks there, pushed up by
            // the next one (`sticky top-0` inside its `section`).
            let at = vp.min.y;
            let mut current: Option<(usize, f32)> = None;
            let mut next_top = f32::INFINITY;
            for (n, item) in model.items.iter().enumerate() {
                if let Item::Head(..) = item {
                    let top = tops.get(n).copied().unwrap_or(0.0);
                    if top <= at {
                        current = Some((n, top));
                    } else {
                        next_top = top;
                        break;
                    }
                }
            }
            if let Some((n, top)) = current
                && top < at
                && let Some(Item::Head(g, count)) = model.items.get(n)
            {
                let y = at.min(next_top - HEAD_H);
                let r = Rect::from_min_size(origin + Vec2::new(0.0, y), Vec2::new(w, HEAD_H));
                let _ = ui.interact(r, ui.id().with("friends-sticky"), Sense::click());
                head(ui, r, *g, *count);
            }
        });
}

/// A group's heading: the section label and its count, on `bg`, a border under.
fn head(ui: &mut Ui, r: Rect, g: Group, count: usize) {
    let tk = ui.tokens();
    let p = ui.painter();
    p.rect_filled(r, 0.0, tk.bg);
    p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, tk.border));
    let inner = Rect::from_min_max(
        Pos2::new(r.left() + size::PAD, r.top()),
        Pos2::new(r.right() - size::PAD, r.bottom() - 1.0),
    );
    let mut lui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_center_size(
                inner.center(),
                Vec2::new(inner.width(), 18.0),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let n = count.to_string();
    SectionLabel::new(t("friends", g.label())).show_with(&mut lui, |ui| {
        let g = paint::galley(ui, &n, font(Face::Mono, size::TEXT_2XS), tk.fg_faint);
        let (rr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(rr.min, g, tk.fg_faint);
    });
}

/// "x ago" for a Unix second.
fn ago(ts: i64) -> String {
    format::relative_at(now_unix(), ts)
}

/// One friend: avatar and dot, name, what they are doing, Steam buttons on hover.
fn friend_row(ui: &mut Ui, r: Rect, cx: &Ctx, f: &FriendDto, acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    let id = ui.id().with(("friend", &f.steam_id));
    let row = ui.interact(r, id, Sense::hover());
    let hovered = ui.rect_contains_pointer(r);
    let hover = paint::ease(ui, id.with("hover"), hovered);
    if hover > 0.0 {
        ui.painter()
            .rect_filled(r, 0.0, alpha(tk.raised, 0.5 * hover));
    }
    ui.painter().hline(
        r.x_range(),
        r.bottom() - 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.5)),
    );
    let _ = row;
    if f.status == FriendStatus::Offline && f.game.is_none() {
        ui.multiply_opacity(0.6);
    }
    let p = ui.painter().clone();
    let cy = r.center().y;

    // The avatar, or the first letter on a plate.
    let av = Rect::from_min_size(
        Pos2::new(r.left() + size::PAD, cy - 16.0),
        Vec2::splat(32.0),
    );
    let drawn = f
        .avatar
        .as_deref()
        .is_some_and(|src| image::paint(&p, av, src, size::RADIUS_SM));
    if !drawn {
        p.rect_filled(av, CornerRadius::same(size::RADIUS_SM), tk.raised);
        let letter = f
            .name
            .chars()
            .next()
            .map_or_else(|| "?".to_string(), |c| c.to_uppercase().collect());
        let g = paint::galley(ui, &letter, font(Face::Display, size::TEXT_SM), tk.fg_muted);
        paint::text_center(&p, &g, av, tk.fg_muted);
    }
    // The status dot, ringed in the panel's colour.
    let c = Pos2::new(av.right() + 2.0 - 5.0, av.bottom() + 2.0 - 5.0);
    p.circle_filled(c, 7.0, tk.panel);
    p.circle_filled(c, 5.0, dot(&tk, f.status));
    let dr = Rect::from_center_size(c, Vec2::splat(10.0));
    let _ = ui
        .interact(dr, id.with("dot"), Sense::hover())
        .tip(status_word(f.status));

    // The Steam buttons (`opacity-0 group-hover:opacity-100`).
    let bw = 22.0 * 2.0 + 1.0 + 4.0;
    let br = Rect::from_min_size(
        Pos2::new(r.right() - size::PAD - bw, cy - 13.0),
        Vec2::new(bw, 26.0),
    );
    let x0 = av.right() + 12.0;
    let tw = (if hover > 0.01 {
        br.left()
    } else {
        r.right() - size::PAD
    }) - 12.0
        - x0;

    let name = paint::galley_trunc(ui, &f.name, font(Face::Sans, size::TEXT_SM), tk.fg, tw);
    let sub = subline(ui, cx, f, &tk, tw);
    let gap = 1.0;
    let h = name.size().y + gap + sub.size().y;
    let y = cy - h / 2.0;
    // The name can be selected and copied (`data-selectable`).
    let nr = Rect::from_min_size(Pos2::new(x0, y), name.size());
    ui.put(nr, egui::Label::new(name.clone()).selectable(true));
    p.galley(Pos2::new(x0, y + name.size().y + gap), sub, tk.fg_muted);

    if hover > 0.01 {
        let mut bui = ui.new_child(
            UiBuilder::new()
                .max_rect(br)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        bui.multiply_opacity(hover);
        paint::plate(bui.painter(), br, size::RADIUS_SM, tk.panel, tk.border);
        bui.add_space(2.0);
        bui.spacing_mut().item_spacing.x = 1.0;
        if bui
            .add(
                IconButton::new(lucide::MESSAGE_SQUARE, t("friends", "sendMessage"))
                    .size(Size::IconXs),
            )
            .clicked()
        {
            acts.push(Act::Message(f.steam_id.clone()));
        }
        if bui
            .add(
                IconButton::new(lucide::USER_ROUND, t("friends", "openProfile")).size(Size::IconXs),
            )
            .clicked()
        {
            acts.push(Act::Profile(f.steam_id.clone()));
        }
    }
}

/// The line under the name: DayZ (and where, and since when), the game, when last seen,
/// or the status word.
fn subline(ui: &Ui, cx: &Ctx, f: &FriendDto, tk: &Tokens, width: f32) -> Arc<egui::Galley> {
    let fid = font(Face::Sans, size::TEXT_2XS);
    let mut job = LayoutJob::default();
    let mut put = |text: &str, color| {
        job.append(text, 0.0, TextFormat::simple(fid.clone(), color));
    };
    if f.in_dayz {
        put("DayZ", tk.accent);
        if f.server.is_none() {
            put(&format!(" · {}", t("friends", "noServer")), tk.fg_muted);
        }
        if let Some(since) = cx.stores.friends.since.get(&f.steam_id) {
            let secs = i64::try_from(since.elapsed().as_secs()).unwrap_or(0);
            let when = ago(now_unix().saturating_sub(secs));
            put(
                &format!(" · {}", tf("friends", "inDayzSince", &[("when", &when)])),
                tk.fg_muted,
            );
        }
    } else if let Some(g) = &f.game {
        put(g, tk.ok);
    } else if let (FriendStatus::Offline, Some(last)) = (f.status, f.last_online) {
        put(
            &tf("friends", "lastOnline", &[("when", &ago(last))]),
            tk.fg_muted,
        );
    } else {
        let c = match f.status {
            FriendStatus::Busy => tk.err,
            FriendStatus::Online => tk.fg_muted,
            _ => tk.warn,
        };
        put(status_word(f.status), c);
    }
    job.wrap = egui::text::TextWrapping {
        max_width: width.max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// The server under a DayZ block, as every server list shows it.
fn server_row(
    ui: &mut Ui,
    r: Rect,
    cx: &Ctx,
    b: &Block,
    cache: &mut RowCache,
    acts: &mut Vec<Act>,
) {
    let tk = ui.tokens();
    ui.painter().rect_filled(r, 0.0, alpha(tk.bg, 0.4));
    ui.painter()
        .hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, tk.border));
    let rr = Rect::from_min_max(Pos2::new(r.left() + SERVER_INDENT, r.top()), r.max);
    let mut sui = ui.new_child(
        UiBuilder::new()
            .max_rect(rr)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let cols = ListColumns::new(Rangef::new(rr.left(), rr.right()), false);
    let selected = cx
        .stores
        .friends
        .selected
        .as_ref()
        .is_some_and(|s| s.ip == b.ip && s.port == b.port);
    let unlisted = t("friends", "unlisted");
    let buttons = [
        if b.listed {
            RowButton {
                id: BTN_SHOW,
                icon: lucide::LIST,
                label: t("friends", "showInList"),
                danger: false,
                tint: None,
            }
        } else {
            RowButton {
                id: BTN_DETAILS,
                icon: lucide::SCAN_SEARCH,
                label: t("friends", "details"),
                danger: false,
                tint: None,
            }
        },
        RowButton {
            id: BTN_DIRECT,
            icon: lucide::PLUG_ZAP,
            label: t("servers", "directConnect"),
            danger: false,
            tint: None,
        },
    ];
    let port = i64::from(b.port);
    let listed_row = cx.stores.servers.row(&b.ip, port);
    let fallback = b
        .name
        .clone()
        .unwrap_or_else(|| format!("{}:{}", b.ip, b.port));
    let map = b.map.clone().unwrap_or_default();
    let key = format!("{}:{}", b.ip, b.port);
    let (data, name, query_port, game_port) = match &listed_row {
        Some(lr) => {
            let d = RowData::from_server(lr);
            let name = d.name.to_string();
            (
                RowData {
                    join_port: port,
                    listed: b.listed,
                    ping_pending: cx.stores.servers.pending.contains(&key),
                    ..d
                },
                name,
                d.query_port,
                d.game_port,
            )
        }
        None => (
            RowData {
                ip: &b.ip,
                query_port: port,
                game_port: port,
                join_port: port,
                name: &fallback,
                map: &map,
                flags: Flags::default(),
                ping_ms: cx.stores.servers.ping_ms(&b.ip, port),
                ping_pending: cx.stores.servers.pending.contains(&key),
                favorite: cx.stores.profile.is_favorite(&b.ip, b.port),
                excluded: cx.stores.profile.is_excluded(&b.ip),
                listed: b.listed,
                ..RowData::default()
            },
            fallback.clone(),
            port,
            port,
        ),
    };
    let favorite = data.favorite;
    let excluded = data.excluded;
    let mut row = ServerListRow::new(data)
        .selected(selected)
        .friends(&b.here)
        .buttons(&buttons)
        .mods_clickable(false);
    if !b.listed {
        row = row.tag(RowTag {
            text: unlisted,
            tone: Tone::Warn,
            tip: "",
        });
    }
    let out = row.show(&mut sui, &cols, cache);
    let qp = u16::try_from(query_port).unwrap_or(b.port);
    let gp = u16::try_from(game_port).unwrap_or(b.port);
    let pick = || {
        Act::Select(Selected {
            ip: b.ip.clone(),
            port: b.port,
            name: name.clone(),
        })
    };
    let Some(a) = out.action else { return };
    acts.push(match a {
        RowAction::Select | RowAction::Details | RowAction::Custom(BTN_DETAILS) => pick(),
        RowAction::Join => Act::Join(b.ip.clone(), b.port),
        RowAction::Custom(BTN_SHOW) => Act::ShowInList(b.ip.clone(), b.port),
        RowAction::Custom(BTN_DIRECT) | RowAction::DirectConnect => {
            Act::Direct(b.ip.clone(), gp, qp)
        }
        RowAction::Custom(_) | RowAction::ShowMods => return,
        RowAction::ToggleFavorite => Act::Favorite(name.clone(), b.ip.clone(), b.port, favorite),
        RowAction::Ping => Act::Ping(b.ip.clone(), query_port),
        RowAction::RefreshPlayers => Act::Players(b.ip.clone(), query_port),
        RowAction::CopyLink => Act::CopyLink(b.ip.clone(), gp, qp, name.clone()),
        RowAction::CopyAddress => Act::CopyAddress(format!("{}:{}", b.ip, gp)),
        RowAction::ToggleExclude => Act::Exclude(b.ip.clone(), !excluded),
    });
}
