//! The stats view (= `features/stats/View.svelte`, `Calendar.svelte`, `stats.svelte.ts`):
//! how much the player plays, where and when: totals for a period, a year of days, the
//! servers and maps played most, the hours of the week, and every session ever.
//!
//! The hub works it all out of the session log (`play_stats`, `play_sessions`); the view
//! asks the first time it shows and again on every `PlaySessionsChanged`. What the frame
//! needs from an answer (days by number, the calendar's steps, the week as a grid, the top
//! place and map) is worked out once per answer ([`Derived`]), not per frame; the history
//! lays out only the rows on screen.
//!
//! Files: `dates` (days and the ways they are written), `calendar` (the year of days),
//! `places` (where you play, the maps), `history` (every session).
//! The view's state lives in one [`StatsState`] in egui's memory, so the commands of
//! `commands.rs` (palette, control channel) and the buttons change the same thing.

mod calendar;
mod dates;
mod history;
mod places;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use dz_hub::features::stats::{PlayStatsDto, SessionDto, SessionsPage, StatsRange};
use dz_hub::{Hub, HubEvent};
use egui::mutex::Mutex;
use egui::{Id, Rect, Ui, UiBuilder, Vec2};
use serde_json::Value;

use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::alpha;
use crate::kit::{Button, Segment, Segmented, SkeletonRows, Variant};
use crate::parts::{Empty, Figure, PageHeader, Section, WeekHeat};
use crate::rt::{Debounce, Fetch, Task};
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::{View, ViewId};

pub use calendar::WEEKS;

/// A page of the history.
const PAGE: u32 = 100;
/// Places shown before "Show all".
const PLACES_SHOWN: usize = 10;
/// Typing in the history's search waits this long.
const SEARCH_WAIT: Duration = Duration::from_millis(200);
/// A running session's length is written again this often.
const RUNNING_TICK: Duration = Duration::from_secs(30);

/// Now, in Unix seconds.
fn now() -> i64 {
    crate::store::now_ms() / 1000
}

/// What the frame reads from one answer, worked out when it arrives.
#[derive(Default)]
pub struct Derived {
    /// Seconds and sessions by day number.
    pub by_day: HashMap<i64, (i64, u32)>,
    /// The thresholds between the calendar's four steps: quartiles of the days played.
    pub steps: [i64; 3],
    /// The first session's day (today when there is none).
    pub first_day: i64,
    pub week: [[f64; 24]; 7],
    pub top_place: i64,
    pub top_map: i64,
}

impl Derived {
    fn new(d: &PlayStatsDto, offset: i64, today: i64) -> Self {
        let by_day = d
            .days
            .iter()
            .map(|x| (x.day, (x.secs, x.sessions)))
            .collect();
        let mut v: Vec<i64> = d.days.iter().map(|x| x.secs).filter(|s| *s > 0).collect();
        v.sort_unstable();
        let q = |p: f64| {
            let n = v.len();
            if n == 0 {
                return 0;
            }
            let i = ((p * n as f64).floor() as usize).min(n - 1);
            v.get(i).copied().unwrap_or(0)
        };
        let mut week = [[0.0; 24]; 7];
        for (d_i, row) in d.week_hours.iter().take(7).enumerate() {
            for (h, secs) in row.iter().take(24).enumerate() {
                if let Some(c) = week.get_mut(d_i).and_then(|r| r.get_mut(h)) {
                    *c = *secs as f64;
                }
            }
        }
        Self {
            by_day,
            steps: [q(0.25), q(0.5), q(0.75)],
            first_day: d.first.map_or(today, |f| dates::day_of(f, offset)),
            week,
            top_place: d.places_played.iter().map(|p| p.secs).fold(1, i64::max),
            top_map: d.maps.iter().map(|m| m.secs).fold(1, i64::max),
        }
    }

    /// The calendar's step (0–4) for a day's seconds.
    pub fn level(&self, secs: i64) -> usize {
        let [a, b, c] = self.steps;
        match secs {
            s if s <= 0 => 0,
            s if s <= a => 1,
            s if s <= b => 2,
            s if s <= c => 3,
            _ => 4,
        }
    }
}

