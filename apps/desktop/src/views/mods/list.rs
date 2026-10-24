//! The table of mods (the list pane of `View.svelte`, `ModState.svelte`, `SourceTag
//! svelte`'s line, `ModMenu*.svelte`): a virtual list of 48px rows under a sticky
//! heading, folded into status groups when sorted by status, a bulk bar at its foot while
//! some are ticked.

use dz_hub::features::mods::{InstalledModDto, ModSourceDto};
use egui::{CornerRadius, Pos2, Rangef, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use serde_json::{Value, json};

use super::UiState;
use super::model::{self, BEHIND, Col, Filter, INCOMING, Item, Row, Where};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{
    Align, Brand, Button, Checkbox, IconButton, MenuItem, Meter, MeterSize, Side, Size, Skeleton,
    Tag, TipExt, Tone, Variant, VirtualList, menu,
};
use crate::parts::{BulkBar, Empty, SortHead, TableHead, draw, ph, ph_trunc};
use crate::store::mods::{Mods, OpState};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

type Run = Vec<(&'static str, Value)>;

const ROW_H: f32 = 48.0;
const HEAD_H: f32 = 28.0;
const GAP: f32 = 12.0;

/// tick · mod · state · in game · size · your version · actions. Narrow, "your
/// version" goes first (< 820px), then "in game" (< 620px).
pub(super) struct Cols {
    tick: Rangef,
    name: Rangef,
    status: Rangef,
    in_game: Option<Rangef>,
    size: Rangef,
    version: Option<Rangef>,
    actions: Rangef,
}

impl Cols {
    pub(super) fn new(x: Rangef) -> Self {
        let w = x.span();
        let show_version = w >= 820.0;
        let show_in_game = w >= 620.0;
        let left = x.min + size::PAD;
        let right = x.max - size::PAD;
        let tick = Rangef::new(left, left + 20.0);
        // From the right: actions, version, size, in game, status.
        let mut r = right;
        let mut take = |w: f32| {
            let c = Rangef::new(r - w, r);
            r -= w + GAP;
            c
        };
        let actions = take(60.0);
        let version = show_version.then(|| take(104.0));
        let size = take(72.0);
        // Room for "Linked to game" whole, larger in the pad density too.
        let grow = size::text(size::TEXT_2XS) / size::TEXT_2XS;
        let in_game = show_in_game.then(|| take(112.0 * grow));
        // The tag's text grows with the pad density ("Mise à jour disponible", "Steam
        // downloading 100%" in 13 px, its spinner wider too).
        let status_w = if size::pad_density() { 178.0 } else { 168.0 };
        let status = take(status_w * grow);
        let name = Rangef::new(tick.max + GAP, (r + GAP - GAP).max(tick.max + GAP + 8.0));
        Self {
            tick,
            name,
            status,
            in_game,
            size,
            version,
            actions,
        }
    }
}

fn cell(x: Rangef, y: Rangef) -> Rect {
    Rect::from_x_y_ranges(x, y)
}

/// The list pane: loading shape, nothing installed, or the table; the bulk bar under.
pub(super) fn show(ui: &mut Ui, cx: &Ctx, st: &mut UiState, run: &mut Run) {
    let rect = ui.available_rect_before_wrap();
    let mods = &cx.stores.mods;
    let bulk_h = if st.ticked.is_empty() {
        0.0
    } else {
        6.0 + size::CONTROL_SM + 6.0
    };
    let list_rect = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - bulk_h));
    let mut lui = ui.new_child(
        UiBuilder::new()
            .max_rect(list_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    lui.set_clip_rect(list_rect.intersect(ui.clip_rect()));
    if !mods.loaded && mods.installed.is_empty() {
        loading(&mut lui);
    } else if st.listing.total == 0 {
        Empty::new()
            .icon(lucide::PUZZLE)
            .title(t("mods", "noMods"))
            .text(t("mods", "noModsHint"))
            .show(&mut lui, |ui| {
                if ui
                    .add(
                        Button::new(t("mods", "install"))
                            .icon(lucide::DOWNLOAD)
                            .variant(Variant::Accent),
                    )
                    .clicked()
                {
                    run.push(("mods.install_dialog", Value::Null));
                }
            });
    } else {
        table(&mut lui, cx, st, run);
    }
    if bulk_h > 0.0 {
        let br = Rect::from_min_max(Pos2::new(rect.left(), list_rect.bottom()), rect.max);
        let mut bui = ui.new_child(
            UiBuilder::new()
                .max_rect(br)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        bulk(&mut bui, mods, st, run);
    }
    ui.allocate_rect(rect, Sense::hover());
}

/// The list's shape while the disk is read: the heading and grey rows, so nothing
/// jumps when the mods arrive.
fn loading(ui: &mut Ui) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (head, _) = ui.allocate_exact_size(Vec2::new(w, HEAD_H), Sense::hover());
    paint_head_ground(ui, head, &tk);
    let cols = Cols::new(head.x_range());
    for (x, k) in [(cols.name, "colName"), (cols.status, "colStatus")] {
        let g = crate::shell::stencil(ui, t("mods", k), egui::Color32::PLACEHOLDER, None);
        draw(ui.painter(), &g, x.min, head.center().y, tk.fg_faint);
    }
    for i in 0..10usize {
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, size::row(ROW_H)), Sense::hover());
        let p = ui.painter();
        p.hline(
            r.x_range(),
            r.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.4)),
        );
        let cy = r.center().y;
        let at = |x: f32, w: f32, h: f32, dy: f32| {
            Rect::from_min_size(Pos2::new(x, cy + dy - h / 2.0), Vec2::new(w, h))
        };
        p.rect_filled(
            at(cols.tick.min + 3.0, 14.0, 14.0, 0.0),
            CornerRadius::same(size::RADIUS_XS),
            tk.raised,
        );
        let nw = cols.name.span() * (40.0 + ((i * 37) % 45) as f32) / 100.0;
        Skeleton::paint(ui, at(cols.name.min, nw, 10.0, -6.0), None, 1.0);
        Skeleton::paint(ui, at(cols.name.min, 96.0, 8.0, 8.0), None, 0.7);
        Skeleton::paint(
            ui,
            at(cols.status.min, 80.0, 16.0, 0.0),
            Some(size::RADIUS_XS),
            1.0,
        );
        if let Some(x) = cols.in_game {
            Skeleton::paint(ui, at(x.min, 48.0, 8.0, 0.0), None, 0.7);
        }
        Skeleton::paint(ui, at(cols.size.max - 40.0, 40.0, 8.0, 0.0), None, 0.7);
        if let Some(x) = cols.version {
            Skeleton::paint(ui, at(x.min, 64.0, 8.0, 0.0), None, 0.7);
        }
    }
}

