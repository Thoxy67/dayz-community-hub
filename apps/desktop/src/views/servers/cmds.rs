//! The server browser's commands (registered in `commands.rs`): the filters, the order,
//! the selection, what a row or the panel does to a server. Arguments are hostile JSON:
//! read with care, clamped.
//!
//! The `list.*` commands act on the server their arguments name (`{"ip", "port"}`, the
//! query port or the game port), else on the browser's selection, else on the server the
//! panel shows (favourites, history): one set of commands for every server list.

use dz_hub::features::browser::{SortCol, Tri};
use serde_json::Value;

use super::detail::{self, DetailTab, ModsFilter};
use super::filters::{PINGS, sort_col, tri_from, tri_next};
use super::{Sel, UiState, index_of, shared};
use crate::app::Ctx;
use crate::i18n::t;
use crate::views::ViewId;

fn arg_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn arg_i64(a: &Value, k: &str) -> Option<i64> {
    a.get(k)
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.trim().parse().ok()))
}

fn arg_bool(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(Value::as_bool)
}

/// A port as a port.
fn port16(p: i64) -> u16 {
    u16::try_from(p).unwrap_or(0)
}

/// The server a command acts on.
#[derive(Clone, Debug)]
pub struct Target {
    pub ip: String,
    pub query_port: i64,
    pub game_port: i64,
    pub name: String,
    pub listed: bool,
}

/// From the arguments, else the selection, else the panel's server.
pub fn target(cx: &Ctx, a: &Value) -> Option<Target> {
    let servers = &cx.stores.servers;
    if let (Some(ip), Some(port)) = (arg_str(a, "ip"), arg_i64(a, "port")) {
        let ip: String = ip.trim().chars().take(255).collect();
        if let Some(r) = servers.row_of(&ip, port) {
            return Some(Target {
                ip,
                query_port: i64::from(r.query_port()),
                game_port: i64::from(r.game_port()),
                name: r.name().to_string(),
                listed: true,
            });
        }
        return Some(Target {
            game_port: arg_i64(a, "gamePort").unwrap_or(port),
            query_port: port,
            name: arg_str(a, "name").unwrap_or("").chars().take(256).collect(),
            ip,
            listed: false,
        });
    }
    let sel = shared(cx.egui).lock().sel.clone();
    if cx.nav.view == ViewId::Servers
        && let Some(s) = sel
    {
        return target(
            cx,
            &serde_json::json!({ "ip": s.ip, "port": s.port, "name": s.name }),
        );
    }
    let d = detail::shared(cx.egui).lock().target.clone()?;
    Some(Target {
        ip: d.ip,
        query_port: d.query_port,
        game_port: d.game_port,
        name: d.name,
        listed: d.listed,
    })
}

fn with_state(cx: &Ctx, f: impl FnOnce(&mut UiState)) {
    let s = shared(cx.egui);
    let mut st = s.lock();
    f(&mut st);
    cx.egui.request_repaint();
}

fn with_detail(cx: &Ctx, f: impl FnOnce(&mut detail::DetailState)) {
    let s = detail::shared(cx.egui);
    let mut st = s.lock();
    f(&mut st);
    cx.egui.request_repaint();
}

// ── the selection ───────────────────────────────────────────────────────────

/// Another view asked for one server: selected, details open, scrolled to.
pub fn focus(cx: &mut Ctx, ip: &str, port: i64) {
    let row = cx.stores.servers.row_of(ip, port).map(|r| {
        (
            r.ip().to_string(),
            i64::from(r.query_port()),
            r.name().to_string(),
            r.index,
        )
    });
    let order = cx.stores.servers.order().cloned();
    with_state(cx, |st| {
        st.show_detail = true;
        let (ip, port, name) = match &row {
            Some((ip, qp, name, _)) => (ip.clone(), *qp, name.clone()),
            None => (ip.to_string(), port, String::new()),
        };
        let index = order
            .as_ref()
            .and_then(|o| index_of(o, &ip, port, 0))
            .unwrap_or(0);
        if order.is_some() && row.is_some() {
            st.scroll_to = Some(index);
        }
        st.sel = Some(Sel {
            ip,
            port,
            name,
            index,
        });
    });
}

