//! The command palette (= `CommandPalette.svelte`), Ctrl K or Menu on a controller: one
//! field for everything the window holds.
//!
//! Empty, it offers what was picked last, the actions and the views. Typed into, it
//! matches views, actions (with their keys), settings sections, favourites, history and
//! installed mods here, fuzzily ([`super::fuzzy`]), and filters the servers store's
//! snapshot for servers (120 ms after typing pauses, six at most, the newest winning);
//! an address ("1.2.3.4:2302") offers to connect to it. Sections are ordered by their best match;
//! ↑↓ move, PgUp/PgDn jump, Tab changes section, Enter does the obvious thing, Esc closes.
//!
//! The sections are rebuilt only when the query, the language or what they list changes,
//! never per frame.

use std::sync::Arc;
use std::time::Duration;

use dz_hub::features::browser::{ServerQuery, ServerRow, ServerSnapshot, order};
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align2, Color32, Context, CornerRadius, Key, Modifiers, Rect, Sense, Stroke, Ui, Vec2, pos2,
    vec2,
};
use serde_json::{Value, json};

use super::fuzzy::{self, Match};
use crate::app::Ctx;
use crate::commands;
use crate::i18n::{self, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::{Kbd, Spinner, paint};
use crate::rt::{self, Task};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};
use crate::views::{PLACES, ViewId};

/// Items per ranked section, and recent picks shown.
const CAP: usize = 5;
/// Recent picks kept.
const RECENT_MAX: usize = 8;
/// The server search waits this long after the last key.
const SEARCH_AFTER: f64 = 0.12;
const ROW_H: f32 = size::ROW;
const HEAD_H: f32 = 44.0;
const FOOT_H: f32 = 26.0;

/// The settings sections the palette finds: id, words key (`None`: the controller's).
const SECTIONS: [(&str, Option<&str>); 12] = [
    ("account", Some("sectionAccount")),
    ("steam", Some("sectionSteam")),
    ("apis", Some("sectionApis")),
    ("location", Some("sectionLocation")),
    ("ping", Some("sectionPing")),
    ("excluded", Some("sectionExcluded")),
    ("appearance", Some("sectionAppearance")),
    ("language", Some("sectionLanguage")),
    ("window", Some("sectionWindow")),
    ("controller", None),
    ("battleye", Some("sectionBattleye")),
    ("data", Some("sectionData")),
];

/// Where `palette.connect_address` leaves the text the palette opens typed into.
pub(super) const SEED: &str = "dzch.palette.seed";
/// Where `palette.navigate` queues its moves.
pub(super) const MOVES: &str = "dzch.palette.moves";