/// The answer and what is worked out of it.
pub struct Loaded {
    pub data: PlayStatsDto,
    pub derived: Derived,
}

/// The calendar's own state: the year shown and the day picked.
#[derive(Default, Clone, Copy)]
pub struct CalState {
    /// The last day shown; `None` is today.
    pub end: Option<i64>,
    /// The day under the pointer or the keyboard.
    pub active: Option<i64>,
    /// The pointer was over the grid last frame.
    pub hovering: bool,
}

/// What the view remembers (for the run, as the Svelte store did).
pub struct StatsState {
    started: bool,
    pub range: StatsRange,
    pub data: Fetch<Arc<Loaded>>,
    pub search: String,
    /// The day the history is narrowed to, if any.
    pub day: Option<i64>,
    pub rows: Vec<SessionDto>,
    pub total: u32,
    /// The history's page on its way (`true`: appended); a newer request replaces it.
    history: Option<Task<(bool, Result<SessionsPage, String>)>>,
    search_wait: Debounce,
    pub all_places: bool,
    pub cal: CalState,
    /// Scroll the history into view on the next frame.
    reveal: bool,
    /// The week heat map's widget id (its keyboard cell lives under it).
    week_id: Option<Id>,
    forgets: Vec<Task<Result<bool, String>>>,
}

impl Default for StatsState {
    fn default() -> Self {
        Self {
            started: false,
            range: StatsRange::All,
            data: Fetch::default(),
            search: String::new(),
            day: None,
            rows: Vec::new(),
            total: 0,
            history: None,
            search_wait: Debounce::new(SEARCH_WAIT),
            all_places: false,
            cal: CalState::default(),
            reveal: false,
            week_id: None,
            forgets: Vec::new(),
        }
    }
}

impl StatsState {
    /// The figures for the period (the last answer stays on screen meanwhile).
    pub fn load(&mut self, hub: &Hub) {
        let (hub, range) = (hub.clone(), self.range);
        self.data.load(async move {
            let offset = dates::utc_offset();
            let mins = i32::try_from(offset / 60).unwrap_or(0);
            let data = hub.play_stats(range, mins).await?;
            let today = dates::day_of(now(), offset);
            let derived = Derived::new(&data, offset, today);
            Ok(Arc::new(Loaded { data, derived }))
        });
    }

    /// The history's first page again (`more` false), or the next one.
    pub fn load_history(&mut self, hub: &Hub, more: bool) {
        let hub = hub.clone();
        let search = self.search.clone();
        let bounds = self.day.map(|d| dates::day_bounds(d, dates::utc_offset()));
        let offset = if more {
            u32::try_from(self.rows.len()).unwrap_or(u32::MAX)
        } else {
            0
        };
        self.history = Some(crate::rt::spawn(async move {
            let r = hub
                .play_sessions(
                    search,
                    bounds.map(|b| b.0),
                    bounds.map(|b| b.1),
                    offset,
                    PAGE,
                )
                .await;
            (more, r)
        }));
    }

    fn start(&mut self, hub: &Hub) {
        if !std::mem::replace(&mut self.started, true) {
            self.load(hub);
            self.load_history(hub, false);
        }
    }

    /// Take finished answers in. Errors of the history and of forgetting are toasts.
    fn poll(&mut self, hub: &Hub, ctx: &egui::Context, bus: &crate::store::Bus) {
        self.data.poll();
        if let Some(task) = self.history.as_mut()
            && let Some((more, r)) = task.poll()
        {
            self.history = None;
            match r {
                Ok(page) => {
                    if more {
                        self.rows.extend(page.rows);
                    } else {
                        self.rows = page.rows;
                    }
                    self.total = page.total;
                }
                Err(e) => bus.err(e),
            }
        }
        let mut failed = false;
        self.forgets.retain_mut(|task| match task.poll() {
            Some(Err(e)) => {
                bus.err(e);
                failed = true;
                false
            }
            Some(Ok(_)) => false,
            None => task.is_pending(),
        });
        if failed {
            self.load_history(hub, false);
        }
        if self.search_wait.ready(ctx) {
            self.load_history(hub, false);
        }
    }

