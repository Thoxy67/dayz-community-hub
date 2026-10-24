//! The favourite's dialogs: "Edit favourite" (its note, labels, group, character name and
//! launch arguments, saved together) and "Groups and labels" (the list's sections and the
//! labels: made, renamed, recoloured, moved, deleted at once).
//!
//! Both are drawn by the shell's overlays ([`show`]) so the list and the server panel open
//! the same ones (`favorites.edit`, `favorites.organize`). What they hold between frames
//! lives in egui memory; the profile store applies every edit optimistically.

use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{Align, Id, Layout, Rect, Sense, Stroke, Ui, Vec2};
use rustc_hash::FxHashMap;
use serde_json::Value;

use super::{label_color, label_color_name};
use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::paint;
use crate::kit::{
    Button, Dialog, DialogSize, IconButton, Input, InputKind, Select, SelectOption, Side, Size,
    TextArea, TipExt, Toggle, Variant,
};
use crate::parts::{draw, ph};
use crate::store::profile::{FavoriteEdit, FavoriteMeta, LabelColor};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// Longest name typed in a label or group field (the profile keeps as much).
const NAME_CHARS: usize = dz_profile::favorites::MAX_NAME;

/// The edit dialog's copy of one favourite.
#[derive(Default)]
struct EditForm {
    ip: String,
    port: u16,
    name: String,
    note: String,
    labels: Vec<String>,
    /// Empty for no group.
    group: String,
    character: String,
    args: String,
    new_label: String,
    new_color: LabelColor,
    new_group: String,
}

#[derive(Default)]
struct DialogsState {
    /// When each dialog last closed, for its fade-out (`None`: drawn since or never
    /// opened), and whether it was open last frame.
    edit_fade: Fade,
    organize_fade: Fade,
    edit_open: bool,
    edit: EditForm,
    organize_open: bool,
    /// Group and label names being typed over, by the name they had.
    group_names: FxHashMap<String, String>,
    label_names: FxHashMap<String, String>,
    new_group: String,
    new_label: String,
    new_color: LabelColor,
}

/// Whether a closed dialog still fades out: [`Dialog`] draws nothing once its fade is
/// over, but what this module gathers for it (the labels, the groups) is skipped too.
#[derive(Default)]
struct Fade {
    was_open: bool,
    closed_at: Option<f64>,
}

impl Fade {
    /// The dialog must be drawn this frame.
    fn needed(&mut self, open: bool, now: f64) -> bool {
        if open {
            self.was_open = true;
            self.closed_at = None;
            return true;
        }
        if std::mem::take(&mut self.was_open) {
            self.closed_at = Some(now);
        }
        // The dialog fades out in 0.10 s; a little longer to be sure it is gone.
        self.closed_at.is_some_and(|t| now - t < 0.3)
    }
}

type Shared = Arc<Mutex<DialogsState>>;

fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.favorites.dialogs"), || {
            Arc::new(Mutex::new(DialogsState::default()))
        })
        .clone()
    })
}

/// Open the edit dialog on the favourite saved as `ip:port`.
pub fn open_edit(cx: &mut Ctx, ip: &str, port: u16) {
    let Some(f) = cx.stores.profile.favorite(ip, port) else {
        cx.stores.bus.warn(t("favorites", "notFavorite"));
        return;
    };
    let form = EditForm {
        ip: f.ip.clone(),
        port: f.port,
        name: f.name.clone(),
        note: f.note.clone(),
        labels: f.labels.clone(),
        group: f.group.clone().unwrap_or_default(),
        character: f.character.clone().unwrap_or_default(),
        args: f.launch_args.clone(),
        ..EditForm::default()
    };
    let s = shared(cx.egui);
    let mut st = s.lock();
    st.edit = form;
    st.edit_open = true;
    cx.egui.request_repaint();
}

/// Open the groups and labels dialog.
pub fn open_organize(cx: &mut Ctx) {
    let s = shared(cx.egui);
    let mut st = s.lock();
    st.group_names.clear();
    st.label_names.clear();
    st.organize_open = true;
    cx.egui.request_repaint();
}