/// What choosing an item does.
#[derive(Clone)]
enum Run {
    View(ViewId),
    Command(&'static str),
    Settings(&'static str),
    Join(String, u16, Option<String>),
    JoinRow(Box<ServerRow>),
    Direct(String, u16),
    Mod(String),
    CheckMods,
    Sheet,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ink {
    Faint,
    Warn,
    Mods,
}

#[derive(Clone)]
struct Item {
    key: String,
    icon: Icon,
    ink: Ink,
    label: String,
    /// Matched beside the label, never shown.
    also: Option<String>,
    sub: Option<String>,
    verb: &'static str,
    kbd: Option<String>,
    run: Run,
    /// For the recent list: a server to rebuild.
    server: Option<(String, u16)>,
    ranges: Vec<(usize, usize)>,
    score: f32,
}

impl Item {
    fn new(key: String, icon: Icon, label: String, verb: &'static str, run: Run) -> Self {
        Self {
            key,
            icon,
            ink: Ink::Faint,
            label,
            also: None,
            sub: None,
            verb,
            kbd: None,
            run,
            server: None,
            ranges: Vec::new(),
            score: 0.0,
        }
    }
}

struct Section {
    title: &'static str,
    items: Vec<Item>,
}

/// A recent pick, kept in the preferences (`dzch.palette.recent`) as JSON.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct Recent {
    key: String,
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sub: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
}

/// What the sections were built from; rebuilt when it changes.
#[derive(Clone, PartialEq)]
struct Sig {
    query: String,
    locale: i18n::Locale,
    favorites: usize,
    history: usize,
    last: Option<String>,
    mods: usize,
    stale: usize,
    found: u64,
    recent: usize,
}

/// What the palette keeps between frames.
#[derive(Default)]
pub(crate) struct Palette {
    was_open: bool,
    query: String,
    active: usize,
    /// The keyboard moved the highlight: scroll it into view.
    reveal: bool,
    found: Vec<ServerRow>,
    found_gen: u64,
    asking: bool,
    search_due: Option<f64>,
    searched: String,
    search: Option<Task<(u64, Vec<ServerRow>)>>,
    generation: u64,
    sections: Vec<Section>,
    sig: Option<Sig>,
}

pub(super) fn show(ctx: &Context, cx: &mut Ctx, st: &mut Palette) {
    let open = cx.stores.dialogs.palette;
    let now = ctx.input(|i| i.time);
    if open && !st.was_open {
        // A fresh palette each time it opens.
        st.query.clear();
        st.active = 0;
        st.found.clear();
        st.asking = false;
        st.search_due = None;
        st.searched.clear();
        st.sig = None;
        // What `palette.connect_address` opens it typed into.
        if let Some(seed) = ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new(SEED))) {
            st.query = seed;
        }
    }
    st.was_open = open;
    let a = ctx.animate_bool_with_time(
        egui::Id::new("dzch.palette.shown"),
        open,
        if open { 0.14 } else { 0.10 },
    );
    if !open && a <= 0.0 {
        st.search = None;
        return;
    }
    poll_search(ctx, st, now, cx.stores.servers.snapshot());
    rebuild(cx, st);
    let flat_len: usize = st.sections.iter().map(|s| s.items.len()).sum();
    let mut chosen: Option<Item> = None;
    let mut close = false;
    if open {
        // The palette's own keys, before the field sees them.
        let (down, up, pgdn, pgup, tab, stab, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::PageDown),
                i.consume_key(Modifiers::NONE, Key::PageUp),
                i.consume_key(Modifiers::NONE, Key::Tab),
                i.consume_key(Modifiers::SHIFT, Key::Tab),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        let n = flat_len.max(1);
        if down {
            go(st, (st.active + 1) % n, flat_len);
        }
        if up {
            go(st, (st.active + n - 1) % n, flat_len);
        }
        if pgdn {
            go(st, st.active + CAP, flat_len);
        }
        if pgup {
            go(st, st.active.saturating_sub(CAP), flat_len);
        }
        if tab {
            section(st, 1);
        }
        if stab {
            section(st, -1);
        }
        if enter {
            chosen = item_at(st, st.active).cloned();
        }
        if esc {
            close = true;
        }
        // Moves asked by `palette.navigate` (a pad, the control channel).
        let moves = ctx.data_mut(|d| d.remove_temp::<Vec<String>>(egui::Id::new(MOVES)));
        for m in moves.unwrap_or_default() {
            match m.as_str() {
                "down" => go(st, (st.active + 1) % n, flat_len),
                "up" => go(st, (st.active + n - 1) % n, flat_len),
                "pagedown" => go(st, st.active + CAP, flat_len),
                "pageup" => go(st, st.active.saturating_sub(CAP), flat_len),
                "next_section" => section(st, 1),
                "prev_section" => section(st, -1),
                "run" => chosen = item_at(st, st.active).cloned(),
                "close" => close = true,
                _ => {}
            }
        }
    }

    let tk = ctx.tokens();
    let screen = ctx.content_rect();
    let w = (screen.width() - 32.0).clamp(200.0, 672.0);
    let max_h = (screen.height() * 0.72).max(160.0);
    let area = egui::Modal::default_area(egui::Id::new("dzch.palette")).anchor(
        Align2::CENTER_TOP,
        vec2(0.0, screen.height() * 0.12 + 2.0 * (1.0 - a)),
    );
    let modal = egui::Modal::new(egui::Id::new("dzch.palette"))
        .area(area)
        .frame(egui::Frame::NONE)
        .backdrop_color(paint::alpha(tk.plot, 0.6 * a));
    let r = modal.show(ctx, |ui| {
        ui.multiply_opacity(a);
        egui::Frame::new()
            .fill(tk.panel)
            .stroke(Stroke::new(1.0, tk.border_strong))
            .corner_radius(CornerRadius::same(size::RADIUS_MD))
            .shadow(paint::shadow_pop())
            .show(ui, |ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                header(ui, st, open, w);
                let list_max = (max_h - HEAD_H - FOOT_H - 2.0).max(80.0);
                if let Some(item) = list(ui, st, w, list_max, &tk) {
                    chosen = Some(item);
                }
                footer(ui, st, w, &tk);
            });
    });
    if open && r.should_close() {
        close = true;
    }
    if a < 1.0 {
        ctx.request_repaint();
    }
    if let Some(item) = chosen {
        choose(cx, item);
    } else if close {
        cx.stores.dialogs.palette = false;
    }
}