fn paint_head_ground(ui: &Ui, r: Rect, tk: &Tokens) {
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::ZERO, alpha(tk.panel, 0.95));
    p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, tk.border));
}

/// A widget placed in a column, left- or right-aligned, centred in `y`.
fn place<R>(ui: &mut Ui, x: Rangef, y: Rangef, right: bool, f: impl FnOnce(&mut Ui) -> R) -> R {
    let layout = if right {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    let mut c = ui.new_child(
        UiBuilder::new()
            .max_rect(cell(x, y))
            .layout(layout)
            .id_salt(("cell", x.min as i32)),
    );
    c.spacing_mut().item_spacing.x = 2.0;
    f(&mut c)
}

fn table(ui: &mut Ui, cx: &Ctx, st: &mut UiState, run: &mut Run) {
    let mods = &cx.stores.mods;
    let UiState {
        listing,
        ticked,
        focus,
        folded,
        open_menu,
        scroll_to,
        col,
        asc,
        filter,
        query,
        ..
    } = st;
    let scroll = scroll_to
        .take()
        .and_then(|id| listing.row_by_id(id))
        .and_then(|r| listing.item_of(r));
    let tickable: Vec<u64> = listing
        .rows
        .iter()
        .filter(|r| r.installed.is_some())
        .map(|r| r.id)
        .collect();
    let all_ticked = !tickable.is_empty() && tickable.iter().all(|id| ticked.contains(id));
    let some_ticked = !all_ticked && tickable.iter().any(|id| ticked.contains(id));
    let mut sort: Option<Col> = None;
    let mut tick_all: Option<bool> = None;
    let mut fold: Option<i8> = None;
    let mut ticks: Vec<(u64, bool)> = Vec::new();
    let mut opened: Option<u64> = None;
    let menu_for = *open_menu;
    let count = listing.items.len();
    // The open mod is the list's cursor; headings above it are not stops.
    let cursor = focus
        .and_then(|id| listing.row_by_id(id))
        .and_then(|r| listing.item_of(r));
    let first = cursor.is_some_and(|c| {
        !listing
            .items
            .iter()
            .take(c)
            .any(|i| matches!(i, model::Item::Row(_)))
    });
    let out = VirtualList::new("mods-list", count)
        .row_height(size::row(ROW_H))
        .header(HEAD_H)
        .scroll_to(scroll)
        .cursor(cursor)
        .cursor_first(first)
        .show(
            ui,
            |ui| {
                let tk = ui.tokens();
                TableHead::new(false).show(ui, |ui, _| {
                    let r = ui.max_rect();
                    let cols = Cols::new(r.x_range());
                    let y = Rangef::new(r.top() - 1.0, r.bottom() - 1.0);
                    let _ = tk;
                    place(ui, cols.tick, y, false, |ui| {
                        let mut on = all_ticked;
                        let label = if all_ticked {
                            t("mods", "deselectAll")
                        } else {
                            t("mods", "selectAll")
                        };
                        if ui
                            .add(
                                Checkbox::new(&mut on)
                                    .indeterminate(some_ticked)
                                    .label(label),
                            )
                            .changed()
                        {
                            tick_all = Some(on);
                        }
                    });
                    let mut head = |ui: &mut Ui, x: Rangef, c: Col, k: &str, right: bool| {
                        place(ui, x, y, right, |ui| {
                            if ui
                                .add(SortHead::new(t("mods", k), *col == c, *asc))
                                .clicked()
                            {
                                sort = Some(c);
                            }
                        });
                    };
                    head(ui, cols.name, Col::Name, "colName", false);
                    head(ui, cols.status, Col::Status, "colStatus", false);
                    if let Some(x) = cols.in_game {
                        head(ui, x, Col::Managed, "colInGame", false);
                    }
                    head(ui, cols.size, Col::Size, "colSize", true);
                    if let Some(x) = cols.version {
                        head(ui, x, Col::Local, "colDownloaded", false);
                    }
                });
            },
            |ui, i| match listing.items.get(i) {
                Some(Item::Group {
                    status,
                    count,
                    size,
                }) => {
                    if group_head(ui, *status, *count, *size, folded.contains(status), i == 0) {
                        fold = Some(*status);
                    }
                }
                Some(Item::Row(r)) => {
                    let Some(row) = listing.rows.get(*r) else {
                        return;
                    };
                    let m = row.installed.and_then(|i| mods.installed.get(i));
                    let look = RowLook {
                        focused: *focus == Some(row.id),
                        ticked: ticked.contains(&row.id),
                        open_menu: menu_for == Some(row.id),
                    };
                    let a = mod_row(ui, mods, row, m, look, run);
                    if let Some(on) = a.tick {
                        ticks.push((row.id, on));
                    }
                    if a.open {
                        opened = Some(row.id);
                    }
                }
                None => {}
            },
        );
    if count == 0 && *filter == Filter::Unused && query.trim().is_empty() {
        // Nothing unused, or not known yet (the strip above says which servers).
        let known = mods.unused().is_some_and(|u| u.unknown.is_empty());
        let mut e = ui.new_child(UiBuilder::new().max_rect(out.viewport));
        let (icon, title, text) = if known {
            (lucide::PACKAGE_CHECK, "noUnused", "noUnusedText")
        } else {
            (
                lucide::CIRCLE_ALERT,
                "unusedUnknownTitle",
                "unusedUnknownText",
            )
        };
        Empty::new()
            .icon(icon)
            .title(t("mods", title))
            .text(t("mods", text))
            .show(&mut e, |_| {});
    } else if count == 0 {
        // Filters left nothing: say so, with the way back.
        let mut e = ui.new_child(UiBuilder::new().max_rect(out.viewport));
        Empty::new()
            .icon(lucide::SEARCH_X)
            .title(t("mods", "noMatch"))
            .show(&mut e, |ui| {
                if ui.add(Button::new(t("mods", "clearFilters"))).clicked() {
                    run.push(("mods.search", json!({ "text": "" })));
                }
            });
    }
    let mut changed = false;
    if let Some(c) = sort {
        if *col == c {
            *asc = !*asc;
        } else {
            *col = c;
            *asc = c.first_asc();
        }
        changed = true;
    }
    if let Some(on) = tick_all {
        if on {
            *ticked = tickable.into_iter().collect();
        } else {
            ticked.clear();
        }
        changed = true;
    }
    if let Some(g) = fold {
        if !folded.remove(&g) {
            folded.insert(g);
        }
        changed = true;
    }
    for (id, on) in ticks {
        if on {
            ticked.insert(id);
        } else {
            ticked.remove(&id);
        }
        changed = true;
    }
    if let Some(id) = opened {
        *focus = Some(id);
        changed = true;
    }
    if menu_for.is_some() {
        *open_menu = None;
    }
    if changed {
        ui.ctx().request_repaint();
    }
}

/// A status group's heading (`groupHead`): chevron, stencilled label in its tone, count,
/// total size at the right; a click folds it. True when clicked. The words sit on a band
/// at the row's foot (the room above parts the group from the one before), as the
/// favourites' do; the list's first heading's band fills its row: nothing above it.
fn group_head(
    ui: &mut Ui,
    status: i8,
    count: usize,
    bytes: u64,
    folded: bool,
    first: bool,
) -> bool {
    let tk = ui.tokens();
    let r = ui.max_rect();
    let resp = ui
        .interact(r, ui.scope_id().with(("group", status)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let h = paint::ease(ui, resp.id.with("h"), resp.hovered());
    let p = ui.painter();
    // The band is the heading's own: the room above it stays the list's ground.
    let band = if first {
        r
    } else {
        Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 30.0), r.max)
    };
    p.rect_filled(band, CornerRadius::ZERO, alpha(tk.bg, 0.5));
    if h > 0.0 {
        p.rect_filled(band, CornerRadius::ZERO, alpha(tk.raised, 0.4 * h));
    }
    p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, tk.border));
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, r.shrink(2.0), size::RADIUS_XS, &tk);
    }
    let cy = band.center().y + 1.5;
    let mut x = r.left() + size::PAD;
    let icon = if folded {
        lucide::CHEVRON_RIGHT
    } else {
        lucide::CHEVRON_DOWN
    };
    icon.paint(
        p,
        Rect::from_center_size(Pos2::new(x + 7.0, cy - 1.5), Vec2::splat(14.0)),
        tk.fg_faint,
    );
    x += 14.0 + 8.0;
    let (key, meaning, ink) = match status {
        INCOMING => ("groupSteam", "steamDownloadsTitle", tk.accent),
        BEHIND => ("groupUpdates", "stateTipUpdate", tk.warn),
        model::UNCHECKED => ("detailsUnknown", "stateTipUnchecked", tk.fg_muted),
        _ => ("detailsUpToDate", "stateTipUpToDate", tk.fg_muted),
    };
    let g = crate::shell::stencil(ui, t("mods", key), egui::Color32::PLACEHOLDER, None);
    draw(p, &g, x, cy, ink);
    x += g.size().x + 8.0;
    let n = ph(ui, &count.to_string(), font(Face::Mono, size::TEXT_2XS));
    draw(p, &n, x, cy - 2.0, tk.fg_faint);
    if bytes > 0 {
        let s = ph(
            ui,
            &format::bytes(bytes as f64),
            font(Face::Mono, size::TEXT_2XS),
        );
        draw(
            p,
            &s,
            r.right() - size::PAD - s.size().x,
            cy - 2.0,
            tk.fg_faint,
        );
    }
    // What the group means, then what a click does.
    let tip = format!("{}\n{}", t("mods", meaning), t("mods", "groupCollapse"));
    let _ = resp.clone().tip(&tip);
    resp.clicked()
}

