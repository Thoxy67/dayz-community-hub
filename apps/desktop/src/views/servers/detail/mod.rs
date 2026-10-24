//! Everything known about one server (= `detail/ServerDetail.svelte`, `Hero.svelte`,
//! `ActionBar.svelte`): the map-coloured hero says which server and whether it is worth
//! joining now; the tabs hold the detail (overview, players, mods against what is
//! installed, rules, DayZ Metrics' long view); the foot keeps join and the other actions
//! in reach. Shared by the server list, favourites, history and Direct Connect (through
//! [`crate::views::set_server_detail`]); works for a server the list does not have, from
//! what it answers directly.
//!
//! Asking the server itself, DayZ Metrics and its long views is debounced as in the
//! Svelte panel (150, 250 and 400 ms after a server is shown), so arrowing through a
//! list does not query every row it passes; the timers ask for exactly the frames they
//! need.

mod country;
pub mod model;
mod overview;
mod stats;
mod tabs;

use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use serde_json::{Value, json};

use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{
    FieldSize, Flag, IconButton, Side, SignalSize, Size, Spinner, Tab, TabStrip, TipExt, Variant,
};
use crate::parts::{
    ExcludeButton, FavoriteButton, Flags, JoinButton, ModsCount, OsIcon, PingCell, PlayersCell,
    ServerFlags, StarSize, TimeOfDay, draw, map_name, ph, ph_trunc,
};
use crate::theme::{TokensExt, font, fonts::Face, size};
use model::{Model, Population};

type Run = Vec<(&'static str, Value)>;

const A2S_AFTER: f64 = 0.150;
const METRICS_AFTER: f64 = 0.250;
const LONG_AFTER: f64 = 0.400;
/// The panel's foot: `py-2` around a large control (taller in the pad density), and its
/// rule.
fn foot_h() -> f32 {
    8.0 + size::control_lg() + 8.0 + 1.0
}

/// Which tab is open: kept across servers, so comparing rules or player lists does not
/// mean clicking the tab each time (`tab.svelte.ts`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Overview,
    Players,
    Mods,
    Rules,
    Stats,
}

impl DetailTab {
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "overview" => Self::Overview,
            "players" => Self::Players,
            "mods" => Self::Mods,
            "rules" => Self::Rules,
            "stats" => Self::Stats,
            _ => return None,
        })
    }
}

/// The Mods tab's filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModsFilter {
    #[default]
    All,
    Missing,
    Stale,
}

/// The panel's state, shared by every view that shows it and by its commands.
#[derive(Default)]
pub struct DetailState {
    pub tab: DetailTab,
    pub players_q: String,
    pub rules_q: String,
    pub mods_filter: ModsFilter,
    /// The Stats tab's span: "1d", "7d", "2w", "1m".
    pub range: &'static str,
    /// The server on show (ip, port as asked) and when it came (egui time).
    shown: Option<(String, i64)>,
    since: f64,
    asked_a2s: bool,
    asked_metrics: bool,
    asked_long: Option<u64>,
    /// When the address was copied from the foot (egui time).
    copied_at: Option<f64>,
    /// The server the commands act on: (ip, query port, game port, search name).
    pub target: Option<Target>,
    /// The model last gathered, and the key it was gathered under.
    model: Option<(u64, Arc<Model>)>,
}

#[derive(Clone, Debug, Default)]
pub struct Target {
    pub ip: String,
    pub query_port: i64,
    pub game_port: i64,
    pub name: String,
    pub listed: bool,
    pub metrics_id: Option<u64>,
    pub missing: Vec<u64>,
    pub stale: Vec<u64>,
}

pub type Shared = Arc<Mutex<DetailState>>;

pub fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.servers.detail"), || {
            Arc::new(Mutex::new(DetailState {
                range: "1d",
                ..Default::default()
            }))
        })
        .clone()
    })
}

/// The shared detail for other views (no close button): `views::server_detail`.
pub fn host(ui: &mut Ui, cx: &mut Ctx, ip: &str, port: i64, name: &str) {
    let run = show(ui, cx, ip, port, name, false);
    super::run_all(cx, run);
}

