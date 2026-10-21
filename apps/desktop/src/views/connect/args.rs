//! The extra launch arguments (= `LaunchArgs.svelte`): the server's mods found by the
//! query (listed first), installed mods the query did not report, flags typed by hand.
//! Each can be switched off, moved and removed; the exact command line is previewed.

use egui::text::{LayoutJob, TextFormat};
use egui::{CornerRadius, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{
    Button, Checkbox, Disclosure, Input, InputKind, Select, SelectOption, Tag, TipExt, Tone,
};
use crate::store::direct::{ArgKind, Resolved};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What a row asked for.
enum Act {
    Toggle(u64),
    Move(u64, i32),
    Remove(u64),
    AddMod(u64),
    AddCustom,
}

/// The folded section under the form: "Extra Mods · n active".
pub fn section(ui: &mut Ui, cx: &mut Ctx, r: &Resolved) {
    let tk = ui.tokens();
    let active = cx.stores.direct.args.iter().filter(|a| a.enabled).count();
    let label = format!(
        "{} · {}",
        t("connect", "extraMods"),
        tf("connect", "argsCount", &[("count", &active)])
    );
    let did = Id::new("connect-args");
    let open = cx.stores.direct.args_open;
    ui.data_mut(|m| m.insert_persisted(did, open));
    let top = ui.cursor().min;
    let bg = ui.painter().add(egui::Shape::Noop);
    let mut acts = Vec::new();
    let max_h = (ui.ctx().content_rect().height() * 0.45).max(120.0);
    let inner = ui.scope(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        Disclosure::new("connect-args", &label).show(ui, |ui| {
            let line = ui.cursor().min.y;
            ui.painter().hline(
                ui.max_rect().x_range(),
                line + 0.5,
                Stroke::new(1.0, tk.border),
            );
            egui::ScrollArea::vertical()
                .id_salt("connect-args-scroll")
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, |ui| body(ui, cx, r, &mut acts));
        });
    });
    cx.stores.direct.args_open = ui.data_mut(|m| m.get_persisted(did)).unwrap_or(open);
    let rect = Rect::from_min_max(
        top,
        Pos2::new(ui.max_rect().right(), inner.response.rect.bottom()),
    );
    ui.painter().set(
        bg,
        egui::Shape::rect_filled(rect, CornerRadius::ZERO, tk.panel),
    );
    for a in acts {
        let d = &mut cx.stores.direct;
        match a {
            Act::Toggle(id) => d.toggle_arg(id),
            Act::Move(id, dir) => d.move_arg(id, dir),
            Act::Remove(id) => d.remove_arg(id),
            Act::AddMod(id) => {
                d.pick = None;
                cx.stores.direct_add_mod(id);
            }
            Act::AddCustom => {
                let v = std::mem::take(&mut d.custom);
                d.add_custom(&v);
            }
        }
    }
}

/// A 20px square button with an icon (`size-5 rounded-xs`).
fn tiny(ui: &mut Ui, icon: Icon, label: &str, enabled: bool, danger: bool, shown: f32) -> bool {
    let tk = ui.tokens();
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), sense);
    let resp = resp.tip(label);
    let hover = paint::ease(ui, resp.id.with("h"), enabled && resp.hovered());
    let a = shown.max(hover);
    if a <= 0.0 {
        return false;
    }
    let p = ui.painter();
    let plate = if danger {
        alpha(tk.err, 0.15)
    } else {
        tk.raised
    };
    p.rect_filled(
        rect,
        CornerRadius::same(size::RADIUS_XS),
        alpha(plate, hover * plate.a() as f32 / 255.0),
    );
    let ink = paint::lerp(tk.fg_faint, if danger { tk.err } else { tk.fg }, hover);
    let ink = if enabled { ink } else { alpha(ink, 0.3) };
    icon.paint(
        p,
        Rect::from_center_size(rect.center(), Vec2::splat(12.0)),
        alpha(ink, a * ink.a() as f32 / 255.0),
    );
    enabled && resp.clicked()
}

fn small_text(ui: &mut Ui, text: &str, color: egui::Color32, px: f32, pad: egui::Margin) {
    egui::Frame::new().inner_margin(pad).show(ui, |ui| {
        let w = ui.available_width();
        let g = paint::galley_wrap(ui, text, font(Face::Sans, px), color, w);
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
        ui.painter().galley(r.min, g, color);
    });
}

