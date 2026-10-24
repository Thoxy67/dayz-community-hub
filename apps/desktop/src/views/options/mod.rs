//! Launch options (= `features/options/View.svelte`): the options DayZ starts with, in
//! cards by what they change, each with its switch and, when it takes one, its value;
//! beside them what the machine suggests, the command line DayZ receives and, on Linux,
//! how Steam really starts it.
//!
//! Files: `catalog` (flags, groups, words, recommendations, Steam's options taken
//! apart), `row` (`OptionRow.svelte`), `aside` (the hardware, command line and Linux
//! cards).
//!
//! The view's choices (search, active only, drafts) and what it asked the hub (the
//! hardware, Steam's launch info) live in one [`UiState`] in egui's memory, so the
//! commands of `commands.rs` (palette, pad, control channel) and the buttons change the
//! same thing. Toggles and values go through the profile store: optimistic, put back
//! with a toast when the write fails.

mod aside;
pub mod catalog;
mod row;

use std::sync::Arc;

use dz_hub::features::launch::SteamLaunchInfoDto;
use dz_hub::features::profile::LaunchOptionDto;
use dz_hub::features::system::SystemSpecsDto;
use egui::mutex::Mutex;
use egui::{Id, Rect, Ui, UiBuilder, Vec2};
use rustc_hash::FxHashMap;
use serde_json::{Value, json};

use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::paint::alpha;
use crate::kit::{Button, Input};
use crate::parts::{Empty, Figure, PageHeader, SectionCard};
use crate::rt::{self, Fetch};
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::{View, ViewId};
use catalog::{GROUPS, Group, OTHER, Rec, SteamOptions};

/// The Linux card's "as written in Steam" disclosure (its open state, by id).
pub(crate) const RAW_ID: &str = "dzch.options.linux-raw";

/// How long the Linux command's copy button shows its tick.
const COPIED_SECS: f64 = 1.4;

/// The aside's width (`w-80`).
const ASIDE_W: f32 = 320.0;

/// Two columns of cards from this window width on (`xl:grid-cols-2`).
const TWO_COLUMNS: f32 = 1280.0;

/// What the view remembers (session only, as in the Svelte view).
#[derive(Default)]
pub struct UiState {
    pub search: String,
    pub active_only: bool,
    specs: Fetch<SystemSpecsDto>,
    launch: Fetch<SteamLaunchInfoDto>,
    /// What the hardware suggests (worked out once per answer).
    recs: Vec<Rec>,
    /// Steam's launch options taken apart (once per answer).
    steam: Option<SteamOptions>,
    drafts: FxHashMap<String, row::Draft>,
    linux_copied_at: Option<f64>,
}

impl UiState {
    fn poll(&mut self) {
        if self.specs.poll() {
            self.recs = self
                .specs
                .value
                .as_ref()
                .map(catalog::recommend)
                .unwrap_or_default();
        }
        if self.launch.poll() {
            self.steam = self
                .launch
                .value
                .as_ref()
                .and_then(|i| i.launch_options.as_deref())
                .map(catalog::parse_steam_options);
        }
    }

    fn recommended(&self, key: &str) -> Option<&str> {
        self.recs
            .iter()
            .find(|r| r.key == key)
            .and_then(|r| r.value.as_deref())
    }
}

type Shared = Arc<Mutex<UiState>>;

/// The view's state, shared with its commands.
fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.options.view"), || {
            Arc::new(Mutex::new(UiState::default()))
        })
        .clone()
    })
}

/// What a click asks for, done once the page is drawn (through the commands).
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Toggle(String),
    Set(String, Option<String>),
    Browse(String),
    ApplyRecommended,
    ActiveOnly,
    CopyLinux,
}

pub struct OptionsView;

