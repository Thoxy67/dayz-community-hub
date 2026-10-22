//! The list pane (= `listPane` and its snippets in `View.svelte`): sticky group heads,
//! friend rows (avatar, status dot, name, what they are doing, Steam buttons on hover)
//! and, under each DayZ block, its server: indented, hung from its friends by a guide,
//! the block set apart from the next. The shared server row when the list knows the
//! server, else a short row (address, why, Join). Only the rows on screen are drawn; the
//! model gives every row's place.

use std::sync::Arc;

use dz_hub::features::friends::{FriendDto, FriendStatus};
use egui::text::{LayoutJob, TextFormat};
use egui::{CornerRadius, Pos2, Rangef, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};

use super::model::{Block, Group, HEAD_H, Item, Model, SERVER_H, SHORT_H, Tie};
use super::{Act, now_unix};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{IconButton, SectionLabel, Size, Tag, TipExt, Tone, menu};
use crate::parts::{
    Empty, JoinButton, ListColumns, RowAction, RowButton, RowCache, RowData, ServerListRow, image,
    server_menu,
};
use crate::store::friends::Selected;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// The avatar's side; the guide runs down its centre.
const AVATAR: f32 = 32.0;
/// The guide's x, from the row's left edge: under the avatars' centres.
const GUIDE_X: f32 = size::PAD + AVATAR / 2.0;
/// The server row sits under its friends, right of the guide's elbow.
const SERVER_INDENT: f32 = GUIDE_X + 14.0;
/// The guide's room around an avatar.
const GUIDE_GAP: f32 = 3.0;
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
                    Item::Friend(i, tie) => {
                        if let Some(f) = list.get(i) {
                            friend_row(&mut rui, r, cx, f, tie, acts);
                        }
                    }
                    Item::Server { block, full } => {
                        if let Some(b) = model.blocks.get(block) {
                            // The row on top, the block's room under it.
                            let h = if full { SERVER_H } else { SHORT_H };
                            let row = Rect::from_min_size(r.min, Vec2::new(w, h));
                            guide_elbow(&rui, row);
                            if full {
                                server_row(&mut rui, row, cx, b, cache, acts);
                            } else {
                                short_row(&mut rui, row, cx, b, acts);
                            }
                            rui.painter().hline(
                                r.x_range(),
                                r.bottom() - 0.5,
                                Stroke::new(1.0, alpha(tk.border, 0.5)),
                            );
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
fn friend_row(ui: &mut Ui, r: Rect, cx: &Ctx, f: &FriendDto, tie: Tie, acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    let id = ui.id().with(("friend", &f.steam_id));
    let row = ui.interact(r, id, Sense::hover());
    let hovered = ui.rect_contains_pointer(r);
    let hover = paint::ease(ui, id.with("hover"), hovered);
    if hover > 0.0 {
        ui.painter()
            .rect_filled(r, 0.0, alpha(tk.raised, 0.5 * hover));
    }
    let cy = r.center().y;
    // A friend on a server and that server are one block: no line parts them, the guide
    // joins them (from under the first avatar, through the others, down to the server).
    match tie {
        Tie::Alone => {
            ui.painter().hline(
                r.x_range(),
                r.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.5)),
            );
        }
        Tie::First | Tie::Next => {
            let x = r.left() + GUIDE_X;
            ui.painter().vline(
                x,
                Rangef::new(cy + AVATAR / 2.0 + GUIDE_GAP, r.bottom()),
                guide_stroke(&tk),
            );
            if tie == Tie::Next {
                ui.painter().vline(
                    x,
                    Rangef::new(r.top(), cy - AVATAR / 2.0 - GUIDE_GAP),
                    guide_stroke(&tk),
                );
            }
        }
    }
    let _ = row;
    if f.status == FriendStatus::Offline && f.game.is_none() {
        ui.multiply_opacity(0.6);
    }
    let p = ui.painter().clone();

    // The avatar, or the first letter on a plate.
    let av = Rect::from_min_size(
        Pos2::new(r.left() + size::PAD, cy - AVATAR / 2.0),
        Vec2::splat(AVATAR),
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

/// The guide's stroke: the accent, quieted, so it reads as a link and not as a control.
fn guide_stroke(tk: &Tokens) -> Stroke {
    Stroke::new(1.5, alpha(tk.accent, 0.7))
}

/// The guide's last stretch, in a server's `row`: down from the friend above, then right
/// into the indented row, so the server hangs under its friends.
fn guide_elbow(ui: &Ui, row: Rect) {
    let tk = ui.tokens();
    let x = row.left() + GUIDE_X;
    let cy = row.center().y;
    let s = guide_stroke(&tk);
    let p = ui.painter();
    p.vline(x, Rangef::new(row.top(), cy), s);
    p.hline(Rangef::new(x, row.left() + SERVER_INDENT), cy, s);
    p.circle_filled(egui::pos2(x, cy), s.width / 2.0, s.color);
}

/// The server's part of `row`, right of the guide.
fn indented(row: Rect) -> Rect {
    Rect::from_min_max(Pos2::new(row.left() + SERVER_INDENT, row.top()), row.max)
}

/// Whether the details show this block's server.
fn is_selected(cx: &Ctx, b: &Block) -> bool {
    cx.stores
        .friends
        .selected
        .as_ref()
        .is_some_and(|s| s.ip == b.ip && s.port == b.port)
}

/// What a server row's action asks for. `query_port` and `game_port` as the list knows
/// them (else the friends' port).
struct Target<'a> {
    b: &'a Block,
    name: &'a str,
    query_port: i64,
    game_port: i64,
    favorite: bool,
    excluded: bool,
}

impl Target<'_> {
    fn act(&self, a: RowAction) -> Option<Act> {
        let b = self.b;
        let qp = u16::try_from(self.query_port).unwrap_or(b.port);
        let gp = u16::try_from(self.game_port).unwrap_or(b.port);
        let name = self.name.to_string();
        Some(match a {
            RowAction::Select | RowAction::Details | RowAction::Custom(BTN_DETAILS) => {
                Act::Select(Selected {
                    ip: b.ip.clone(),
                    port: b.port,
                    name,
                })
            }
            RowAction::Join => Act::Join(b.ip.clone(), b.port),
            RowAction::Custom(BTN_SHOW) => Act::ShowInList(b.ip.clone(), b.port),
            RowAction::Custom(BTN_DIRECT) | RowAction::DirectConnect => {
                Act::Direct(b.ip.clone(), gp, qp)
            }
            RowAction::Custom(_) | RowAction::ShowMods => return None,
            RowAction::ToggleFavorite => Act::Favorite(name, b.ip.clone(), b.port, self.favorite),
            RowAction::Ping => Act::Ping(b.ip.clone(), self.query_port),
            RowAction::RefreshPlayers => Act::Players(b.ip.clone(), self.query_port),
            RowAction::CopyLink => Act::CopyLink(b.ip.clone(), gp, qp, name),
            RowAction::CopyAddress => Act::CopyAddress(format!("{}:{}", b.ip, gp)),
            RowAction::ToggleExclude => Act::Exclude(b.ip.clone(), !self.excluded),
        })
    }
}