/// Both dialogs, while open (and for their fade-out).
pub fn show(ctx: &egui::Context, cx: &mut Ctx) {
    let s = shared(ctx);
    let mut run: Vec<(&'static str, Value)> = Vec::new();
    {
        let mut st = s.lock();
        let st = &mut *st;
        let now = ctx.input(|i| i.time);
        if st.edit_fade.needed(st.edit_open, now) {
            edit_dialog(ctx, cx, st);
        }
        if st.organize_fade.needed(st.organize_open, now) {
            organize_dialog(ctx, cx, st, &mut run);
        }
    }
    for (id, a) in run {
        if let Err(e) = cx.run(id, &a) {
            cx.stores.bus.err(e);
        }
    }
}

// ── pieces ──────────────────────────────────────────────────────────────────

/// A field's name over it.
fn field_label(ui: &mut Ui, text: &str) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
    let g = ph(ui, text, font(Face::SansMedium, size::TEXT_2XS));
    draw(ui.painter(), &g, r.left(), r.center().y, tk.fg);
}

/// A faint line under a field saying what it does, wrapped.
fn hint(ui: &mut Ui, text: &str) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let g = paint::galley_wrap(ui, text, font(Face::Sans, size::TEXT_2XS), tk.fg_faint, w);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, tk.fg_faint);
}

/// Wrapped text in `color` at the panel's small size (the server panel's notes).
pub(super) fn hint_line(ui: &mut Ui, text: &str, color: egui::Color32) {
    let w = ui.available_width();
    let g = paint::galley_wrap(ui, text, font(Face::Sans, size::TEXT_XS), color, w);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, color);
}

/// A label as a tinted pill, as the list rows draw it.
pub(super) fn label_pill(ui: &mut Ui, text: &str, color: egui::Color32) {
    let g = ph(ui, text, font(Face::SansMedium, size::TEXT_2XS));
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x + 12.0, 18.0), Sense::hover());
    ui.painter().rect_filled(
        r,
        egui::CornerRadius::same(size::RADIUS_SM),
        paint::alpha(color, 0.16),
    );
    draw(ui.painter(), &g, r.left() + 6.0, r.center().y, color);
}

/// A section's heading inside a dialog, with a hairline under.
fn heading(ui: &mut Ui, text: &str) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::hover());
    let g = ph(ui, text, font(Face::SansSemibold, size::TEXT_XS));
    draw(ui.painter(), &g, r.left(), r.center().y - 1.0, tk.fg);
    ui.painter().hline(
        r.x_range(),
        r.bottom() - 0.5,
        Stroke::new(1.0, paint::alpha(tk.border, 0.6)),
    );
}

const SWATCH: f32 = 14.0;
const SWATCH_GAP: f32 = 4.0;

/// The width [`swatches`] takes.
fn swatches_width() -> f32 {
    LabelColor::ALL.len() as f32 * (SWATCH + SWATCH_GAP) - SWATCH_GAP
}

/// The label colours as small squares, the chosen one ringed. Returns whether it changed.
fn swatches(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    color: &mut LabelColor,
) -> bool {
    let tk = ui.tokens();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(swatches_width(), size::CONTROL), Sense::hover());
    let base = ui.id().with(("label-swatches", id));
    let mut changed = false;
    for (i, c) in LabelColor::ALL.into_iter().enumerate() {
        let x = rect.left() + i as f32 * (SWATCH + SWATCH_GAP);
        let r = Rect::from_min_size(
            egui::pos2(x, rect.center().y - SWATCH / 2.0),
            Vec2::splat(SWATCH),
        );
        let resp = ui.interact(r, base.with(i), Sense::click());
        if resp.clicked() && *color != c {
            *color = c;
            changed = true;
        }
        let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
        let p = ui.painter();
        p.rect_filled(
            r.shrink(1.0 - hover),
            egui::CornerRadius::same(size::RADIUS_XS),
            label_color(&tk, c),
        );
        if *color == c {
            p.rect_stroke(
                r.expand(1.5),
                egui::CornerRadius::same(size::RADIUS_SM),
                Stroke::new(1.5, tk.fg),
                egui::StrokeKind::Outside,
            );
        }
        let _ = resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(label_color_name(c), "", Side::Top);
    }
    changed
}

// ── edit ────────────────────────────────────────────────────────────────────

