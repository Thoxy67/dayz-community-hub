//! The one long list (the list pane of `View.svelte`, `ServerRow.svelte`): the sticky
//! heading that sorts, then every matching server as a 48px row, of which only those on
//! screen are drawn, read by index from the hub's snapshot through the order it answered.
//! Nothing is paged: scrolling 9 500 rows costs the rows on screen, laid out once per
//! server in the view's [`RowCache`].

use dz_hub::features::browser::SortCol;
use egui::{Pos2, Rect, Ui, Vec2};
use serde_json::{Value, json};

use super::{Run, UiState};
use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::{Button, Skeleton, Variant, VirtualList};
use crate::parts::{
    Empty, ListColumns, ROW_PX, RowAction, RowButton, RowCache, RowData, ServerListRow, SortHead,
    TableHead, exclude_label, head_cell,
};
use crate::theme::TokensExt;

const HEAD_H: f32 = 28.0;
/// Rows laid out past each edge of the screen.
const OVERSCAN: usize = 8;

/// The heading: ping, players, server; map and time together; mods.
fn heading(ui: &mut Ui, st: &UiState, run: &mut Run) {
    let f = &st.f;
    TableHead::new(false).show(ui, |ui, cols| {
        let mut head = |ui: &mut Ui, label: &str, col: SortCol, tip: &str| {
            let r = ui.add(SortHead::new(label, f.sort == col, f.asc).tip(tip));
            if r.clicked() {
                run.push(("servers.sort_by", json!({ "col": col_id(col) })));
            }
        };
        head_cell(ui, cols.ping, |ui| {
            head(ui, t("servers", "colPing"), SortCol::Ping, "")
        });
        head_cell(ui, cols.players, |ui| {
            head(ui, t("servers", "colPlayers"), SortCol::Players, "")
        });
        head_cell(ui, cols.server, |ui| {
            head(ui, t("servers", "colServer"), SortCol::Name, "")
        });
        if let Some(m) = cols.map {
            head_cell(ui, m, |ui| {
                head(ui, t("servers", "colMap"), SortCol::Map, "");
                head(
                    ui,
                    t("servers", "colTime"),
                    SortCol::Time,
                    t("servers", "colTimeTitle"),
                );
            });
        }
        head_cell(ui, cols.mods, |ui| {
            head(ui, t("servers", "colMods"), SortCol::Mods, "")
        });
    });
}

pub fn col_id(c: SortCol) -> &'static str {
    match c {
        SortCol::None => "none",
        SortCol::Ping => "ping",
        SortCol::Players => "players",
        SortCol::Name => "name",
        SortCol::Map => "map",
        SortCol::Mods => "mods",
        SortCol::Time => "time",
    }
}

/// The row's shape while the first list comes: cells, not a spinner.
fn placeholder(ui: &mut Ui, cols: &ListColumns, rect: Rect) {
    let tk = ui.tokens();
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, crate::kit::paint::alpha(tk.border, 0.5)),
    );
    let cy = rect.center().y;
    let bar =
        |x: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(x, cy - h / 2.0), Vec2::new(w, h));
    Skeleton::paint(ui, bar(cols.ping.min, 48.0, 12.0), None, 1.0);
    Skeleton::paint(ui, bar(cols.players.min, 64.0, 12.0), None, 1.0);
    Skeleton::paint(
        ui,
        bar(cols.server.min, cols.server.span() * 0.6, 10.0),
        None,
        1.0,
    );
    if let Some(m) = cols.map {
        Skeleton::paint(ui, bar(m.min, 80.0, 12.0), None, 1.0);
    }
}