impl OptionsView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OptionsView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for OptionsView {
    fn on_show(&mut self, cx: &mut Ctx) {
        // The hardware and Steam's launch info, asked once per visit (the hub keeps the
        // hardware; the last answers stay on screen meanwhile).
        let st = shared(cx.egui);
        let mut st = st.lock();
        let hub = cx.hub.clone();
        st.specs.load(async move { hub.get_system_specs().await });
        let hub = cx.hub.clone();
        st.launch.load(async move { hub.steam_launch_info().await });
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let shared = shared(ui.ctx());
        let mut acts = Vec::new();
        {
            let mut st = shared.lock();
            st.poll();
            if cx.nav.take_focus_search() {
                ui.memory_mut(|m| m.request_focus(Id::new("dzch.options.search")));
            }
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let opts: &[LaunchOptionDto] = cx
                .stores
                .profile
                .data
                .as_ref()
                .map_or(&[], |p| p.options.as_slice());
            header(ui, &mut st, opts, &mut acts);
            body(ui, &mut st, opts, &mut acts);
        }
        for a in acts {
            let r = match a {
                Act::Toggle(key) => cx.run("options.toggle", &json!({ "key": key })),
                Act::Set(key, value) => {
                    cx.run("options.set_value", &json!({ "key": key, "value": value }))
                }
                Act::Browse(key) => cx.run("options.browse_folder", &json!({ "key": key })),
                Act::ApplyRecommended => cx.run("options.apply_recommended", &json!({})),
                Act::ActiveOnly => cx.run("options.active_only", &json!({})),
                Act::CopyLinux => cx.run("options.copy_linux_command", &json!({})),
            };
            if let Err(e) = r {
                cx.stores.bus.err(e);
            }
        }
    }
}

fn header(ui: &mut Ui, st: &mut UiState, opts: &[LaunchOptionDto], acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    let enabled = opts.iter().filter(|o| o.enabled).count();
    let with_value = opts
        .iter()
        .filter(|o| o.enabled && o.value.as_deref().is_some_and(|v| !v.is_empty()))
        .count();
    let wide = ui.ctx().content_rect().width() >= TWO_COLUMNS;
    PageHeader::new(t("options", "title")).show(
        ui,
        |ui| {
            ui.add(
                Figure::new(t("options", "statActive"))
                    .amount(enabled as f64)
                    .tone(tk.accent),
            );
            ui.add(Figure::new(t("options", "statWithValue")).amount(with_value as f64));
            ui.add(
                Figure::new(t("options", "statAvailable"))
                    .amount(opts.len() as f64)
                    .tone(tk.fg_muted),
            );
        },
        |_| {},
        Some(|ui: &mut Ui| {
            ui.horizontal(|ui| toolbar(ui, st, wide, acts));
        }),
    );
}

/// Search and filter on the toolbar row, as in the lists.
fn toolbar(ui: &mut Ui, st: &mut UiState, wide: bool, acts: &mut Vec<Act>) {
    {
        ui.spacing_mut().item_spacing.x = 6.0;
        // As tall as the search beside it.
        let chip =
            Button::chip(t("options", "showActiveOnly"), st.active_only).size(crate::kit::Size::Sm);
        let chip_w = chip.natural_width(ui) + 6.0;
        let room = (ui.available_width() - chip_w).max(128.0);
        let w = if wide {
            room.min(320.0)
        } else {
            room.clamp(128.0, 240.0)
        };
        ui.add(
            Input::new(&mut st.search)
                .search()
                .width(w)
                .placeholder(t("options", "searchPlaceholder"))
                .clear_label(t("common", "clearSearch"))
                .char_limit(256)
                .id_salt("dzch.options.search"),
        );
        if ui.add(chip).clicked() {
            acts.push(Act::ActiveOnly);
        }
    }
}

/// Whether `x` shows under the search `q` (lowercased) and the "active only" chip.
fn matches(x: &LaunchOptionDto, q: &str, active_only: bool) -> bool {
    if active_only && !x.enabled {
        return false;
    }
    if q.is_empty() {
        return true;
    }
    let hit = |h: &str| h.to_lowercase().contains(q);
    if hit(&x.key) || hit(&x.description) {
        return true;
    }
    catalog::meta(&x.key)
        .is_some_and(|m| hit(m.flag) || hit(t("options", m.label)) || hit(t("options", m.desc)))
}