/// The debounced fetches for the server on show; returns nothing, asks for frames.
fn tick(cx: &mut Ctx, st: &mut DetailState, ip: &str, port: i64, m: &Model) {
    let now = cx.egui.input(|i| i.time);
    let same = st
        .shown
        .as_ref()
        .is_some_and(|(i, p)| i == ip && *p == port);
    if !same {
        st.shown = Some((ip.to_string(), port));
        st.since = now;
        st.asked_a2s = false;
        st.asked_metrics = false;
        st.asked_long = None;
    }
    let age = now - st.since;
    let mut wait = f64::INFINITY;
    if !st.asked_a2s {
        if age >= A2S_AFTER {
            st.asked_a2s = true;
            let s = &mut *cx.stores;
            s.server_data.refresh_a2s(&s.servers, ip, m.query_port);
        } else {
            wait = wait.min(A2S_AFTER - age);
        }
    }
    if !st.asked_metrics {
        if age >= METRICS_AFTER {
            st.asked_metrics = true;
            let gp = u32::try_from(m.game_port).unwrap_or(0);
            let qp = u32::try_from(m.query_port).unwrap_or(0);
            cx.stores
                .server_data
                .fetch_metrics(ip, gp, qp, &m.search_name, false);
        } else {
            wait = wait.min(METRICS_AFTER - age);
        }
    }
    if let Some(id) = m.metrics_data().map(|x| x.id) {
        if st.asked_long != Some(id) {
            if age >= LONG_AFTER {
                st.asked_long = Some(id);
                cx.stores.server_data.fetch_heatmap(id);
                cx.stores.server_data.fetch_rank_history(id);
            } else {
                wait = wait.min(LONG_AFTER - age);
            }
        }
        if st.range != "1d" {
            cx.stores.server_data.fetch_history(id, st.range);
        }
    }
    if wait.is_finite() {
        cx.egui
            .request_repaint_after(std::time::Duration::from_secs_f64(wait.max(0.0)));
    }
}

/// Gather the server's model and run the debounced fetches (before drawing, while the
/// context can be borrowed mutably).
pub fn prepare(cx: &mut Ctx, ip: &str, port: i64, name: &str) -> Arc<Model> {
    let shared = shared(cx.egui);
    let mut st = shared.lock();
    // Made again only when something it reads has moved (`model::key`).
    let key = model::key(cx, ip, port, name);
    let m = match &st.model {
        Some((k, m)) if *k == key && st.target.is_some() => Arc::clone(m),
        _ => {
            let m = Arc::new(Model::of(cx, ip, port, name));
            st.target = Some(Target {
                ip: m.ip.clone(),
                query_port: m.query_port,
                game_port: m.game_port,
                name: m.search_name.clone(),
                listed: m.listed.is_some(),
                metrics_id: m.metrics_data().map(|x| x.id),
                missing: m.missing_ids(),
                stale: m.stale_ids(),
            });
            st.model = Some((key, Arc::clone(&m)));
            m
        }
    };
    tick(cx, &mut st, ip, port, &m);
    m
}

/// Draws the panel of `m` into `ui`; `closable` adds the ✕ (the server list's). Returns
/// what was asked, to run once the frame's borrows are over.
pub fn panel(ui: &mut Ui, cx: &Ctx, m: &Model, closable: bool) -> Run {
    let shared = shared(ui.ctx());
    let mut st = shared.lock();
    let mut run = Run::new();
    draw_panel(ui, cx, &mut st, m, closable, &mut run);
    run
}

/// Both at once, for a host that holds the context mutably.
pub fn show(ui: &mut Ui, cx: &mut Ctx, ip: &str, port: i64, name: &str, closable: bool) -> Run {
    let m = prepare(cx, ip, port, name);
    panel(ui, cx, &m, closable)
}