/// The field: the search icon (a spinner while servers are asked), the query, Esc.
fn header(ui: &mut Ui, st: &mut Palette, open: bool, w: f32) {
    let tk = ui.tokens();
    let (r, _) = ui.allocate_exact_size(vec2(w, HEAD_H), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, tk.border));
    let cy = r.center().y;
    let icon_r = Rect::from_min_size(pos2(r.left() + 12.0, cy - 8.0), Vec2::splat(size::ICON));
    if st.asking {
        Spinner::paint(ui, icon_r, tk.accent);
    } else {
        lucide::SEARCH.paint(ui.painter(), icon_r, tk.accent);
    }
    let kbd_w = Kbd::width(ui, "Esc");
    Kbd::paint_at(ui, "Esc", r.right() - 12.0 - kbd_w, cy - 0.5);
    let field = Rect::from_min_max(
        pos2(icon_r.right() + 8.0, r.top()),
        pos2(r.right() - 12.0 - kbd_w - 8.0, r.bottom() - 1.0),
    );
    let mut fui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(field)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let hint = egui::RichText::new(t("keys", "placeholder"))
        .color(tk.fg_faint)
        .font(font(Face::Sans, size::TEXT_SM));
    let before = st.query.clone();
    let resp = fui.add(
        egui::TextEdit::singleline(&mut st.query)
            .id_salt("dzch.palette.field")
            .frame(egui::Frame::NONE)
            .font(font(Face::Sans, size::TEXT_SM))
            .text_color(tk.fg)
            .hint_text(hint)
            .desired_width(field.width())
            .margin(egui::Margin::ZERO),
    );
    if open && !resp.has_focus() {
        resp.request_focus();
    }
    if st.query != before {
        st.active = 0;
        st.reveal = true;
    }
}

/// The sections; returns the item clicked.
fn list(ui: &mut Ui, st: &mut Palette, w: f32, max_h: f32, tk: &Tokens) -> Option<Item> {
    let mut chosen = None;
    let q = fuzzy::fold(st.query.trim());
    let empty_q = q.is_empty();
    let flat_len: usize = st.sections.iter().map(|s| s.items.len()).sum();
    egui::ScrollArea::vertical()
        .id_salt("dzch.palette.list")
        .max_height(max_h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.add_space(4.0);
            let inner_w = w - 8.0;
            let mut i = 0usize;
            let pointer_moved = ui.input(|inp| inp.pointer.delta() != Vec2::ZERO);
            for sec in &st.sections {
                let (hr, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
                let g = super::stencil(ui, sec.title, tk.fg_faint, None);
                paint::text_at(
                    ui.painter(),
                    &g,
                    hr.left() + 4.0 + 8.0,
                    hr.top() + 8.0 + 6.0,
                    tk.fg_faint,
                );
                for item in &sec.items {
                    let (row, _) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
                    let r = Rect::from_min_size(
                        pos2(row.left() + 4.0, row.top()),
                        vec2(inner_w, ROW_H),
                    );
                    let resp = ui.interact(r, ui.id().with(("pal", i)), Sense::click());
                    if resp.hovered() && pointer_moved {
                        st.active = i;
                    }
                    let on = i == st.active;
                    if on && st.reveal {
                        ui.scroll_to_rect(r, None);
                    }
                    draw_item(ui, r, item, on, tk);
                    if resp.clicked() {
                        chosen = Some(item.clone());
                    }
                    i += 1;
                }
            }
            if flat_len == 0 {
                let text = tf("keys", "nothing", &[("query", &st.query.trim())]);
                let (r, _) = ui.allocate_exact_size(vec2(w, 20.0 + 8.0 + 16.0), Sense::hover());
                let g = paint::galley_trunc(
                    ui,
                    &text,
                    font(Face::Sans, size::TEXT_XS),
                    tk.fg_muted,
                    w - 24.0,
                );
                paint::text_center(
                    ui.painter(),
                    &g,
                    Rect::from_min_size(pos2(r.left(), r.top() + 20.0), vec2(w, 16.0)),
                    tk.fg_muted,
                );
            }
            if empty_q || flat_len == 0 {
                try_box(ui, w, tk);
            }
            ui.add_space(4.0);
        });
    st.reveal = false;
    chosen
}