/// `servers.row.select {ip, port, index, detail}`.
pub fn select(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return;
    };
    let index = arg_i64(a, "index")
        .and_then(|i| usize::try_from(i).ok())
        .unwrap_or(0);
    let open = arg_bool(a, "detail").unwrap_or(false);
    with_state(cx, |st| {
        st.sel = Some(Sel {
            ip: tg.ip,
            port: tg.query_port,
            name: tg.name,
            index,
        });
        if open {
            st.show_detail = true;
        }
    });
}

/// `servers.row.show_mods`: picked, details open on the Mods tab.
pub fn show_mods(cx: &mut Ctx, a: &Value) {
    select(cx, a);
    with_state(cx, |st| st.show_detail = true);
    with_detail(cx, |d| d.tab = DetailTab::Mods);
}

/// Move the selection `delta` rows (clamped), scrolled into view.
fn move_by(cx: &mut Ctx, delta: i64) {
    let Some(o) = cx.stores.servers.order().cloned() else {
        return;
    };
    let n = o.total();
    if n == 0 {
        return;
    }
    with_state(cx, |st| {
        let cur = st
            .sel
            .as_ref()
            .and_then(|s| index_of(&o, &s.ip, s.port, s.index))
            .map_or(-1, |i| i as i64);
        let last = i64::try_from(n - 1).unwrap_or(i64::MAX);
        let i = cur.saturating_add(delta).clamp(0, last);
        let i = usize::try_from(i).unwrap_or(0);
        if let Some(r) = o.get(i) {
            st.sel = Some(Sel {
                ip: r.ip().to_string(),
                port: i64::from(r.query_port()),
                name: r.name().to_string(),
                index: i,
            });
            st.scroll_to = Some(i);
        }
    });
}

pub fn list_move(cx: &mut Ctx, a: &Value) {
    if let Some(k) = crate::views::saved::cmds::on_screen(cx) {
        return crate::views::saved::cmds::list_move(cx, k, a);
    }
    let d = arg_i64(a, "delta").unwrap_or(1).clamp(-100_000, 100_000);
    move_by(cx, d);
}

pub fn list_page(cx: &mut Ctx, a: &Value) {
    if let Some(k) = crate::views::saved::cmds::on_screen(cx) {
        return crate::views::saved::cmds::list_page(cx, k, a);
    }
    let rows = shared(cx.egui).lock().page_rows.max(1) as i64;
    let down = arg_bool(a, "down").unwrap_or(true);
    move_by(cx, if down { rows } else { -rows });
}

pub fn list_edges(cx: &mut Ctx, a: &Value) {
    if let Some(k) = crate::views::saved::cmds::on_screen(cx) {
        return crate::views::saved::cmds::list_edges(cx, k, a);
    }
    let end = arg_bool(a, "end").unwrap_or(false);
    move_by(cx, if end { i64::MAX / 2 } else { i64::MIN / 2 });
}

pub fn list_close(cx: &mut Ctx, a: &Value) {
    if let Some(k) = crate::views::saved::cmds::on_screen(cx) {
        return crate::views::saved::cmds::list_close(cx, k, a);
    }
    with_state(cx, |st| st.sel = None);
}

pub fn toggle_detail(cx: &mut Ctx, _: &Value) {
    with_state(cx, |st| st.show_detail = !st.show_detail);
}

// ── what is done to a server ────────────────────────────────────────────────

fn no_target(cx: &Ctx) {
    cx.stores.bus.warn(t("servers", "selectServer"));
}

pub fn join(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return no_target(cx);
    };
    let row = cx
        .stores
        .servers
        .row_of(&tg.ip, tg.query_port)
        .map(|r| r.to_row());
    match row {
        Some(r) => cx.join_row(r, None),
        None => cx.join_address(tg.ip, port16(tg.game_port), None),
    }
}

