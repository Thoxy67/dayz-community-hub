//! The server browser (= `features/servers/View.svelte`): the header with its figures,
//! the line of filters and the filters in force, the sources bar, the stale-data strip,
//! then the one long list beside the details of the server picked.
//!
//! Files: `filters` (the choices, the toolbar, the filters in words), `sources` (the
//! sources bar), `table` (the list), `detail` (the server panel, shared with favourites,
//! history and Direct Connect), `cmds` (the commands of `commands.rs`).
//!
//! Native first: the list is the hub's snapshot read through the order it answered
//! (`servers` store); nothing is paged, nothing waits. The view's choices live in one
//! [`UiState`] in egui's memory, so the keys, the palette, the pad and the control
//! channel change the same thing as the buttons (which push commands).

pub mod cmds;
pub mod detail;
pub mod filters;
mod sources;
mod table;

use std::sync::Arc;

use dz_hub::HubEvent;
use dz_hub::features::browser::{ServerOrder, SortCol, Tri};
use egui::mutex::Mutex;
use egui::{CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use serde_json::{Value, json};

use crate::app::Ctx;
use crate::commands::{self, Scope};
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, IconButton, Side};
use crate::parts::{Empty, Figure, MasterDetail, PageHeader, RowCache, ShortcutsButton, draw, ph};
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::View;
use filters::Filters;

pub type Run = Vec<(&'static str, Value)>;

/// The server picked: its address (the query port) and what it is called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sel {
    pub ip: String,
    pub port: i64,
    pub name: String,
    /// Where it was in the order when picked (a hint; the order moves).
    pub index: usize,
}

/// What the view remembers for the life of the window (the Svelte session's keep).
pub struct UiState {
    pub f: Filters,
    pub sel: Option<Sel>,
    pub show_detail: bool,
    /// Scroll to the top on the next frame (a new request).
    pub scroll_top: Option<f32>,
    /// Bring this row into view on the next frame.
    pub scroll_to: Option<usize>,
    /// A command asked for "More filters" / the sources' details.
    pub open_more: bool,
    pub open_sources: bool,
    /// Rows a screenful holds (PgUp/PgDn), and the list's scroll, from the last frame.
    pub page_rows: usize,
    pub list_offset: f32,
    /// The request last sent: a different one starts at the top.
    last_q: Option<dz_hub::features::browser::ServerQuery>,
    /// The choices the request was last made from: the request is made again (and
    /// sent) only when they moved, not each frame.
    last_f: Option<Filters>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            f: Filters::default(),
            sel: None,
            show_detail: true,
            scroll_top: None,
            scroll_to: None,
            open_more: false,
            open_sources: false,
            page_rows: 10,
            list_offset: 0.0,
            last_q: None,
            last_f: None,
        }
    }
}

pub type Shared = Arc<Mutex<UiState>>;

/// The view's state, shared with its commands.
pub fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.servers.view"), || {
            Arc::new(Mutex::new(UiState::default()))
        })
        .clone()
    })
}

/// Install what the view owns outside its area: the shared server panel.
pub fn install() {
    crate::views::set_server_detail(detail::host);
}

/// Run what a frame asked, once its borrows are over; a refusal is a toast.
pub fn run_all(cx: &mut Ctx, run: Run) {
    for (id, args) in run {
        if let Err(e) = cx.run(id, &args) {
            cx.stores.bus.err(e);
        }
    }
}

/// The position of `(ip, query port)` in `o`, trying `hint` first. Only an IPv4
/// address is listed.
pub fn index_of(o: &ServerOrder, ip: &str, port: i64, hint: usize) -> Option<usize> {
    let addr = dz_api::addr(ip, port)?;
    if o.get(hint).is_some_and(|r| r.addr() == addr) {
        return Some(hint);
    }
    let at = o.snapshot.find_by_query_port(addr)?;
    o.order.iter().position(|i| *i == at)
}

pub struct ServersView {
    cache: RowCache,
}

impl ServersView {
    pub fn new() -> Self {
        install();
        Self {
            cache: RowCache::new(),
        }
    }
}

impl Default for ServersView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for ServersView {
    fn on_show(&mut self, cx: &mut Ctx) {
        // Another view asked for one server (`server:ip:port`, the Friends view's "show
        // in the list"): selected, details open, scrolled to.
        if let Some(f) = cx.nav.take_focus()
            && let Some(key) = f.strip_prefix("server:")
            && let Some((ip, port)) = key.rsplit_once(':')
            && let Ok(port) = port.parse::<i64>()
        {
            cmds::focus(cx, ip, port);
        }
    }

