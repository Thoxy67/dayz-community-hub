//! What the browser asks for (= `filters.svelte.ts`), its line of filters (=
//! `Toolbar.svelte`, `TriChip.svelte`) and the filters in force in words (=
//! `ActiveFilters.svelte`).
//!
//! The choices live in the view's [`super::UiState`]; each frame they become the
//! `servers` store's [`ServerQuery`] (typing debounced there, toggles at once), and the
//! hub's order answers it. Buttons here push commands (`servers.filter.*`), so the
//! palette, the keys and the control channel change the same thing.

use std::sync::Arc;

use dz_hub::features::browser::{DayTime, ServerQuery, SortCol, Tri};
use egui::{CornerRadius, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};
use serde_json::{Value, json};

use super::Run;
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{
    Align, FieldSize, Input, MultiSelect, Segment, Segmented, SelectOption, Side, Switch, TipExt,
    menu,
};
use crate::parts::{draw, ph};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// The ping ceilings "More filters" offers (0 = any).
pub const PINGS: [u32; 5] = [0, 50, 100, 150, 250];

/// The browser's choices: the search as typed, the filters, the order.
#[derive(Clone, Debug, PartialEq)]
pub struct Filters {
    pub search: String,
    /// The maps shown (none for every map).
    pub maps: Vec<String>,
    pub first_person: Tri,
    pub password: Tri,
    pub battleye: Tri,
    pub modded: Tri,
    pub official: Tri,
    pub hide_empty: bool,
    pub hide_full: bool,
    /// Hide servers slower than this (ms); 0 for any.
    pub max_ping: u32,
    pub show_excluded: bool,
    /// Servers at this time of day now (in game).
    pub time_of_day: DayTime,
    /// Servers running a mod: part of its name, or its Workshop id.
    pub with_mod: String,
    /// The mod picked from the suggestions, by Workshop id: `with_mod` is then its name,
    /// shown in the field and the chip, and the filter matches this one mod only.
    pub with_mod_id: Option<u64>,
    /// Only servers a join starts at once (mods installed and current, same version).
    pub ready: bool,
    pub sort: SortCol,
    pub asc: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            search: String::new(),
            maps: Vec::new(),
            first_person: Tri::All,
            password: Tri::All,
            battleye: Tri::All,
            modded: Tri::All,
            official: Tri::All,
            hide_empty: false,
            hide_full: false,
            max_ping: 0,
            show_excluded: false,
            time_of_day: DayTime::Any,
            with_mod: String::new(),
            with_mod_id: None,
            ready: false,
            sort: SortCol::None,
            asc: true,
        }
    }
}

/// A time of day as the commands write it.
pub fn day_time_id(d: DayTime) -> &'static str {
    match d {
        DayTime::Any => "any",
        DayTime::Day => "day",
        DayTime::Night => "night",
    }
}

pub fn day_time_from(id: &str) -> Option<DayTime> {
    Some(match id {
        "any" | "all" => DayTime::Any,
        "day" => DayTime::Day,
        "night" => DayTime::Night,
        _ => return None,
    })
}

/// A time of day in words.
pub fn day_time_words(d: DayTime) -> &'static str {
    match d {
        DayTime::Any => t("servers", "timeAny"),
        DayTime::Day => t("servers", "timeDay"),
        DayTime::Night => t("servers", "timeNight"),
    }
}

/// One click moves to the next answer: every server, only these, none of these.
pub fn tri_next(t: Tri) -> Tri {
    match t {
        Tri::All => Tri::Only,
        Tri::Only => Tri::None,
        Tri::None => Tri::All,
    }
}

/// A column by the id the commands and the Svelte interface use.
pub fn sort_col(id: &str) -> Option<SortCol> {
    Some(match id {
        "ping" => SortCol::Ping,
        "players" => SortCol::Players,
        "name" | "server" => SortCol::Name,
        "map" => SortCol::Map,
        "time" => SortCol::Time,
        "mods" => SortCol::Mods,
        "none" => SortCol::None,
        _ => return None,
    })
}

impl Filters {
    /// The request (the search trimmed; the window is the whole order).
    pub fn query(&self) -> ServerQuery {
        ServerQuery {
            search: self.search.trim().to_string(),
            maps: self.maps.clone(),
            first_person: self.first_person,
            password: self.password,
            battleye: self.battleye,
            modded: self.modded,
            official: self.official,
            hide_empty: self.hide_empty,
            hide_full: self.hide_full,
            max_ping: self.max_ping,
            show_excluded: self.show_excluded,
            time_of_day: self.time_of_day,
            with_mod: self
                .with_mod_id
                .map_or_else(|| self.with_mod.trim().to_string(), |id| id.to_string()),
            ready: self.ready,
            // The store sets the game's version.
            game_version: None,
            sort: self.sort,
            asc: self.asc,
            offset: 0,
            limit: 0,
        }
    }

    /// Any filter in force (not the order, not "show excluded").
    pub fn active(&self) -> bool {
        !self.search.trim().is_empty()
            || !self.maps.is_empty()
            || self.first_person != Tri::All
            || self.password != Tri::All
            || self.battleye != Tri::All
            || self.modded != Tri::All
            || self.official != Tri::All
            || self.hide_empty
            || self.hide_full
            || self.max_ping > 0
            || self.time_of_day != DayTime::Any
            || !self.with_mod.trim().is_empty()
            || self.ready
    }

