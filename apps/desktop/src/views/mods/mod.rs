//! The mods view (= `features/mods/View.svelte`): installed Workshop mods and the
//! subscriptions Steam is still fetching. The header says how many there are, what they
//! weigh and how many are behind, with the one button that brings them level; each row
//! says in words whether that mod is current, whose folder it is in and whether the game
//! loads it; everything else is in its menu, its details, or the bar that appears once
//! some are ticked.
//!
//! Files: `model` (rows, sort, groups, where, ids, progress lines), `list` (the table),
//! `detail` (`ModDetail.svelte`), `dialogs` (install and review), `op` (the operation's
//! dialog, installed with [`crate::shell::set_mod_op_host`]).
//!
//! The view's choices (filter, sort, ticks, the open mod, a pending review) live in one
//! [`UiState`] in egui's memory, so the commands of `commands.rs` (keys, palette, pad,
//! control channel) and the buttons change the same thing.

mod detail;
mod dialogs;
mod list;
pub mod model;
pub mod op;

use std::sync::Arc;

use dz_hub::HubEvent;
use egui::mutex::Mutex;
use egui::{Id, Pos2, Rect, Ui, UiBuilder, Vec2};
use rustc_hash::FxHashSet;
use serde_json::{Value, json};

use crate::app::Ctx;
use crate::commands::{self, Scope};
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::{
    Alert, Align, Brand, Button, IconButton, Input, MenuItem, Segment, Segmented, Side, Size, Tag,
    Tone, Variant, menu,
};
use crate::parts::{Figure, PageHeader};
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::{View, ViewId};

pub use dialogs::Review;
use model::{Col, Filter, Listing, Query};

/// Steam is asked again this often while it downloads, and while it did not answer.
const STEAM_BUSY_SECS: f64 = 5.0;
const STEAM_ERROR_SECS: f64 = 15.0;

/// What the view remembers (session only, as in the Svelte view).
#[derive(Default)]
pub struct UiState {
    pub filter: Filter,
    pub query: String,
    pub col: Col,
    pub asc: bool,
    /// Status groups folded away.
    pub folded: FxHashSet<i8>,
    /// Mods ticked for the bulk bar.
    pub ticked: FxHashSet<u64>,
    /// The mod shown in the details pane.
    pub focus: Option<u64>,
    /// Bring this mod's row into view on the next frame.
    pub scroll_to: Option<u64>,
    pub install_open: bool,
    pub install_text: String,
    /// An update or install waiting for the player's go-ahead.
    pub review: Option<Review>,
    /// A command asked for a row's menu (`mods.row_menu`).
    pub open_menu: Option<u64>,
    /// A command asked for the header's "⋯" menu.
    pub open_more: bool,
    pub listing: Listing,
    /// When Steam was last asked (egui time).
    steam_at: f64,
    started: bool,
}

impl UiState {
    fn new() -> Self {
        Self {
            asc: true,
            ..Default::default()
        }
    }

    pub fn sort_by(&mut self, c: Col) {
        if self.col == c {
            self.asc = !self.asc;
        } else {
            self.col = c;
            self.asc = c.first_asc();
        }
    }

    pub fn tick(&mut self, id: u64, on: bool) {
        if on {
            self.ticked.insert(id);
        } else {
            self.ticked.remove(&id);
        }
    }

    pub fn fold(&mut self, status: i8) {
        if !self.folded.remove(&status) {
            self.folded.insert(status);
        }
    }

    /// Show the mod `n` rows along the visible ones (clamped), scrolled into view.
    fn show_at(&mut self, n: isize) {
        let vis: Vec<usize> = self.listing.visible().collect();
        if vis.is_empty() {
            self.focus = None;
            return;
        }
        let i = n.clamp(0, vis.len() as isize - 1) as usize;
        let id = vis
            .get(i)
            .and_then(|r| self.listing.rows.get(*r))
            .map(|r| r.id);
        self.focus = id;
        self.scroll_to = id;
    }

    fn focus_index(&self) -> Option<isize> {
        let f = self.focus?;
        self.listing
            .visible()
            .position(|r| self.listing.rows.get(r).is_some_and(|x| x.id == f))
            .map(|i| i as isize)
    }
}

type Shared = Arc<Mutex<UiState>>;

/// The view's state, shared with its commands.
pub fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.mods.view"), || {
            Arc::new(Mutex::new(UiState::new()))
        })
        .clone()
    })
}