fn edit_dialog(ctx: &egui::Context, cx: &mut Ctx, st: &mut DialogsState) {
    let f = &mut st.edit;
    // The favourite went away meanwhile (removed elsewhere): nothing to edit.
    if st.edit_open && cx.stores.profile.favorite(&f.ip, f.port).is_none() {
        st.edit_open = false;
    }
    let player = cx
        .stores
        .profile
        .data
        .as_ref()
        .and_then(|p| p.player.clone())
        .filter(|p| !p.trim().is_empty());
    let labels: Vec<(String, LabelColor)> = cx
        .stores
        .profile
        .labels
        .iter()
        .map(|l| (l.name.clone(), l.color))
        .collect();
    let groups: Vec<String> = cx
        .stores
        .profile
        .groups
        .iter()
        .map(|g| g.name.clone())
        .collect();
    let mut edits: Vec<FavoriteEdit> = Vec::new();
    let mut save = false;
    Dialog::new("dzch.favorites.edit", t("favorites", "editTitle"))
        .description(&f.name)
        .size(DialogSize::Md)
        .close_label(t("common", "close"))
        .show(
            ctx,
            &mut st.edit_open,
            |ui, _| {
                // Rows never push the plate wider than the dialog.
                ui.set_max_width(ui.available_width());
                let tk = ui.tokens();
                // Note.
                field_label(ui, t("favorites", "note"));
                ui.add(
                    TextArea::new(&mut f.note)
                        .rows(3)
                        .placeholder(t("favorites", "notePlaceholder")),
                );
                ui.add_space(4.0);
                // Labels.
                field_label(ui, t("favorites", "labels"));
                if labels.is_empty() {
                    hint(ui, t("favorites", "noLabels"));
                } else {
                    let w = ui.available_width();
                    let col = ((w - 12.0) / 3.0).floor().max(80.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                        for (name, color) in &labels {
                            let mut on = f.labels.contains(name);
                            let r = ui
                                .allocate_ui_with_layout(
                                    Vec2::new(col, size::control_sm()),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.add(
                                            Toggle::new(&mut on, name)
                                                .compact()
                                                .swatch(label_color(&tk, *color)),
                                        )
                                    },
                                )
                                .inner;
                            if r.changed() {
                                if on {
                                    f.labels.push(name.clone());
                                } else {
                                    f.labels.retain(|l| l != name);
                                }
                            }
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let add = t("favorites", "addLabel");
                    let add_w = ph(ui, add, font(Face::SansMedium, size::TEXT_XS)).size().x + 44.0;
                    let input_w =
                        (ui.available_width() - swatches_width() - add_w - 12.0).max(80.0);
                    let r = ui.add(
                        Input::new(&mut f.new_label)
                            .placeholder(t("favorites", "newLabelPlaceholder"))
                            .width(input_w)
                            .char_limit(NAME_CHARS)
                            .id_salt("fav-edit-new-label"),
                    );
                    swatches(ui, "edit", &mut f.new_color);
                    let name = f.new_label.trim().to_string();
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let clicked = ui
                        .add(
                            Button::new(add)
                                .icon(lucide::PLUS)
                                .enabled(!name.is_empty()),
                        )
                        .clicked();
                    if (clicked || enter) && !name.is_empty() {
                        // A label of that name already: ticked, nothing made.
                        let name = labels
                            .iter()
                            .find(|(l, _)| *l == name)
                            .map(|(l, _)| l.clone())
                            .unwrap_or_else(|| {
                                edits.push(FavoriteEdit::SetLabel {
                                    old: None,
                                    name: name.clone(),
                                    color: f.new_color,
                                });
                                name
                            });
                        if !f.labels.contains(&name) {
                            f.labels.push(name);
                        }
                        f.new_label.clear();
                    }
                });
                ui.add_space(4.0);
                // Group.
                field_label(ui, t("favorites", "group"));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let mut opts =
                        vec![SelectOption::new(String::new(), t("favorites", "noGroup"))];
                    opts.extend(
                        groups
                            .iter()
                            .map(|g| SelectOption::new(g.clone(), g.as_str())),
                    );
                    let w = ui.available_width();
                    let sel_w = (w * 0.42).clamp(120.0, 220.0);
                    ui.add(
                        Select::new("fav-edit-group", &mut f.group, &opts)
                            .width(sel_w)
                            .label(t("favorites", "group")),
                    );
                    let add = t("favorites", "addGroup");
                    let add_w = ph(ui, add, font(Face::SansMedium, size::TEXT_XS)).size().x + 44.0;
                    let input_w = (ui.available_width() - add_w - 6.0).max(80.0);
                    let r = ui.add(
                        Input::new(&mut f.new_group)
                            .placeholder(t("favorites", "newGroupPlaceholder"))
                            .width(input_w)
                            .char_limit(NAME_CHARS)
                            .id_salt("fav-edit-new-group"),
                    );
                    let name = f.new_group.trim().to_string();
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let clicked = ui
                        .add(
                            Button::new(add)
                                .icon(lucide::PLUS)
                                .enabled(!name.is_empty()),
                        )
                        .clicked();
                    if (clicked || enter) && !name.is_empty() {
                        // A group of that name already: chosen, nothing made.
                        if !groups.contains(&name) {
                            edits.push(FavoriteEdit::AddGroup { name: name.clone() });
                        }
                        f.group = name;
                        f.new_group.clear();
                    }
                });
                ui.add_space(4.0);
                // Character name.
                field_label(ui, t("favorites", "character"));
                let fallback = player.as_deref().unwrap_or("Survivor");
                ui.add(
                    Input::new(&mut f.character)
                        .placeholder(fallback)
                        .char_limit(dz_profile::favorites::MAX_CHARACTER)
                        .id_salt("fav-edit-character"),
                );
                hint(
                    ui,
                    &tf("favorites", "characterHint", &[("player", &fallback)]),
                );
                ui.add_space(4.0);
                // Launch arguments.
                field_label(ui, t("favorites", "launchArgs"));
                ui.add(
                    Input::new(&mut f.args)
                        .kind(InputKind::Mono)
                        .placeholder("-maxMem=4096 -world=empty")
                        .char_limit(dz_profile::favorites::MAX_ARGS)
                        .id_salt("fav-edit-args"),
                );
                hint(ui, t("favorites", "launchArgsHint"));
            },
            |ui, open| {
                if ui
                    .add(Button::new(t("common", "cancel")).variant(Variant::Ghost))
                    .clicked()
                {
                    *open = false;
                }
                if ui
                    .add(
                        Button::new(t("common", "save"))
                            .icon(lucide::SAVE)
                            .variant(Variant::Accent),
                    )
                    .clicked()
                {
                    save = true;
                    *open = false;
                }
            },
        );
    let p = &mut cx.stores.profile;
    for e in edits {
        p.edit_favorites(e, None);
    }
    if save {
        let meta = FavoriteMeta {
            note: f.note.clone(),
            labels: f.labels.clone(),
            group: Some(f.group.clone()).filter(|g| !g.is_empty()),
            character: Some(f.character.clone()).filter(|c| !c.trim().is_empty()),
            launch_args: f.args.clone(),
        };
        p.edit_favorites(
            FavoriteEdit::Meta {
                ip: f.ip.clone(),
                port: f.port,
                meta,
            },
            Some(t("favorites", "saved").to_string()),
        );
    }
}