fn body(ui: &mut Ui, cx: &mut Ctx, r: &Resolved, acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    small_text(
        ui,
        t("connect", "extraModsHint"),
        tk.fg_faint,
        size::TEXT_3XS,
        egui::Margin {
            left: 12,
            right: 12,
            top: 8,
            bottom: 0,
        },
    );
    let d = &mut cx.stores.direct;
    if d.args.is_empty() {
        small_text(
            ui,
            t("connect", "noModes"),
            tk.fg_muted,
            size::TEXT_2XS,
            egui::Margin::symmetric(12, 8),
        );
    } else {
        ui.add_space(6.0);
        let n = d.args.len();
        let list_h = (n as f32 * size::ROW).min(256.0);
        let top = ui.cursor().min.y;
        let x = ui.max_rect().x_range();
        ui.painter()
            .hline(x, top + 0.5, Stroke::new(1.0, alpha(tk.border, 0.6)));
        egui::ScrollArea::vertical()
            .id_salt("connect-args-list")
            .max_height(list_h)
            .auto_shrink([false, true])
            .show_rows(ui, size::ROW, n, |ui, range| {
                for i in range {
                    let Some(a) = d.args.get_mut(i) else { continue };
                    let w = ui.available_width();
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(w, size::ROW), Sense::hover());
                    let fade = if a.enabled { 1.0 } else { 0.5 };
                    let shown = paint::ease(
                        ui,
                        Id::new(("connect-arg-row", a.id)),
                        ui.rect_contains_pointer(rect),
                    );
                    if i + 1 < n {
                        ui.painter().hline(
                            rect.x_range(),
                            rect.bottom() - 0.5,
                            Stroke::new(1.0, alpha(tk.border, 0.4)),
                        );
                    }
                    let mut row = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect.shrink2(Vec2::new(12.0, 0.0)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    row.multiply_opacity(fade);
                    row.spacing_mut().item_spacing.x = 8.0;
                    let mut on = a.enabled;
                    let tip = if a.enabled {
                        t("connect", "disable")
                    } else {
                        t("connect", "enable")
                    };
                    if row.add(Checkbox::new(&mut on).label(tip)).changed() {
                        acts.push(Act::Toggle(a.id));
                    }
                    let tag = if a.from_server {
                        (t("connect", "detected"), Tone::Accent)
                    } else if a.kind == ArgKind::Mod {
                        (t("connect", "mod"), Tone::Neutral)
                    } else {
                        (t("connect", "custom"), Tone::Neutral)
                    };
                    // Tag and the three buttons on the right; the words take the rest.
                    let tag_w = paint::galley(&row, tag.0, font(Face::Mono, size::TEXT_3XS), tk.fg)
                        .size()
                        .x
                        + 12.0;
                    let words_w = (row.available_width() - tag_w - 8.0 - 60.0 - 8.0).max(20.0);
                    let (wr, _) = row.allocate_exact_size(Vec2::new(words_w, 30.0), Sense::hover());
                    let shown_v = if a.kind == ArgKind::Mod {
                        format!("-mod=@{}", a.value)
                    } else {
                        a.value.clone()
                    };
                    let g1 = paint::galley_trunc(
                        &row,
                        &a.label,
                        font(Face::Sans, size::TEXT_2XS),
                        tk.fg,
                        words_w,
                    );
                    let g2 = paint::galley_trunc(
                        &row,
                        &shown_v,
                        font(Face::Mono, size::TEXT_3XS),
                        tk.fg_faint,
                        words_w,
                    );
                    let p = row.painter();
                    p.galley(Pos2::new(wr.left(), wr.top() + 1.0), g1, tk.fg);
                    p.galley(Pos2::new(wr.left(), wr.top() + 16.0), g2, tk.fg_faint);
                    row.add(Tag::new(tag.0).tone(tag.1));
                    row.spacing_mut().item_spacing.x = 0.0;
                    let _ = resp;
                    let id = a.id;
                    if tiny(
                        &mut row,
                        lucide::CHEVRON_UP,
                        t("connect", "moveUp"),
                        i > 0,
                        false,
                        shown,
                    ) {
                        acts.push(Act::Move(id, -1));
                    }
                    if tiny(
                        &mut row,
                        lucide::CHEVRON_DOWN,
                        t("connect", "moveDown"),
                        i + 1 < n,
                        false,
                        shown,
                    ) {
                        acts.push(Act::Move(id, 1));
                    }
                    if tiny(
                        &mut row,
                        lucide::X,
                        t("connect", "remove"),
                        true,
                        true,
                        shown,
                    ) {
                        acts.push(Act::Remove(id));
                    }
                }
            });
        let bottom = ui.cursor().min.y;
        ui.painter()
            .hline(x, bottom - 0.5, Stroke::new(1.0, alpha(tk.border, 0.6)));
    }

    // Add an installed mod, or a flag.
    let available = d.available_mods(&cx.stores.mods);
    let installed = !cx.stores.mods.installed.is_empty();
    let d = &mut cx.stores.direct;
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            if !installed {
                small_text(
                    ui,
                    t("connect", "noInstalledMods"),
                    tk.fg_faint,
                    size::TEXT_3XS,
                    egui::Margin::ZERO,
                );
            } else if available.is_empty() {
                small_text(
                    ui,
                    t("connect", "allModsAdded"),
                    tk.fg_faint,
                    size::TEXT_3XS,
                    egui::Margin::ZERO,
                );
            } else {
                let opts: Vec<SelectOption<Option<u64>>> = available
                    .iter()
                    .map(|(id, name)| SelectOption::new(Some(*id), name))
                    .collect();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let add_w = button_width(ui, t("connect", "addMod"));
                    ui.add(
                        Select::new("connect-pick-mod", &mut d.pick, &opts)
                            .placeholder(t("connect", "pickMod"))
                            .label(t("connect", "pickMod"))
                            .width((ui.available_width() - add_w - 6.0).max(60.0)),
                    );
                    if ui
                        .add(
                            Button::new(t("connect", "addMod"))
                                .icon(lucide::PLUS)
                                .enabled(d.pick.is_some()),
                        )
                        .clicked()
                        && let Some(id) = d.pick
                    {
                        acts.push(Act::AddMod(id));
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let add_w = button_width(ui, t("connect", "addArg"));
                let resp = ui.add(
                    Input::new(&mut d.custom)
                        .kind(InputKind::Mono)
                        .placeholder(t("connect", "customArgPlaceholder"))
                        .width((ui.available_width() - add_w - 6.0).max(60.0))
                        .id_salt("connect-custom-arg"),
                );
                let entered = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let clicked = ui
                    .add(
                        Button::new(t("connect", "addArg"))
                            .icon(lucide::PLUS)
                            .enabled(!d.custom.trim().is_empty()),
                    )
                    .clicked();
                if (entered || clicked) && !d.custom.trim().is_empty() {
                    acts.push(Act::AddCustom);
                }
            });
        });

    // The command line, as it will be.
    let top = ui.cursor().min.y;
    ui.painter().hline(
        ui.max_rect().x_range(),
        top + 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.6)),
    );
    let preview = preview_job(ui, cx, r);
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                lucide::TERMINAL.show(ui, 12.0, tk.fg_faint);
                let g = paint::galley(
                    ui,
                    &t("connect", "launchPreview").to_uppercase(),
                    font(Face::Sans, size::TEXT_3XS),
                    tk.fg_faint,
                );
                let (rr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(rr.min, g, tk.fg_faint);
            });
            egui::Frame::new()
                .fill(tk.plot)
                .stroke(Stroke::new(1.0, tk.border))
                .corner_radius(CornerRadius::same(size::RADIUS_SM))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical()
                        .id_salt("connect-preview")
                        .max_height(96.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.add(egui::Label::new(preview).selectable(true).wrap());
                        });
                });
        });
}