fn draw_panel(
    ui: &mut Ui,
    cx: &Ctx,
    st: &mut DetailState,
    m: &Model,
    closable: bool,
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
    // The in-game clock now, estimated as the rows, the filter and the sort read it (the
    // model is made again only when the server changes, so it is read here); else the
    // last A2S answer's.
    let time = cx
        .stores
        .servers
        .row_of(&m.ip, m.query_port)
        .map(|r| r.time_now())
        .or_else(|| {
            m.a2s
                .as_ref()
                .and_then(|a| a.dayz.as_ref())
                .and_then(|d| d.game_time.clone())
        })
        .unwrap_or_default();
    hero(&mut v, m, &time, closable, run);

    // The tabs: the icons go under ~480px (except the warnings), so the labels stay whole.
    let wide = full.width() >= 480.0;
    let (players_n, rules_n) = (
        m.count
            .map_or(m.players.len() as u64, |c| c.players.max(0) as u64),
        m.a2s.as_ref().map_or(0, |a| a.rules.len() as u64),
    );
    let mut tab = st.tab;
    let mut tabs = vec![
        Tab::new(DetailTab::Overview, t("detail", "tabOverview")),
        Tab::new(DetailTab::Players, t("detail", "tabPlayers")).count(players_n),
        Tab::new(DetailTab::Mods, t("detail", "tabMods")).count(m.mods_count as u64),
        Tab::new(DetailTab::Rules, t("detail", "tabRules")).count(rules_n),
        Tab::new(DetailTab::Stats, t("detail", "tabStats")),
    ];
    if wide {
        let icons = [
            lucide::LAYOUT_DASHBOARD,
            lucide::USERS,
            lucide::PUZZLE,
            lucide::SCROLL_TEXT,
            lucide::CHART_LINE,
        ];
        for (tab, icon) in tabs.iter_mut().zip(icons) {
            tab.icon = Some(icon);
        }
    }
    if m.missing > 0 || m.stale > 0 {
        let c = if m.missing > 0 { tk.err } else { tk.warn };
        if let Some(tb) = tabs.get_mut(2) {
            tb.icon = Some(lucide::PUZZLE);
            tb.tint = Some(c);
            tb.dot = Some(c);
        }
    }
    match m.population {
        Some(Population::Fake) => {
            if let Some(tb) = tabs.get_mut(4) {
                tb.icon = Some(lucide::SHIELD_ALERT);
                tb.tint = Some(tk.err);
            }
        }
        Some(Population::Suspect) => {
            if let Some(tb) = tabs.get_mut(4) {
                tb.icon = Some(lucide::TRIANGLE_ALERT);
                tb.tint = Some(tk.warn);
            }
        }
        _ => {}
    }
    let tabs_rect = Rect::from_min_size(v.cursor().min, Vec2::new(full.width(), size::CONTROL));
    let mut tui = v.new_child(UiBuilder::new().max_rect(tabs_rect.shrink2(Vec2::new(4.0, 0.0))));
    let mut strip = TabStrip::new("server-detail-tabs", &mut tab, &tabs).size(FieldSize::Xs);
    if wide {
        strip = strip.fill();
    }
    tui.add(strip);
    v.allocate_rect(tabs_rect, Sense::hover());
    v.painter().hline(
        full.x_range(),
        tabs_rect.bottom() - 0.5,
        Stroke::new(1.0, tk.border),
    );
    if tab != st.tab {
        run.push(("servers.detail.tab", json!({ "tab": tab_id(tab) })));
    }

    // The body scrolls between the tabs and the foot.
    let top = v.cursor().top();
    let body = Rect::from_min_max(
        Pos2::new(full.left(), top),
        Pos2::new(full.right(), (full.bottom() - foot_h()).max(top)),
    );
    let mut bui = v.new_child(
        UiBuilder::new()
            .max_rect(body)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    bui.set_clip_rect(body.intersect(ui.clip_rect()));
    bui.spacing_mut().item_spacing = Vec2::ZERO;
    match st.tab {
        DetailTab::Players | DetailTab::Rules => {
            // Their lists scroll themselves under a search field.
            if st.tab == DetailTab::Players {
                tabs::players(&mut bui, cx, st, m, run);
            } else {
                tabs::rules(&mut bui, st, m);
            }
        }
        _ => {
            egui::ScrollArea::vertical()
                .id_salt(("server-detail-body", tab_id(st.tab)))
                .auto_shrink([false, false])
                .show(&mut bui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    match st.tab {
                        DetailTab::Overview => overview::show(ui, cx, m, run),
                        DetailTab::Mods => tabs::mods(ui, st, m, run),
                        _ => stats::show(ui, cx, st, m, run),
                    }
                    // What a controller's right stick scrolls.
                    crate::pad::reading_pane(ui);
                });
        }
    }

    let foot = Rect::from_min_max(Pos2::new(full.left(), full.bottom() - foot_h()), full.max);
    action_bar(ui, st, m, foot, run);
}