/// Install what the view owns outside its area: the mod operation's dialog.
pub fn install() {
    crate::shell::set_mod_op_host(op::host);
}

pub struct ModsView;

impl ModsView {
    pub fn new() -> Self {
        install();
        Self
    }
}

impl Default for ModsView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for ModsView {
    fn on_show(&mut self, cx: &mut Ctx) {
        let st = shared(cx.egui);
        let mut st = st.lock();
        // The palette shows a mod: `search:<name>` fills the search.
        if let Some(f) = cx.nav.take_focus()
            && let Some(q) = f.strip_prefix("search:")
        {
            st.query = q.chars().take(256).collect();
            st.filter = Filter::All;
        }
        st.started = true;
        st.steam_at = cx.egui.input(|i| i.time);
        drop(st);
        let key = cx.stores.profile.has_api_key();
        let m = &mut cx.stores.mods;
        if !m.loaded && !m.loading {
            m.load();
        }
        m.check_updates(false, key);
        m.load_steam();
    }

    fn on_event(&mut self, _ev: &HubEvent, _cx: &mut Ctx) {}

    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let shared = shared(ui.ctx());
        let mut run: Vec<(&'static str, Value)> = Vec::new();
        {
            let mut st = shared.lock();
            poll_steam(ui, cx, &mut st);
            let UiState {
                listing,
                query,
                filter,
                col,
                asc,
                folded,
                ..
            } = &mut *st;
            listing.update(
                &cx.stores.mods,
                &Query {
                    query,
                    filter: *filter,
                    col: *col,
                    asc: *asc,
                    folded,
                },
            );
            // Forget ticks for mods that are gone.
            let mods = &cx.stores.mods;
            st.ticked.retain(|id| mods.by_id(*id).is_some());
            if st.focus.is_some_and(|id| mods.by_id(id).is_none()) {
                st.focus = None;
            }
            keys(ui.ctx(), cx, &mut st, &mut run);
            draw(ui, cx, &mut st, &mut run);
        }
        for (id, args) in run {
            if let Err(e) = cx.run(id, &args) {
                cx.stores.bus.err(e);
            }
        }
        dialogs::show(ui.ctx(), cx);
    }
}

/// Steam's view: asked when the view shows, then every few seconds while it downloads
/// (never faster: each question opens a short Steam session), and now and then while it
/// did not answer.
fn poll_steam(ui: &Ui, cx: &mut Ctx, st: &mut UiState) {
    let now = ui.input(|i| i.time);
    let m = &mut cx.stores.mods;
    let every = if m.steam_busy() {
        STEAM_BUSY_SECS
    } else if m.steam_error.is_some() {
        STEAM_ERROR_SECS
    } else {
        return;
    };
    let due = st.steam_at + every;
    if now >= due {
        st.steam_at = now;
        m.load_steam();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(every));
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(due - now));
    }
}

/// The view's keys (registry scope `mods`), while nothing else has them.
fn keys(ctx: &egui::Context, cx: &Ctx, st: &mut UiState, run: &mut Vec<(&'static str, Value)>) {
    let overlay = cx.stores.dialogs.open()
        || ctx.any_popup_open()
        || ctx.memory(|m| m.top_modal_layer().is_some())
        || st.review.is_some()
        || st.install_open;
    if overlay {
        return;
    }
    let (down, end) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::End),
        )
    });
    let Some(id) = commands::scoped(ctx, Scope::Mods) else {
        return;
    };
    let i = st.focus_index();
    match id {
        "mods.move" => {
            let next = match i {
                Some(i) => i + if down { 1 } else { -1 },
                None if down => 0,
                None => isize::MAX,
            };
            st.show_at(next);
        }
        "mods.edges" => st.show_at(if end { isize::MAX } else { 0 }),
        "mods.close" => st.focus = None,
        _ => {
            let Some(f) = st.focus else {
                return;
            };
            match id {
                "mods.tick" => {
                    let on = !st.ticked.contains(&f);
                    st.tick(f, on);
                }
                "mods.managed" => run.push(("mods.toggle_managed", json!({ "id": f }))),
                "mods.delete" => run.push(("mods.delete", json!({ "id": f }))),
                "mods.update" => run.push(("mods.update_one", json!({ "id": f }))),
                _ => {}
            }
        }
    }
}