    pub fn loaded(&self) -> Option<&Arc<Loaded>> {
        self.data.value.as_ref()
    }

    pub fn set_range(&mut self, hub: &Hub, r: StatsRange) {
        if self.range != r {
            self.range = r;
            self.load(hub);
        }
    }

    /// Narrow the history to one day (`None`: every day).
    pub fn set_day(&mut self, hub: &Hub, day: Option<i64>) {
        self.day = day;
        self.load_history(hub, false);
    }

    /// The search changed by typing: ask once typing stops.
    pub fn typed(&mut self, ctx: &egui::Context) {
        self.search_wait.touch(ctx);
    }

    /// Narrow the history to what matches `text`, at once (a server, a map).
    pub fn pick_search(&mut self, hub: &Hub, text: &str) {
        self.search_wait = Debounce::new(SEARCH_WAIT);
        self.search = text.to_owned();
        self.load_history(hub, false);
    }

    /// How many more rows "Show more" brings.
    pub fn next_page(&self) -> u32 {
        PAGE.min(
            self.total
                .saturating_sub(u32::try_from(self.rows.len()).unwrap_or(0)),
        )
    }

    /// Forget a finished session: off the list at once, the hub's event reloads it.
    pub fn forget(&mut self, hub: &Hub, start: i64, name: &str) {
        let before = self.rows.len();
        self.rows
            .retain(|r| r.open || r.start != start || r.name != name);
        if self.rows.len() == before {
            return;
        }
        self.total = self.total.saturating_sub(1);
        let (hub, name) = (hub.clone(), name.to_owned());
        self.forgets.push(crate::rt::spawn(async move {
            hub.delete_session(start, name).await
        }));
    }

    /// Today's number, the year's last day shown.
    fn today() -> i64 {
        dates::day_of(now(), dates::utc_offset())
    }

    /// Move the year shown by `by` days, within the first session's year and today.
    pub fn shift_year(&mut self, by: i64) {
        let today = Self::today();
        let first = self
            .loaded()
            .map_or(today, |l| l.derived.first_day.min(today));
        let end = self.cal.end.unwrap_or(today);
        let lo = first + WEEKS * 7 - 7;
        let next = (end + by).max(lo).min(today);
        self.cal.end = (next != today).then_some(next);
        self.cal.active = None;
    }

    /// Arrow keys on the calendar: `by` days (±7 a week, ±1 a day), never past today; off
    /// the left edge the year moves with it.
    pub fn move_day(&mut self, by: i64) {
        let today = Self::today();
        let end = self.cal.end.unwrap_or(today);
        let start = dates::first_monday(end, WEEKS);
        let next = (self.cal.active.unwrap_or(today) + by).min(today);
        if next < start {
            let e = (end - WEEKS * 7 + 7).max(next + (WEEKS - 1) * 7).min(today);
            self.cal.end = (e != today).then_some(e);
        }
        self.cal.active = Some(next);
    }
}

type Shared = Arc<Mutex<StatsState>>;

/// The view's state, shared with its commands.
pub fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.stats.view"), || {
            Arc::new(Mutex::new(StatsState::default()))
        })
        .clone()
    })
}

fn with<R>(cx: &Ctx, f: impl FnOnce(&mut StatsState) -> R) -> R {
    let s = shared(cx.egui);
    let mut st = s.lock();
    f(&mut st)
}

/// A place's name as the player knows it: an offline mission's map, "Started outside
/// the launcher" for an unknown one, else the server's name.
pub fn name_of(kind: dz_hub::features::stats::PlayKind, name: &str) -> String {
    use dz_hub::features::stats::PlayKind;
    match kind {
        PlayKind::Offline => crate::views::offline::describe(name).map,
        PlayKind::Unknown => t("stats", "unknownPlace").to_owned(),
        PlayKind::Server => name.to_owned(),
    }
}