/// "What you can type": three examples.
fn try_box(ui: &mut Ui, w: f32, tk: &Tokens) {
    ui.add_space(8.0);
    let inner = w - 16.0 - 24.0;
    let lines = [
        t("keys", "tryServer"),
        t("keys", "tryAddress"),
        t("keys", "tryOther"),
    ];
    let gs: Vec<Arc<egui::Galley>> = lines
        .iter()
        .map(|l| {
            paint::galley_wrap(
                ui,
                l,
                font(Face::Sans, size::TEXT_2XS),
                tk.fg_muted,
                inner - 16.0,
            )
        })
        .collect();
    let body_h: f32 = gs.iter().map(|g| g.size().y + 2.0).sum();
    let h = 8.0 + 12.0 + 4.0 + body_h + 8.0;
    let (row, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let r = Rect::from_min_max(
        pos2(row.left() + 8.0, row.top()),
        pos2(row.right() - 8.0, row.bottom()),
    );
    let p = ui.painter();
    p.rect_stroke(
        r,
        CornerRadius::same(size::RADIUS_SM),
        Stroke::new(1.0, paint::alpha(tk.border, 0.6)),
        egui::StrokeKind::Inside,
    );
    let title = super::stencil(ui, t("keys", "tryTitle"), tk.fg_faint, None);
    paint::text_at(p, &title, r.left() + 12.0, r.top() + 8.0 + 6.0, tk.fg_faint);
    let mut y = r.top() + 8.0 + 12.0 + 4.0;
    for g in gs {
        p.circle_filled(pos2(r.left() + 12.0 + 5.0, y + 7.0), 1.5, tk.fg_muted);
        let h = g.size().y;
        p.galley(pos2(r.left() + 12.0 + 16.0, y), g, tk.fg_muted);
        y += h + 2.0;
    }
}

fn ink_of(i: Ink, tk: &Tokens) -> Color32 {
    match i {
        Ink::Faint => tk.fg_faint,
        Ink::Warn => tk.warn,
        Ink::Mods => tk.mods,
    }
}

/// One row (`h-row rounded-xs px-2 gap-2.5 text-xs`): icon, label with its matches, the
/// detail, the verb when highlighted, the key.
fn draw_item(ui: &Ui, r: Rect, item: &Item, on: bool, tk: &Tokens) {
    let p = ui.painter();
    if on {
        p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.raised);
        p.rect_filled(
            Rect::from_min_max(
                pos2(r.left(), r.top() + 6.0),
                pos2(r.left() + 2.0, r.bottom() - 6.0),
            ),
            CornerRadius::same(1),
            tk.accent,
        );
    }
    let cy = r.center().y;
    let mut x = r.left() + 8.0;
    item.icon.paint(
        p,
        Rect::from_min_size(pos2(x, cy - 8.0), Vec2::splat(size::ICON)),
        ink_of(item.ink, tk),
    );
    x += size::ICON + 10.0;
    let mut right = r.right() - 8.0;
    if let Some(k) = &item.kbd {
        let kw = Kbd::width(ui, k);
        right -= kw;
        Kbd::paint_at(ui, k, right, cy - 0.5);
        right -= 10.0;
    }
    if on {
        // "Join ↵": the faces have no ↵, so it is drawn.
        right -= 9.0;
        enter_glyph(
            p,
            Rect::from_min_size(pos2(right, cy - 4.5), vec2(9.0, 8.0)),
            tk.accent,
        );
        right -= 4.0;
        let g = paint::galley(ui, item.verb, font(Face::Sans, size::TEXT_2XS), tk.accent);
        right -= g.size().x;
        paint::text_at(p, &g, right, cy, tk.accent);
        right -= 10.0;
    }
    let ink = if on { tk.fg } else { tk.fg_muted };
    let label_max = (right - x).max(0.0);
    let lg = label_galley(ui, item, ink, tk, label_max);
    paint::text_at(p, &lg, x, cy, ink);
    x += lg.size().x + 10.0;
    if let Some(sub) = &item.sub
        && right - x > 12.0
    {
        let g = paint::galley_trunc(
            ui,
            sub,
            font(Face::Mono, size::TEXT_3XS),
            tk.fg_faint,
            right - x,
        );
        paint::text_at(p, &g, x, cy, tk.fg_faint);
    }
}

/// ↵, in `r`: down from the top right, then left, with its head.
fn enter_glyph(p: &egui::Painter, r: Rect, c: Color32) {
    let st = Stroke::new(1.2, c);
    let corner = pos2(r.right() - 0.5, r.bottom() - 2.0);
    p.line_segment([pos2(r.right() - 0.5, r.top()), corner], st);
    p.line_segment([corner, pos2(r.left() + 0.5, corner.y)], st);
    p.line_segment(
        [
            pos2(r.left() + 0.5, corner.y),
            pos2(r.left() + 3.0, corner.y - 2.5),
        ],
        st,
    );
    p.line_segment(
        [
            pos2(r.left() + 0.5, corner.y),
            pos2(r.left() + 3.0, corner.y + 2.5),
        ],
        st,
    );
}