/// The direct-connect button, as every friend's server offers it.
fn direct_button() -> RowButton<'static> {
    RowButton {
        id: BTN_DIRECT,
        icon: lucide::PLUG_ZAP,
        label: t("servers", "directConnect"),
        danger: false,
        tint: None,
    }
}

/// The server under a DayZ block, as every server list shows it (the list knows it).
fn server_row(
    ui: &mut Ui,
    row: Rect,
    cx: &Ctx,
    b: &Block,
    cache: &mut RowCache,
    acts: &mut Vec<Act>,
) {
    let port = i64::from(b.port);
    let Some(lr) = cx.stores.servers.row(&b.ip, port) else {
        // The list lost it since the model was built: the short row until it is rebuilt.
        short_row(ui, row, cx, b, acts);
        return;
    };
    let tk = ui.tokens();
    let rr = indented(row);
    ui.painter().rect_filled(rr, 0.0, alpha(tk.bg, 0.4));
    let mut sui = ui.new_child(
        UiBuilder::new()
            .max_rect(rr)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let cols = ListColumns::new(Rangef::new(rr.left(), rr.right()), false);
    let buttons = [
        RowButton {
            id: BTN_SHOW,
            icon: lucide::LIST,
            label: t("friends", "showInList"),
            danger: false,
            tint: None,
        },
        direct_button(),
    ];
    let key = b.key();
    let d = RowData::from_server(&lr);
    let name = d.name.to_string();
    let (query_port, game_port) = (d.query_port, d.game_port);
    let data = RowData {
        join_port: port,
        listed: b.listed,
        ping_pending: cx.stores.servers.pending.contains(&key),
        ..d
    };
    let target = Target {
        b,
        name: &name,
        query_port,
        game_port,
        favorite: data.favorite,
        excluded: data.excluded,
    };
    let out = ServerListRow::new(data)
        .selected(is_selected(cx, b))
        .friends(&b.here)
        .buttons(&buttons)
        .mods_clickable(false)
        .show(&mut sui, &cols, cache);
    if let Some(a) = out.action.and_then(|a| target.act(a)) {
        acts.push(a);
    }
}

/// The server of a block the list does not know: ping, players, map and mods would be
/// blanks, so the row is short. Its address (and its name, when Steam gave one), why it
/// is short, Join; Details and Direct connect on hover, the server menu on a right click.
fn short_row(ui: &mut Ui, row: Rect, cx: &Ctx, b: &Block, acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    let rr = indented(row);
    let key = b.key();
    let id = ui.id().with(("friend-server", &key));
    let resp = ui.interact(rr, id, Sense::click());
    let selected = is_selected(cx, b);
    let hovered = resp.contains_pointer() || resp.has_focus();
    let hover = paint::ease(ui, id.with("hover"), hovered);
    {
        let p = ui.painter();
        p.rect_filled(rr, 0.0, alpha(tk.bg, 0.4));
        let ground = if selected {
            alpha(tk.accent, 0.10)
        } else {
            alpha(tk.raised, 0.5 * hover)
        };
        if ground.a() > 0 {
            p.rect_filled(rr, 0.0, ground);
        }
        if selected {
            p.vline(rr.left() + 1.0, rr.y_range(), Stroke::new(2.0, tk.accent));
        }
        p.hline(
            rr.x_range(),
            rr.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.5)),
        );
    }
    let cols = ListColumns::new(Rangef::new(rr.left(), rr.right()), false);
    let cy = rr.center().y;
    let favorite = cx.stores.profile.is_favorite(&b.ip, b.port);
    let excluded = cx.stores.profile.is_excluded(&b.ip);
    let address = key;
    let name = b
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| address.clone());
    let target = Target {
        b,
        name: &name,
        query_port: i64::from(b.port),
        game_port: i64::from(b.port),
        favorite,
        excluded,
    };
    let mut action = None;
    if resp.clicked() {
        action = Some(RowAction::Select);
    }
    if resp.double_clicked() {
        action = Some(RowAction::Join);
    }

    // Join, where the full rows have it.
    let join = Rect::from_min_size(
        Pos2::new(cols.actions.max - size::CONTROL, cy - size::CONTROL / 2.0),
        Vec2::splat(size::CONTROL),
    );
    if ui
        .put(
            join,
            JoinButton::new()
                .compact()
                .quiet()
                .lit(selected)
                .group_hover(hovered),
        )
        .clicked()
    {
        action = Some(RowAction::Join);
    }

    // Details and Direct connect, on hover (as the full rows' actions).
    let shown = paint::ease(ui, id.with("actions"), hovered || selected);
    let mut text_end = join.left() - 8.0;
    if shown > 0.0 {
        let bw = size::CONTROL_SM;
        let gw = 2.0 + 2.0 * bw + 1.0 + 2.0;
        let g = Rect::from_min_size(
            Pos2::new(join.left() - 6.0 - gw, cy - (bw + 4.0) / 2.0),
            Vec2::new(gw, bw + 4.0),
        );
        text_end = g.left() - 8.0;
        let mut gui = ui.new_child(
            UiBuilder::new()
                .max_rect(g.shrink(2.0))
                .layout(egui::Layout::left_to_right(egui::Align::Center))
                .id_salt("short-actions"),
        );
        gui.multiply_opacity(shown);
        gui.spacing_mut().item_spacing.x = 1.0;
        paint::plate(gui.painter(), g, size::RADIUS_SM, tk.panel, tk.border);
        let direct = direct_button();
        if gui
            .add(IconButton::new(lucide::SCAN_SEARCH, t("friends", "details")).size(Size::IconXs))
            .clicked()
        {
            action = Some(RowAction::Custom(BTN_DETAILS));
        }
        if gui
            .add(IconButton::new(direct.icon, direct.label).size(Size::IconXs))
            .clicked()
        {
            action = Some(RowAction::Custom(direct.id));
        }
    }

    // The address (after Steam's name for it, when there is one), then why it is short,
    // from the row's start: the columns they would line up with are not drawn.
    let x0 = rr.left() + size::PAD;
    let room = (text_end - x0).max(0.0);
    let mut tui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                Pos2::new(x0, rr.top()),
                Pos2::new(x0 + room, rr.bottom()),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt("short-text"),
    );
    tui.set_clip_rect(tui.max_rect().intersect(ui.clip_rect()));
    tui.spacing_mut().item_spacing.x = 8.0;
    let tag_w = if b.listed { 0.0 } else { 160.0 };
    if name != address {
        let g = paint::galley_trunc(
            &tui,
            &name,
            font(Face::SansSemibold, size::TEXT_XS),
            tk.fg,
            (room - tag_w - 160.0).max(40.0),
        );
        let (r, _) = tui.allocate_exact_size(g.size(), Sense::hover());
        tui.painter().galley(r.min, g, tk.fg);
    }
    let ink = if b.listed { tk.fg_muted } else { tk.fg };
    let g = paint::galley(&tui, &address, font(Face::Mono, size::TEXT_XS), ink);
    let (r, _) = tui.allocate_exact_size(g.size(), Sense::hover());
    tui.painter().galley(r.min, g, ink);
    if !b.listed {
        tui.add(Tag::new(t("friends", "unlisted")).tone(Tone::Warn));
    }

    if let Some(inner) = menu::context(&resp, |ui| {
        server_menu(ui, &b.ip, favorite, excluded, false, &[])
    }) && let Some(a) = inner.inner
    {
        action = Some(a);
    }
    if let Some(a) = action.and_then(|a| target.act(a)) {
        acts.push(a);
    }
}