/// "1 session" / "12 sessions".
pub fn sessions_text(n: u32) -> String {
    if n == 1 {
        t("stats", "placeSessionsOne").to_owned()
    } else {
        tf("stats", "placeSessions", &[("count", &n)])
    }
}

/// A duration, or a dash for none.
pub fn dur_or_dash(secs: i64) -> String {
    if secs > 0 {
        format::duration(secs)
    } else {
        format::NONE.to_owned()
    }
}

fn range_id(r: StatsRange) -> &'static str {
    match r {
        StatsRange::Week => "week",
        StatsRange::Month => "month",
        StatsRange::Year => "year",
        StatsRange::All => "all",
    }
}

fn range_of(id: &str) -> Option<StatsRange> {
    match id {
        "week" => Some(StatsRange::Week),
        "month" => Some(StatsRange::Month),
        "year" => Some(StatsRange::Year),
        "all" => Some(StatsRange::All),
        _ => None,
    }
}

/// What a click asked for: a command and its arguments, run once the frame is drawn.
pub type Run = Vec<(&'static str, Value)>;

pub struct StatsView;

impl StatsView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StatsView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for StatsView {
    fn on_show(&mut self, cx: &mut Ctx) {
        let hub = cx.hub.clone();
        with(cx, |st| st.start(&hub));
    }

    fn on_event(&mut self, ev: &HubEvent, cx: &mut Ctx) {
        if matches!(ev, HubEvent::PlaySessionsChanged(_)) {
            let hub = cx.hub.clone();
            with(cx, |st| {
                if st.started {
                    st.load(&hub);
                    st.load_history(&hub, false);
                }
            });
        }
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let shared = shared(ui.ctx());
        let mut run: Run = Vec::new();
        {
            let mut st = shared.lock();
            st.poll(cx.hub, ui.ctx(), &cx.stores.bus);
            // Ctrl+F / `/`: the history's search field (when the history is drawn).
            if cx.nav.take_focus_search() {
                ui.memory_mut(|m| m.request_focus(egui::Id::new("stats-history-search")));
            }
            draw(ui, cx, &mut st, &mut run);
        }
        for (id, args) in run {
            if let Err(e) = cx.run(id, &args) {
                cx.stores.bus.err(e);
            }
        }
    }
}

/// Header, then the page under it, scrolling.
fn draw(ui: &mut Ui, cx: &Ctx, st: &mut StatsState, run: &mut Run) {
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
    header(&mut v, st, run);
    let rest = v.available_rect_before_wrap();
    let mut body = v.new_child(
        UiBuilder::new()
            .max_rect(rest)
            .id_salt("stats-body")
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    body.set_clip_rect(rest.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical()
        .id_salt("stats-scroll")
        .auto_shrink([false, false])
        .show(&mut body, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            page(ui, cx, st, run);
        });
}

fn header(ui: &mut Ui, st: &mut StatsState, out: &mut Run) {
    let tk = ui.tokens();
    let run = std::cell::RefCell::new(Vec::<(&'static str, Value)>::new());
    let loaded = st.loaded().cloned();
    let mut range = st.range;
    let toolbar = |ui: &mut Ui| {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(w, size::CONTROL),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let opts = [
                    Segment::new(StatsRange::Week, t("stats", "rangeWeek")),
                    Segment::new(StatsRange::Month, t("stats", "rangeMonth")),
                    Segment::new(StatsRange::Year, t("stats", "rangeYear")),
                    Segment::new(StatsRange::All, t("stats", "rangeAll")),
                ];
                let r = ui.add(Segmented::new(&mut range, &opts));
                if r.changed() {
                    run.borrow_mut().push((
                        "stats.set_range",
                        serde_json::json!({ "range": range_id(range) }),
                    ));
                }
                if let Some(cur) = loaded.as_ref().and_then(|l| l.data.current.as_ref()) {
                    playing_now(ui, cur);
                }
            },
        );
        ui.add_space(1.0);
    };
    PageHeader::new(t("nav", "stats")).show(
        ui,
        |ui| {
            let Some(l) = loaded.as_ref() else {
                return;
            };
            let d = &l.data;
            // No session yet: a dash, not "<1m" (which reads as a session too short to count).
            let (time, tone) = if d.sessions == 0 && d.total_secs <= 0 {
                (format::NONE.to_owned(), tk.fg_muted)
            } else {
                (format::duration(d.total_secs), tk.accent)
            };
            ui.add(Figure::new(t("stats", "figTime")).value(&time).tone(tone));
            ui.add(Figure::new(t("stats", "figSessions")).amount(f64::from(d.sessions)));
            ui.add(Figure::new(t("stats", "figPlaces")).amount(f64::from(d.places)));
            let avg = if d.average_secs > 0 {
                format::duration(d.average_secs)
            } else {
                format::NONE.to_owned()
            };
            ui.add(
                Figure::new(t("stats", "figAverage"))
                    .value(&avg)
                    .secondary(),
            );
            let (longest, longest_tip) = match &d.longest {
                Some(s) => (
                    format::duration(s.secs),
                    format!(
                        "{} · {}",
                        name_of(s.kind, &s.name),
                        format::date_time(s.start)
                    ),
                ),
                None => (format::NONE.to_owned(), String::new()),
            };
            ui.add(
                Figure::new(t("stats", "figLongest"))
                    .value(&longest)
                    .tip(&longest_tip)
                    .secondary(),
            );
            let streak = tf("stats", "streakDays", &[("count", &d.streak)]);
            let best = tf("stats", "streakDays", &[("count", &d.best_streak)]);
            let streak_tip = tf("stats", "streakTitle", &[("best", &best)]);
            ui.add(
                Figure::new(t("stats", "figStreak"))
                    .value(&streak)
                    .tone(if d.streak > 0 { tk.ok } else { tk.fg_muted })
                    .tip(&streak_tip),
            );
        },
        |ui| {
            if ui
                .add(
                    Button::new(t("stats", "exportProfile"))
                        .icon(lucide::FILE_DOWN)
                        .tip(t("stats", "exportHint")),
                )
                .clicked()
            {
                run.borrow_mut().push((
                    "profile.export",
                    serde_json::json!({ "include_mods": false }),
                ));
            }
            if ui
                .add(
                    Button::new(t("stats", "importProfile"))
                        .icon(lucide::FILE_UP)
                        .tip(t("stats", "exportHint")),
                )
                .clicked()
            {
                run.borrow_mut().push(("profile.import", Value::Null));
            }
        },
        Some(toolbar),
    );
    out.extend(run.into_inner());
}

/// The green pill at the end of the toolbar while a session runs: "Playing … for 1h 13m".
fn playing_now(ui: &mut Ui, cur: &SessionDto) {
    let tk = ui.tokens();
    let n = now();
    let secs = cur.secs.max(n - cur.start);
    let text = tf(
        "stats",
        "playingNow",
        &[
            ("name", &name_of(cur.kind, &cur.name)),
            ("duration", &format::duration(secs)),
        ],
    );
    let ctx = ui.ctx().clone();
    ctx.request_repaint_after(RUNNING_TICK);
    let avail = ui.available_width();
    let f = font(Face::Sans, size::TEXT_2XS);
    // px-2 py-1, icon 14, gap 6.
    let inner_max = (avail - 16.0 - 14.0 - 6.0).max(0.0);
    let g = crate::parts::ph_trunc(ui, &text, f, inner_max);
    let w = (16.0 + 14.0 + 6.0 + g.size().x).min(avail);
    let h = 26.0;
    let x0 = ui.cursor().left() + (avail - w).max(0.0);
    let cy = ui.max_rect().center().y;
    let rect = Rect::from_min_size(egui::pos2(x0, cy - h / 2.0), Vec2::new(w, h));
    let resp = ui.allocate_rect(
        Rect::from_min_max(egui::pos2(ui.cursor().left(), rect.top()), rect.max),
        egui::Sense::hover(),
    );
    let p = ui.painter();
    crate::kit::paint::plate(
        p,
        rect,
        size::RADIUS_SM,
        alpha(tk.ok, 0.1),
        alpha(tk.ok, 0.4),
    );
    // `animate-pulse`: opacity 1 → .5 → 1 over 2 s; still with reduced motion.
    let pulse = if crate::app::reduced_motion(&ctx) {
        1.0
    } else {
        let tnow = ctx.input(|i| i.time);
        ctx.request_repaint_after(Duration::from_millis(50));
        let ph = (tnow % 2.0) / 2.0;
        1.0 - 0.5 * (std::f64::consts::PI * ph).sin().powi(2) as f32
    };
    let ir = Rect::from_min_size(
        egui::pos2(rect.left() + 8.0, rect.center().y - 7.0),
        Vec2::splat(14.0),
    );
    lucide::RADIO.paint(p, ir, tk.ok.gamma_multiply(pulse));
    crate::parts::draw(p, &g, ir.right() + 6.0, rect.center().y, tk.ok);
    if crate::parts::is_cut(&g, &text) {
        resp.on_hover_text(text);
    }
}

/// The page under the header: a skeleton, the error, the empty state, or the sections.
fn page(ui: &mut Ui, cx: &Ctx, st: &mut StatsState, run: &mut Run) {
    let _ = cx;
    if st.data.first_load() {
        ui.add(SkeletonRows::new(10));
        return;
    }
    if let Some(e) = st.data.error.clone() {
        let h = ui.available_height().max(240.0);
        ui.allocate_ui(Vec2::new(ui.available_width(), h), |ui| {
            Empty::new()
                .icon(lucide::CHART_NO_AXES_COLUMN)
                .title(t("stats", "emptyTitle"))
                .detail(&e)
                .show(ui, |_| {});
        });
        return;
    }
    let Some(l) = st.loaded().cloned() else {
        return;
    };
    if l.data.first.is_none() {
        let h = ui.available_height().max(240.0);
        ui.allocate_ui(Vec2::new(ui.available_width(), h), |ui| {
            Empty::new()
                .icon(lucide::CHART_NO_AXES_COLUMN)
                .title(t("stats", "emptyTitle"))
                .text(t("stats", "emptyHint"))
                .show(ui, |ui| {
                    if ui
                        .add(Button::new(t("favorites", "browseServers")).variant(Variant::Accent))
                        .clicked()
                    {
                        run.push(("view.servers", Value::Null));
                    }
                });
        });
        return;
    }
    let tk = ui.tokens();

    // Activity: the year of days.
    Section::new(t("stats", "sectionActivity"))
        .icon(lucide::CALENDAR_DAYS)
        .first()
        .show(
            ui,
            |_| {},
            |ui| {
                let w = ui.available_width().min(1024.0);
                ui.allocate_ui(Vec2::new(w, 0.0), |ui| {
                    ui.set_width(w);
                    if let Some(day) = calendar::show(ui, &l, st, run) {
                        run.push(("stats.filter_day", serde_json::json!({ "day": day })));
                    }
                });
                ui.add_space(4.0);
                let g = crate::parts::ph_trunc(
                    ui,
                    t("stats", "pickHint"),
                    font(Face::Sans, size::TEXT_3XS),
                    ui.available_width(),
                );
                let (r, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), g.size().y),
                    egui::Sense::hover(),
                );
                crate::parts::draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
            },
        );

    // Where, when, which maps: two columns, one under 1100px of window.
    let stacked = ui.ctx().content_rect().width() < 1100.0;
    let w = ui.available_width();
    let top = ui.cursor().top();
    let x0 = ui.cursor().left();
    if stacked {
        places::places(ui, &l, st, run);
        week(ui, &l, st);
        places::maps(ui, &l, run);
    } else {
        let left_w = ((w - 1.0) * 1.6 / 2.6).floor();
        let lr = Rect::from_min_size(egui::pos2(x0, top), Vec2::new(left_w, 10_000.0));
        let rr = Rect::from_min_max(
            egui::pos2(x0 + left_w + 1.0, top),
            egui::pos2(x0 + w, top + 10_000.0),
        );
        let mut lu = ui.new_child(
            UiBuilder::new()
                .max_rect(lr)
                .id_salt("stats-left")
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        lu.spacing_mut().item_spacing = Vec2::ZERO;
        places::places(&mut lu, &l, st, run);
        let mut ru = ui.new_child(
            UiBuilder::new()
                .max_rect(rr)
                .id_salt("stats-right")
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        ru.spacing_mut().item_spacing = Vec2::ZERO;
        week(&mut ru, &l, st);
        places::maps(&mut ru, &l, run);
        let bottom = lu.min_rect().bottom().max(ru.min_rect().bottom());
        ui.painter().vline(
            x0 + left_w + 0.5,
            egui::Rangef::new(top, bottom),
            egui::Stroke::new(1.0, tk.border),
        );
        ui.allocate_rect(
            Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x0 + w, bottom)),
            egui::Sense::hover(),
        );
    }

    // Every session.
    let hist_top = ui.cursor().top();
    history::show(ui, st, run);
    if std::mem::take(&mut st.reveal) {
        // `scroll-mt-2`: the section's top 8px under the edge.
        let r = Rect::from_min_size(
            egui::pos2(x0, hist_top - 8.0),
            Vec2::new(w, ui.clip_rect().height().max(1.0)),
        );
        ui.scroll_to_rect(r, Some(egui::Align::Min));
    }
}

