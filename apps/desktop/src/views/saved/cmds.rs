//! A saved list's commands (registered in `commands.rs` per list: `favorites.*`), and the
//! `list.move/page/edges/close` keys when a saved list is on screen. Arguments are hostile
//! JSON: read with care, clamped.

use serde_json::Value;

use super::{Kind, State, shared};
use crate::app::Ctx;

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

fn with_state(cx: &Ctx, kind: Kind, f: impl FnOnce(&mut State)) {
    let s = shared(cx.egui, kind);
    let mut st = s.lock();
    f(&mut st);
    cx.egui.request_repaint();
}

/// Move the selection `delta` rows through the rows shown (clamped), scrolled to.
pub fn move_by(st: &mut State, delta: i64) {
    let n = st.shown.len();
    if n == 0 {
        return;
    }
    let cur = st
        .sel
        .as_ref()
        .and_then(|k| st.shown.iter().position(|s| s == k))
        .map_or(-1, |i| i64::try_from(i).unwrap_or(-1));
    let last = i64::try_from(n - 1).unwrap_or(i64::MAX);
    let i = usize::try_from(cur.saturating_add(delta).clamp(0, last)).unwrap_or(0);
    if let Some(k) = st.shown.get(i) {
        st.sel = Some(k.clone());
        st.scroll_to = Some(i);
    }
}

/// The saved list on screen, if one is (the `list.*` commands act on it then).
pub fn on_screen(cx: &Ctx) -> Option<Kind> {
    Kind::of_view(cx.nav.view)
}

pub fn list_move(cx: &mut Ctx, kind: Kind, a: &Value) {
    let d = arg_i64(a, "delta").unwrap_or(1).clamp(-100_000, 100_000);
    with_state(cx, kind, |st| move_by(st, d));
}

pub fn list_page(cx: &mut Ctx, kind: Kind, a: &Value) {
    let down = arg_bool(a, "down").unwrap_or(true);
    with_state(cx, kind, |st| {
        let rows = i64::try_from(st.page_rows.saturating_sub(2).max(1)).unwrap_or(1);
        move_by(st, if down { rows } else { -rows });
    });
}

pub fn list_edges(cx: &mut Ctx, kind: Kind, a: &Value) {
    let end = arg_bool(a, "end").unwrap_or(false);
    with_state(cx, kind, |st| {
        move_by(st, if end { i64::MAX / 2 } else { i64::MIN / 2 })
    });
}

pub fn list_close(cx: &mut Ctx, kind: Kind, _: &Value) {
    with_state(cx, kind, |st| st.sel = None);
}

/// `<list>.toggle_detail`: the details pane on or off (not remembered).
pub fn toggle_detail(cx: &mut Ctx, kind: Kind, _: &Value) {
    with_state(cx, kind, |st| st.show_detail = !st.show_detail);
}

/// `<list>.toggle_listed_only {on?}`.
pub fn toggle_listed_only(cx: &mut Ctx, kind: Kind, a: &Value) {
    let on = arg_bool(a, "on");
    with_state(cx, kind, |st| {
        st.listed_only = on.unwrap_or(!st.listed_only)
    });
}

/// `<list>.sort_by {col, asc?}`: a column as a heading click sorts it, or the way named.
pub fn sort_by(cx: &mut Ctx, kind: Kind, a: &Value) {
    let Some(col) = arg_str(a, "col").and_then(super::SortCol::from_id) else {
        return;
    };
    with_state(cx, kind, |st| match arg_bool(a, "asc") {
        Some(asc) => {
            st.sort = col;
            st.asc = asc;
        }
        None => st.sort_by(col),
    });
}

/// `<list>.search {text?}`: the search set to `text` (when given), the list shown and its
/// field focused.
pub fn search(cx: &mut Ctx, kind: Kind, a: &Value) {
    let text = arg_str(a, "text").map(|s| s.chars().take(200).collect::<String>());
    with_state(cx, kind, |st| {
        if let Some(t) = text {
            st.query = t;
        }
        st.focus_search = true;
    });
    cx.go(kind.view());
}

/// The entry the arguments name (`{ip, port}`, its saved port or a listed port), else the
/// selection.
fn entry(cx: &Ctx, kind: Kind, a: &Value) -> Option<super::Entry> {
    let entries = super::entries(cx, kind);
    if let (Some(ip), Some(port)) = (arg_str(a, "ip"), arg_i64(a, "port")) {
        let ip = ip.trim();
        let listed = cx
            .stores
            .servers
            .row(ip, port)
            .map(|r| (r.query_port(), r.game_port()));
        return entries.into_iter().find(|e| {
            e.ip == ip
                && (i64::from(e.port) == port
                    || listed
                        .is_some_and(|(q, g)| i64::from(e.port) == q || i64::from(e.port) == g))
        });
    }
    let sel = shared(cx.egui, kind).lock().sel.clone()?;
    entries.into_iter().find(|e| e.key() == sel)
}

/// `<list>.select {ip, port, detail?}`: that server picked (and the details opened).
pub fn select(cx: &mut Ctx, kind: Kind, a: &Value) {
    let Some(e) = entry(cx, kind, a) else {
        return;
    };
    let open = arg_bool(a, "detail").unwrap_or(false);
    with_state(cx, kind, |st| {
        let key = e.key();
        if let Some(i) = st.shown.iter().position(|k| *k == key) {
            st.scroll_to = Some(i);
        }
        st.sel = Some(key);
        if open {
            st.show_detail = true;
        }
    });
}

/// `<list>.remove {ip?, port?}`: the server named, else the one picked, removed after
/// asking.
pub fn remove(cx: &mut Ctx, kind: Kind, a: &Value) {
    let Some(e) = entry(cx, kind, a) else {
        cx.stores
            .bus
            .warn(crate::i18n::t("favorites", "selectHint"));
        return;
    };
    let p = &cx.stores.profile;
    match kind {
        Kind::Favorites => {
            if let Some(f) = p
                .favorites
                .iter()
                .find(|f| f.ip == e.ip && f.port == e.port)
            {
                p.confirm_remove_favorite(f);
            }
        }
        Kind::History => {
            if let Some(h) = p.history.iter().find(|h| h.ip == e.ip && h.port == e.port) {
                p.remove_history(h);
            }
        }
    }
}