pub fn tab_id(t: DetailTab) -> &'static str {
    match t {
        DetailTab::Overview => "overview",
        DetailTab::Players => "players",
        DetailTab::Mods => "mods",
        DetailTab::Rules => "rules",
        DetailTab::Stats => "stats",
    }
}

/// A target as command arguments.
fn target_args(m: &Model) -> Value {
    json!({
        "ip": m.ip,
        "port": m.query_port,
        "gamePort": m.game_port,
        "name": m.title,
        "listed": m.listed.is_some(),
    })
}

/// A stencilled label (`label-stencil text-fg-faint`), allocated.
pub(super) fn stencil(ui: &mut Ui, text: &str, color: Color32) {
    let g =
        crate::views::servers::stencil_galley(ui, text, color, Some(ui.available_width().max(8.0)));
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, color);
}

/// Wrapped words, allocated.
pub(super) fn words(ui: &mut Ui, s: &str, f: egui::FontId, c: Color32) -> egui::Response {
    let w = ui.available_width().max(8.0);
    let g = paint::galley_wrap(ui, s, f, c, w);
    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, c);
    resp
}

// ── the hero ────────────────────────────────────────────────────────────────

/// A galley's first baseline, from its top.
fn baseline(g: &egui::Galley) -> f32 {
    g.rows.first().map_or(g.size().y * 0.8, |r| {
        r.pos.y
            + r.row
                .glyphs
                .first()
                .map_or(r.row.size.y * 0.8, |gl| gl.pos.y)
    })
}