    pub fn clear(&mut self) {
        self.search.clear();
        self.maps.clear();
        self.first_person = Tri::All;
        self.password = Tri::All;
        self.battleye = Tri::All;
        self.modded = Tri::All;
        self.official = Tri::All;
        self.hide_empty = false;
        self.hide_full = false;
        self.max_ping = 0;
        self.time_of_day = DayTime::Any;
        self.with_mod.clear();
        self.with_mod_id = None;
        self.ready = false;
    }

    /// The same column flips; a new one starts A→Z for names, smallest first for ping,
    /// biggest first for the figures.
    pub fn toggle_sort(&mut self, col: SortCol) {
        if col == SortCol::None {
            self.sort = SortCol::None;
            self.asc = true;
        } else if self.sort == col {
            self.asc = !self.asc;
        } else {
            self.sort = col;
            self.asc = matches!(col, SortCol::Name | SortCol::Map | SortCol::Ping);
        }
    }

    /// How many of "More filters"' choices are on (its badge).
    pub fn more_count(&self) -> usize {
        usize::from(self.max_ping > 0)
            + usize::from(self.hide_empty)
            + usize::from(self.hide_full)
            + usize::from(self.show_excluded)
            + usize::from(self.time_of_day != DayTime::Any)
            + usize::from(!self.with_mod.trim().is_empty())
            + usize::from(self.ready)
    }

    /// One of the three-way filters by its key.
    pub fn tri_mut(&mut self, key: &str) -> Option<&mut Tri> {
        Some(match key {
            "fp" | "1pp" | "first_person" => &mut self.first_person,
            "pwd" | "password" => &mut self.password,
            "be" | "battleye" => &mut self.battleye,
            "mods" | "modded" => &mut self.modded,
            "off" | "official" => &mut self.official,
            _ => return None,
        })
    }

    /// Clear one filter by its chip's key (`ActiveFilters`).
    pub fn remove(&mut self, key: &str) {
        match key {
            "search" => self.search.clear(),
            "map" => self.maps.clear(),
            "ping" => self.max_ping = 0,
            "empty" => self.hide_empty = false,
            "full" => self.hide_full = false,
            "time" => self.time_of_day = DayTime::Any,
            "mod" => {
                self.with_mod.clear();
                self.with_mod_id = None;
            }
            "ready" => self.ready = false,
            k => {
                if let Some(t) = self.tri_mut(k) {
                    *t = Tri::All;
                }
            }
        }
    }
}

// ── the line of filters ─────────────────────────────────────────────────────

/// What the toolbar needs from the stores, read before drawing.
pub struct ToolbarData<'a> {
    pub shown: usize,
    pub total: u32,
    pub maps: &'a [dz_hub::features::browser::MapCount],
    /// Moves when `maps` was replaced (a new list): what is made from them is kept
    /// until then.
    pub maps_generation: u64,
    pub has_key: bool,
    pub excluded: usize,
    pub focus_search: bool,
    pub open_more: bool,
    /// The mods on disk, for the mod filter's suggestions.
    pub installed: &'a [dz_hub::features::mods::InstalledModDto],
}