/// Header, banner, then the list and the details.
fn draw(ui: &mut Ui, cx: &Ctx, st: &mut UiState, run: &mut Vec<(&'static str, Value)>) {
    let tk = ui.tokens();
    let full = ui.max_rect();
    ui.painter()
        .rect_filled(full, egui::CornerRadius::ZERO, tk.panel);
    let mut v = ui.new_child(
        UiBuilder::new()
            .max_rect(full)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    v.spacing_mut().item_spacing = Vec2::ZERO;
    header(&mut v, cx, st, run);
    let mods = &cx.stores.mods;
    if !cx.stores.profile.has_api_key() && !mods.installed.is_empty() {
        let r = Alert::new(
            Tone::Info,
            lucide::KEY_ROUND,
            t("mods", "apiKeyMissingTitle"),
        )
        .text(t("mods", "apiKeyMissingBody"))
        .banner()
        .show(&mut v, |ui| {
            if ui
                .add(Button::new(t("mods", "openSettings")).size(Size::Xs))
                .clicked()
            {
                run.push(("mods.open_settings_steam", Value::Null));
            }
        });
        let _ = r;
    }
    let rest = v.available_rect_before_wrap();
    let mut body = v.new_child(
        UiBuilder::new()
            .max_rect(rest)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let focused = st.focus.and_then(|id| mods.by_id(id));
    let mut detail_run: Vec<(&'static str, Value)> = Vec::new();
    crate::parts::MasterDetail::new("mods-detail")
        .selected(focused.is_some())
        .initial(360.0)
        .range(300.0, 520.0)
        .breakpoint(900.0)
        .show(
            &mut body,
            |ui| list::show(ui, cx, st, run),
            |ui| {
                if let Some(m) = focused {
                    detail::show(ui, cx, m, &mut detail_run);
                }
            },
        );
    run.extend(detail_run);
}

fn header(ui: &mut Ui, cx: &Ctx, st: &mut UiState, out: &mut Vec<(&'static str, Value)>) {
    let run = std::cell::RefCell::new(Vec::<(&'static str, Value)>::new());
    let tk = ui.tokens();
    let mods = &cx.stores.mods;
    let has_key = cx.stores.profile.has_api_key();
    let n = mods.installed.len();
    let stale = mods.stale_count();
    let size_s = format::bytes(mods.total_size() as f64);
    let stale_tip = if stale > 0 {
        tf("mods", "toDownload", &[("count", &stale)])
    } else {
        t("mods", "upToDate").to_string()
    };
    let checked = if mods.checking {
        t("mods", "checking").to_string()
    } else if let Some(at) = mods.last_checked() {
        let secs = i64::try_from(at.elapsed().as_secs()).unwrap_or(0);
        format::relative(crate::store::now_ms() / 1000 - secs)
    } else {
        t("mods", "neverChecked").to_string()
    };
    let update_label = tf("mods", "updateCount", &[("count", &stale)]);
    let update_tip = tf("mods", "updateCountTitle", &[("count", &stale)]);
    let total = st.listing.total;
    let shown = st.listing.rows.len();
    let incoming = st.listing.incoming;
    let unmanaged = mods.installed.iter().filter(|m| !m.managed).count();
    let labels = [
        format!("{} · {}", t("mods", "filterAll"), total),
        format!("{} · {}", t("mods", "filterUpdates"), stale + incoming),
        format!("{} · {}", t("mods", "filterUnmanaged"), unmanaged),
    ];
    let steam_active = mods.steam_active().count();
    let steam_dl = tf("mods", "steamDownloads", &[("count", &steam_active)]);
    let counter = format!("{shown} / {total}");
    let open_more = std::mem::take(&mut st.open_more);
    let UiState { query, filter, .. } = st;
    let toolbar = (total > 0).then_some(|ui: &mut Ui| {
        // The Svelte toolbar sits a pixel lower and the header ends two pixels later.
        ui.add_space(1.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add(
                Input::new(query)
                    .search()
                    .placeholder(t("mods", "searchPlaceholder"))
                    .clear_label(t("common", "clear"))
                    .width(288.0_f32.min(ui.available_width()))
                    .id_salt("mods-search"),
            );
            let segs = [
                Segment::new(Filter::All, &labels[0]),
                Segment::new(Filter::Updates, &labels[1]),
                Segment::new(Filter::Unmanaged, &labels[2]),
            ];
            ui.add(Segmented::new(filter, &segs));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let g = crate::parts::ph(ui, &counter, font(Face::Mono, size::TEXT_2XS));
                let (r, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
                crate::parts::draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
                if steam_active > 0 {
                    ui.add(
                        Tag::new(&steam_dl)
                            .tone(Tone::Accent)
                            .spinner()
                            .brand(Brand::Steam)
                            .tip(t("mods", "steamDownloadsTitle")),
                    );
                }
                if let Some(err) = &mods.steam_error {
                    let r = ui.add(
                        Tag::new(t("mods", "steamNoAnswer"))
                            .tone(Tone::Warn)
                            .brand(Brand::Steam)
                            .trailing(lucide::ROTATE_CW)
                            .tip(err),
                    );
                    let r = ui
                        .interact(r.rect, r.id.with("retry"), egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if r.clicked() {
                        run.borrow_mut().push(("mods.steam_retry", Value::Null));
                    }
                }
            });
        });
        ui.add_space(2.0);
    });
    PageHeader::new(t("mods", "title")).show(
        ui,
        |ui| {
            if n == 0 {
                return;
            }
            ui.add(Figure::new(t("mods", "statInstalled")).amount(n as f64));
            ui.add(Figure::new(t("mods", "statSize")).value(&size_s));
            if has_key {
                ui.add(
                    Figure::new(t("mods", "filterUpdates"))
                        .amount(stale as f64)
                        .tone(if stale > 0 { tk.warn } else { tk.ok })
                        .tip(&stale_tip),
                );
                ui.add(
                    Figure::new(t("mods", "statChecked"))
                        .value(&checked)
                        .secondary(),
                );
            }
        },
        |ui| {
            if stale > 0
                && ui
                    .add(
                        Button::new(&update_label)
                            .icon(lucide::REFRESH_CW)
                            .variant(Variant::Accent)
                            .tip(&update_tip),
                    )
                    .clicked()
            {
                run.borrow_mut().push(("mods.update_stale", Value::Null));
            }
            if has_key && n > 0 {
                let b = Button::new(t("mods", "checkUpdates"))
                    .icon(lucide::CLOUD_DOWNLOAD)
                    .spinner(mods.checking)
                    .enabled(!mods.checking)
                    .variant(if stale > 0 {
                        Variant::Ghost
                    } else {
                        Variant::Default
                    })
                    .tip_kbd(t("mods", "checkUpdatesTitle"), "", Side::Bottom);
                if ui.add(b).clicked() {
                    run.borrow_mut().push(("mods.check_updates", Value::Null));
                }
            }
            if ui
                .add(
                    Button::new(t("mods", "install"))
                        .icon(lucide::DOWNLOAD)
                        .tip(t("mods", "installTitle")),
                )
                .clicked()
            {
                run.borrow_mut().push(("mods.install_dialog", Value::Null));
            }
            let more = ui.add(
                IconButton::new(lucide::ELLIPSIS, t("mods", "moreActions"))
                    .size(Size::Icon)
                    .variant(Variant::Ghost)
                    .active(false)
                    .side(Side::Bottom),
            );
            if open_more {
                egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&more));
            }
            menu::dropdown(&more, Side::Bottom, Align::End, |ui| {
                if ui
                    .add(
                        MenuItem::new(t("mods", "rescan"))
                            .icon(lucide::ROTATE_CW)
                            .enabled(!mods.loading),
                    )
                    .clicked()
                {
                    run.borrow_mut().push(("mods.refresh", Value::Null));
                }
                if ui
                    .add(
                        MenuItem::new(t("mods", "redownloadAll"))
                            .icon(lucide::REFRESH_CW)
                            .enabled(n > 0),
                    )
                    .clicked()
                {
                    run.borrow_mut().push(("mods.update_all", Value::Null));
                }
                if ui
                    .add(MenuItem::new(t("mods", "openFolderTitle")).icon(lucide::FOLDER_OPEN))
                    .clicked()
                {
                    run.borrow_mut()
                        .push(("mods.open_workshop_dir", Value::Null));
                }
                menu::separator(ui);
                if ui
                    .add(
                        MenuItem::new(t("mods", "cleanup"))
                            .icon(lucide::BRUSH_CLEANING)
                            .danger(),
                    )
                    .clicked()
                {
                    run.borrow_mut().push(("mods.cleanup", Value::Null));
                }
            });
        },
        toolbar,
    );
    out.extend(run.into_inner());
}

// ── commands (registered in `commands.rs`) ──────────────────────────────────

fn arg_u64(a: &Value, k: &str) -> Option<u64> {
    a.get(k)
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
}

fn arg_ids(a: &Value) -> Vec<u64> {
    match (a.get("ids").and_then(Value::as_array), arg_u64(a, "id")) {
        (Some(v), _) => v.iter().filter_map(Value::as_u64).take(10_000).collect(),
        (None, Some(id)) => vec![id],
        _ => Vec::new(),
    }
}

/// The mod a command means: its `id`, else the one open in the details.
fn target(cx: &Ctx, a: &Value) -> Option<u64> {
    arg_u64(a, "id").or_else(|| shared(cx.egui).lock().focus)
}

fn with<R>(cx: &Ctx, f: impl FnOnce(&mut UiState) -> R) -> R {
    let s = shared(cx.egui);
    let mut st = s.lock();
    f(&mut st)
}

fn review(cx: &mut Ctx, r: Option<Review>) {
    if let Some(r) = r {
        with(cx, |st| st.review = Some(r));
        cx.go(ViewId::Mods);
    }
}

/// `mods.update_stale`: review the mods that are behind.
pub fn cmd_update_stale(cx: &mut Ctx, _: &Value) {
    let r = Review::stale(&cx.stores.mods);
    review(cx, r);
}

/// `mods.update_all`: review every mod, to download again.
pub fn cmd_update_all(cx: &mut Ctx, _: &Value) {
    let r = Review::all(&cx.stores.mods);
    review(cx, r);
}

/// `mods.update_one {"id"}` (or `{"ids": […]}`; else the open mod): review it.
pub fn cmd_update_one(cx: &mut Ctx, a: &Value) {
    let mut ids = arg_ids(a);
    if ids.is_empty()
        && let Some(id) = target(cx, a)
    {
        ids.push(id);
    }
    let r = Review::selected(&cx.stores.mods, &ids);
    review(cx, r);
}

/// `mods.update` (pad Y): the ticked mods, else the one under the focus, else every
/// stale one straight away.
pub fn cmd_update(cx: &mut Ctx, a: &Value) {
    let (ticked, focus) = with(cx, |st| {
        let mut v: Vec<u64> = st.ticked.iter().copied().collect();
        v.sort_unstable();
        (v, st.focus)
    });
    let one = arg_u64(a, "id").or(focus);
    if !ticked.is_empty() {
        let r = Review::selected(&cx.stores.mods, &ticked);
        review(cx, r);
    } else if let Some(id) = one.filter(|id| cx.stores.mods.by_id(*id).is_some()) {
        let r = Review::selected(&cx.stores.mods, &[id]);
        review(cx, r);
    } else if cx.stores.mods.stale_count() > 0 {
        let via = cx.stores.profile.via_steam();
        cx.stores.mods.update_stale(via);
    }
}

/// `mods.install_dialog`: the install dialog, empty.
pub fn cmd_install_dialog(cx: &mut Ctx, _: &Value) {
    with(cx, |st| {
        st.install_text.clear();
        st.install_open = true;
    });
    cx.go(ViewId::Mods);
}

/// `mods.install {"ids": […]}` or `{"text": "ids or links"}`: review installing them;
/// with neither, the text typed in the install dialog.
pub fn cmd_install(cx: &mut Ctx, a: &Value) {
    let mut ids = arg_ids(a);
    if ids.is_empty() {
        let text = a
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| with(cx, |st| st.install_text.clone()));
        ids = model::parse_workshop_ids(&text);
    }
    if ids.is_empty() {
        return;
    }
    with(cx, |st| st.install_open = false);
    let r = Review::install(&cx.stores.mods, &ids);
    review(cx, r);
}

/// `mods.review_confirm` (`{"cancel": true}` to drop it): start what the review shows.
pub fn cmd_review_confirm(cx: &mut Ctx, a: &Value) {
    let Some(r) = with(cx, |st| st.review.take()) else {
        return;
    };
    if a.get("cancel").and_then(Value::as_bool) == Some(true) {
        return;
    }
    r.confirm(cx);
}

/// `mods.more_menu`: the header's "⋯" menu.
pub fn cmd_more_menu(cx: &mut Ctx, _: &Value) {
    with(cx, |st| st.open_more = true);
    cx.go(ViewId::Mods);
}

/// `mods.search {"text"}`: fill the search (the filter back to All).
pub fn cmd_search(cx: &mut Ctx, a: &Value) {
    let text: String = a
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(256)
        .collect();
    with(cx, |st| {
        st.query = text;
        st.filter = Filter::All;
    });
    cx.go(ViewId::Mods);
}

/// `mods.filter {"filter": "all" | "updates" | "unmanaged"}`.
pub fn cmd_filter(cx: &mut Ctx, a: &Value) {
    if let Some(f) = a
        .get("filter")
        .and_then(Value::as_str)
        .and_then(Filter::from_id)
    {
        with(cx, |st| st.filter = f);
    }
}

/// `mods.steam_retry`: ask Steam again.
pub fn cmd_steam_retry(cx: &mut Ctx, _: &Value) {
    cx.stores.mods.load_steam();
}

/// `mods.open_settings_steam`: the settings' Steam part (to add an API key).
pub fn cmd_open_settings_steam(cx: &mut Ctx, _: &Value) {
    cx.go_focus(ViewId::Settings, "steam");
}

/// `mods.tick_all {"on"?}`: tick every installed row shown, or none (toggles).
pub fn cmd_tick_all(cx: &mut Ctx, a: &Value) {
    let on = a.get("on").and_then(Value::as_bool);
    with(cx, |st| {
        let shown: Vec<u64> = st
            .listing
            .rows
            .iter()
            .filter(|r| r.installed.is_some())
            .map(|r| r.id)
            .collect();
        let all = !shown.is_empty() && shown.iter().all(|id| st.ticked.contains(id));
        if on.unwrap_or(!all) {
            st.ticked = shown.into_iter().collect();
        } else {
            st.ticked.clear();
        }
    });
}

fn sort(cx: &Ctx, c: Col) {
    with(cx, |st| st.sort_by(c));
}

pub fn cmd_sort_name(cx: &mut Ctx, _: &Value) {
    sort(cx, Col::Name);
}
pub fn cmd_sort_status(cx: &mut Ctx, _: &Value) {
    sort(cx, Col::Status);
}
pub fn cmd_sort_managed(cx: &mut Ctx, _: &Value) {
    sort(cx, Col::Managed);
}
pub fn cmd_sort_size(cx: &mut Ctx, _: &Value) {
    sort(cx, Col::Size);
}
pub fn cmd_sort_local(cx: &mut Ctx, _: &Value) {
    sort(cx, Col::Local);
}

/// `mods.group.fold {"group": -1 | 0 | 1 | 2}`: fold or unfold a status group.
pub fn cmd_fold(cx: &mut Ctx, a: &Value) {
    if let Some(g) = a
        .get("group")
        .and_then(Value::as_i64)
        .and_then(|g| i8::try_from(g).ok())
        .filter(|g| (-1..=2).contains(g))
    {
        with(cx, |st| st.fold(g));
    }
}

/// `mods.tick {"id"?, "on"?}`: tick or untick a mod (the open one by default).
pub fn cmd_tick(cx: &mut Ctx, a: &Value) {
    let Some(id) = target(cx, a).filter(|id| cx.stores.mods.by_id(*id).is_some()) else {
        return;
    };
    let on = a.get("on").and_then(Value::as_bool);
    with(cx, |st| {
        let v = on.unwrap_or(!st.ticked.contains(&id));
        st.tick(id, v);
    });
}

/// `mods.select {"id"}`: open a mod in the details.
pub fn cmd_select(cx: &mut Ctx, a: &Value) {
    if let Some(id) = arg_u64(a, "id").filter(|id| cx.stores.mods.by_id(*id).is_some()) {
        with(cx, |st| {
            st.focus = Some(id);
            st.scroll_to = Some(id);
        });
        cx.go(ViewId::Mods);
    }
}

/// `mods.close_detail`: close the details.
pub fn cmd_close_detail(cx: &mut Ctx, _: &Value) {
    with(cx, |st| st.focus = None);
}

/// `mods.row_menu {"id"?}` (and `mods.row_context_menu`): open a row's menu.
pub fn cmd_row_menu(cx: &mut Ctx, a: &Value) {
    if let Some(id) = target(cx, a) {
        with(cx, |st| {
            st.open_menu = Some(id);
            st.scroll_to = Some(id);
        });
        cx.go(ViewId::Mods);
    }
}

/// `mods.open_workshop {"id"}`: its Workshop page in the browser.
pub fn cmd_open_workshop(cx: &mut Ctx, a: &Value) {
    if let Some(id) = target(cx, a) {
        let url = model::workshop_url(id);
        let bus = cx.stores.bus.clone();
        crate::rt::fire(async move {
            if let Err(e) = open::that_detached(&url) {
                bus.err(e.to_string());
            }
        });
    }
}

/// `mods.copy_id {"id"}`: its Workshop id to the clipboard.
pub fn cmd_copy_id(cx: &mut Ctx, a: &Value) {
    if let Some(id) = target(cx, a) {
        cx.egui.copy_text(id.to_string());
    }
}

/// `mods.tick_stale`: tick exactly the mods that are behind.
pub fn cmd_tick_stale(cx: &mut Ctx, _: &Value) {
    let ids: FxHashSet<u64> = cx.stores.mods.stale().map(|m| m.id).collect();
    with(cx, |st| st.ticked = ids);
}

fn ticked_ids(cx: &Ctx) -> Vec<u64> {
    let mut v: Vec<u64> = with(cx, |st| st.ticked.iter().copied().collect());
    v.sort_unstable();
    v
}

/// `mods.bulk_update`: review the ticked mods that are behind.
pub fn cmd_bulk_update(cx: &mut Ctx, _: &Value) {
    let ids: Vec<u64> = ticked_ids(cx)
        .into_iter()
        .filter(|id| {
            cx.stores
                .mods
                .by_id(*id)
                .is_some_and(|m| m.update_available)
        })
        .collect();
    let r = Review::selected(&cx.stores.mods, &ids);
    review(cx, r);
}

/// `mods.bulk_redownload`: review downloading every ticked mod again.
pub fn cmd_bulk_redownload(cx: &mut Ctx, _: &Value) {
    let ids = ticked_ids(cx);
    let r = Review::selected(&cx.stores.mods, &ids);
    review(cx, r);
}

fn link_ticked(cx: &mut Ctx, link: bool) {
    for id in ticked_ids(cx) {
        if cx.stores.mods.by_id(id).is_some_and(|m| m.managed != link) {
            cx.stores.mods.toggle_managed(id);
        }
    }
}

/// `mods.bulk_link`: link the ticked mods into the game.
pub fn cmd_bulk_link(cx: &mut Ctx, _: &Value) {
    link_ticked(cx, true);
}

/// `mods.bulk_unlink`: remove the ticked mods' game links.
pub fn cmd_bulk_unlink(cx: &mut Ctx, _: &Value) {
    link_ticked(cx, false);
}

/// `mods.bulk_delete`: delete the ticked mods, after asking.
pub fn cmd_bulk_delete(cx: &mut Ctx, _: &Value) {
    let ids = ticked_ids(cx);
    cx.stores.mods.remove_many(ids);
}

/// `mods.clear_ticks`.
pub fn cmd_clear_ticks(cx: &mut Ctx, _: &Value) {
    with(cx, |st| st.ticked.clear());
}

/// `mods.move {"dir": "up" | "down"}` and `mods.edges {"to": "first" | "last"}`: walk
/// the open mod through the rows shown.
pub fn cmd_move(cx: &mut Ctx, a: &Value) {
    let down = a.get("dir").and_then(Value::as_str) != Some("up");
    with(cx, |st| {
        let next = match st.focus_index() {
            Some(i) => i + if down { 1 } else { -1 },
            None if down => 0,
            None => isize::MAX,
        };
        st.show_at(next);
    });
}

pub fn cmd_edges(cx: &mut Ctx, a: &Value) {
    let last = a.get("to").and_then(Value::as_str) == Some("last");
    with(cx, |st| st.show_at(if last { isize::MAX } else { 0 }));
}

/// The rect for `n` stacked lines of `h` centred in `r`.
pub(super) fn centred(r: Rect, h: f32) -> Rect {
    Rect::from_min_size(
        Pos2::new(r.left(), r.center().y - h / 2.0),
        Vec2::new(r.width(), h),
    )
}

/// Whether the mod operation's dialog has the keys now.
pub fn op_open(cx: &Ctx) -> bool {
    cx.stores.mods.op.active && !cx.stores.mods.op.minimised
}