#[derive(Clone, Copy)]
struct RowLook {
    focused: bool,
    ticked: bool,
    open_menu: bool,
}

#[derive(Default)]
struct RowAct {
    tick: Option<bool>,
    open: bool,
}

fn mod_row(
    ui: &mut Ui,
    mods: &Mods,
    row: &Row,
    m: Option<&InstalledModDto>,
    look: RowLook,
    run: &mut Run,
) -> RowAct {
    let tk = ui.tokens();
    let r = ui.max_rect();
    let mut act = RowAct::default();
    let sense = if m.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(r, ui.scope_id().with(("mod", row.id)), sense);
    let menu_open = menu::is_open(&resp);
    let h = paint::ease(ui, resp.id.with("h"), resp.hovered());
    let fill = if look.focused {
        alpha(tk.accent, 0.10)
    } else if menu_open {
        alpha(tk.raised, 0.7)
    } else if look.ticked {
        alpha(tk.raised, 0.5)
    } else {
        alpha(tk.raised, 0.4 * h)
    };
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::ZERO, fill);
    p.hline(
        r.x_range(),
        r.bottom() - 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.4)),
    );
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(ui.painter(), r.shrink(2.0), size::RADIUS_XS, &tk);
    }
    if let Some(m) = m {
        if resp.clicked() {
            act.open = true;
        }
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        menu::context(&resp, |ui| menu_items(ui, mods, m, run));
    }
    let cols = Cols::new(r.x_range());
    let y = r.y_range();
    let cy = r.center().y;

    // tick
    if m.is_some() {
        place(ui, cols.tick, y, false, |ui| {
            let mut on = look.ticked;
            if ui.add(Checkbox::new(&mut on).label(&row.name)).changed() {
                act.tick = Some(on);
            }
        });
    }

    // name and whose copy it is
    let ng = ph_trunc(
        ui,
        &row.name,
        font(Face::SansMedium, size::TEXT_XS),
        cols.name.span(),
    );
    let name_ink = if m.is_some() { tk.fg } else { tk.fg_muted };
    draw(ui.painter(), &ng, cols.name.min, cy - 11.0, name_ink);
    let line = match m {
        Some(m) => source_line(mods, m),
        None => SourceLine {
            text: t("mods", "whereSubscribed").to_string(),
            tip: String::new(),
            steam: true,
            muted: true,
        },
    };
    let lr = Rect::from_min_size(
        Pos2::new(cols.name.min, cy + 8.0 - 7.0),
        Vec2::new(cols.name.span(), 14.0),
    );
    paint_source_line(ui, lr, &line);
    if !line.tip.is_empty() {
        let _ = ui
            .interact(lr, ui.scope_id().with(("src", row.id)), Sense::hover())
            .tip(&line.tip);
    }

    // state
    let lk = state_look(mods, row.id, m);
    let second = lk.second_height(true);
    let sh = 18.0 + if second > 0.0 { 4.0 + second } else { 0.0 };
    // The Svelte column sits its two lines a little above the middle.
    let lift = match lk.second {
        Second::Behind(_) => 2.0,
        Second::Bar(_) => 1.0,
        Second::None => 0.0,
    };
    let sr = super::centred(cell(cols.status, y), sh).translate(Vec2::new(0.0, -lift));
    let mut sui = ui.new_child(
        UiBuilder::new()
            .max_rect(sr)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .id_salt(("state", row.id)),
    );
    sui.spacing_mut().item_spacing.y = 4.0;
    lk.show(&mut sui, true);

    match m {
        Some(m) => {
            if let Some(x) = cols.in_game {
                let (icon, word, tip, ink) = if m.managed {
                    (
                        lucide::LINK,
                        "inGameLinked",
                        "inGameLinkedTitle",
                        tk.fg_muted,
                    )
                } else {
                    (
                        lucide::UNLINK,
                        "inGameNotLinked",
                        "inGameNotLinkedTitle",
                        tk.fg_faint,
                    )
                };
                let g = ph_trunc(
                    ui,
                    t("mods", word),
                    font(Face::Sans, size::TEXT_XS),
                    x.span() - 16.0,
                );
                icon.paint(
                    ui.painter(),
                    Rect::from_center_size(Pos2::new(x.min + 6.0, cy), Vec2::splat(12.0)),
                    ink,
                );
                draw(ui.painter(), &g, x.min + 16.0, cy, ink);
                let cr = Rect::from_min_size(
                    Pos2::new(x.min, cy - 8.0),
                    Vec2::new(16.0 + g.size().x, 16.0),
                );
                let _ = ui
                    .interact(cr, ui.scope_id().with(("ingame", row.id)), Sense::hover())
                    .tip(t("mods", tip));
            }
            let sg = ph(ui, &m.size_human, font(Face::Mono, size::TEXT_2XS));
            draw(
                ui.painter(),
                &sg,
                cols.size.max - sg.size().x,
                cy,
                tk.fg_muted,
            );
            if let Some(x) = cols.version {
                let d = format::date(m.local_updated);
                let g = ph_trunc(ui, &d, font(Face::Mono, size::TEXT_2XS), x.span());
                draw(ui.painter(), &g, x.min, cy, tk.fg_muted);
                // The date is your copy's; the Workshop's, when known, on hover.
                let mut tip = format!("{}: {d}", t("mods", "yourCopy"));
                if let Some(remote) = m.remote_updated {
                    tip.push_str(&format!(
                        "\n{}: {}",
                        t("mods", "workshopCopy"),
                        format::date(remote)
                    ));
                }
                let vr =
                    Rect::from_min_size(Pos2::new(x.min, cy - 8.0), Vec2::new(g.size().x, 16.0));
                let _ = ui
                    .interact(vr, ui.scope_id().with(("ver", row.id)), Sense::hover())
                    .tip(&tip);
            }
        }
        None => {
            let s = if row.size > 0 {
                format::bytes(row.size as f64)
            } else {
                format::NONE.to_string()
            };
            let sg = ph(ui, &s, font(Face::Mono, size::TEXT_2XS));
            draw(
                ui.painter(),
                &sg,
                cols.size.max - sg.size().x,
                cy,
                tk.fg_faint,
            );
        }
    }

    // actions
    place(ui, cols.actions, y, true, |ui| match m {
        Some(m) => {
            let trig = ui.add(
                IconButton::new(lucide::ELLIPSIS, t("mods", "moreActions"))
                    .size(Size::IconXs)
                    .variant(Variant::Ghost)
                    .side(Side::Left)
                    .active(menu_open),
            );
            if look.open_menu {
                egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&trig));
            }
            menu::dropdown(&trig, Side::Bottom, Align::End, |ui| {
                menu_items(ui, mods, m, run);
            });
            let s = mods.steam_item(m.id);
            if m.update_available
                && mods.op_state(m.id).is_none()
                && !s.is_some_and(|s| s.downloading || s.pending)
                && ui
                    .add(
                        IconButton::new(lucide::REFRESH_CW, t("mods", "update"))
                            .size(Size::IconXs)
                            .variant(Variant::Accent)
                            .kbd("U"),
                    )
                    .clicked()
            {
                run.push(("mods.update_one", json!({ "id": m.id })));
            }
        }
        None => {
            if ui
                .add(
                    IconButton::new(lucide::EXTERNAL_LINK, t("mods", "openWorkshop"))
                        .size(Size::IconXs)
                        .variant(Variant::Ghost),
                )
                .clicked()
            {
                run.push(("mods.open_workshop", json!({ "id": row.id })));
            }
        }
    });
    act
}