/// "When you play": the week's hours.
fn week(ui: &mut Ui, l: &Loaded, st: &mut StatsState) {
    Section::new(t("stats", "sectionWhen"))
        .icon(lucide::CLOCK)
        .show(
            ui,
            |_| {},
            |ui| {
                let resp = ui
                    .push_id("stats-week", |ui| {
                        WeekHeat::new(
                            &l.derived.week,
                            |day, hour, v| {
                                tf(
                                    "stats",
                                    "whenCell",
                                    &[
                                        ("day", &day),
                                        ("hour", &hour),
                                        ("duration", &dur_or_dash(v as i64)),
                                    ],
                                )
                            },
                            t("stats", "whenHint"),
                        )
                        .show(ui)
                    })
                    .inner;
                st.week_id = Some(resp.id);
            },
        );
}

// ── commands ────────────────────────────────────────────────────────────────

fn hub_of(cx: &Ctx) -> Hub {
    cx.hub.clone()
}

/// Every stats command: `stats.<name>`; the shell's ids (`commands.rs`) call this.
pub fn command(cx: &mut Ctx, name: &str, a: &Value) {
    let hub = hub_of(cx);
    let num = |k: &str| a.get(k).and_then(Value::as_i64);
    let text = |k: &str| a.get(k).and_then(Value::as_str).map(str::to_owned);
    match name {
        "set_range" => {
            if let Some(r) = text("range").as_deref().and_then(range_of) {
                with(cx, |st| st.set_range(&hub, r));
            }
        }
        "calendar_prev" => with(cx, |st| st.shift_year(-WEEKS * 7 + 7)),
        "calendar_next" => with(cx, |st| st.shift_year(WEEKS * 7 - 7)),
        "calendar_navigate" => {
            let by = match text("dir").as_deref() {
                Some("left") => -7,
                Some("right") => 7,
                Some("up") => -1,
                Some("down") => 1,
                _ => num("by").unwrap_or(0).clamp(-3660, 3660),
            };
            with(cx, |st| st.move_day(by));
        }
        "filter_day" => {
            let day = num("day").filter(|d| (0..=200_000).contains(d));
            with(cx, |st| {
                st.set_day(&hub, day);
                st.reveal = day.is_some();
            });
        }
        "clear_day" => with(cx, |st| st.set_day(&hub, None)),
        "filter_text" => {
            let t = text("text").unwrap_or_default();
            with(cx, |st| {
                st.pick_search(&hub, &t);
                st.reveal = true;
            });
        }
        "history_search" => {
            let t = text("text").unwrap_or_default();
            with(cx, |st| st.pick_search(&hub, &t));
        }
        "history_more" => with(cx, |st| {
            if st.next_page() > 0 {
                st.load_history(&hub, true);
            }
        }),
        "toggle_all_places" => with(cx, |st| st.all_places = !st.all_places),
        "forget_session" => {
            if let (Some(start), Some(name)) = (num("start"), text("name")) {
                with(cx, |st| st.forget(&hub, start, &name));
            }
        }
        "week_navigate" => {
            let mv = match text("dir").as_deref() {
                Some("up") => (-1, 0),
                Some("down") => (1, 0),
                Some("left") => (0, -1),
                Some("right") => (0, 1),
                Some("home") => (0, -24),
                Some("end") => (0, 24),
                _ => (0, 0),
            };
            if let Some(id) = with(cx, |st| st.week_id) {
                use chrono::{Datelike, Timelike};
                let key = id.with("key-cell");
                let n = chrono::Local::now();
                let now_cell = (n.weekday().num_days_from_monday(), n.hour());
                cx.egui.data_mut(|d| {
                    let a: (u32, u32) = d.get_temp(key).unwrap_or(now_cell);
                    let next = (
                        (a.0 as i32 + mv.0).clamp(0, 6) as u32,
                        (a.1 as i32 + mv.1).clamp(0, 23) as u32,
                    );
                    d.insert_temp(key, next);
                });
                cx.egui.memory_mut(|m| m.request_focus(id));
            }
        }
        _ => {}
    }
    cx.go(ViewId::Stats);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dz_hub::features::stats::DayStatDto;

    fn dto(days: &[(i64, i64)]) -> PlayStatsDto {
        PlayStatsDto {
            total_secs: 0,
            sessions: 0,
            places: 0,
            average_secs: 0,
            longest: None,
            days_played: 0,
            streak: 0,
            best_streak: 0,
            first: None,
            current: None,
            places_played: Vec::new(),
            maps: Vec::new(),
            days: days
                .iter()
                .map(|(day, secs)| DayStatDto {
                    day: *day,
                    secs: *secs,
                    sessions: 1,
                })
                .collect(),
            week_hours: vec![vec![0; 24]; 7],
        }
    }

    #[test]
    fn steps_are_quartiles_of_played_days() {
        let d = Derived::new(&dto(&[(1, 10), (2, 20), (3, 30), (4, 40), (5, 0)]), 0, 9);
        assert_eq!(d.steps, [20, 30, 40]);
        assert_eq!(d.level(0), 0);
        assert_eq!(d.level(10), 1);
        assert_eq!(d.level(20), 1);
        assert_eq!(d.level(25), 2);
        assert_eq!(d.level(41), 4);
        assert_eq!(d.first_day, 9);
        assert_eq!(d.top_place, 1);
        let empty = Derived::new(&dto(&[]), 0, 0);
        assert_eq!(empty.steps, [0, 0, 0]);
    }

    #[test]
    fn ranges_round_trip() {
        for r in [
            StatsRange::Week,
            StatsRange::Month,
            StatsRange::Year,
            StatsRange::All,
        ] {
            assert_eq!(range_of(range_id(r)), Some(r));
        }
        assert_eq!(range_of("decade"), None);
    }

    #[test]
    fn next_page_counts_what_is_left() {
        let mut st = StatsState {
            total: 180,
            ..Default::default()
        };
        assert_eq!(st.next_page(), 100);
        st.rows = vec![
            SessionDto {
                kind: dz_hub::features::stats::PlayKind::Server,
                name: "a".into(),
                ip: None,
                port: None,
                map: None,
                start: 0,
                end: 0,
                secs: 0,
                open: false,
                measured: true,
            };
            100
        ];
        assert_eq!(st.next_page(), 80);
    }
}