pub fn favorite(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return no_target(cx);
    };
    cx.stores
        .profile
        .toggle_favorite(&tg.name, &tg.ip, port16(tg.query_port));
    cx.stores.servers.touch();
}

pub fn ping(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return no_target(cx);
    };
    let (_, _, manual) = cx.stores.profile.ping_settings();
    cx.stores.servers.ping_one(&tg.ip, tg.query_port, manual);
}

pub fn refresh_live(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return no_target(cx);
    };
    let s = &mut *cx.stores;
    s.server_data.refresh_a2s(&s.servers, &tg.ip, tg.query_port);
}

pub fn direct(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return no_target(cx);
    };
    let (qp, gp) = (port16(tg.query_port), port16(tg.game_port));
    // A favourite's saved password goes along.
    let password = cx
        .stores
        .profile
        .favorites
        .iter()
        .find(|f| f.ip == tg.ip && (f.port == qp || f.port == gp))
        .and_then(|f| f.password.clone());
    cx.stores
        .connect
        .open_in_direct(tg.ip, gp, Some(qp), password);
}

/// C: the address to paste into the game or a chat, "ip:port".
pub fn copy_address(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return no_target(cx);
    };
    let address = format!("{}:{}", tg.ip, tg.game_port);
    cx.egui.copy_text(address.clone());
    cx.stores
        .bus
        .ok(format!("{} · {address}", t("common", "copied")));
}

pub fn copy_link(cx: &mut Ctx, a: &Value) {
    let Some(tg) = target(cx, a) else {
        return no_target(cx);
    };
    cx.stores.connect.copy_link(
        cx.egui,
        &tg.ip,
        port16(tg.game_port),
        Some(port16(tg.query_port)),
        Some(&tg.name),
    );
}

pub fn exclude(cx: &mut Ctx, a: &Value) {
    let ip = match arg_str(a, "ip") {
        Some(ip) => ip.trim().chars().take(255).collect::<String>(),
        None => match target(cx, a) {
            Some(tg) => tg.ip,
            None => return no_target(cx),
        },
    };
    if ip.is_empty() {
        return;
    }
    let p = &mut cx.stores.profile;
    if p.is_excluded(&ip) {
        p.unexclude_ip(&ip);
    } else {
        p.exclude_ip(&ip);
    }
    cx.stores.servers.touch();
}

// ── the filters and the order ───────────────────────────────────────────────

fn filters(cx: &mut Ctx, f: impl FnOnce(&mut super::Filters)) {
    with_state(cx, |st| f(&mut st.f));
}

pub fn sort_by(cx: &mut Ctx, a: &Value) {
    let Some(col) = arg_str(a, "col").and_then(sort_col) else {
        return;
    };
    filters(cx, |f| match arg_bool(a, "asc") {
        Some(asc) => {
            f.sort = col;
            f.asc = asc;
        }
        None => f.toggle_sort(col),
    });
}

pub fn sort_busiest(cx: &mut Ctx, _: &Value) {
    filters(cx, |f| {
        f.sort = SortCol::Players;
        f.asc = false;
    });
}

pub fn sort_closest(cx: &mut Ctx, _: &Value) {
    filters(cx, |f| {
        f.sort = SortCol::Ping;
        f.asc = true;
    });
}

pub fn toggle_hide_full(cx: &mut Ctx, _: &Value) {
    filters(cx, |f| f.hide_full = !f.hide_full);
}

pub fn toggle_hide_empty(cx: &mut Ctx, _: &Value) {
    filters(cx, |f| f.hide_empty = !f.hide_empty);
}

pub fn toggle_modded_only(cx: &mut Ctx, _: &Value) {
    filters(cx, |f| {
        f.modded = if f.modded == Tri::Only {
            Tri::All
        } else {
            Tri::Only
        };
    });
}

pub fn search(cx: &mut Ctx, a: &Value) {
    let text: String = arg_str(a, "text").unwrap_or("").chars().take(200).collect();
    filters(cx, |f| f.search = text);
    cx.go(ViewId::Servers);
}