fn button_width(ui: &Ui, label: &str) -> f32 {
    paint::galley(
        ui,
        label,
        font(Face::SansMedium, size::TEXT_XS),
        egui::Color32::WHITE,
    )
    .size()
    .x + 2.0 * 10.0
        + size::ICON_SM
        + 6.0
}

/// `-connect=<ip> -port=<game> [-password=••••] <args in the mods colour>`, breaking anywhere.
fn preview_job(ui: &Ui, cx: &Ctx, r: &Resolved) -> LayoutJob {
    let tk = ui.tokens();
    let f = font(Face::Mono, size::TEXT_3XS);
    let base = TextFormat {
        font_id: f,
        color: tk.fg_muted,
        line_height: Some(size::TEXT_3XS * 1.625),
        ..Default::default()
    };
    let modc = TextFormat {
        color: tk.mods,
        ..base.clone()
    };
    let d = &cx.stores.direct;
    let ip = if r.ip.is_empty() {
        "…"
    } else {
        r.ip.as_str()
    };
    let port = if r.game_port == 0 {
        "…".to_string()
    } else {
        r.game_port.to_string()
    };
    let mut job = LayoutJob::default();
    job.append(&format!("-connect={ip} -port={port}"), 0.0, base.clone());
    if !d.password.is_empty() {
        job.append(" -password=••••", 0.0, base.clone());
    }
    for a in d.launch_args() {
        job.append(" ", 0.0, base.clone());
        job.append(&a, 0.0, modc.clone());
    }
    job.wrap.break_anywhere = true;
    job
}