/// The groups to show: each with its matching options and its count over all of them.
struct Shown<'a> {
    group: &'a Group,
    opts: Vec<&'a LaunchOptionDto>,
    on: usize,
    total: usize,
}

fn groups<'a>(opts: &'a [LaunchOptionDto], q: &str, active_only: bool) -> Vec<Shown<'a>> {
    let unlisted: Vec<&LaunchOptionDto> =
        opts.iter().filter(|o| !catalog::listed(&o.key)).collect();
    GROUPS
        .iter()
        .map(|g| {
            let all: Vec<&LaunchOptionDto> = g
                .keys
                .iter()
                .filter_map(|k| opts.iter().find(|o| o.key == *k))
                .collect();
            (g, all)
        })
        .chain(std::iter::once((&OTHER, unlisted)))
        .filter_map(|(g, all)| {
            let shown: Vec<&LaunchOptionDto> = all
                .iter()
                .copied()
                .filter(|x| matches(x, q, active_only))
                .collect();
            (!shown.is_empty()).then(|| Shown {
                group: g,
                on: all.iter().filter(|x| x.enabled).count(),
                total: all.len(),
                opts: shown,
            })
        })
        .collect()
}

fn body(ui: &mut Ui, st: &mut UiState, opts: &[LaunchOptionDto], acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, egui::Sense::hover());
    let aside_w = ASIDE_W.min(full.width() * 0.5);
    let main = Rect::from_min_max(full.min, egui::pos2(full.right() - aside_w, full.bottom()));
    let side = Rect::from_min_max(egui::pos2(main.right(), full.top()), full.max);

    // The options, by what they change.
    let mut mui = ui.new_child(
        UiBuilder::new()
            .max_rect(main)
            .id_salt("options-main")
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    mui.set_clip_rect(main.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical()
        .id_salt("options-main-scroll")
        .auto_shrink([false, false])
        .show(&mut mui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    cards(ui, st, opts, acts);
                });
        });

    // What the machine suggests, and what DayZ will be told.
    ui.painter()
        .rect_filled(side, egui::CornerRadius::ZERO, alpha(tk.bg, 0.4));
    ui.painter().vline(
        side.left() + 0.5,
        side.y_range(),
        egui::Stroke::new(1.0, tk.border),
    );
    let mut aui = ui.new_child(
        UiBuilder::new()
            .max_rect(side)
            .id_salt("options-aside")
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    aui.set_clip_rect(side.intersect(ui.clip_rect()));
    let flags: Vec<String> = opts
        .iter()
        .filter(|o| o.enabled)
        .map(|o| catalog::flag_text(&o.key, o.value.as_deref()))
        .collect();
    let now = ui.input(|i| i.time);
    let copied = st.linux_copied_at.is_some_and(|at| now - at < COPIED_SECS);
    if copied {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    }
    egui::ScrollArea::vertical()
        .id_salt("options-aside-scroll")
        .auto_shrink([false, false])
        .show(&mut aui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    aside::hardware(ui, st, opts, acts);
                    ui.add_space(12.0);
                    aside::command(ui, &flags);
                    if let Some(info) = st.launch.value.as_ref().filter(|i| i.linux) {
                        ui.add_space(12.0);
                        aside::linux(ui, info, st.steam.as_ref(), &flags, copied, acts);
                    }
                });
        });
}