/// Everything that can be done to one mod: its "⋯" menu and its right-click menu.
pub(super) fn menu_items(ui: &mut Ui, mods: &Mods, m: &InstalledModDto, run: &mut Run) {
    let key = |id: &str| {
        crate::commands::shortcut(id)
            .and_then(crate::commands::Shortcut::key_text)
            .unwrap_or_default()
    };
    let (ku, km, kd) = (key("mods.update"), key("mods.managed"), key("mods.delete"));
    let id = json!({ "id": m.id });
    let word = if m.update_available {
        "update"
    } else {
        "redownload"
    };
    if ui
        .add(
            MenuItem::new(t("mods", word))
                .icon(lucide::REFRESH_CW)
                .kbd(&ku),
        )
        .clicked()
    {
        run.push(("mods.update_one", id.clone()));
    }
    if ui
        .add(MenuItem::new(t("mods", "repair")).icon(lucide::WRENCH))
        .clicked()
    {
        run.push(("mods.repair", id.clone()));
    }
    menu::separator(ui);
    let (icon, word) = if m.managed {
        (lucide::UNLINK, "unlinkAction")
    } else {
        (lucide::LINK, "linkAction")
    };
    if ui
        .add(MenuItem::new(t("mods", word)).icon(icon).kbd(&km))
        .clicked()
    {
        run.push(("mods.toggle_managed", id.clone()));
    }
    if crate::platform::deck::folders()
        && ui
            .add(MenuItem::new(t("mods", "openModFolder")).icon(lucide::FOLDER_OPEN))
            .clicked()
    {
        run.push(("mods.open_mod_dir", id.clone()));
    }
    if ui
        .add(MenuItem::new(t("mods", "openWorkshop")).icon(lucide::EXTERNAL_LINK))
        .clicked()
    {
        run.push(("mods.open_workshop", id.clone()));
    }
    if ui
        .add(MenuItem::new(t("mods", "copyId")).icon(lucide::HASH))
        .clicked()
    {
        run.push(("mods.copy_id", id.clone()));
    }
    menu::separator(ui);
    if ui
        .add(
            MenuItem::new(t("mods", "delete"))
                .icon(lucide::TRASH_2)
                .kbd(&kd)
                .danger(),
        )
        .clicked()
    {
        run.push(("mods.delete", id));
    }
    let _ = mods;
}