/// `servers.filter.cycle {key, value?}`: the next answer, or the one named.
pub fn cycle(cx: &mut Ctx, a: &Value) {
    let key = arg_str(a, "key").unwrap_or("").to_string();
    cycle_key(cx, &key, a);
}

fn cycle_key(cx: &mut Ctx, key: &str, a: &Value) {
    let value = arg_str(a, "value").and_then(tri_from);
    filters(cx, |f| {
        if let Some(t) = f.tri_mut(key) {
            *t = value.unwrap_or_else(|| tri_next(*t));
        }
    });
}

pub fn cycle_1pp(cx: &mut Ctx, a: &Value) {
    cycle_key(cx, "fp", a);
}

pub fn cycle_password(cx: &mut Ctx, a: &Value) {
    cycle_key(cx, "pwd", a);
}

pub fn cycle_battleye(cx: &mut Ctx, a: &Value) {
    cycle_key(cx, "be", a);
}

pub fn cycle_modded(cx: &mut Ctx, a: &Value) {
    cycle_key(cx, "mods", a);
}

pub fn cycle_official(cx: &mut Ctx, a: &Value) {
    cycle_key(cx, "off", a);
}

/// `servers.filter.map {maps: [..]}`, or the old `{map: "enoch" | null}`: the maps shown
/// (none for all). `map` may be a list too.
pub fn map(cx: &mut Ctx, a: &Value) {
    let maps = map_args(a);
    filters(cx, |f| f.maps = maps);
}

/// Most maps a filter holds.
const MAPS_MAX: usize = 64;