fn cards(ui: &mut Ui, st: &mut UiState, opts: &[LaunchOptionDto], acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    let q = st.search.trim().to_lowercase();
    let shown = groups(opts, &q, st.active_only);
    if shown.is_empty() {
        // Say which filter emptied the list, with the way back from each.
        let searching = !q.is_empty();
        let title = if searching {
            tf("options", "noMatch", &[("search", &st.search)])
        } else {
            t("options", "noneActive").to_string()
        };
        let (mut clear, mut show_all) = (false, false);
        Empty::new()
            .icon(lucide::SEARCH_X)
            .title(&title)
            .show(ui, |ui| {
                if searching && ui.add(Button::new(t("common", "clearSearch"))).clicked() {
                    clear = true;
                }
                if st.active_only && ui.add(Button::new(t("options", "showAll"))).clicked() {
                    show_all = true;
                }
            });
        if clear {
            st.search.clear();
        }
        if show_all {
            acts.push(Act::ActiveOnly);
        }
        return;
    }
    let cols = if ui.ctx().content_rect().width() >= TWO_COLUMNS {
        2
    } else {
        1
    };
    let gap = 12.0;
    let w = ui.available_width();
    let cw = ((w - gap * (cols as f32 - 1.0)) / cols as f32).floor();
    for (n, line) in shown.chunks(cols).enumerate() {
        if n > 0 {
            ui.add_space(gap);
        }
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(gap, 0.0);
            for s in line {
                ui.allocate_ui_with_layout(
                    Vec2::new(cw, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(cw);
                        card(ui, st, s, acts, &tk);
                    },
                );
            }
        });
    }
}

fn card(ui: &mut Ui, st: &mut UiState, s: &Shown, acts: &mut Vec<Act>, tk: &crate::theme::Tokens) {
    let tone = s.group.tone.color(tk);
    SectionCard::new(t("options", s.group.label))
        .icon(s.group.icon)
        .tone(tone)
        .show(
            ui,
            |ui| {
                let text = tf(
                    "options",
                    "enabledCount",
                    &[("enabled", &s.on), ("total", &s.total)],
                );
                ui.label(
                    egui::RichText::new(text)
                        .font(font(Face::Mono, size::TEXT_3XS))
                        .color(tk.fg_faint),
                );
            },
            |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for (i, opt) in s.opts.iter().enumerate() {
                    if i > 0 {
                        let w = ui.available_width();
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::new(w, 1.0), egui::Sense::hover());
                        ui.painter().hline(
                            r.x_range(),
                            r.center().y,
                            egui::Stroke::new(1.0, alpha(tk.border, 0.5)),
                        );
                    }
                    let rec = st.recommended(&opt.key).map(str::to_string);
                    let draft = st.drafts.entry(opt.key.clone()).or_default();
                    row::show(ui, opt, tone, rec.as_deref(), draft, acts);
                }
            },
        );
}

// ── commands (`options.*`) ──────────────────────────────────────────────────────

fn key_arg(a: &Value) -> Option<String> {
    a.get("key")
        .and_then(Value::as_str)
        .map(|s| s.chars().take(128).collect())
}

/// The options as the profile has them now.
fn options(cx: &Ctx) -> Vec<LaunchOptionDto> {
    cx.stores
        .profile
        .data
        .as_ref()
        .map(|p| p.options.clone())
        .unwrap_or_default()
}

fn known(cx: &Ctx, key: &str) -> bool {
    cx.stores
        .profile
        .data
        .as_ref()
        .is_some_and(|p| p.options.iter().any(|o| o.key == key))
}

/// Write a value (trimmed; empty clears it); a value also turns the option on.
fn set_value(cx: &mut Ctx, key: &str, value: Option<&str>) {
    let v = value
        .map(|v| v.trim().chars().take(1024).collect::<String>())
        .filter(|v| !v.is_empty());
    cx.stores.profile.set_option_value(key, v);
}

/// Every recommendation in place: a value written (which turns it on) or the option on.
fn apply_recommended(cx: &mut Ctx) {
    let recs = shared(cx.egui).lock().recs.clone();
    let opts = options(cx);
    for r in recs {
        let Some(x) = opts.iter().find(|o| o.key == r.key) else {
            continue;
        };
        match r.value {
            Some(v) => {
                if x.value.as_deref() != Some(v.as_str()) || !x.enabled {
                    cx.stores.profile.set_option_value(r.key, Some(v));
                }
            }
            None if !x.enabled => cx.stores.profile.toggle_option(r.key),
            None => {}
        }
    }
}