/// `Toolbar.svelte`: search, the three-way chips, the map, "More filters", the count.
/// The line never wraps.
pub fn toolbar(ui: &mut Ui, f: &mut Filters, d: &ToolbarData, run: &mut Run) {
    let tk = ui.tokens();
    let win_w = ui.ctx().content_rect().width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        // The count, at the right end, is measured first so the search can flex.
        let count_l = format::num(d.shown as f64);
        let count_r = format!(" / {}", format::num(f64::from(d.total)));
        let mono = font(Face::Mono, size::TEXT_2XS);
        let gl = ph(ui, &count_l, mono.clone());
        let gr = ph(ui, &count_r, mono);
        let count_w = gl.size().x + gr.size().x + 4.0;

        let compact = win_w < 1500.0;
        let chips_w = chips_width(ui, compact);
        let more_w =
            if win_w < 1536.0 { 34.0 } else { 118.0 } + if f.more_count() > 0 { 22.0 } else { 0.0 };
        let rest = chips_w + 160.0 + more_w + count_w + 6.0 * 4.0;
        let max_search = if win_w >= 1280.0 {
            320.0
        } else {
            f32::INFINITY
        };
        let search_w = (ui.available_width() - rest).clamp(128.0, max_search);
        let r = ui.add(
            Input::new(&mut f.search)
                .search()
                .placeholder(t("servers", "searchPlaceholder"))
                .clear_label(t("servers", "clearSearch"))
                .width(search_w)
                .char_limit(200)
                .id_salt("servers-search"),
        );
        if d.focus_search {
            r.request_focus();
        }
        if r.changed() {
            ui.ctx().request_repaint();
        }

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let fp = t("servers", "filterFpTitleAll");
            chip(ui, f.first_person, "1PP", fp, None, false, "fp", run);
            chip(
                ui,
                f.password,
                t("servers", "passwordProtected"),
                t("servers", "filterPwdTitleAll"),
                Some(lucide::LOCK),
                compact,
                "pwd",
                run,
            );
            chip(
                ui,
                f.battleye,
                "BattlEye",
                t("servers", "filterBeTitleAll"),
                Some(lucide::SHIELD_CHECK),
                compact,
                "be",
                run,
            );
            chip(
                ui,
                f.modded,
                t("servers", "colMods"),
                t("servers", "filterModsTitleAll"),
                Some(lucide::PUZZLE),
                compact,
                "mods",
                run,
            );
            let off_tip = if d.has_key {
                t("servers", "filterOfficialTitleAll")
            } else {
                t("servers", "filterOfficialNoKey")
            };
            chip(
                ui,
                f.official,
                t("servers", "official"),
                off_tip,
                Some(lucide::BADGE_CHECK),
                compact,
                "off",
                run,
            );
        });

        // The maps: "<map> · <count>", busiest first, several at once, found by typing;
        // the placeholder clears them.
        let names = map_names(ui.ctx(), d.maps, d.maps_generation);
        let opts: Vec<SelectOption<&str>> = d
            .maps
            .iter()
            .zip(&names.labels)
            .map(|(m, l)| SelectOption::new(m.map.as_str(), l.as_str()))
            .collect();
        // As the list spells them (a command may have named one in another case).
        let mut maps: Vec<&str> = f
            .maps
            .iter()
            .map(|m| {
                let low = m.to_lowercase();
                d.maps
                    .iter()
                    .zip(&names.lower)
                    .find(|(_, l)| **l == low)
                    .map_or(m.as_str(), |(c, _)| c.map.as_str())
            })
            .collect();
        let summary = match maps.as_slice() {
            [] => String::new(),
            [one] => (*one).to_string(),
            many => tf("servers", "mapsCount", &[("count", &many.len())]),
        };
        let r = ui.add(
            MultiSelect::new("servers-map", &mut maps, &opts)
                .placeholder(t("servers", "allMaps"))
                .summary(&summary)
                .search(t("servers", "mapSearch"), t("servers", "clearSearch"))
                .empty(t("servers", "mapNoMatch"))
                .width(160.0)
                .list_width(240.0)
                .label(t("servers", "colMap")),
        );
        // A change, or a name a command gave in another case, now spelled as listed.
        if r.changed() || !maps.iter().copied().eq(f.maps.iter().map(String::as_str)) {
            run.push(("servers.filter.map", json!({ "maps": maps })));
        }

        more_filters(ui, f, d, win_w, run);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(count_w, size::CONTROL), Sense::hover());
            let x = rect.right() - gr.size().x - gl.size().x;
            draw(ui.painter(), &gl, x, rect.center().y, tk.fg);
            draw(
                ui.painter(),
                &gr,
                x + gl.size().x,
                rect.center().y,
                tk.fg_faint,
            );
        });
    });
}

/// The map choices' words, made once per list of maps (not each frame, nor hashed
/// each frame: keyed on the list's generation): each map's label ("<map> · <count>")
/// and its name lowercased.
#[derive(Default)]
struct MapNames {
    key: u64,
    labels: Vec<String>,
    lower: Vec<String>,
}

fn map_names(
    ctx: &egui::Context,
    maps: &[dz_hub::features::browser::MapCount],
    generation: u64,
) -> Arc<MapNames> {
    // The length too: a guard should the maps ever change under one generation.
    let key = generation ^ ((maps.len() as u64) << 48);
    let id = egui::Id::new("dzch.servers.map_names");
    if let Some(n) = ctx.data(|d| d.get_temp::<Arc<MapNames>>(id))
        && n.key == key
    {
        return n;
    }
    let n = Arc::new(MapNames {
        key,
        labels: maps
            .iter()
            .map(|m| format!("{} · {}", m.map, m.count))
            .collect(),
        lower: maps.iter().map(|m| m.map.to_lowercase()).collect(),
    });
    ctx.data_mut(|d| d.insert_temp(id, Arc::clone(&n)));
    n
}

/// The chips' width, for the search to take what is left.
fn chips_width(ui: &Ui, compact: bool) -> f32 {
    let f = font(Face::SansMedium, size::TEXT_2XS);
    let w = |label: &str, icon: bool| {
        let text = if compact && icon {
            0.0
        } else {
            paint::galley(ui, label, f.clone(), egui::Color32::WHITE)
                .size()
                .x
        };
        12.0 + 2.0 + text + if icon { 16.0 } else { 0.0 } + 16.0
    };
    w("1PP", false)
        + w(t("servers", "passwordProtected"), true)
        + w("BattlEye", true)
        + w(t("servers", "colMods"), true)
        + w(t("servers", "official"), true)
        + 4.0 * 4.0
}

#[allow(clippy::too_many_arguments)]
fn chip(
    ui: &mut Ui,
    value: Tri,
    label: &str,
    title: &str,
    icon: Option<Icon>,
    compact: bool,
    key: &str,
    run: &mut Run,
) {
    let tip = format!("{label} — {title}");
    let r = ui.add(TriChip {
        value,
        label,
        icon,
        compact,
        tip: &tip,
    });
    if r.clicked() {
        run.push((
            "servers.filter.cycle",
            json!({ "key": key, "value": tri_id(tri_next(value)) }),
        ));
    }
}

/// A three-way value as the commands write it.
pub fn tri_id(t: Tri) -> &'static str {
    match t {
        Tri::All => "all",
        Tri::Only => "only",
        Tri::None => "none",
    }
}

pub fn tri_from(id: &str) -> Option<Tri> {
    Some(match id {
        "all" => Tri::All,
        "only" => Tri::Only,
        "none" => Tri::None,
        _ => return None,
    })
}