    fn on_event(&mut self, _ev: &HubEvent, _cx: &mut Ctx) {}

    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let shared = shared(ui.ctx());
        let mut run = Run::new();
        // The request follows the choices: typing debounced by the store, the rest at
        // once; a different request starts at the top.
        {
            let mut st = shared.lock();
            if st.last_f.as_ref() != Some(&st.f) {
                let q = st.f.query();
                // Typing (the search, the mod's name) waits for the last key.
                let debounce = q.search != cx.stores.servers.query().search
                    || q.with_mod != cx.stores.servers.query().with_mod;
                if st.last_q.as_ref() != Some(&q) {
                    if st.last_q.is_some() {
                        st.scroll_top = Some(0.0);
                    }
                    st.last_q = Some(q.clone());
                }
                cx.stores.servers.set_query(q, debounce);
                st.last_f = Some(st.f.clone());
            }
            keys(ui.ctx(), cx, &mut st, &mut run);
        }
        // The panel's model and its debounced fetches, before the frame borrows.
        let sel = shared.lock().sel.clone();
        let model = sel
            .as_ref()
            .map(|s| detail::prepare(cx, &s.ip, s.port, &s.name));
        let focus_search = cx.nav.take_focus_search();
        {
            let mut st = shared.lock();
            frame(
                ui,
                cx,
                &mut st,
                &mut self.cache,
                model.as_deref(),
                focus_search,
                &mut run,
            );
        }
        run_all(cx, run);
    }
}

/// The list's keys (registry scope `list`), while nothing else has them.
fn keys(ctx: &egui::Context, cx: &Ctx, st: &mut UiState, run: &mut Run) {
    let overlay = cx.stores.dialogs.open()
        || ctx.any_popup_open()
        || ctx.memory(|m| m.top_modal_layer().is_some())
        || cx.stores.connect.request.is_some();
    if overlay {
        return;
    }
    let (down, page_down, end) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::PageDown),
            i.key_pressed(egui::Key::End),
        )
    });
    // Enter on a focused button presses that button, not Join as well.
    let focused_widget = ctx.memory(|m| m.focused().is_some());
    let Some(id) = commands::scoped(ctx, Scope::List) else {
        return;
    };
    match id {
        "list.move" => run.push(("list.move", json!({ "delta": if down { 1 } else { -1 } }))),
        "list.page" => run.push(("list.page", json!({ "down": page_down }))),
        "list.edges" => run.push(("list.edges", json!({ "end": end }))),
        "list.close" => run.push(("list.close", Value::Null)),
        "list.detail" => st.show_detail = !st.show_detail,
        "list.join" if focused_widget => {}
        other => {
            if st.sel.is_some() {
                run.push((other, Value::Null));
            }
        }
    }
}