/// The maps named by `maps` or `map` (a name, a list, or null), trimmed, each once
/// whatever its case, bounded.
pub fn map_args(a: &Value) -> Vec<String> {
    let v = a
        .get("maps")
        .or_else(|| a.get("map"))
        .unwrap_or(&Value::Null);
    let names: Vec<&str> = match v {
        Value::String(s) => vec![s.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let n: String = n.trim().chars().take(128).collect();
        if n.is_empty() || out.iter().any(|o| o.to_lowercase() == n.to_lowercase()) {
            continue;
        }
        out.push(n);
        if out.len() >= MAPS_MAX {
            break;
        }
    }
    out
}

pub fn more_popover(cx: &mut Ctx, _: &Value) {
    with_state(cx, |st| st.open_more = true);
    cx.go(ViewId::Servers);
}

pub fn max_ping(cx: &mut Ctx, a: &Value) {
    let ms = arg_i64(a, "ms").unwrap_or(0);
    let ms = PINGS
        .iter()
        .copied()
        .find(|p| i64::from(*p) == ms)
        .unwrap_or_else(|| u32::try_from(ms.clamp(0, 9999)).unwrap_or(0));
    filters(cx, |f| f.max_ping = ms);
}

pub fn hide_empty(cx: &mut Ctx, a: &Value) {
    let on = arg_bool(a, "on");
    filters(cx, |f| f.hide_empty = on.unwrap_or(!f.hide_empty));
}

pub fn hide_full(cx: &mut Ctx, a: &Value) {
    let on = arg_bool(a, "on");
    filters(cx, |f| f.hide_full = on.unwrap_or(!f.hide_full));
}

pub fn toggle_excluded(cx: &mut Ctx, _: &Value) {
    filters(cx, |f| f.show_excluded = !f.show_excluded);
}

pub fn remove_one(cx: &mut Ctx, a: &Value) {
    let key = arg_str(a, "key").unwrap_or("").to_string();
    filters(cx, |f| f.remove(&key));
}

pub fn clear(cx: &mut Ctx, _: &Value) {
    filters(cx, super::Filters::clear);
}

pub fn sources_details(cx: &mut Ctx, _: &Value) {
    with_state(cx, |st| st.open_sources = true);
    cx.go(ViewId::Servers);
}

// ── the panel ───────────────────────────────────────────────────────────────

/// `servers.detail.resize {width}`: the panel's width (320–620), remembered.
pub fn resize(cx: &mut Ctx, a: &Value) {
    let w = a
        .get("width")
        .and_then(Value::as_f64)
        .filter(|w| w.is_finite())
        .unwrap_or(400.0)
        .clamp(320.0, 620.0) as f32;
    cx.egui
        .data_mut(|d| d.insert_persisted(egui::Id::new("servers-detail"), w));
    cx.egui.request_repaint();
}

pub fn tab(cx: &mut Ctx, a: &Value) {
    if let Some(tab) = arg_str(a, "tab").and_then(DetailTab::from_id) {
        with_detail(cx, |d| d.tab = tab);
    }
}

pub fn detail_copy_address(cx: &mut Ctx, a: &Value) {
    copy_address(cx, a);
}

/// The mods that are missing or behind, then the game: the join flow does both.
pub fn download_and_join(cx: &mut Ctx, a: &Value) {
    join(cx, a);
}

fn ids(a: &Value, fallback: impl FnOnce() -> Vec<u64>) -> Vec<u64> {
    match a.get("ids").and_then(Value::as_array) {
        Some(v) => v.iter().filter_map(Value::as_u64).take(10_000).collect(),
        None => fallback(),
    }
}

pub fn install_missing(cx: &mut Ctx, a: &Value) {
    let d = detail::shared(cx.egui);
    let ids = ids(a, || {
        d.lock()
            .target
            .as_ref()
            .map(|t| t.missing.clone())
            .unwrap_or_default()
    });
    if ids.is_empty() {
        return;
    }
    let via = cx.stores.profile.via_steam();
    cx.stores.mods.install(ids, via);
}

pub fn update_stale(cx: &mut Ctx, a: &Value) {
    let d = detail::shared(cx.egui);
    let ids = ids(a, || {
        d.lock()
            .target
            .as_ref()
            .map(|t| t.stale.clone())
            .unwrap_or_default()
    });
    if ids.is_empty() {
        return;
    }
    let via = cx.stores.profile.via_steam();
    cx.stores.mods.update_many(ids, via);
}

pub fn search_players(cx: &mut Ctx, a: &Value) {
    let text: String = arg_str(a, "text").unwrap_or("").chars().take(100).collect();
    with_detail(cx, |d| {
        d.players_q = text;
        d.tab = DetailTab::Players;
    });
}

pub fn search_rules(cx: &mut Ctx, a: &Value) {
    let text: String = arg_str(a, "text").unwrap_or("").chars().take(100).collect();
    with_detail(cx, |d| {
        d.rules_q = text;
        d.tab = DetailTab::Rules;
    });
}

pub fn mods_filter(cx: &mut Ctx, a: &Value) {
    let f = match arg_str(a, "filter") {
        Some("missing") => ModsFilter::Missing,
        Some("stale") => ModsFilter::Stale,
        _ => ModsFilter::All,
    };
    with_detail(cx, |d| {
        d.mods_filter = f;
        d.tab = DetailTab::Mods;
    });
}

pub fn refresh_stats(cx: &mut Ctx, _: &Value) {
    let Some(tg) = detail::shared(cx.egui).lock().target.clone() else {
        return;
    };
    cx.stores.server_data.fetch_metrics(
        &tg.ip,
        u32::try_from(tg.game_port).unwrap_or(0),
        u32::try_from(tg.query_port).unwrap_or(0),
        &tg.name,
        true,
    );
}

pub fn stats_range(cx: &mut Ctx, a: &Value) {
    if let Some(r) = arg_str(a, "range").and_then(detail::stats_range_of) {
        with_detail(cx, |d| {
            d.range = r;
            d.tab = DetailTab::Stats;
        });
    }
}

/// A page the server lists, in the system browser: web addresses only.
pub fn open_link(cx: &mut Ctx, a: &Value) {
    let Some(url) = arg_str(a, "url").map(str::trim) else {
        return;
    };
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.len() > 2048 {
        return;
    }
    if let Err(e) = open::that_detached(url) {
        cx.stores.bus.err(e.to_string());
    }
}