/// `TriChip.svelte`: a filter with three answers; a tick or a cross says which, so the
/// state reads without hovering. `compact` keeps only the icon below 1500px of window.
struct TriChip<'a> {
    value: Tri,
    label: &'a str,
    icon: Option<Icon>,
    compact: bool,
    tip: &'a str,
}

impl Widget for TriChip<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let f = font(Face::SansMedium, size::TEXT_2XS);
        let show_label = !(self.compact && self.icon.is_some());
        let g = show_label.then(|| ph(ui, self.label, f));
        let mark = matches!(self.value, Tri::Only | Tri::None);
        let mut w = 12.0;
        let mut parts = 0;
        if self.icon.is_some() {
            w += 12.0;
            parts += 1;
        }
        if mark {
            w += 12.0;
            parts += 1;
        }
        if let Some(g) = &g {
            w += g.size().x;
            parts += 1;
        }
        w += 4.0 * (parts as f32 - 1.0).max(0.0) + 2.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL), Sense::click());
        let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
        let (border, fill, ink) = match self.value {
            Tri::All => (
                paint::lerp(tk.border, tk.border_strong, hover),
                egui::Color32::TRANSPARENT,
                paint::lerp(tk.fg_muted, tk.fg, hover),
            ),
            Tri::Only => (alpha(tk.ok, 0.5), alpha(tk.ok, 0.1), tk.ok),
            Tri::None => (alpha(tk.err, 0.5), alpha(tk.err, 0.1), tk.err),
        };
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            p.rect(
                rect.shrink(0.5),
                CornerRadius::same(size::RADIUS_SM),
                fill,
                Stroke::new(1.0, border),
                egui::StrokeKind::Inside,
            );
            let mut x = rect.left() + 7.0;
            let cy = rect.center().y;
            let icon_at =
                |x: f32| Rect::from_center_size(Pos2::new(x + 6.0, cy), Vec2::splat(12.0));
            if let Some(i) = self.icon {
                i.paint(p, icon_at(x), ink);
                x += 16.0;
            }
            match self.value {
                Tri::Only => {
                    lucide::CHECK.paint(p, icon_at(x), ink);
                    x += 16.0;
                }
                Tri::None => {
                    lucide::X.paint(p, icon_at(x), ink);
                    x += 16.0;
                }
                Tri::All => {}
            }
            if let Some(g) = &g {
                draw(p, g, x, cy, ink);
            }
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(p, rect, size::RADIUS_SM, &tk);
            }
        }
        let pressed = match self.value {
            Tri::All => false,
            Tri::Only | Tri::None => true,
        };
        let label = self.label;
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, true, pressed, label)
        });
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(self.tip, "", Side::Bottom)
    }
}

/// "More filters": the ping ceiling, empty and full servers, excluded addresses; its
/// badge says how many of those are on.
fn more_filters(ui: &mut Ui, f: &mut Filters, d: &ToolbarData, win_w: f32, run: &mut Run) {
    let tk = ui.tokens();
    let n = f.more_count();
    let label = t("servers", "moreFilters");
    let show_label = win_w >= 1536.0;
    let g = show_label.then(|| ph(ui, label, font(Face::Sans, size::TEXT_XS)));
    let mut w = 16.0 + size::ICON_SM;
    if let Some(g) = &g {
        w += 6.0 + g.size().x;
    }
    if n > 0 {
        w += 6.0 + 16.0;
    }
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL), Sense::click());
    let open = menu::is_open(&resp);
    let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered() || open);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let (border, ink) = if n > 0 {
            (alpha(tk.accent, 0.6), tk.accent)
        } else {
            (
                paint::lerp(tk.border, tk.border_strong, hover),
                paint::lerp(tk.fg_muted, tk.fg, hover),
            )
        };
        p.rect(
            rect.shrink(0.5),
            CornerRadius::same(size::RADIUS_SM),
            paint::lerp(egui::Color32::TRANSPARENT, alpha(tk.raised, 0.6), hover),
            Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );
        let cy = rect.center().y;
        let mut x = rect.left() + 8.0;
        lucide::SLIDERS_HORIZONTAL.paint(
            p,
            Rect::from_min_size(
                Pos2::new(x, cy - size::ICON_SM / 2.0),
                Vec2::splat(size::ICON_SM),
            ),
            ink,
        );
        x += size::ICON_SM + 6.0;
        if let Some(g) = &g {
            draw(p, g, x, cy, ink);
            x += g.size().x + 6.0;
        }
        if n > 0 {
            let c = Pos2::new(x + 8.0, cy);
            p.circle_filled(c, 8.0, tk.accent);
            let ng = ph(ui, &n.to_string(), font(Face::Mono, size::TEXT_3XS));
            paint::text_center(
                p,
                &ng,
                Rect::from_center_size(c, Vec2::splat(16.0)),
                tk.accent_fg,
            );
        }
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(p, rect, size::RADIUS_SM, &tk);
        }
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let resp = resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .tip_kbd(label, "", Side::Bottom);
    if d.open_more && !open {
        egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&resp));
    }
    menu::popover(&resp, Side::Bottom, Align::End, |ui| {
        ui.set_width(288.0);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        // The ping ceiling.
        egui::Frame::new()
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 6.0;
                let g = crate::views::servers::stencil_galley(
                    ui,
                    t("servers", "maxPing"),
                    tk.fg_faint,
                    None,
                );
                let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(r.min, g, tk.fg_faint);
                let labels: Vec<String> = PINGS
                    .iter()
                    .map(|p| {
                        if *p == 0 {
                            t("servers", "anyPing").to_string()
                        } else {
                            format!("≤{p}")
                        }
                    })
                    .collect();
                let segs: Vec<Segment<u32>> = PINGS
                    .iter()
                    .zip(&labels)
                    .map(|(p, l)| Segment::new(*p, l.as_str()))
                    .collect();
                let mut v = f.max_ping;
                ui.add(Segmented::new(&mut v, &segs).size(FieldSize::Xs).fill());
                if v != f.max_ping {
                    run.push(("servers.filter.max_ping", json!({ "ms": v })));
                }
                ui.add_space(4.0);
                time_of_day(ui, f.time_of_day, run);
                ui.add_space(4.0);
                with_mod(ui, f, d.installed, run);
            });
        let r = ui.painter().add(egui::Shape::Noop);
        let y = ui.cursor().top();
        ui.painter().set(
            r,
            egui::Shape::hline(
                ui.max_rect().x_range(),
                y - 0.5,
                Stroke::new(1.0, tk.border),
            ),
        );
        let mut ready = f.ready;
        ui.add(
            Switch::new(&mut ready, t("servers", "readyToJoin"))
                .hint(t("servers", "readyToJoinHint")),
        );
        if ready != f.ready {
            run.push(("servers.filter.ready", json!({ "on": ready })));
        }
        let mut e = f.hide_empty;
        ui.add(Switch::new(&mut e, t("servers", "hideEmpty")));
        if e != f.hide_empty {
            run.push(("servers.filter.hide_empty", json!({ "on": e })));
        }
        let mut full = f.hide_full;
        ui.add(Switch::new(&mut full, t("servers", "hideFull")));
        if full != f.hide_full {
            run.push(("servers.filter.hide_full", json!({ "on": full })));
        }
        if d.excluded > 0 && excluded_row(ui, f.show_excluded, d.excluded).clicked() {
            run.push(("servers.filter.toggle_excluded", Value::Null));
        }
    });
}