// ── whose copy (`SourceTag.svelte`) ─────────────────────────────────────────

pub(super) struct SourceLine {
    pub text: String,
    pub tip: String,
    /// A Steam copy (the Steam mark), else the launcher's (a drive).
    pub steam: bool,
    /// Subscribed: a shade brighter.
    pub muted: bool,
}

/// The quiet line under a name: whose copy, also subscribed, what requires it, which
/// favourites run it.
pub(super) fn source_line(mods: &Mods, m: &InstalledModDto) -> SourceLine {
    let item = mods.steam_item(m.id);
    let answered = mods.steam.as_ref().is_some_and(|s| s.available);
    let w = model::where_of(m, item, answered);
    let mut text = w.label(true).to_string();
    if w == Where::Launcher && item.is_some_and(|i| i.subscribed) {
        text.push_str(" · ");
        text.push_str(t("mods", "alsoSubscribed"));
    }
    let parents = mods.required_by(m.id);
    if let Some(first) = parents.first() {
        text.push_str(" · ");
        text.push_str(&tf(
            "mods",
            "requiredByLine",
            &[("name", &mods.name_of(*first))],
        ));
        if parents.len() > 1 {
            text.push_str(&format!(" +{}", parents.len() - 1));
        }
    }
    let used = mods.usage.get(&m.id);
    let used_line = match used {
        Some(u) if u.favorites.len() == 1 => t("mods", "usedByFavOne").to_string(),
        Some(u) if !u.favorites.is_empty() => {
            tf("mods", "usedByFav", &[("count", &u.favorites.len())])
        }
        Some(u) if u.history.len() == 1 => t("mods", "playedOnOne").to_string(),
        Some(u) if !u.history.is_empty() => tf("mods", "playedOn", &[("count", &u.history.len())]),
        _ => String::new(),
    };
    if !used_line.is_empty() {
        text.push_str(" · ");
        text.push_str(&used_line);
    }
    SourceLine {
        text,
        tip: source_tip(mods, m, w),
        steam: m.source == ModSourceDto::Steam,
        muted: w == Where::Subscribed,
    }
}