/// The map is what a player recognises first, so it is the anchor, set large over
/// contour lines in the map's colour; then the server's name, what it asks of you, where
/// it is, and the live strip (line, head-count, clock, mods) that decides whether to join.
fn hero(ui: &mut Ui, m: &Model, time: &str, closable: bool, run: &mut Run) {
    let tk = ui.tokens();
    let full = ui.max_rect();
    let start = ui.cursor().top();
    // The map's ground, under the words: a wash of its colour over survey contours, as
    // tall as the hero was last frame (a change is redrawn at once).
    let hid = ui.id().with("hero-h");
    let last_h: f32 = ui.data(|d| d.get_temp(hid)).unwrap_or(180.0);
    let ground = Rect::from_min_size(
        Pos2::new(full.left(), start),
        Vec2::new(full.width(), last_h),
    );
    {
        let mut mesh = egui::Mesh::default();
        let c = |a: f32| alpha(tk.map, a);
        mesh.colored_vertex(ground.left_top(), c(0.18));
        mesh.colored_vertex(ground.right_top(), c(0.05));
        mesh.colored_vertex(ground.right_bottom(), Color32::TRANSPARENT);
        mesh.colored_vertex(ground.left_bottom(), c(0.05));
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        ui.painter().add(egui::Shape::mesh(mesh));
        let p = ui
            .painter()
            .with_clip_rect(ground.intersect(ui.clip_rect()));
        crate::kit::topo::paint_tinted(&p, ground, tk.map, 0.28);
    }
    let inner_w = full.width() - 2.0 * size::PAD;
    let resp = egui::Frame::new()
        .inner_margin(egui::Margin {
            left: size::PAD as i8,
            right: size::PAD as i8,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(inner_w);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            // Map, name, the buttons (desktop shortcut, refresh, close).
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let btn_w = if closable {
                    3.0 * size::CONTROL_SM + 4.0
                } else {
                    2.0 * size::CONTROL_SM + 2.0
                };
                let w = (ui.available_width() - btn_w - 8.0).max(40.0);
                ui.vertical(|ui| {
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let shown = map_name(&m.map);
                        let shown = if shown.is_empty() {
                            crate::i18n::format::NONE.to_string()
                        } else {
                            shown.into_owned()
                        };
                        let raw_differs =
                            !m.map.is_empty() && shown.to_lowercase() != m.map.to_lowercase();
                        let raw =
                            raw_differs.then(|| ph(ui, &m.map, font(Face::Mono, size::TEXT_3XS)));
                        let room = w - raw.as_ref().map_or(0.0, |g| g.size().x + 8.0);
                        let g = paint::tracked(
                            ui,
                            &shown,
                            font(Face::DisplayBold, size::TEXT_2XL),
                            tk.map,
                            0.02,
                            true,
                            Some(room.max(20.0)),
                        );
                        let (r, resp) =
                            ui.allocate_exact_size(Vec2::new(g.size().x, 30.0), Sense::hover());
                        let gy = r.center().y - g.size().y / 2.0;
                        let title_base = gy + baseline(&g);
                        ui.painter().galley(Pos2::new(r.left(), gy), g, tk.map);
                        if !m.map.is_empty() {
                            let _ = resp.tip_kbd(&m.map, "", Side::Bottom);
                        }
                        if let Some(raw) = raw {
                            let (r, _) = ui
                                .allocate_exact_size(Vec2::new(raw.size().x, 30.0), Sense::hover());
                            // On the title's baseline, read from both galleys' glyphs.
                            ui.painter().galley(
                                Pos2::new(r.left(), title_base - baseline(&raw)),
                                raw,
                                tk.fg_faint,
                            );
                        }
                    });
                    // The name, two lines at most.
                    let mut job = egui::text::LayoutJob::simple(
                        m.title.clone(),
                        font(Face::SansSemibold, size::TEXT_SM),
                        tk.fg,
                        w,
                    );
                    job.wrap.max_rows = 2;
                    job.wrap.break_anywhere = false;
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                    ui.painter().galley(r.min, g, tk.fg);
                    let _ = resp.tip_kbd(&m.title, "", Side::Bottom);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if closable
                        && ui
                            .add(
                                IconButton::new(lucide::X, t("detail", "close"))
                                    .size(Size::IconXs)
                                    .variant(Variant::Ghost)
                                    .kbd("Esc"),
                            )
                            .clicked()
                    {
                        run.push(("list.close", Value::Null));
                    }
                    if m.live_loading {
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::splat(size::CONTROL_SM), Sense::hover());
                        Spinner::paint(
                            ui,
                            Rect::from_center_size(r.center(), Vec2::splat(14.0)),
                            tk.fg_faint,
                        );
                    } else if ui
                        .add(
                            IconButton::new(lucide::REFRESH_CW, t("detail", "refreshLive"))
                                .size(Size::IconXs)
                                .variant(Variant::Ghost),
                        )
                        .clicked()
                    {
                        run.push(("servers.detail.refresh_live", target_args(m)));
                    }
                    if ui
                        .add(
                            IconButton::new(lucide::MONITOR, t("servers", "desktopShortcut"))
                                .size(Size::IconXs)
                                .variant(Variant::Ghost),
                        )
                        .clicked()
                    {
                        run.push(("list.shortcut", target_args(m)));
                    }
                });
            });

            meta_line(ui, m);
            live_strip(ui, m, time, run);
            population_warning(ui, m, true);
            if m.listed.is_none() {
                line_top(ui, 16.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    lucide::TRIANGLE_ALERT.paint(
                        ui.painter(),
                        r.translate(Vec2::new(0.0, 1.0)),
                        tk.warn,
                    );
                    words(
                        ui,
                        t("detail", "notListed"),
                        font(Face::Sans, size::TEXT_2XS),
                        tk.warn,
                    );
                });
            }
        });
    let rect = Rect::from_min_max(
        Pos2::new(full.left(), start),
        Pos2::new(full.right(), resp.response.rect.bottom()),
    );
    if (rect.height() - last_h).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(hid, rect.height()));
        ui.ctx().request_discard("server hero height");
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, tk.border),
    );
}