/// The label, its matched pieces in the accent and underlined.
fn label_galley(ui: &Ui, item: &Item, ink: Color32, tk: &Tokens, max_w: f32) -> Arc<egui::Galley> {
    let f = font(Face::SansMedium, size::TEXT_XS);
    let mut job = LayoutJob::default();
    for (range, hit) in fuzzy::pieces(&item.label, &item.ranges) {
        let Some(text) = item.label.get(range) else {
            continue;
        };
        let format = if hit {
            TextFormat {
                font_id: f.clone(),
                color: tk.accent,
                underline: Stroke::new(1.0, paint::alpha(tk.accent, 0.5)),
                ..Default::default()
            }
        } else {
            TextFormat {
                font_id: f.clone(),
                color: ink,
                ..Default::default()
            }
        };
        job.append(text, 0.0, format);
    }
    job.wrap = egui::text::TextWrapping {
        max_width: max_w.max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// The keys, and "searching the server list…" while it does.
fn footer(ui: &mut Ui, st: &Palette, w: f32, tk: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(w, FOOT_H), Sense::hover());
    let p = ui.painter();
    p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, tk.border));
    let cy = r.center().y + 0.5;
    let mut x = r.left() + 12.0;
    for (keys, word) in [
        ("↑↓", "footerMove"),
        ("Enter", "footerRun"),
        ("Tab", "footerSection"),
        ("Esc", "footerClose"),
    ] {
        let kr = Kbd::paint_at(ui, keys, x, cy - 0.5);
        x = kr.right() + 4.0;
        let g = paint::galley(
            ui,
            t("keys", word),
            font(Face::Sans, size::TEXT_3XS),
            tk.fg_faint,
        );
        paint::text_at(ui.painter(), &g, x, cy, tk.fg_faint);
        x += g.size().x + 12.0;
    }
    if st.asking {
        let g = paint::galley(
            ui,
            t("keys", "searching"),
            font(Face::Sans, size::TEXT_3XS),
            tk.fg_faint,
        );
        paint::text_at(
            ui.painter(),
            &g,
            r.right() - 12.0 - g.size().x,
            cy,
            tk.fg_faint,
        );
    }
}

fn item_at(st: &Palette, i: usize) -> Option<&Item> {
    st.sections.iter().flat_map(|s| s.items.iter()).nth(i)
}

fn go(st: &mut Palette, i: usize, len: usize) {
    if len == 0 {
        return;
    }
    st.active = i.min(len - 1);
    st.reveal = true;
}

/// Tab: the next (or previous) section's first item.
fn section(st: &mut Palette, delta: i32) {
    let mut starts = Vec::with_capacity(st.sections.len());
    let mut at = 0usize;
    for s in &st.sections {
        starts.push(at);
        at += s.items.len();
    }
    if starts.is_empty() {
        return;
    }
    let cur = starts.iter().rposition(|s| *s <= st.active).unwrap_or(0) as i32;
    let n = starts.len() as i32;
    let g = (cur + delta).rem_euclid(n) as usize;
    st.active = starts.get(g).copied().unwrap_or(0);
    st.reveal = true;
}

/// The server search: debounced, superseded by a newer one.
fn poll_search(ctx: &Context, st: &mut Palette, now: f64, snap: &Arc<ServerSnapshot>) {
    if let Some(task) = &mut st.search
        && let Some((generation, r)) = task.poll()
    {
        st.search = None;
        if generation == st.generation {
            st.found = r;
            st.found_gen = st.found_gen.wrapping_add(1);
            st.asking = false;
        }
    }
    let term = st.query.trim().to_string();
    let wanted = term.chars().count() >= 2 && fuzzy::address_of(&term).is_none();
    if !wanted {
        if !st.found.is_empty() {
            st.found.clear();
            st.found_gen = st.found_gen.wrapping_add(1);
        }
        st.asking = false;
        st.search_due = None;
        st.searched.clear();
        st.generation = st.generation.wrapping_add(u64::from(st.search.is_some()));
        st.search = None;
        return;
    }
    if term != st.searched {
        st.searched = term;
        st.search_due = Some(now + SEARCH_AFTER);
        st.asking = true;
        ctx.request_repaint_after(Duration::from_secs_f64(SEARCH_AFTER));
    }
    if let Some(due) = st.search_due {
        if now >= due {
            st.search_due = None;
            st.generation = st.generation.wrapping_add(1);
            let generation = st.generation;
            // The browser's own filter and sort over the list in process (the store's
            // snapshot), off the UI thread; full rows only for the six shown.
            let q = query_for(&st.searched);
            let snap = Arc::clone(snap);
            st.search = Some(rt::spawn(async move {
                let rows = q.map_or_else(Vec::new, |q| {
                    let o = order(&snap, &q);
                    o.order
                        .iter()
                        .take(FOUND_MAX)
                        .filter_map(|&i| snap.get(i).map(|r| r.to_row()))
                        .collect()
                });
                (generation, rows)
            }));
        } else {
            ctx.request_repaint_after(Duration::from_secs_f64(due - now));
        }
    }
}