/// A section's small capitals label in the popover.
fn popover_label(ui: &mut Ui, text: &str) {
    let tk = ui.tokens();
    let g = crate::views::servers::stencil_galley(ui, text, tk.fg_faint, None);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, tk.fg_faint);
}

/// The time of day the servers are at now: any, day or night.
fn time_of_day(ui: &mut Ui, v: DayTime, run: &mut Run) {
    popover_label(ui, t("servers", "timeOfDay"));
    let hint = t("servers", "timeOfDayHint");
    let segs = [
        Segment::new(DayTime::Any, t("servers", "timeAny")).title(hint),
        Segment::new(DayTime::Day, t("servers", "timeDay"))
            .icon(lucide::SUN)
            .title(hint),
        Segment::new(DayTime::Night, t("servers", "timeNight"))
            .icon(lucide::MOON)
            .title(hint),
    ];
    let mut now = v;
    ui.add(Segmented::new(&mut now, &segs).size(FieldSize::Xs).fill());
    if now != v {
        run.push((
            "servers.filter.time_of_day",
            json!({ "value": day_time_id(now) }),
        ));
    }
}

/// Most suggestions the mod filter offers from the mods on disk.
const MOD_SUGGESTIONS: usize = 5;

/// The mods on disk matching what is typed in the mod field, worked out when the
/// text or the list changes rather than every frame.
#[derive(Clone, Default)]
struct Suggestions {
    /// What it was worked out for: the text typed, the id picked, the list's address
    /// and length.
    key: (String, Option<u64>, usize, usize),
    /// Indexes into the list, at most [`MOD_SUGGESTIONS`].
    hits: Vec<usize>,
}

fn suggestions(
    installed: &[dz_hub::features::mods::InstalledModDto],
    typed: &str,
    picked: Option<u64>,
) -> Vec<usize> {
    let typed = typed.trim();
    let low = typed.to_lowercase();
    let mut hits = Vec::new();
    for (i, m) in installed.iter().enumerate() {
        if hits.len() >= MOD_SUGGESTIONS {
            break;
        }
        let name = m.name.trim();
        // The one already chosen is left out.
        if name.is_empty() || picked == Some(m.id) || name.eq_ignore_ascii_case(typed) {
            continue;
        }
        if !low.is_empty() && !name.to_lowercase().contains(&low) {
            continue;
        }
        hits.push(i);
    }
    hits
}