/// The source's hint: what it means, the other copy, who runs it.
pub(super) fn source_tip(mods: &Mods, m: &InstalledModDto, w: Where) -> String {
    let mut lines: Vec<String> = vec![w.hint().to_string()];
    if m.other_copy {
        lines.push(t("mods", "otherCopy").to_string());
    }
    if let Some(u) = mods.usage.get(&m.id)
        && (!u.favorites.is_empty() || !u.history.is_empty())
    {
        let names: Vec<&str> = u
            .favorites
            .iter()
            .chain(u.history.iter())
            .map(String::as_str)
            .collect();
        lines.push(tf("mods", "usedByTitle", &[("names", &names.join(", "))]));
    }
    lines.join("\n")
}

fn paint_source_line(ui: &Ui, r: Rect, line: &SourceLine) {
    let tk = ui.tokens();
    let ink = if line.muted { tk.fg_muted } else { tk.fg_faint };
    let ir = Rect::from_center_size(Pos2::new(r.left() + 5.0, r.center().y), Vec2::splat(10.0));
    if line.steam {
        Brand::Steam.paint(ui.painter(), ir, ink);
    } else {
        lucide::HARD_DRIVE_DOWNLOAD.paint(ui.painter(), ir, ink);
    }
    let g = ph_trunc(
        ui,
        &line.text,
        font(Face::Sans, size::TEXT_2XS),
        (r.width() - 14.0).max(8.0),
    );
    draw(ui.painter(), &g, r.left() + 14.0, r.center().y, ink);
}