/// Flags and system, the country, the version, an exclusion.
fn meta_line(ui: &mut Ui, m: &Model) {
    let tk = ui.tokens();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
        if let Some(l) = &m.listed {
            ui.add(ServerFlags::new(Flags {
                password: l.password,
                first_person: l.first_person,
                battleye: l.battleye.unwrap_or(false),
                saved_password: false,
                official: l.official,
                mimics_official: l.mimics_official,
            }));
            ui.add(OsIcon::new(&l.environment).size(12.0));
        } else if let Some(d) = m.a2s.as_ref().and_then(|a| a.dayz.as_ref()) {
            ui.add(ServerFlags::new(Flags {
                password: false,
                first_person: d.first_person_only,
                battleye: d.battleye,
                saved_password: false,
                official: d.official,
                mimics_official: false,
            }));
        }
        if let Some(code) = m.metrics_data().and_then(|x| x.country.as_deref()) {
            let name = country::name(code);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add(Flag::new(code).size(14.0).tip(&name));
                let g = ph(ui, &name, font(Face::Sans, size::TEXT_2XS));
                let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, 16.0), Sense::hover());
                draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
            });
        }
        let version = m
            .listed
            .as_ref()
            .map(|l| l.version.clone())
            .filter(|v| !v.is_empty())
            .or_else(|| m.a2s.as_ref().map(|a| a.version.clone()))
            .unwrap_or_default();
        if !version.is_empty() {
            let g = ph(ui, &format!("v{version}"), font(Face::Mono, size::TEXT_2XS));
            let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, 16.0), Sense::hover());
            draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
        }
        if m.listed.as_ref().is_some_and(|l| l.excluded) {
            let g = ph(
                ui,
                t("detail", "excluded"),
                font(Face::Sans, size::TEXT_2XS),
            );
            let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, 16.0), Sense::hover());
            draw(ui.painter(), &g, r.left(), r.center().y, tk.err);
        }
    });
}

/// The live strip: what decides whether to join right now. Four columns from 26rem,
/// two lines of two below.
fn live_strip(ui: &mut Ui, m: &Model, time: &str, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let four = w >= 416.0;
    let queue = m
        .a2s
        .as_ref()
        .and_then(|a| a.dayz.as_ref())
        .and_then(|d| d.login_queue)
        .unwrap_or(0);
    let cell_h = 11.0 + 4.0 + 24.0 + if queue > 0 { 16.0 } else { 0.0 };
    let rows = if four { 1.0 } else { 2.0 };
    let h = 16.0 + rows * cell_h + (rows - 1.0) * 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(size::RADIUS_SM),
        alpha(tk.panel, 0.8),
        Stroke::new(1.0, tk.border),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink2(Vec2::new(10.0, 8.0));
    let gap = 16.0;
    // [auto, 1fr, auto, auto]: the head-count takes the room.
    let (ping_w, time_w, mods_w) = (84.0, 64.0, 64.0);
    let cells: [Rect; 4] = if four {
        let players_w = (inner.width() - ping_w - time_w - mods_w - 3.0 * gap).max(60.0);
        let mut x = inner.left();
        let mut take = |cw: f32| {
            let r = Rect::from_min_size(Pos2::new(x, inner.top()), Vec2::new(cw, cell_h));
            x += cw + gap;
            r
        };
        [take(ping_w), take(players_w), take(time_w), take(mods_w)]
    } else {
        let c2 = (inner.width() - ping_w - gap).max(60.0);
        let y2 = inner.top() + cell_h + 8.0;
        [
            Rect::from_min_size(inner.min, Vec2::new(ping_w, cell_h)),
            Rect::from_min_size(
                Pos2::new(inner.left() + ping_w + gap, inner.top()),
                Vec2::new(c2, cell_h),
            ),
            Rect::from_min_size(Pos2::new(inner.left(), y2), Vec2::new(ping_w, cell_h)),
            Rect::from_min_size(
                Pos2::new(inner.left() + ping_w + gap, y2),
                Vec2::new(c2, cell_h),
            ),
        ]
    };
    let cell = |ui: &mut Ui, r: Rect, label: &str, i: usize, body: &mut dyn FnMut(&mut Ui)| {
        let mut c = ui.new_child(
            UiBuilder::new()
                .max_rect(r)
                .id_salt(("live-cell", i))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        c.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        stencil(&mut c, label, tk.fg_faint);
        body(&mut c);
    };
    let [c0, c1, c2, c3] = cells;
    cell(ui, c0, t("detail", "ping"), 0, &mut |ui| {
        let r = ui.add(
            PingCell::new(m.ping)
                .pending(m.ping_pending)
                .size(SignalSize::Md),
        );
        if r.clicked() {
            run.push(("list.ping", target_args(m)));
        }
    });
    cell(ui, c1, t("detail", "players"), 1, &mut |ui| {
        let c = m.count.unwrap_or_default();
        let r = ui.add(
            PlayersCell::new(
                u32::try_from(c.players.max(0)).unwrap_or(0),
                u32::try_from(c.max.max(0)).unwrap_or(0),
            )
            .bots(c.bots)
            .loading(m.live_loading),
        );
        if r.clicked() {
            run.push(("servers.detail.refresh_live", target_args(m)));
        }
        if queue > 0 {
            let s = format!(
                "{} · {}",
                t("detail", "loginQueue"),
                tf("detail", "queueWaiting", &[("count", &queue)])
            );
            let g = ph_trunc(
                ui,
                &s,
                font(Face::Mono, size::TEXT_3XS),
                ui.available_width(),
            );
            let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
            draw(ui.painter(), &g, r.left(), r.center().y, tk.warn);
        }
    });
    cell(ui, c2, t("detail", "time"), 2, &mut |ui| {
        ui.add(TimeOfDay::new(time));
    });
    cell(ui, c3, t("detail", "tabMods"), 3, &mut |ui| {
        if let Some(l) = &m.listed {
            let r = ui.add(
                ModsCount::new(l.mods_count)
                    .missing(l.mods_missing)
                    .stale(l.mods_stale)
                    .clickable(true),
            );
            if r.clicked() {
                run.push(("servers.detail.tab", json!({ "tab": "mods" })));
            }
        } else {
            let s = if m.mods_count > 0 {
                m.mods_count.to_string()
            } else {
                crate::i18n::format::NONE.to_string()
            };
            let g = ph(ui, &s, font(Face::Mono, size::TEXT_2XS));
            let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, 20.0), Sense::hover());
            draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
        }
    });
}