/// Servers running a mod: its name (or Workshop id) typed, or one of the mods on disk
/// picked from the suggestions under the field (that mod then, by its id).
fn with_mod(
    ui: &mut Ui,
    f: &mut Filters,
    installed: &[dz_hub::features::mods::InstalledModDto],
    run: &mut Run,
) {
    popover_label(ui, t("servers", "withMod"));
    let r = ui.add(
        Input::new(&mut f.with_mod)
            .search()
            .placeholder(t("servers", "withModPlaceholder"))
            .clear_label(t("servers", "clearWithMod"))
            .size(FieldSize::Xs)
            .width(ui.available_width())
            .char_limit(128)
            .id_salt("servers-with-mod"),
    );
    // Named for screen readers by its section's label (the placeholder is only a hint).
    r.widget_info(|| {
        let mut i = egui::WidgetInfo::text_edit(
            true,
            &f.with_mod,
            &f.with_mod,
            t("servers", "withModPlaceholder"),
        );
        i.label = Some(t("servers", "withMod").to_string());
        i
    });
    if r.changed() {
        // Typing makes it a text match again.
        f.with_mod_id = None;
        ui.ctx().request_repaint();
    }
    // The suggestions show while the field has focus. Clicking one takes the focus
    // away from the field before the click lands (on the press or the release), so
    // they stay up while a click that began with them open is under way.
    let open_id = r.id.with("suggest-open");
    let was_open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let clicking = ui.input(|i| i.pointer.any_down() || i.pointer.any_released());
    let open = r.has_focus() || (was_open && clicking);
    ui.data_mut(|d| d.insert_temp(open_id, open));
    if !open {
        return;
    }
    let cache_id = r.id.with("suggest");
    let key = (
        f.with_mod.clone(),
        f.with_mod_id,
        installed.as_ptr() as usize,
        installed.len(),
    );
    let mut cache = ui
        .data(|d| d.get_temp::<Arc<Suggestions>>(cache_id))
        .unwrap_or_default();
    if cache.key != key {
        let hits = suggestions(installed, &f.with_mod, f.with_mod_id);
        cache = Arc::new(Suggestions { key, hits });
        ui.data_mut(|d| d.insert_temp(cache_id, Arc::clone(&cache)));
    }
    if cache.hits.is_empty() {
        return;
    }
    // A short window keeps the popover under its button: three then.
    let room = if ui.ctx().content_rect().height() < 700.0 {
        3
    } else {
        MOD_SUGGESTIONS
    };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for &i in cache.hits.iter().take(room) {
            let Some(m) = installed.get(i) else { continue };
            let name = m.name.trim();
            if suggestion_row(ui, name).clicked() {
                run.push((
                    "servers.filter.with_mod",
                    json!({ "name": name, "id": m.id }),
                ));
                ui.data_mut(|d| d.insert_temp(open_id, false));
            }
        }
    });
}

/// One mod offered under the mod field: a compact row, puzzle icon and name.
fn suggestion_row(ui: &mut Ui, name: &str) -> Response {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    if ui.is_rect_visible(rect) {
        let hover = paint::ease(
            ui,
            resp.id.with("hover"),
            resp.hovered() || resp.has_focus(),
        );
        let p = ui.painter();
        p.rect_filled(
            rect,
            CornerRadius::same(size::RADIUS_XS),
            alpha(tk.raised, hover),
        );
        let ink = paint::lerp(tk.fg_muted, tk.fg, hover);
        let cy = rect.center().y;
        let x = rect.left() + 6.0;
        lucide::PUZZLE.paint(
            p,
            Rect::from_min_size(
                Pos2::new(x, cy - size::ICON_SM / 2.0),
                Vec2::splat(size::ICON_SM),
            ),
            ink,
        );
        let tx = x + size::ICON_SM + 8.0;
        let g = paint::galley_trunc(
            ui,
            name,
            font(Face::Sans, size::TEXT_XS),
            ink,
            (rect.right() - 6.0 - tx).max(0.0),
        );
        paint::text_at(p, &g, tx, cy, ink);
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(p, rect, size::RADIUS_XS, &tk);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// "Show the excluded servers" / "Hide the excluded servers", with how many there are.
fn excluded_row(ui: &mut Ui, shown: bool, n: usize) -> Response {
    let tk = ui.tokens();
    let w = ui.available_width();
    let words = if shown {
        t("servers", "excludedHide")
    } else {
        t("servers", "excludedReveal")
    };
    let count = ph(ui, &n.to_string(), font(Face::Mono, size::TEXT_2XS));
    // In the switches' columns above it: the icon centred on their track (8px in,
    // 28px wide), the words on their labels' edge, in their size.
    let icon_cx = 8.0 + 14.0;
    let x0 = 8.0 + 28.0 + 8.0;
    let room = w - x0 - 10.0 - count.size().x - 8.0;
    // `flex-1`: the words wrap rather than cut.
    let g = paint::galley_wrap(
        ui,
        words,
        font(Face::Sans, size::TEXT_2XS),
        egui::Color32::PLACEHOLDER,
        room.max(20.0),
    );
    let h = (g.size().y + 16.0).max(32.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
    let p = ui.painter();
    p.hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, tk.border),
    );
    p.rect_filled(
        rect.shrink2(Vec2::new(0.0, 0.5)),
        CornerRadius::ZERO,
        alpha(tk.raised, hover),
    );
    let ink = paint::lerp(tk.fg_muted, tk.fg, hover);
    let cy = rect.center().y;
    let icon = if shown { lucide::EYE } else { lucide::EYE_OFF };
    icon.paint(
        p,
        Rect::from_min_size(
            Pos2::new(
                rect.left() + icon_cx - size::ICON_SM / 2.0,
                cy - size::ICON_SM / 2.0,
            ),
            Vec2::splat(size::ICON_SM),
        ),
        ink,
    );
    p.galley(Pos2::new(rect.left() + x0, cy - g.size().y / 2.0), g, ink);
    draw(
        p,
        &count,
        rect.right() - 10.0 - count.size().x,
        cy,
        tk.fg_faint,
    );
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, rect.shrink(2.0), size::RADIUS_XS, &tk);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, words));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ── the filters in force, in words ──────────────────────────────────────────

/// `ActiveFilters.svelte`: each filter in force as a sentence with its own ✕, and one
/// button that clears them all. Nothing while none is on.
pub fn active_line(ui: &mut Ui, f: &Filters, run: &mut Run) {
    let chips = chips(f);
    if chips.is_empty() {
        return;
    }
    chip_line(ui, &chips, run);
}