/// Servers the palette lists at most.
const FOUND_MAX: usize = 6;

/// The browser's own query: matching servers, the most populated first, six at most.
fn query_for(term: &str) -> Option<ServerQuery> {
    serde_json::from_value(json!({
        "search": term,
        "maps": [],
        "firstPerson": "all",
        "password": "all",
        "battleye": "all",
        "modded": "all",
        "official": "all",
        "hideEmpty": false,
        "hideFull": false,
        "maxPing": 0,
        "showExcluded": false,
        "sort": "players",
        "asc": false,
        "offset": 0,
        "limit": 6,
    }))
    .ok()
}

/// Rebuild the sections if what they show changed.
fn rebuild(cx: &mut Ctx, st: &mut Palette) {
    let s = &cx.stores;
    let sig = Sig {
        query: st.query.clone(),
        locale: i18n::locale(),
        favorites: s.profile.favorites.len(),
        history: s.profile.history.len(),
        last: s.profile.history.first().map(|h| h.name.clone()),
        mods: s.mods.installed.len(),
        stale: s.mods.stale_count(),
        found: st.found_gen,
        recent: cx.prefs.get().palette_recent.len(),
    };
    if st.sig.as_ref() == Some(&sig) {
        return;
    }
    st.sig = Some(sig);
    st.sections = build(cx, st);
    let len: usize = st.sections.iter().map(|s| s.items.len()).sum();
    if st.active >= len {
        st.active = 0;
    }
}

fn views() -> Vec<Item> {
    PLACES
        .iter()
        .map(|p| {
            let mut it = Item::new(
                format!("view:{}", p.id.id()),
                p.icon,
                t("nav", p.label).to_string(),
                t("keys", "verbOpen"),
                Run::View(p.id),
            );
            it.also = Some(p.id.id().to_string());
            it.kbd = commands::view_key(p.id);
            it
        })
        .collect()
}

fn key_text(id: &str) -> Option<String> {
    commands::shortcut(id).and_then(commands::Shortcut::key_text)
}

fn actions(cx: &Ctx) -> Vec<Item> {
    let s = &cx.stores;
    let mut out = Vec::with_capacity(5);
    let mut refresh = Item::new(
        "act:refresh".into(),
        lucide::REFRESH_CW,
        t("keys", "refresh").into(),
        t("keys", "verbRun"),
        Run::Command("servers.refresh"),
    );
    refresh.kbd = key_text("servers.refresh");
    out.push(refresh);
    if let Some(last) = s.profile.history.first() {
        let mut it = Item::new(
            "act:rejoin".into(),
            lucide::ROTATE_CCW,
            t("keys", "rejoin").into(),
            t("keys", "verbJoin"),
            Run::Command("rejoin"),
        );
        it.sub = Some(last.name.clone());
        it.kbd = key_text("rejoin");
        out.push(it);
    }
    let stale = s.mods.stale_count();
    if stale > 0 {
        let mut it = Item::new(
            "act:update".into(),
            lucide::DOWNLOAD,
            t("keys", "updateMods").into(),
            t("keys", "verbRun"),
            Run::Command("mods.update_behind"),
        );
        it.ink = Ink::Warn;
        it.sub = Some(stale.to_string());
        it.kbd = key_text("mods.update_behind");
        out.push(it);
    }
    out.push(Item::new(
        "act:check".into(),
        lucide::CLOUD_CHECK,
        t("keys", "checkMods").into(),
        t("keys", "verbRun"),
        Run::CheckMods,
    ));
    let mut keys = Item::new(
        "act:keys".into(),
        lucide::KEYBOARD,
        t("keys", "sheet").into(),
        t("keys", "verbShow"),
        Run::Sheet,
    );
    keys.kbd = key_text("sheet.toggle");
    out.push(keys);
    out
}

fn settings() -> Vec<Item> {
    SECTIONS
        .iter()
        .map(|(id, label)| {
            let label = match label {
                Some(k) => t("settings", k),
                None => t("pad", "section"),
            };
            let mut it = Item::new(
                format!("set:{id}"),
                lucide::SETTINGS,
                label.into(),
                t("keys", "verbOpen"),
                Run::Settings(id),
            );
            it.also = Some(if *id == "appearance" {
                "theme colours appearance".to_string()
            } else {
                (*id).to_string()
            });
            it.sub = Some(t("keys", "settings").into());
            it
        })
        .collect()
}