/// Header, sources, the stale strip, then the list and the details.
fn frame(
    ui: &mut Ui,
    cx: &Ctx,
    st: &mut UiState,
    cache: &mut RowCache,
    model: Option<&detail::model::Model>,
    focus_search: bool,
    run: &mut Run,
) {
    let tk = ui.tokens();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, CornerRadius::ZERO, tk.panel);
    let mut v = ui.new_child(
        UiBuilder::new()
            .max_rect(full)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    v.spacing_mut().item_spacing = Vec2::ZERO;
    header(&mut v, cx, st, focus_search, run);
    if let Some(s) = cx.stores.sources.now.as_deref() {
        let open = std::mem::take(&mut st.open_sources);
        sources::bar(&mut v, s, open);
    }
    stale_strip(&mut v, cx, run);

    let rest = v.available_rect_before_wrap();
    let mut body = v.new_child(
        UiBuilder::new()
            .max_rect(rest)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let mut detail_run = Run::new();
    MasterDetail::new("servers-detail")
        .detail_shown(st.show_detail)
        .selected(st.sel.is_some())
        .initial(400.0)
        .range(320.0, 620.0)
        .breakpoint(980.0)
        .show(
            &mut body,
            |ui| table::show(ui, cx, st, cache, run),
            |ui| match model {
                Some(m) => detail_run = detail::panel(ui, cx, m, true),
                None => Empty::new()
                    .icon(lucide::SERVER)
                    .title(t("servers", "selectServer"))
                    .show(ui, |_| {}),
            },
        );
    run.extend(detail_run);
}

fn header(ui: &mut Ui, cx: &Ctx, st: &mut UiState, focus_search: bool, run: &mut Run) {
    let tk = ui.tokens();
    let servers = &cx.stores.servers;
    let order = servers.order();
    let fig = order.map(|o| &o.stats);
    let shown = order.map_or(0, ServerOrder::total);
    let f = &st.f;
    let modded = match fig {
        Some(s) if s.shown > 0 => format!(
            "{}%",
            ((f64::from(s.modded) / f64::from(s.shown)) * 100.0).round()
        ),
        _ => format::NONE.to_string(),
    };
    let best = fig
        .and_then(|s| s.best_ping)
        .map_or_else(|| format::NONE.to_string(), |p| format!("{p} ms"));
    let full_tip = if f.hide_full {
        t("servers", "figShowFull")
    } else {
        t("servers", "figHideFull")
    };
    let empty_tip = if f.hide_empty {
        t("servers", "figShowEmpty")
    } else {
        t("servers", "figHideEmpty")
    };
    let modded_tip = if f.modded == Tri::Only {
        t("servers", "figAllMods")
    } else {
        t("servers", "filterModsTitleAll")
    };
    let sheet_kbd = commands::shortcut("sheet.toggle")
        .and_then(commands::Shortcut::key_text)
        .unwrap_or_default();
    let detail_kbd = commands::shortcut("list.detail")
        .and_then(commands::Shortcut::key_text)
        .unwrap_or_default();
    let refreshing = servers.refreshing;
    let data = filters::ToolbarData {
        shown,
        total: servers.total,
        maps: &servers.maps,
        maps_generation: servers.maps_generation,
        has_key: cx.stores.profile.has_api_key(),
        excluded: cx
            .stores
            .profile
            .data
            .as_ref()
            .map_or(0, |p| p.excluded_ips.len()),
        focus_search,
        open_more: std::mem::take(&mut st.open_more),
        installed: &cx.stores.mods.installed,
    };
    let show_detail = st.show_detail;
    let run_cell = std::cell::RefCell::new(std::mem::take(run));
    let (sort, asc, hide_full, hide_empty, modded_only) = (
        f.sort,
        f.asc,
        f.hide_full,
        f.hide_empty,
        f.modded == Tri::Only,
    );
    let toolbar = Some(|ui: &mut Ui| {
        ui.add_space(-1.0);
        let mut r = run_cell.borrow_mut();
        filters::toolbar(ui, &mut st.f, &data, &mut r);
        if st.f.active() {
            ui.add_space(6.0);
            filters::active_line(ui, &st.f, &mut r);
        }
        ui.add_space(1.0);
    });
    PageHeader::new(t("nav", "servers")).show(
        ui,
        |ui| {
            let mut r = run_cell.borrow_mut();
            ui.add(Figure::new(t("servers", "statShown")).amount(shown as f64));
            let fig_amount = |v: Option<f64>| v.unwrap_or(f64::NAN);
            if ui
                .add(
                    Figure::new(t("servers", "statPlayers"))
                        .amount(fig_amount(fig.map(|s| s.players as f64)))
                        .format(format::compact)
                        .tone(tk.ok)
                        .tip(t("servers", "figSortPlayers"))
                        .clickable()
                        .active(sort == SortCol::Players && !asc),
                )
                .clicked()
            {
                r.push(("servers.sort_busiest", Value::Null));
            }
            let full_n = fig.map_or(0, |s| s.full);
            if ui
                .add(
                    Figure::new(t("servers", "statFull"))
                        .amount(fig_amount(fig.map(|s| f64::from(s.full))))
                        .tone(if full_n > 0 { tk.err } else { tk.fg_muted })
                        .tip(full_tip)
                        .clickable()
                        .active(hide_full),
                )
                .clicked()
            {
                r.push(("servers.toggle_hide_full", Value::Null));
            }
            if ui
                .add(
                    Figure::new(t("servers", "statEmpty"))
                        .amount(fig_amount(fig.map(|s| f64::from(s.empty))))
                        .tone(tk.fg_muted)
                        .tip(empty_tip)
                        .secondary()
                        .clickable()
                        .active(hide_empty),
                )
                .clicked()
            {
                r.push(("servers.toggle_hide_empty", Value::Null));
            }
            if ui
                .add(
                    Figure::new(t("servers", "statModded"))
                        .value(&modded)
                        .tone(tk.mods)
                        .tip(modded_tip)
                        .clickable()
                        .active(modded_only),
                )
                .clicked()
            {
                r.push(("servers.toggle_modded_only", Value::Null));
            }
            ui.add(
                Figure::new(t("servers", "statPinged"))
                    .amount(fig_amount(fig.map(|s| f64::from(s.pinged))))
                    .tone(tk.fg_muted)
                    .secondary(),
            );
            if ui
                .add(
                    Figure::new(t("servers", "statBestPing"))
                        .value(&best)
                        .tone(tk.ok)
                        .tip(t("servers", "figSortPing"))
                        .secondary()
                        .clickable()
                        .active(sort == SortCol::Ping && asc),
                )
                .clicked()
            {
                r.push(("servers.sort_closest", Value::Null));
            }
        },
        |ui| {
            let mut r = run_cell.borrow_mut();
            if ui.add(ShortcutsButton::new(&sheet_kbd)).clicked() {
                r.push(("sheet.toggle", Value::Null));
            }
            if ui
                .add(
                    IconButton::new(lucide::PANEL_RIGHT, t("servers", "toggleDetails"))
                        .kbd(&detail_kbd)
                        .active(show_detail)
                        .side(Side::Bottom),
                )
                .clicked()
            {
                r.push(("servers.toggle_detail", Value::Null));
            }
            if ui
                .add(
                    Button::new(t("servers", "refresh"))
                        .icon(lucide::REFRESH_CW)
                        .spinner(refreshing)
                        .enabled(!refreshing)
                        .tip_kbd(t("servers", "refreshTitle"), "", Side::Bottom),
                )
                .clicked()
            {
                r.push(("servers.refresh", Value::Null));
            }
        },
        toolbar,
    );
    *run = run_cell.into_inner();
}

/// "Server data is N minutes old", with a refresh, past ten minutes (hidden while a
/// refresh loads). Wakes exactly when it turns stale and when its minutes change.
fn stale_strip(ui: &mut Ui, cx: &Ctx, run: &mut Run) {
    let servers = &cx.stores.servers;
    if servers.last_refreshed <= 0 {
        return;
    }
    let now = crate::store::now_ms();
    let age = now - servers.last_refreshed;
    let stale_at = crate::store::servers::STALE_MS;
    if age <= stale_at {
        let left = u64::try_from(stale_at - age + 50).unwrap_or(1000);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(left));
        return;
    }
    let next_minute = u64::try_from(60_000 - age % 60_000).unwrap_or(60_000);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(next_minute));
    if servers.loading {
        return;
    }
    let tk = ui.tokens();
    let minutes = age / 60_000;
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, alpha(tk.warn, 0.1));
    p.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, alpha(tk.warn, 0.25)),
    );
    let cy = rect.center().y;
    lucide::CLOCK.paint(
        p,
        Rect::from_center_size(
            Pos2::new(rect.left() + size::PAD + 7.0, cy),
            Vec2::splat(14.0),
        ),
        tk.warn,
    );
    let g = ph(
        ui,
        &tf("servers", "staleData", &[("minutes", &minutes)]),
        font(Face::Sans, size::TEXT_2XS),
    );
    draw(p, &g, rect.left() + size::PAD + 22.0, cy, tk.warn);
    let label = t("servers", "refresh");
    let bg = ph(ui, label, font(Face::SansMedium, size::TEXT_2XS));
    let br = Rect::from_min_size(
        Pos2::new(rect.right() - size::PAD - bg.size().x - 12.0, cy - 9.0),
        Vec2::new(bg.size().x + 12.0, 18.0),
    );
    let b = ui.interact(br, ui.id().with("stale-refresh"), Sense::click());
    let h = paint::ease(ui, b.id.with("hover"), b.hovered());
    ui.painter().rect_filled(
        br,
        CornerRadius::same(size::RADIUS_XS),
        alpha(tk.warn, 0.15 * h),
    );
    draw(ui.painter(), &bg, br.left() + 6.0, cy, tk.warn);
    if paint::keyboard_focus(ui, &b) {
        paint::focus_ring(ui.painter(), br, size::RADIUS_XS, &tk);
    }
    b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if b.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        run.push(("servers.refresh", Value::Null));
    }
}

/// `label-stencil`: display 800, text-2xs, tracked 0.12em, upper case (as the header's
/// figures write their labels).
pub fn stencil_galley(
    ui: &Ui,
    text: &str,
    color: egui::Color32,
    max_width: Option<f32>,
) -> std::sync::Arc<egui::Galley> {
    paint::tracked(
        ui,
        text,
        font(Face::DisplayBold, size::TEXT_2XS),
        color,
        0.12,
        true,
        max_width,
    )
}