// ── groups and labels ───────────────────────────────────────────────────────

fn organize_dialog(
    ctx: &egui::Context,
    cx: &mut Ctx,
    st: &mut DialogsState,
    run: &mut Vec<(&'static str, Value)>,
) {
    let groups: Vec<String> = cx
        .stores
        .profile
        .groups
        .iter()
        .map(|g| g.name.clone())
        .collect();
    let labels: Vec<(String, LabelColor)> = cx
        .stores
        .profile
        .labels
        .iter()
        .map(|l| (l.name.clone(), l.color))
        .collect();
    let mut edits: Vec<FavoriteEdit> = Vec::new();
    let DialogsState {
        organize_open,
        group_names,
        label_names,
        new_group,
        new_label,
        new_color,
        ..
    } = st;
    Dialog::new("dzch.favorites.organize", t("favorites", "organize"))
        .description(t("favorites", "organizeDesc"))
        .size(DialogSize::Md)
        .close_label(t("common", "close"))
        .show(
            ctx,
            organize_open,
            |ui, _| {
                ui.set_max_width(ui.available_width());
                let icon_w = size::CONTROL;
                // Groups.
                heading(ui, t("favorites", "groups"));
                if groups.is_empty() {
                    hint(ui, t("favorites", "noGroups"));
                }
                let last = groups.len().saturating_sub(1);
                for (i, g) in groups.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let input_w = (ui.available_width() - 3.0 * (icon_w + 4.0)).max(80.0);
                        let buf = group_names.entry(g.clone()).or_insert_with(|| g.clone());
                        let r = ui.add(
                            Input::new(buf)
                                .width(input_w)
                                .char_limit(NAME_CHARS)
                                .id_salt(("fav-group-name", g)),
                        );
                        // Enter or a click elsewhere renames; Escape puts the name back.
                        if r.lost_focus() {
                            let name = buf.trim().to_string();
                            let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                            if !cancel && !name.is_empty() && name != *g {
                                edits.push(FavoriteEdit::RenameGroup {
                                    old: g.clone(),
                                    name,
                                });
                            }
                            group_names.remove(g);
                        }
                        let b = |icon, label| {
                            IconButton::new(icon, label)
                                .size(Size::Icon)
                                .variant(Variant::Ghost)
                        };
                        if ui
                            .add(b(lucide::ARROW_UP, t("favorites", "moveUp")).enabled(i > 0))
                            .clicked()
                        {
                            edits.push(FavoriteEdit::MoveGroup {
                                name: g.clone(),
                                delta: -1,
                            });
                        }
                        if ui
                            .add(
                                b(lucide::ARROW_DOWN, t("favorites", "moveDown")).enabled(i < last),
                            )
                            .clicked()
                        {
                            edits.push(FavoriteEdit::MoveGroup {
                                name: g.clone(),
                                delta: 1,
                            });
                        }
                        if ui
                            .add(b(lucide::TRASH_2, t("favorites", "removeGroup")).danger_hover())
                            .clicked()
                        {
                            run.push(("favorites.group_remove", serde_json::json!({ "name": g })));
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let add = t("favorites", "addGroup");
                    let add_w = ph(ui, add, font(Face::SansMedium, size::TEXT_XS)).size().x + 44.0;
                    let input_w = (ui.available_width() - add_w - 6.0).max(80.0);
                    let r = ui.add(
                        Input::new(new_group)
                            .placeholder(t("favorites", "newGroupPlaceholder"))
                            .width(input_w)
                            .char_limit(NAME_CHARS)
                            .id_salt("fav-org-new-group"),
                    );
                    let name = new_group.trim().to_string();
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui
                        .add(
                            Button::new(add)
                                .icon(lucide::PLUS)
                                .enabled(!name.is_empty()),
                        )
                        .clicked()
                        || enter)
                        && !name.is_empty()
                    {
                        edits.push(FavoriteEdit::AddGroup { name });
                        new_group.clear();
                    }
                });
                ui.add_space(8.0);
                // Labels.
                heading(ui, t("favorites", "labels"));
                if labels.is_empty() {
                    hint(ui, t("favorites", "noLabelsYet"));
                }
                for (name, color) in &labels {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let mut c = *color;
                        if swatches(ui, ("org", name), &mut c) {
                            edits.push(FavoriteEdit::SetLabel {
                                old: Some(name.clone()),
                                name: name.clone(),
                                color: c,
                            });
                        }
                        let input_w = (ui.available_width() - icon_w - 6.0).max(80.0);
                        let buf = label_names
                            .entry(name.clone())
                            .or_insert_with(|| name.clone());
                        let r = ui.add(
                            Input::new(buf)
                                .width(input_w)
                                .char_limit(NAME_CHARS)
                                .id_salt(("fav-label-name", name)),
                        );
                        if r.lost_focus() {
                            let new = buf.trim().to_string();
                            let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                            if !cancel && !new.is_empty() && new != *name {
                                edits.push(FavoriteEdit::SetLabel {
                                    old: Some(name.clone()),
                                    name: new,
                                    color: c,
                                });
                            }
                            label_names.remove(name);
                        }
                        if ui
                            .add(
                                IconButton::new(lucide::TRASH_2, t("favorites", "removeLabel"))
                                    .size(Size::Icon)
                                    .variant(Variant::Ghost)
                                    .danger_hover(),
                            )
                            .clicked()
                        {
                            run.push((
                                "favorites.label_remove",
                                serde_json::json!({ "name": name }),
                            ));
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    swatches(ui, "org-new", new_color);
                    let add = t("favorites", "addLabel");
                    let add_w = ph(ui, add, font(Face::SansMedium, size::TEXT_XS)).size().x + 44.0;
                    let input_w = (ui.available_width() - add_w - 6.0).max(80.0);
                    let r = ui.add(
                        Input::new(new_label)
                            .placeholder(t("favorites", "newLabelPlaceholder"))
                            .width(input_w)
                            .char_limit(NAME_CHARS)
                            .id_salt("fav-org-new-label"),
                    );
                    let name = new_label.trim().to_string();
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui
                        .add(
                            Button::new(add)
                                .icon(lucide::PLUS)
                                .enabled(!name.is_empty()),
                        )
                        .clicked()
                        || enter)
                        && !name.is_empty()
                    {
                        edits.push(FavoriteEdit::SetLabel {
                            old: None,
                            name,
                            color: *new_color,
                        });
                        new_label.clear();
                    }
                });
            },
            |ui, open| {
                if ui
                    .add(Button::new(t("common", "close")).variant(Variant::Accent))
                    .clicked()
                {
                    *open = false;
                }
            },
        );
    for e in edits {
        cx.stores.profile.edit_favorites(e, None);
    }
}