fn favorites(cx: &Ctx) -> Vec<Item> {
    cx.stores
        .profile
        .favorites
        .iter()
        .map(|f| {
            let addr = format!("{}:{}", f.ip, f.port);
            let mut it = Item::new(
                format!("srv:{addr}"),
                lucide::STAR,
                f.name.clone(),
                t("keys", "verbJoin"),
                Run::Join(f.ip.clone(), f.port, f.password.clone()),
            );
            it.ink = Ink::Warn;
            it.also = Some(addr.clone());
            it.sub = Some(addr);
            it.server = Some((f.ip.clone(), f.port));
            it
        })
        .collect()
}

fn history(cx: &Ctx) -> Vec<Item> {
    cx.stores
        .profile
        .history
        .iter()
        .map(|h| {
            let addr = format!("{}:{}", h.ip, h.port);
            let mut it = Item::new(
                format!("srv:{addr}"),
                lucide::HISTORY,
                h.name.clone(),
                t("keys", "verbJoin"),
                Run::Join(h.ip.clone(), h.port, None),
            );
            it.also = Some(addr.clone());
            it.sub = Some(addr);
            it.server = Some((h.ip.clone(), h.port));
            it
        })
        .collect()
}

fn mod_items(cx: &Ctx) -> Vec<Item> {
    cx.stores
        .mods
        .installed
        .iter()
        .map(|m| {
            let mut it = Item::new(
                format!("mod:{}", m.id),
                lucide::PUZZLE,
                m.name.clone(),
                t("keys", "verbShow"),
                Run::Mod(m.name.clone()),
            );
            it.ink = Ink::Mods;
            it.also = Some(m.id.to_string());
            it.sub = Some(format!("{} · {}", m.id, m.size_human));
            it
        })
        .collect()
}

fn listed(st: &Palette) -> Vec<Item> {
    st.found
        .iter()
        .filter_map(|r| {
            let port = u16::try_from(r.game_port).ok()?;
            let mut it = Item::new(
                format!("srv:{}:{port}", r.ip),
                lucide::SERVER,
                r.name.clone(),
                t("keys", "verbJoin"),
                Run::JoinRow(Box::new(r.clone())),
            );
            it.sub = Some(format!(
                "{}/{} · {} · {}:{port}",
                r.players, r.max_players, r.map, r.ip
            ));
            it.server = Some((r.ip.clone(), port));
            Some(it)
        })
        .collect()
}