// ── where a mod stands (`ModState.svelte`) ──────────────────────────────────

pub(super) enum Second {
    None,
    /// A download's bar (its share when known).
    Bar(Option<f32>),
    /// How far behind.
    Behind(String),
}

pub(super) struct StateLook {
    text: String,
    tone: Tone,
    spinner: bool,
    tip: &'static str,
    second: Second,
}

fn pct(f: Option<f32>) -> String {
    f.map_or_else(String::new, |f| format!(" {}%", (f * 100.0).floor() as i64))
}

/// Where one mod stands, first match wins: the launcher downloads it or has it
/// waiting; Steam downloads it or has it waiting; it is behind; up to date; not checked.
pub(super) fn state_look(mods: &Mods, id: u64, m: Option<&InstalledModDto>) -> StateLook {
    let op = mods.op_state(id);
    let steam = mods.steam_item(id);
    let waiting =
        steam.is_some_and(|s| !s.downloading && (s.pending || (s.subscribed && s.needs_update)));
    let mk =
        |text: String, tone: Tone, spinner: bool, tip: &'static str, second: Second| StateLook {
            text,
            tone,
            spinner,
            tip,
            second,
        };
    if op == Some(OpState::Downloading) {
        let share = model::op_progress(&mods.op).map(|p| (p.percent / 100.0) as f32);
        return mk(
            format!("{}{}", t("mods", "stateDownloading"), pct(share)),
            Tone::Accent,
            true,
            t("mods", "stateTipDownloading"),
            Second::Bar(share),
        );
    }
    if op == Some(OpState::Queued) {
        return mk(
            t("mods", "stateQueued").into(),
            Tone::Neutral,
            false,
            t("mods", "stateTipQueued"),
            Second::None,
        );
    }
    if let Some(s) = steam.filter(|s| s.downloading) {
        let share = (s.bytes_total > 0).then(|| s.bytes_done as f32 / s.bytes_total as f32);
        return mk(
            format!("{}{}", t("mods", "steamDownloading"), pct(share)),
            Tone::Accent,
            true,
            t("mods", "steamDownloadsTitle"),
            Second::Bar(share),
        );
    }
    if waiting || (m.is_none() && steam.is_some()) {
        return mk(
            t("mods", "steamQueued").into(),
            Tone::Neutral,
            false,
            t("mods", "steamDownloadsTitle"),
            Second::None,
        );
    }
    match m {
        Some(m) if m.update_available => {
            let days = model::days_behind(m.local_updated, m.remote_updated);
            let behind = if days > 0 {
                tf("mods", "daysBehind", &[("days", &days)])
            } else {
                t("mods", "behindToday").to_string()
            };
            mk(
                t("mods", "updateAvailable").into(),
                Tone::Warn,
                false,
                t("mods", "stateTipUpdate"),
                Second::Behind(behind),
            )
        }
        Some(m) if m.remote_updated.is_some() => mk(
            t("mods", "detailsUpToDate").into(),
            Tone::Ok,
            false,
            t("mods", "stateTipUpToDate"),
            Second::None,
        ),
        _ => mk(
            t("mods", "detailsUnknown").into(),
            Tone::Neutral,
            false,
            t("mods", "stateTipUnchecked"),
            Second::None,
        ),
    }
}

impl StateLook {
    /// The height of the second line (`behind` only), 0 without one.
    pub(super) fn second_height(&self, behind: bool) -> f32 {
        if !behind {
            return 0.0;
        }
        match self.second {
            Second::None => 0.0,
            Second::Bar(_) => 2.0,
            Second::Behind(_) => 14.0,
        }
    }