/// Whether "Apply" has something to do.
pub fn can_apply(cx: &Ctx) -> bool {
    let st = shared(cx.egui);
    let st = st.lock();
    let opts = cx
        .stores
        .profile
        .data
        .as_ref()
        .map_or(&[][..], |p| p.options.as_slice());
    !st.recs.is_empty() && !st.recs.iter().all(|r| aside::rec_done(r, opts))
}

/// The flags DayZ receives, as the command line shows them.
fn flags(cx: &Ctx) -> Vec<String> {
    options(cx)
        .iter()
        .filter(|o| o.enabled)
        .map(|o| catalog::flag_text(&o.key, o.value.as_deref()))
        .collect()
}

/// Run `options.<what>` (see `commands.rs`).
pub fn command(cx: &mut Ctx, what: &str, a: &Value) {
    match what {
        "search" => {
            let text: String = a
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(256)
                .collect();
            shared(cx.egui).lock().search = text;
            if cx.nav.view != ViewId::Options {
                cx.go(ViewId::Options);
            }
        }
        "active_only" => {
            let st = shared(cx.egui);
            let mut st = st.lock();
            st.active_only = a
                .get("on")
                .and_then(Value::as_bool)
                .unwrap_or(!st.active_only);
        }
        "toggle" => {
            if let Some(k) = key_arg(a).filter(|k| known(cx, k)) {
                match a.get("on").and_then(Value::as_bool) {
                    Some(on) if options(cx).iter().any(|o| o.key == k && o.enabled == on) => {}
                    _ => cx.stores.profile.toggle_option(&k),
                }
            }
        }
        "set_value" => {
            if let Some(k) = key_arg(a).filter(|k| known(cx, k)) {
                let v = a.get("value").and_then(Value::as_str);
                set_value(cx, &k, v);
            }
        }
        "clear_value" => {
            if let Some(k) = key_arg(a).filter(|k| known(cx, k)) {
                set_value(cx, &k, None);
            }
        }
        "use_recommended" => {
            if let Some(k) = key_arg(a).filter(|k| known(cx, k)) {
                let v = shared(cx.egui).lock().recommended(&k).map(str::to_string);
                if let Some(v) = v {
                    set_value(cx, &k, Some(&v));
                }
            }
        }
        "browse_folder" => {
            let Some(k) = key_arg(a).filter(|k| known(cx, k)) else {
                return;
            };
            let bus = cx.stores.bus.clone();
            rt::fire(async move {
                let pick = crate::parts::file_picker::Pick::folder(t("options", "browseFolder"));
                let Some(dir) = crate::parts::file_picker::pick(pick).await else {
                    return;
                };
                let path = dir.to_string_lossy().into_owned();
                bus.run(move |cx| set_value(cx, &k, Some(&path)));
            });
        }
        "apply_recommended" => apply_recommended(cx),
        "copy_command" => {
            let f = flags(cx);
            if f.is_empty() {
                return;
            }
            cx.egui.copy_text(f.join(" "));
            cx.stores.bus.ok(t("common", "copied"));
        }
        "copy_prefix" => {
            let st = shared(cx.egui);
            let prefix = st
                .lock()
                .launch
                .value
                .as_ref()
                .and_then(|i| i.prefix.clone());
            if let Some(p) = prefix {
                cx.egui.copy_text(p);
                cx.stores.bus.ok(t("common", "copied"));
            }
        }
        "copy_linux_command" => {
            let f = flags(cx);
            let st = shared(cx.egui);
            let mut st = st.lock();
            if let Some(info) = st.launch.value.as_ref() {
                cx.egui.copy_text(aside::linux_command(info, &f));
                st.linux_copied_at = Some(cx.egui.input(|i| i.time));
                cx.egui.request_repaint();
            }
        }
        "linux_raw" => {
            let id = Id::new(RAW_ID);
            cx.egui.data_mut(|d| {
                let open = d.get_persisted::<bool>(id).unwrap_or(false);
                let want = a.get("open").and_then(Value::as_bool).unwrap_or(!open);
                d.insert_persisted(id, want);
            });
            cx.egui.request_repaint();
        }
        _ => {}
    }
}