/// The list pane: loading shape, the list's error, nothing matching, or the rows.
pub fn show(ui: &mut Ui, cx: &Ctx, st: &mut UiState, cache: &mut RowCache, run: &mut Run) {
    let servers = &cx.stores.servers;
    let full = ui.available_rect_before_wrap();
    let order = servers.order().cloned();
    let loading =
        order.is_none() && (servers.loading || !cx.stores.app.initialized || servers.ordering());
    if servers.total == 0
        && order.as_ref().is_none_or(|o| o.total() == 0)
        && let Some(err) = servers.list_error.clone().filter(|_| !loading)
    {
        heading(ui, st, run);
        let refreshing = servers.refreshing;
        Empty::new()
            .icon(lucide::CLOUD_OFF)
            .title(t("servers", "listUnavailable"))
            .error(&err)
            .text(t("servers", "listUnavailableHint"))
            .show(ui, |ui| {
                let b = Button::new(t("servers", "refresh"))
                    .icon(lucide::REFRESH_CW)
                    .spinner(refreshing)
                    .enabled(!refreshing)
                    .variant(Variant::Accent);
                if ui.add(b).clicked() {
                    run.push(("servers.refresh", Value::Null));
                }
            });
        return;
    }
    let Some(order) = order else {
        heading(ui, st, run);
        let cols = ListColumns::new(full.x_range(), false);
        let top = ui.cursor().top();
        for i in 0..14 {
            let r = Rect::from_min_size(
                Pos2::new(full.left(), top + i as f32 * ROW_PX),
                Vec2::new(full.width(), ROW_PX),
            );
            if r.top() > full.bottom() {
                break;
            }
            placeholder(ui, &cols, r);
        }
        return;
    };
    if order.total() == 0 {
        heading(ui, st, run);
        if st.f.active() {
            // Which filters hide everything, in the chips' words, and the way out.
            let names: Vec<String> = super::filters::chips(&st.f)
                .into_iter()
                .map(|(_, w)| w)
                .collect();
            let why = tf(
                "servers",
                "noMatchFilters",
                &[("filters", &names.join(" · "))],
            );
            Empty::new()
                .icon(lucide::SEARCH_X)
                .title(t("servers", "noMatch"))
                .text(&why)
                .show(ui, |ui| {
                    if ui
                        .add(Button::new(t("servers", "clearFilters")).variant(Variant::Accent))
                        .clicked()
                    {
                        run.push(("servers.filter.clear", Value::Null));
                    }
                });
        } else {
            // No filter, no server: the sources brought nothing (yet); ask again.
            let refreshing = servers.refreshing;
            Empty::new()
                .icon(lucide::SERVER)
                .title(t("servers", "listEmpty"))
                .text(t("servers", "listEmptyText"))
                .show(ui, |ui| {
                    let b = Button::new(t("servers", "refresh"))
                        .icon(lucide::REFRESH_CW)
                        .spinner(refreshing)
                        .enabled(!refreshing)
                        .variant(Variant::Accent);
                    if ui.add(b).clicked() {
                        run.push(("servers.refresh", Value::Null));
                    }
                });
        }
        return;
    }

    let total = order.total();
    let offset = st.scroll_top.take();
    let scroll_to = st.scroll_to.take();
    let sel = st.sel.clone();
    let friends = &cx.stores.friends;
    let has_friends = !friends.list().is_empty();
    let pending = &servers.pending;
    let data = &cx.stores.server_data;
    let profile = &cx.stores.profile;
    let direct = t("servers", "directConnect");
    let mut acts: Vec<(RowAction, usize)> = Vec::new();
    let out = VirtualList::new("servers-list", total)
        .row_height(ROW_PX)
        .overscan(OVERSCAN)
        .header(HEAD_H)
        .offset(offset)
        .scroll_to(scroll_to)
        .cursor(
            sel.as_ref()
                .and_then(|s| super::index_of(&order, &s.ip, s.port, s.index)),
        )
        .show(
            ui,
            |ui| heading(ui, st, run),
            |ui, i| {
                let Some(r) = order.get(i) else {
                    return;
                };
                let mut d = RowData::from_server(&r);
                let key = format!("{}:{}", d.ip, d.query_port);
                d.ping_pending = pending.contains(&key);
                d.players_loading = data.a2s_loading(&key);
                // The profile's own state, changed at once by a click (the snapshot's
                // flags follow when the hub has written it).
                let (qp, gp) = (
                    u16::try_from(d.query_port).unwrap_or(0),
                    u16::try_from(d.game_port).unwrap_or(0),
                );
                d.favorite = profile.is_favorite(d.ip, qp) || profile.is_favorite(d.ip, gp);
                d.excluded = profile.is_excluded(d.ip);
                let selected = sel
                    .as_ref()
                    .is_some_and(|s| s.port == d.query_port && s.ip == d.ip);
                let names: Vec<String> = if has_friends {
                    let port = u16::try_from(d.query_port).unwrap_or(0);
                    friends.on(d.ip, port).map(|f| f.name.clone()).collect()
                } else {
                    Vec::new()
                };
                let ex = exclude_label(d.ip, d.excluded);
                let tk = ui.tokens();
                let buttons = [
                    RowButton {
                        id: BTN_DIRECT,
                        icon: lucide::PLUG_ZAP,
                        label: direct,
                        danger: false,
                        tint: None,
                    },
                    RowButton {
                        id: BTN_EXCLUDE,
                        icon: lucide::BAN,
                        label: &ex,
                        danger: true,
                        tint: d.excluded.then_some(tk.err),
                    },
                ];
                let w = ui.available_width();
                let cols = ListColumns::new(
                    egui::Rangef::new(ui.max_rect().left(), ui.max_rect().left() + w),
                    false,
                );
                let resp = ServerListRow::new(d)
                    .selected(selected)
                    .friends(&names)
                    .buttons(&buttons)
                    .show(ui, &cols, cache);
                if let Some(a) = resp.action {
                    acts.push((a, i));
                }
            },
        );
    st.page_rows = ((out.viewport.height() / ROW_PX).floor() as usize).max(1);
    st.list_offset = out.offset;
    for (a, i) in acts {
        let Some(r) = order.get(i) else {
            continue;
        };
        let target = json!({
            "ip": r.ip(),
            "port": r.query_port(),
            "gamePort": r.game_port(),
            "name": r.name(),
            "index": i,
        });
        let id = match a {
            RowAction::Select | RowAction::Details => "servers.row.select",
            RowAction::Join => "list.join",
            RowAction::ToggleFavorite => "list.fav",
            RowAction::Ping => "list.ping",
            RowAction::RefreshPlayers => "servers.row.refresh_players",
            RowAction::ShowMods => "servers.row.show_mods",
            RowAction::CopyLink => "list.link",
            RowAction::CopyAddress => "servers.row.copy_address",
            RowAction::DirectConnect | RowAction::Custom(BTN_DIRECT) => "list.direct",
            RowAction::ToggleExclude | RowAction::Custom(BTN_EXCLUDE) => "servers.row.exclude_ip",
            RowAction::Custom(_) => continue,
        };
        let mut args = target;
        if a == RowAction::Details
            && let Some(o) = args.as_object_mut()
        {
            o.insert("detail".into(), Value::Bool(true));
        }
        run.push((id, args));
    }
}

const BTN_DIRECT: u32 = 1;
const BTN_EXCLUDE: u32 = 2;