/// Each filter in force, by its key, in words (the chips' text; the empty list's reason).
pub fn chips(f: &Filters) -> Vec<(&'static str, String)> {
    let mut chips: Vec<(&'static str, String)> = Vec::new();
    let q = f.search.trim();
    if !q.is_empty() {
        chips.push(("search", tf("servers", "filterSearch", &[("query", &q)])));
    }
    if !f.maps.is_empty() {
        let value = f.maps.join(", ");
        chips.push((
            "map",
            tf(
                "servers",
                "filterValue",
                &[("label", &t("servers", "colMap")), ("value", &value)],
            ),
        ));
    }
    let tri = |chips: &mut Vec<(&'static str, String)>, key, v: Tri, label: &str| match v {
        Tri::All => {}
        Tri::Only => chips.push((key, tf("servers", "filterYes", &[("label", &label)]))),
        Tri::None => chips.push((key, tf("servers", "filterNo", &[("label", &label)]))),
    };
    tri(&mut chips, "fp", f.first_person, "1PP");
    tri(
        &mut chips,
        "pwd",
        f.password,
        t("servers", "passwordProtected"),
    );
    tri(&mut chips, "be", f.battleye, "BattlEye");
    tri(&mut chips, "mods", f.modded, t("servers", "colMods"));
    tri(&mut chips, "off", f.official, t("servers", "official"));
    if f.max_ping > 0 {
        chips.push((
            "ping",
            format!("{} ≤ {} ms", t("servers", "colPing"), f.max_ping),
        ));
    }
    if f.hide_empty {
        chips.push(("empty", t("servers", "hideEmpty").to_string()));
    }
    if f.hide_full {
        chips.push(("full", t("servers", "hideFull").to_string()));
    }
    if f.time_of_day != DayTime::Any {
        chips.push((
            "time",
            tf(
                "servers",
                "filterValue",
                &[
                    ("label", &t("servers", "timeOfDay")),
                    ("value", &day_time_words(f.time_of_day)),
                ],
            ),
        ));
    }
    let m = f.with_mod.trim();
    if !m.is_empty() {
        chips.push((
            "mod",
            tf(
                "servers",
                "filterValue",
                &[("label", &t("servers", "withMod")), ("value", &m)],
            ),
        ));
    }
    if f.ready {
        chips.push(("ready", t("servers", "readyToJoin").to_string()));
    }
    chips
}

fn chip_line(ui: &mut Ui, chips: &[(&'static str, String)], run: &mut Run) {
    let tk = ui.tokens();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        for (key, text) in chips {
            let g = ph(ui, text, font(Face::Sans, size::TEXT_2XS));
            let w = 8.0 + g.size().x + 4.0 + 16.0 + 2.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL_SM), Sense::hover());
            let p = ui.painter();
            p.rect(
                rect.shrink(0.5),
                CornerRadius::same(size::RADIUS_SM),
                alpha(tk.accent, 0.08),
                Stroke::new(1.0, alpha(tk.accent, 0.35)),
                egui::StrokeKind::Inside,
            );
            draw(p, &g, rect.left() + 8.0, rect.center().y, tk.fg);
            let xr = Rect::from_center_size(
                Pos2::new(rect.right() - 2.0 - 8.0, rect.center().y),
                Vec2::splat(16.0),
            );
            let remove = tf("servers", "filterRemove", &[("label", text)]);
            let x = ui.interact(xr, ui.id().with(("chip-x", *key)), Sense::click());
            let h = paint::ease(ui, x.id.with("hover"), x.hovered());
            let p = ui.painter();
            p.rect_filled(
                xr,
                CornerRadius::same(size::RADIUS_XS),
                alpha(tk.accent, 0.15 * h),
            );
            lucide::X.paint(
                p,
                Rect::from_center_size(xr.center(), Vec2::splat(12.0)),
                paint::lerp(tk.fg_faint, tk.fg, h),
            );
            if paint::keyboard_focus(ui, &x) {
                paint::focus_ring(p, xr, size::RADIUS_XS, &tk);
            }
            x.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, remove.as_str())
            });
            let x = x.on_hover_cursor(egui::CursorIcon::PointingHand).tip_kbd(
                &remove,
                "",
                Side::Bottom,
            );
            if x.clicked() {
                run.push(("servers.filter.remove_one", json!({ "key": key })));
            }
        }
        ui.add_space(4.0);
        // "Clear filters": accent words with the filter-x mark.
        let label = t("servers", "clearFilters");
        let g = ph(ui, label, font(Face::SansMedium, size::TEXT_2XS));
        let w = 6.0 + 12.0 + 4.0 + g.size().x + 6.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL_SM), Sense::click());
        let h = paint::ease(ui, resp.id.with("hover"), resp.hovered());
        let p = ui.painter();
        p.rect_filled(
            rect,
            CornerRadius::same(size::RADIUS_SM),
            alpha(tk.accent, 0.1 * h),
        );
        lucide::FILTER_X.paint(
            p,
            Rect::from_center_size(
                Pos2::new(rect.left() + 6.0 + 6.0, rect.center().y),
                Vec2::splat(12.0),
            ),
            tk.accent,
        );
        draw(p, &g, rect.left() + 22.0, rect.center().y, tk.accent);
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(p, rect, size::RADIUS_SM, &tk);
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        if resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            run.push(("servers.filter.clear", Value::Null));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri_cycles_and_sorts_flip() {
        assert_eq!(tri_next(Tri::All), Tri::Only);
        assert_eq!(tri_next(Tri::Only), Tri::None);
        assert_eq!(tri_next(Tri::None), Tri::All);
        let mut f = Filters::default();
        f.toggle_sort(SortCol::Players);
        assert!(!f.asc);
        f.toggle_sort(SortCol::Players);
        assert!(f.asc);
        f.toggle_sort(SortCol::Ping);
        assert!(f.asc);
        f.toggle_sort(SortCol::Name);
        assert!(f.asc && f.sort == SortCol::Name);
    }

    #[test]
    fn active_and_clear() {
        let mut f = Filters::default();
        assert!(!f.active());
        f.search = "  ".into();
        assert!(!f.active());
        f.search = " de ".into();
        f.show_excluded = true;
        f.max_ping = 100;
        assert!(f.active());
        assert_eq!(f.query().search, "de");
        assert_eq!(f.more_count(), 2);
        f.remove("ping");
        assert_eq!(f.max_ping, 0);
        f.clear();
        assert!(!f.active());
        // "Show excluded" is not a filter the clear button resets.
        assert!(f.show_excluded);
    }

    #[test]
    fn time_mod_and_ready_are_filters_of_more_filters() {
        let mut f = Filters {
            time_of_day: DayTime::Night,
            with_mod: "  CF ".into(),
            ready: true,
            ..Filters::default()
        };
        assert!(f.active());
        assert_eq!(f.more_count(), 3);
        let q = f.query();
        assert_eq!(q.time_of_day, DayTime::Night);
        assert_eq!(q.with_mod, "CF");
        assert!(q.ready && q.game_version.is_none());
        let keys: Vec<&str> = chips(&f).into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["time", "mod", "ready"]);
        f.remove("time");
        f.remove("mod");
        assert_eq!(f.more_count(), 1);
        f.clear();
        assert!(!f.active() && !f.ready);
        assert_eq!(day_time_from(day_time_id(DayTime::Day)), Some(DayTime::Day));
        assert_eq!(day_time_from("dusk"), None);
    }

    #[test]
    fn a_picked_mod_filters_by_its_id_and_reads_by_its_name() {
        use super::super::cmds::with_mod_args;
        let picked = with_mod_args(&json!({ "name": " CF ", "id": 1559212036 }));
        assert_eq!(picked, ("CF".to_string(), Some(1559212036)));
        assert_eq!(
            with_mod_args(&json!({ "id": "2545327648" })),
            ("2545327648".to_string(), None)
        );
        assert_eq!(
            with_mod_args(&json!({ "name": "tools" })),
            ("tools".to_string(), None)
        );
        assert_eq!(with_mod_args(&Value::Null), (String::new(), None));
        let mut f = Filters {
            with_mod: picked.0,
            with_mod_id: picked.1,
            ..Filters::default()
        };
        // The hub matches the one mod by its id; the chip says its name.
        assert_eq!(f.query().with_mod, "1559212036");
        assert!(
            chips(&f)
                .iter()
                .any(|(k, v)| *k == "mod" && v.contains("CF"))
        );
        f.remove("mod");
        assert!(f.with_mod.is_empty() && f.with_mod_id.is_none());
        assert_eq!(f.query().with_mod, "");
    }

    #[test]
    fn mod_suggestions_match_what_is_typed_and_leave_out_the_pick() {
        let m = |name: &str, id: u64| dz_hub::features::mods::InstalledModDto {
            name: name.into(),
            id,
            local_updated: 0,
            size: 0,
            size_human: String::new(),
            managed: true,
            source: dz_hub::features::mods::ModSourceDto::Launcher,
            path: String::new(),
            other_copy: false,
            remote_updated: None,
            update_available: false,
        };
        let installed = [
            m("CF", 1),
            m("Code Lock", 2),
            m("", 3),
            m("Dabs Framework", 4),
        ];
        assert_eq!(suggestions(&installed, "", None), [0, 1, 3]);
        assert_eq!(suggestions(&installed, " co", None), [1]);
        assert_eq!(suggestions(&installed, "CF", Some(1)), Vec::<usize>::new());
        assert_eq!(suggestions(&installed, "", Some(1)), [1, 3]);
    }

    #[test]
    fn the_map_command_takes_one_map_a_list_or_none() {
        use super::super::cmds::map_args;
        assert_eq!(map_args(&json!({ "map": "enoch" })), ["enoch"]);
        assert!(map_args(&json!({ "map": null })).is_empty());
        assert!(map_args(&json!({ "map": "  " })).is_empty());
        assert!(map_args(&Value::Null).is_empty());
        assert_eq!(
            map_args(&json!({ "maps": ["enoch", " Sakhal ", "ENOCH", 3] })),
            ["enoch", "Sakhal"]
        );
        assert_eq!(map_args(&json!({ "map": ["a", "b"] })), ["a", "b"]);
        let many: Vec<String> = (0..100).map(|i| format!("m{i}")).collect();
        assert_eq!(map_args(&json!({ "maps": many })).len(), 64);

        let mut f = Filters {
            maps: vec!["enoch".into(), "sakhal".into()],
            ..Filters::default()
        };
        assert!(f.active());
        assert_eq!(f.query().maps, ["enoch", "sakhal"]);
        f.remove("map");
        assert!(f.maps.is_empty() && !f.active());
    }
}