    /// The tag; with `behind`, the second line under it.
    pub(super) fn show(&self, ui: &mut Ui, behind: bool) {
        let mut tag = Tag::new(&self.text).tone(self.tone).tip(self.tip);
        if self.spinner {
            tag = tag.spinner();
        }
        ui.add(tag);
        if !behind {
            return;
        }
        match &self.second {
            Second::None => {}
            Second::Bar(share) => {
                ui.add(
                    Meter::new(share.unwrap_or(0.0))
                        .size(MeterSize::Xs)
                        .width(96.0)
                        .animate(share.is_some()),
                );
            }
            Second::Behind(s) => {
                let tk = ui.tokens();
                let g = ph_trunc(
                    ui,
                    s,
                    font(Face::Mono, size::TEXT_2XS),
                    ui.available_width(),
                );
                let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, 14.0), Sense::hover());
                draw(
                    ui.painter(),
                    &g,
                    r.left(),
                    r.center().y,
                    alpha(tk.warn, 0.8),
                );
            }
        }
    }
}

// ── the bulk bar ────────────────────────────────────────────────────────────

fn bulk(ui: &mut Ui, mods: &Mods, st: &UiState, out: &mut Run) {
    let run = std::cell::RefCell::new(Vec::<(&'static str, Value)>::new());
    let ticked: Vec<&InstalledModDto> = mods
        .installed
        .iter()
        .filter(|m| st.ticked.contains(&m.id))
        .collect();
    let n = st.ticked.len();
    let size: u64 = ticked.iter().map(|m| m.size).sum();
    let stale = ticked.iter().filter(|m| m.update_available).count();
    let linked = ticked.iter().filter(|m| m.managed).count();
    let label = if n == 1 {
        tf("mods", "selectedOne", &[("count", &1)])
    } else {
        tf("mods", "selected", &[("count", &n)])
    };
    let info = tf(
        "mods",
        "selectedSize",
        &[("size", &format::bytes(size as f64))],
    );
    let all_stale = mods.stale_count();
    let words = |k: &str, c: usize| tf("mods", k, &[("count", &c)]);
    let (us, ust) = (
        words("updateStale", stale),
        words("updateStaleTitle", stale),
    );
    let (rv, rvt) = (words("revalidateCount", n), words("revalidateTitle", n));
    let (lk, lkt) = (words("link", n - linked), words("linkTitle", n - linked));
    let (ul, ult) = (words("unlink", linked), words("unlinkTitle", linked));
    let (dl, dlt) = (words("deleteCount", n), words("deleteTitle", n));
    BulkBar::new(&label).info(&info).show(
        ui,
        |ui| {
            if all_stale > 0
                && stale < all_stale
                && ui
                    .add(
                        Button::new(t("mods", "selectAllUpdates"))
                            .size(Size::Xs)
                            .variant(Variant::Ghost)
                            .tip(t("mods", "selectAllUpdatesTitle")),
                    )
                    .clicked()
            {
                run.borrow_mut().push(("mods.tick_stale", Value::Null));
            }
        },
        |ui| {
            // Right to left: the last first.
            if ui
                .add(
                    IconButton::new(lucide::X, t("mods", "clearSelectionTitle"))
                        .size(Size::IconXs)
                        .variant(Variant::Ghost),
                )
                .clicked()
            {
                run.borrow_mut().push(("mods.clear_ticks", Value::Null));
            }
            let b =
                |ui: &mut Ui, label: &str, tip: &str, icon: Icon, v: Variant, id: &'static str| {
                    if ui
                        .add(
                            Button::new(label)
                                .icon(icon)
                                .icon_size(12.0)
                                .size(Size::Xs)
                                .variant(v)
                                .tip(tip),
                        )
                        .clicked()
                    {
                        run.borrow_mut().push((id, Value::Null));
                    }
                };
            b(
                ui,
                &dl,
                &dlt,
                lucide::TRASH_2,
                Variant::Danger,
                "mods.bulk_delete",
            );
            if linked > 0 {
                b(
                    ui,
                    &ul,
                    &ult,
                    lucide::UNLINK,
                    Variant::Default,
                    "mods.bulk_unlink",
                );
            }
            if linked < n {
                b(
                    ui,
                    &lk,
                    &lkt,
                    lucide::LINK,
                    Variant::Default,
                    "mods.bulk_link",
                );
            }
            b(
                ui,
                &rv,
                &rvt,
                lucide::ROTATE_CW,
                Variant::Default,
                "mods.bulk_redownload",
            );
            if stale > 0 {
                b(
                    ui,
                    &us,
                    &ust,
                    lucide::REFRESH_CW,
                    Variant::Accent,
                    "mods.bulk_update",
                );
            }
        },
    );
    out.extend(run.into_inner());
}