/// DayZ Metrics' verdict on the player count, when it is not clean (`PopulationWarning`):
/// shown where a player decides to join.
pub(super) fn population_warning(ui: &mut Ui, m: &Model, compact: bool) {
    let Some(p @ (Population::Fake | Population::Suspect)) = m.population else {
        return;
    };
    let tk = ui.tokens();
    let fake = p == Population::Fake;
    let tone = if fake { tk.err } else { tk.warn };
    let x = m.metrics_data();
    let mut reasons: Vec<String> = Vec::new();
    if let Some(x) = x {
        reasons.extend(x.fake_reasons.iter().filter(|r| !r.is_empty()).cloned());
        if let Some(r) = x.flag_reason.as_ref().filter(|r| !r.is_empty()) {
            reasons.push(r.clone());
        } else if x.flagged {
            reasons.push(t("detail", "dmFlagged").to_string());
        }
        if x.mimics_official {
            reasons.push(t("detail", "dmMimics").to_string());
        }
    }
    let w = ui.available_width();
    let (px, py) = if compact { (8, 4) } else { (10, 8) };
    let r = egui::Frame::new()
        .fill(alpha(tone, 0.1))
        .stroke(Stroke::new(1.0, alpha(tone, 0.4)))
        .corner_radius(CornerRadius::same(size::RADIUS_SM))
        .inner_margin(egui::Margin::symmetric(px, py))
        .show(ui, |ui| {
            ui.set_width(w - 2.0 * f32::from(px) - 2.0);
            line_top(ui, 16.0, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let (ir, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                let icon = if fake {
                    lucide::SHIELD_ALERT
                } else {
                    lucide::TRIANGLE_ALERT
                };
                icon.paint(ui.painter(), ir.translate(Vec2::new(0.0, 1.0)), tone);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let title = if fake {
                        t("detail", "dmFake")
                    } else {
                        t("detail", "dmSuspect")
                    };
                    words(ui, title, font(Face::SansSemibold, size::TEXT_2XS), tone);
                    if !compact {
                        if fake {
                            words(
                                ui,
                                t("detail", "dmFakeHint"),
                                font(Face::Sans, size::TEXT_2XS),
                                tk.fg_muted,
                            );
                        }
                        if !reasons.is_empty() {
                            ui.add_space(2.0);
                        }
                        for r in &reasons {
                            words(
                                ui,
                                &format!("•  {r}"),
                                font(Face::Sans, size::TEXT_2XS),
                                tk.fg_muted,
                            );
                        }
                    }
                });
            });
        });
    let _ = r;
}