/// The items of a section that match, best first, at most `CAP`.
fn rank(q: &[char], items: Vec<Item>) -> Vec<Item> {
    let mut out: Vec<Item> = items
        .into_iter()
        .filter_map(|mut it| {
            let m = fuzzy::matches(q, &it.label);
            let best = match m {
                Some(m) => m,
                None => {
                    let m2 = it.also.as_deref().and_then(|a| fuzzy::matches(q, a))?;
                    Match {
                        score: m2.score - 50.0,
                        ranges: Vec::new(),
                    }
                }
            };
            it.ranges = best.ranges;
            it.score = best.score;
            Some(it)
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out.truncate(CAP);
    out
}

fn recents(cx: &Ctx) -> Vec<Recent> {
    cx.prefs
        .get()
        .palette_recent
        .iter()
        .filter_map(|s| serde_json::from_str::<Recent>(s).ok())
        .take(RECENT_MAX)
        .collect()
}

fn build(cx: &Ctx, st: &Palette) -> Vec<Section> {
    let q = fuzzy::fold(st.query.trim());
    let views = views();
    let actions = actions(cx);
    let settings = settings();
    let favorites = favorites(cx);
    let history = history(cx);
    let mods = mod_items(cx);
    if q.is_empty() {
        let pool: Vec<&Item> = views
            .iter()
            .chain(&actions)
            .chain(&settings)
            .chain(&favorites)
            .chain(&history)
            .chain(&mods)
            .collect();
        let rec: Vec<Item> = recents(cx)
            .into_iter()
            .filter_map(|r| {
                if let Some(it) = pool.iter().find(|i| i.key == r.key) {
                    return Some((*it).clone());
                }
                let (ip, port) = (r.ip?, r.port?);
                let mut it = Item::new(
                    r.key,
                    lucide::SERVER,
                    r.label,
                    t("keys", "verbJoin"),
                    Run::Join(ip.clone(), port, None),
                );
                it.sub = r.sub;
                it.server = Some((ip, port));
                Some(it)
            })
            .take(CAP)
            .collect();
        return [
            (t("keys", "recent"), rec),
            (t("keys", "actions"), actions),
            (t("keys", "views"), views),
        ]
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(title, items)| Section { title, items })
        .collect();
    }
    let mut out = Vec::new();
    if let Some((host, port)) = fuzzy::address_of(&st.query) {
        let a = format!("{host}:{port}");
        let mut join = Item::new(
            format!("srv:{a}"),
            lucide::PLAY,
            tf("keys", "connectTo", &[("address", &a)]),
            t("keys", "verbJoin"),
            Run::Join(host.clone(), port, None),
        );
        join.server = Some((host.clone(), port));
        let direct = Item::new(
            format!("direct:{a}"),
            lucide::PLUG_ZAP,
            tf("keys", "directTo", &[("address", &a)]),
            t("keys", "verbOpen"),
            Run::Direct(host, port),
        );
        out.push(Section {
            title: t("keys", "address"),
            items: vec![join, direct],
        });
    }
    let fav_keys: Vec<String> = favorites.iter().map(|f| f.key.clone()).collect();
    let servers: Vec<Item> = listed(st)
        .into_iter()
        .filter(|i| !fav_keys.contains(&i.key))
        .map(|mut i| {
            i.ranges = fuzzy::matches(&q, &i.label)
                .map(|m| m.ranges)
                .unwrap_or_default();
            i.score = 500.0;
            i
        })
        .collect();
    let mut ranked: Vec<Section> = [
        (t("keys", "favorites"), rank(&q, favorites)),
        (
            t("keys", "history"),
            rank(
                &q,
                history
                    .into_iter()
                    .filter(|h| !fav_keys.contains(&h.key))
                    .collect(),
            ),
        ),
        (t("keys", "servers"), servers),
        (t("keys", "mods"), rank(&q, mods)),
        (t("keys", "views"), rank(&q, views)),
        (t("keys", "actions"), rank(&q, actions)),
        (t("keys", "settings"), rank(&q, settings)),
    ]
    .into_iter()
    .filter(|(_, items)| !items.is_empty())
    .map(|(title, items)| Section { title, items })
    .collect();
    // The section with the best match first; ties keep the order above.
    ranked.sort_by(|a, b| {
        let sa = a.items.first().map_or(0.0, |i| i.score);
        let sb = b.items.first().map_or(0.0, |i| i.score);
        sb.total_cmp(&sa)
    });
    out.extend(ranked);
    out
}

/// Remember the pick, close, and do it.
fn choose(cx: &mut Ctx, item: Item) {
    let r = Recent {
        key: item.key.clone(),
        label: item.label.clone(),
        sub: item.sub.clone(),
        ip: item.server.as_ref().map(|s| s.0.clone()),
        port: item.server.as_ref().map(|s| s.1),
    };
    let mut list = recents(cx);
    list.retain(|x| x.key != r.key);
    list.insert(0, r);
    list.truncate(RECENT_MAX);
    let enc: Vec<String> = list
        .iter()
        .filter_map(|r| serde_json::to_string(r).ok())
        .collect();
    if cx.prefs.get().palette_recent != enc {
        cx.prefs.edit().palette_recent = enc;
    }
    cx.stores.dialogs.palette = false;
    match item.run {
        Run::View(v) => cx.go(v),
        Run::Command(id) => {
            let _ = cx.run(id, &Value::Null);
        }
        Run::Settings(id) => cx.go_focus(ViewId::Settings, id),
        Run::Join(ip, port, pw) => cx.join_address(ip, port, pw),
        Run::JoinRow(row) => cx.join_row(*row, None),
        Run::Direct(ip, port) => {
            let _ = cx.run("connect.open_in_direct", &json!({ "ip": ip, "port": port }));
        }
        Run::Mod(name) => cx.go_focus(ViewId::Mods, format!("search:{name}")),
        Run::CheckMods => {
            let _ = cx.run("mods.check_updates", &json!({ "open": true }));
        }
        // After the palette has closed, or closing it would close the sheet too.
        Run::Sheet => cx.stores.dialogs.shortcuts = true,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_server_query_parses() {
        let q = query_for("deer isle").unwrap();
        assert_eq!(q.search, "deer isle");
        assert_eq!(q.limit, 6);
    }

    #[test]
    fn ranking_keeps_the_best_five() {
        let q = fuzzy::fold("se");
        let items: Vec<Item> = (0..9)
            .map(|i| {
                Item::new(
                    format!("k{i}"),
                    lucide::SERVER,
                    format!("Server {i}"),
                    "Open",
                    Run::Sheet,
                )
            })
            .collect();
        let r = rank(&q, items);
        assert_eq!(r.len(), CAP);
        assert!(r.iter().all(|i| i.ranges == vec![(0, 2)]));
    }
}