// ── the foot ────────────────────────────────────────────────────────────────

/// Always in reach whatever tab is open: join, keep, look closer, hide, copy.
fn action_bar(ui: &mut Ui, st: &mut DetailState, m: &Model, foot: Rect, run: &mut Run) {
    let tk = ui.tokens();
    let p = ui.painter();
    p.rect_filled(foot, CornerRadius::ZERO, alpha(tk.bg, 0.6));
    p.hline(
        foot.x_range(),
        foot.top() + 0.5,
        Stroke::new(1.0, tk.border),
    );
    let inner = Rect::from_min_max(
        Pos2::new(foot.left() + size::PAD, foot.top() + 9.0),
        Pos2::new(foot.right() - size::PAD, foot.bottom() - 8.0),
    );
    let mut c = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .id_salt("server-detail-foot")
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    c.spacing_mut().item_spacing.x = 6.0;
    // The star's square, three icon buttons and the gaps (taller in the pad density).
    let rest = size::control_lg() + 3.0 * size::control() + 4.0 * 6.0;
    let join_w = (inner.width() - rest).max(60.0);
    if c.add(JoinButton::new().size(Size::Lg).width(join_w))
        .clicked()
    {
        run.push(("list.join", target_args(m)));
    }
    // The star in a bordered square as tall as Join.
    let (sq, _) = c.allocate_exact_size(Vec2::splat(size::control_lg()), Sense::hover());
    c.painter().rect_stroke(
        sq,
        CornerRadius::same(size::RADIUS_SM),
        Stroke::new(1.0, tk.border),
        egui::StrokeKind::Inside,
    );
    let mut s = c.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_center_size(
                sq.center(),
                Vec2::splat(size::CONTROL),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    if s.add(FavoriteButton::new(m.favorite).size(StarSize::Md))
        .clicked()
    {
        run.push(("list.fav", target_args(m)));
    }
    if c.add(
        IconButton::new(lucide::PLUG_ZAP, t("detail", "openDirect"))
            .size(Size::Icon)
            .variant(Variant::Default),
    )
    .clicked()
    {
        run.push(("list.direct", target_args(m)));
    }
    if c.add(
        ExcludeButton::new(&m.ip, m.excluded)
            .size(Size::Icon)
            .variant(Variant::Default),
    )
    .clicked()
    {
        run.push(("servers.row.exclude_ip", json!({ "ip": m.ip })));
    }
    let now = ui.input(|i| i.time);
    let copied = st.copied_at.is_some_and(|at| now - at < 1.4);
    if copied {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    }
    let mut b = IconButton::new(
        if copied { lucide::CHECK } else { lucide::COPY },
        t("detail", "copyIp"),
    )
    .size(Size::Icon)
    .variant(Variant::Default);
    if copied {
        b = b.tint(tk.ok);
    }
    if c.add(b).clicked() {
        st.copied_at = Some(now);
        ui.ctx().copy_text(m.address().to_owned());
    }
}
pub use stats::range_of as stats_range_of;

/// A line of `h` points, left to right, centred (`ui.horizontal` would take a control's
/// height).
pub(super) fn line<R>(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(w, h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_height(h);
            add(ui)
        },
    )
    .inner
}

/// A line whose content may wrap below its first `h` points (values of several lines).
pub(super) fn line_top<R>(ui: &mut Ui, h: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(w, h),
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.set_min_height(h);
            add(ui)
        },
    )
    .inner
}
