//! The dialog host (= `ConfirmHost`, `PasswordHost`, `ConnectDialog`, `ShortcutsDialog`):
//! whatever a flow is waiting on, drawn over the window. Each keeps a copy of what it
//! asked so it can fade out after the answer.

use egui::{Context, Key, Rect, Sense, Ui, Vec2, pos2, vec2};

use crate::app::Ctx;
use crate::commands::{KEY_GROUPS, PAD_GROUPS, PadButton as Btn, SHORTCUTS, Scope};
use crate::i18n::{format, plural, t, tf};
use crate::icons::lucide;
use crate::kit::{
    Button, Checkbox, Dialog, DialogSize, IconButton, Meter, Side, Size, Switch, Tag, TipExt, Tone,
    Variant, paint,
};
use crate::parts::{
    PadButton, SecretInput, ShortcutGroup, ShortcutList, ShortcutMode, ShortcutRow,
};
use crate::store::ConfirmRequest;
use crate::store::connect::{ConnectMod, RequestKind};
use crate::store::mods::ModOpPhase;
use crate::store::pad::InputMode;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What the dialogs keep between frames.
#[derive(Default)]
pub(crate) struct DialogState {
    confirm: Option<ConfirmRequest>,
    password_for: Option<String>,
    password: String,
    save: bool,
    connect: Option<ConnectCopy>,
    fetch: bool,
    shortcuts_mode: Option<ShortcutMode>,
    sheet_until: f64,
    pub mod_op_until: f64,
}

/// The mods dialog's question, kept for its fade-out.
#[derive(Clone)]
struct ConnectCopy {
    server: String,
    kind: RequestKind,
    mods: Vec<ConnectMod>,
}

/// "Are you sure" (= `ConfirmHost.svelte`).
pub(super) fn confirm(ctx: &Context, cx: &mut Ctx, st: &mut DialogState) {
    let pending = cx.stores.dialogs.pending().cloned();
    let mut open = pending.is_some();
    if let Some(p) = pending {
        st.confirm = Some(p);
    }
    let Some(req) = st.confirm.clone() else {
        return;
    };
    let cancel = req
        .cancel_label
        .clone()
        .unwrap_or_else(|| t("shell", "confirmCancel").to_string());
    let ok = req
        .confirm_label
        .clone()
        .unwrap_or_else(|| t("shell", "confirmConfirm").to_string());
    let mut answer: Option<bool> = None;
    let closed = Dialog::new("dzch.confirm", &req.title)
        .close_label(&cancel)
        .show(
            ctx,
            &mut open,
            |ui, _| {
                let tk = ui.tokens();
                let w = ui.available_width();
                let g = paint::galley_wrap(
                    ui,
                    &req.message,
                    font(Face::Sans, size::TEXT_XS),
                    tk.fg_muted,
                    w,
                );
                let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), Sense::hover());
                ui.painter().galley(r.min, g, tk.fg_muted);
                if !req.details.is_empty() {
                    confirm_details(ui, &req.details);
                }
            },
            |ui, _| {
                if ui
                    .add(Button::new(&cancel).variant(Variant::Ghost))
                    .clicked()
                {
                    answer = Some(false);
                }
                let v = if req.danger {
                    Variant::Danger
                } else {
                    Variant::Accent
                };
                if ui.add(Button::new(&ok).variant(v)).clicked() {
                    answer = Some(true);
                }
            },
        );
    if pending_is(cx) {
        match answer {
            Some(a) => cx.stores.dialogs.answer(a),
            None if closed || !open => cx.stores.dialogs.answer(false),
            None => {}
        }
    }
}

/// The lists under a question's message: each heading, then its lines, in a well that
/// scrolls past a few lines.
fn confirm_details(ui: &mut Ui, details: &[(String, Vec<String>)]) {
    let tk = ui.tokens();
    ui.add_space(8.0);
    let w = ui.available_width();
    egui::Frame::new()
        .fill(tk.bg)
        .stroke(egui::Stroke::new(1.0, tk.border))
        .corner_radius(egui::CornerRadius::same(size::RADIUS_SM))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(w - 20.0);
            egui::ScrollArea::vertical()
                .id_salt("dzch.confirm.details")
                .max_height(220.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
                    for (i, (heading, lines)) in details.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(8.0);
                        }
                        let inner = ui.available_width();
                        let g = paint::galley_wrap(
                            ui,
                            heading,
                            font(Face::SansMedium, size::TEXT_2XS),
                            tk.fg,
                            inner,
                        );
                        let (r, _) =
                            ui.allocate_exact_size(vec2(inner, g.size().y), Sense::hover());
                        ui.painter().galley(r.min, g, tk.fg);
                        for line in lines {
                            let g = paint::galley_wrap(
                                ui,
                                line,
                                font(Face::Sans, size::TEXT_2XS),
                                tk.fg_muted,
                                inner - 12.0,
                            );
                            let (r, _) =
                                ui.allocate_exact_size(vec2(inner, g.size().y), Sense::hover());
                            ui.painter().circle_filled(
                                pos2(r.left() + 3.0, r.top() + g.size().y.min(16.0) / 2.0),
                                1.5,
                                tk.fg_faint,
                            );
                            ui.painter()
                                .galley(pos2(r.left() + 12.0, r.top()), g, tk.fg_muted);
                        }
                    }
                });
        });
}

fn pending_is(cx: &Ctx) -> bool {
    cx.stores.dialogs.pending().is_some()
}

/// A server's password (= `PasswordHost.svelte`).
pub(super) fn password(ctx: &Context, cx: &mut Ctx, st: &mut DialogState) {
    let asked = cx.stores.dialogs.password_for().map(str::to_string);
    let mut open = asked.is_some();
    if let Some(name) = asked {
        if st.password_for.as_deref() != Some(name.as_str()) || !st_open(ctx) {
            // A fresh question starts empty.
            st.password.clear();
            st.save = true;
        }
        st.password_for = Some(name);
    }
    let Some(server) = st.password_for.clone() else {
        return;
    };
    let cancel = t("shell", "confirmCancel");
    let submit = std::cell::Cell::new(false);
    let mut give_up = false;
    let can = !st.password.is_empty();
    let value = &mut st.password;
    let save = &mut st.save;
    let closed = Dialog::new("dzch.password", t("connect", "passwordAskTitle"))
        .close_label(cancel)
        .show(
            ctx,
            &mut open,
            |ui, _| {
                let tk = ui.tokens();
                let w = ui.available_width();
                let msg = tf("connect", "passwordAskMessage", &[("name", &server)]);
                let g =
                    paint::galley_wrap(ui, &msg, font(Face::Sans, size::TEXT_XS), tk.fg_muted, w);
                let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), Sense::hover());
                ui.painter().galley(r.min, g, tk.fg_muted);
                ui.add_space(6.0);
                let field = ui.add(
                    SecretInput::new(value)
                        .placeholder(t("connect", "password"))
                        .id_salt("dzch.password.field"),
                );
                if !ui.ctx().memory(|m| m.focused().is_some()) {
                    field.request_focus();
                }
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    submit.set(true);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add(Checkbox::new(save).label(t("connect", "passwordAskSave")));
                    let g = paint::galley(
                        ui,
                        t("connect", "passwordAskSave"),
                        font(Face::Sans, size::TEXT_XS),
                        tk.fg_muted,
                    );
                    // The checkbox is the Tab stop; its words only take the click.
                    let (r, resp) = ui.allocate_exact_size(g.size(), Sense::CLICK);
                    ui.painter().galley(r.min, g, tk.fg_muted);
                    if resp.clicked() {
                        *save = !*save;
                    }
                });
            },
            |ui, _| {
                if ui
                    .add(Button::new(cancel).variant(Variant::Ghost))
                    .clicked()
                {
                    give_up = true;
                }
                if ui
                    .add(
                        Button::new(t("connect", "passwordAskJoin"))
                            .variant(Variant::Accent)
                            .enabled(can),
                    )
                    .clicked()
                {
                    submit.set(true);
                }
            },
        );
    if cx.stores.dialogs.password_for().is_none() {
        return;
    }
    if submit.get() && !st.password.is_empty() {
        let answer = crate::store::PasswordAnswer {
            password: std::mem::take(&mut st.password),
            save: st.save,
        };
        cx.stores.dialogs.answer_password(Some(answer));
    } else if give_up || closed || !open {
        st.password.clear();
        cx.stores.dialogs.answer_password(None);
    }
}

fn st_open(ctx: &Context) -> bool {
    ctx.memory(|m| m.top_modal_layer().is_some())
}

/// Which mods a server runs that are missing or maybe behind, and whether to fetch them
/// first (= `ConnectDialog.svelte`).
pub(super) fn connect(ctx: &Context, cx: &mut Ctx, st: &mut DialogState) {
    let mut open = cx.stores.connect.request.is_some();
    if let Some(req) = &cx.stores.connect.request {
        let fresh = st
            .connect
            .as_ref()
            .is_none_or(|c| c.server != req.server_name || c.mods.len() != req.mods.len());
        if fresh || !st_open(ctx) && st.connect.is_none() {
            st.fetch = req.kind == RequestKind::Missing;
        }
        if fresh {
            st.connect = Some(ConnectCopy {
                server: req.server_name.clone(),
                kind: req.kind,
                mods: req.mods.clone(),
            });
        }
    }
    let Some(req) = st.connect.clone() else {
        return;
    };
    let via_steam = cx.stores.profile.via_steam();
    let count = i64::try_from(req.mods.len()).unwrap_or(i64::MAX);
    let missing = req.kind == RequestKind::Missing;
    let title_key = if missing {
        plural(
            count,
            "connectModalModsNotInstalled",
            "connectModalModsNotInstalledPlural",
        )
    } else {
        plural(
            count,
            "connectModalModsToCheck",
            "connectModalModsToCheckPlural",
        )
    };
    let tag = tf("connect", title_key, &[("count", &count)]);
    let cancel = t("connect", "connectModalCancel");
    let fetch_now = st.fetch;
    let fetch = &mut st.fetch;
    let mut go = false;
    let mut cancelled = false;
    let closed = Dialog::new("dzch.connect", t("connect", "connectModalTitle"))
        .description(&req.server)
        .size(DialogSize::Md)
        .close_label(cancel)
        .show(
            ctx,
            &mut open,
            |ui, _| {
                let tk = ui.tokens();
                ui.add(Tag::new(&tag).tone(if missing { Tone::Warn } else { Tone::Neutral }));
                ui.add_space(2.0);
                mod_list(ui, &req.mods);
                ui.add_space(6.0);
                let hint = if *fetch {
                    if via_steam {
                        t("mods", "confirmWarningSteam")
                    } else {
                        t("mods", "confirmWarning")
                    }
                } else if missing {
                    t("connect", "connectModalWarning")
                } else {
                    ""
                };
                let label = if missing {
                    t("connect", "connectModalInstallMods")
                } else {
                    t("connect", "connectModalUpdateMods")
                };
                egui::Frame::new()
                    .fill(tk.bg)
                    .stroke(egui::Stroke::new(1.0, tk.border))
                    .corner_radius(egui::CornerRadius::same(size::RADIUS_SM))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(Switch::new(fetch, label).hint(hint));
                    });
                if !*fetch && missing {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        lucide::TRIANGLE_ALERT.show(ui, 14.0, tk.warn);
                        let w = ui.available_width();
                        let g = paint::galley_wrap(
                            ui,
                            t("connect", "connectModalWarning"),
                            font(Face::Sans, size::TEXT_2XS),
                            tk.warn,
                            w,
                        );
                        let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                        ui.painter().galley(r.min, g, tk.warn);
                    });
                }
            },
            |ui, _| {
                if ui
                    .add(Button::new(cancel).variant(Variant::Ghost))
                    .clicked()
                {
                    cancelled = true;
                }
                let label = if fetch_now {
                    if missing {
                        t("connect", "connectModalInstallConnect")
                    } else {
                        t("connect", "connectModalUpdateConnect")
                    }
                } else {
                    t("connect", "connectModalConnect")
                };
                if ui
                    .add(
                        Button::new(label)
                            .icon(lucide::PLAY)
                            .variant(Variant::Play)
                            .size(Size::Lg),
                    )
                    .clicked()
                {
                    go = true;
                }
            },
        );
    if cx.stores.connect.request.is_none() {
        if !open && closed {
            st.connect = None;
        }
        return;
    }
    if go {
        let f = st.fetch;
        cx.answer_connect(f);
    } else if cancelled || closed || !open {
        cx.stores.connect.request = None;
    }
}

/// The server's mods: name, id, dates, size, how much newer, a link to the Workshop.
fn mod_list(ui: &mut Ui, mods: &[ConnectMod]) {
    let tk = ui.tokens();
    let w = ui.available_width();
    egui::Frame::new()
        .fill(tk.bg)
        .stroke(egui::Stroke::new(1.0, tk.border))
        .corner_radius(egui::CornerRadius::same(size::RADIUS_SM))
        .show(ui, |ui| {
            ui.set_width(w - 2.0);
            // `show_rows` counts the item spacing between rows: none, or the box ends in
            // a blank band as tall as the gaps the rows never take.
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            egui::ScrollArea::vertical()
                .id_salt("dzch.connect.mods")
                .max_height(288.0)
                .auto_shrink([false, true])
                .show_rows(ui, 38.0, mods.len(), |ui, range| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for i in range {
                        let Some(m) = mods.get(i) else { continue };
                        mod_row(ui, m, i + 1 < mods.len());
                    }
                });
        });
}

fn mod_row(ui: &mut Ui, m: &ConnectMod, divider: bool) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 38.0), Sense::hover());
    let p = ui.painter();
    if divider {
        p.hline(
            r.x_range(),
            r.bottom() - 0.5,
            egui::Stroke::new(1.0, paint::alpha(tk.border, 0.6)),
        );
    }
    let left = r.left() + 8.0;
    let mut right = r.right() - 8.0;
    // The Workshop link.
    let br = Rect::from_min_size(
        pos2(right - size::CONTROL_SM, r.center().y - 11.0),
        Vec2::splat(size::CONTROL_SM),
    );
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(br));
    let label = t("mods", "confirmOpenWorkshop");
    if bui
        .add(
            IconButton::new(lucide::EXTERNAL_LINK, label)
                .size(Size::IconXs)
                .variant(Variant::Ghost),
        )
        .tip_kbd(label, "", Side::Top)
        .clicked()
    {
        let url = format!(
            "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
            m.id
        );
        let _ = crate::platform::deck::open(url);
    }
    right = br.left() - 8.0;
    let days = match (m.local_updated, m.remote_updated) {
        (Some(l), Some(r)) if l > 0 && r > l => (r - l) / 86_400,
        _ => 0,
    };
    if days > 0 {
        let text = tf("mods", "confirmNewer", &[("days", &days)]);
        let tw = paint::galley(ui, &text, font(Face::Sans, size::TEXT_2XS), tk.warn)
            .size()
            .x
            + 14.0;
        let tr = Rect::from_min_size(pos2(right - tw, r.center().y - 9.0), vec2(tw, 18.0));
        let mut tui = ui.new_child(egui::UiBuilder::new().max_rect(tr));
        tui.add(Tag::new(&text).tone(Tone::Warn));
        right = tr.left() - 8.0;
    }
    let ng = paint::galley_trunc(
        ui,
        &m.name,
        font(Face::SansMedium, size::TEXT_XS),
        tk.fg,
        (right - left).max(0.0),
    );
    paint::text_at(ui.painter(), &ng, left, r.top() + 6.0 + 8.0, tk.fg);
    let mut facts = format!("#{}", m.id);
    if let Some(l) = m.local_updated.filter(|v| *v > 0) {
        facts.push_str(&format!(
            "   {} {}",
            t("mods", "confirmLocalDate"),
            format::date(l)
        ));
    }
    if let Some(rm) = m.remote_updated.filter(|v| *v > 0) {
        facts.push_str(&format!(
            "   {} {}",
            t("mods", "confirmRemoteDate"),
            format::date(rm)
        ));
    }
    if let Some(s) = m.size_human.as_deref().filter(|s| !s.is_empty()) {
        facts.push_str("   ");
        facts.push_str(s);
    }
    let fg = paint::galley_trunc(
        ui,
        &facts,
        font(Face::Sans, size::TEXT_3XS),
        tk.fg_faint,
        (right - left).max(0.0),
    );
    paint::text_at(ui.painter(), &fg, left, r.bottom() - 6.0 - 6.0, tk.fg_faint);
}

fn pad_button(b: Btn) -> PadButton {
    match b {
        Btn::A => PadButton::A,
        Btn::B => PadButton::B,
        Btn::X => PadButton::X,
        Btn::Y => PadButton::Y,
        Btn::Lb => PadButton::Lb,
        Btn::Rb => PadButton::Rb,
        Btn::Lt => PadButton::Lt,
        Btn::Rt => PadButton::Rt,
        Btn::Menu => PadButton::Menu,
        Btn::View => PadButton::View,
        Btn::L3 => PadButton::L3,
        Btn::R3 => PadButton::R3,
        Btn::Dpad => PadButton::Dpad,
        Btn::Rs => PadButton::Rs,
    }
}

pub(super) fn pad_buttons(b: &[Btn]) -> Vec<PadButton> {
    b.iter().copied().map(pad_button).collect()
}

/// A group's title (= `ShortcutList.svelte`'s `titleOf`).
fn scope_title(s: Scope) -> &'static str {
    match s {
        Scope::Servers => t("nav", "servers"),
        Scope::Connect => t("nav", "connect"),
        Scope::Offline => t("nav", "offline"),
        Scope::Options => t("nav", "options"),
        Scope::Friends => t("nav", "friends"),
        other => t("keys", other.label()),
    }
}

/// Every shortcut, keyboard or controller (= `ShortcutsDialog.svelte`).
pub(super) fn shortcuts(ctx: &Context, cx: &mut Ctx, st: &mut DialogState) {
    let mut open = cx.stores.dialogs.shortcuts;
    if !fading(ctx, open, &mut st.sheet_until) {
        st.shortcuts_mode = None;
        return;
    }
    let pad = cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) == InputMode::Gamepad;
    let kind = cx.stores.pad.kind(cx.prefs.get().pad_glyphs);
    let mode = st.shortcuts_mode.get_or_insert(if pad {
        ShortcutMode::Pad
    } else {
        ShortcutMode::Keys
    });
    Dialog::new("dzch.shortcuts", t("keys", "title"))
        .size(DialogSize::Lg)
        .close_label(t("common", "close"))
        .without_footer()
        .show(
            ctx,
            &mut open,
            |ui, _| {
                shortcut_columns(ui, mode, kind, 2);
            },
            |_, _| {},
        );
    if cx.stores.dialogs.shortcuts && !open {
        cx.stores.dialogs.shortcuts = false;
    }
}

/// Every shortcut by where it works, keyboard or controller, from the one registry
/// (the `?` sheet and the About page's card both draw it).
pub(crate) fn shortcut_list(ui: &mut Ui, mode: &mut ShortcutMode, kind: crate::parts::PadKind) {
    shortcut_columns(ui, mode, kind, 1);
}

/// [`shortcut_list`] in `columns` columns (the `?` sheet's two).
fn shortcut_columns(
    ui: &mut Ui,
    mode: &mut ShortcutMode,
    kind: crate::parts::PadKind,
    columns: usize,
) {
    // The rows, owned for this frame (only while the sheet shows).
    struct Owned {
        label: &'static str,
        keys: Vec<String>,
        pad: Vec<PadButton>,
        pair: bool,
    }
    let build = |scopes: &[Scope]| -> Vec<(Scope, Vec<Owned>)> {
        scopes
            .iter()
            .map(|s| {
                let rows = SHORTCUTS
                    .iter()
                    .filter(|x| x.scope == *s)
                    .map(|x| Owned {
                        label: x.words(),
                        keys: x.keys.iter().map(|c| c.text()).collect(),
                        pad: pad_buttons(x.pad),
                        pair: x.pair,
                    })
                    .collect();
                (*s, rows)
            })
            .collect()
    };
    let keys = build(KEY_GROUPS);
    let pads = build(PAD_GROUPS);
    let key_strs: Vec<Vec<Vec<&str>>> = keys
        .iter()
        .map(|(_, rows)| {
            rows.iter()
                .map(|r| r.keys.iter().map(String::as_str).collect())
                .collect()
        })
        .collect();
    let pad_strs: Vec<Vec<Vec<&str>>> = pads
        .iter()
        .map(|(_, rows)| {
            rows.iter()
                .map(|r| r.keys.iter().map(String::as_str).collect())
                .collect()
        })
        .collect();
    let key_rows: Vec<Vec<ShortcutRow<'_>>> = keys
        .iter()
        .zip(&key_strs)
        .map(|((_, rows), ks)| {
            rows.iter()
                .zip(ks)
                .map(|(r, k)| ShortcutRow {
                    label: r.label,
                    keys: k,
                    pad: &r.pad,
                    pair: r.pair,
                })
                .collect()
        })
        .collect();
    let pad_rows: Vec<Vec<ShortcutRow<'_>>> = pads
        .iter()
        .zip(&pad_strs)
        .map(|((_, rows), ks)| {
            rows.iter()
                .zip(ks)
                .map(|(r, k)| ShortcutRow {
                    label: r.label,
                    keys: k,
                    pad: &r.pad,
                    pair: r.pair,
                })
                .collect()
        })
        .collect();
    let key_groups: Vec<ShortcutGroup<'_>> = keys
        .iter()
        .zip(&key_rows)
        .map(|((s, _), rows)| ShortcutGroup {
            title: scope_title(*s),
            views: *s == Scope::Views,
            rows,
        })
        .collect();
    let pad_groups: Vec<ShortcutGroup<'_>> = pads
        .iter()
        .zip(&pad_rows)
        .map(|((s, _), rows)| ShortcutGroup {
            title: scope_title(*s),
            views: *s == Scope::Views,
            rows,
        })
        .collect();
    ShortcutList::new(&key_groups, &pad_groups, mode)
        .pad_kind(kind)
        .columns(columns)
        .show(ui);
}

/// Until the mods view installs its own: the operation's progress, and a way to
/// minimise, cancel or close it.
pub(super) fn mod_op_fallback(ctx: &Context, cx: &mut Ctx, until: &mut f64) {
    let op = &cx.stores.mods.op;
    let mut open = op.active && !op.minimised;
    if !fading(ctx, open, until) {
        return;
    }
    let finished = op.phase == ModOpPhase::Finished;
    let title = match op.kind {
        dz_hub::features::steamcmd::ModOpType::Login => t("progress", "kindLogin"),
        dz_hub::features::steamcmd::ModOpType::Repair => t("progress", "kindRepair"),
        dz_hub::features::steamcmd::ModOpType::InstallServer
        | dz_hub::features::steamcmd::ModOpType::InstallManual => t("progress", "kindInstall"),
        _ => t("progress", "kindUpdate"),
    };
    let name = op.current_name.clone();
    let frac = if op.total > 0 {
        (op.ok + op.failed) as f32 / op.total as f32
    } else {
        0.0
    };
    let counts = format!("{}/{}", op.current, op.total);
    let mut act: Option<&'static str> = None;
    Dialog::new("dzch.modop", title)
        .description(&name)
        .size(DialogSize::Md)
        .close_label(t("progress", "minimise"))
        .show(
            ctx,
            &mut open,
            |ui, _| {
                let tk = ui.tokens();
                let m = if finished {
                    Meter::new(1.0)
                } else if frac <= 0.0 {
                    Meter::new(0.0).indeterminate()
                } else {
                    Meter::new(frac)
                };
                ui.add(m.tone(Tone::Info));
                let g = paint::galley(ui, &counts, font(Face::Sans, size::TEXT_2XS), tk.fg_muted);
                let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(r.min, g, tk.fg_muted);
            },
            |ui, _| {
                if finished {
                    if ui
                        .add(Button::new(t("progress", "dismiss")).variant(Variant::Accent))
                        .clicked()
                    {
                        act = Some("mods.op.dismiss");
                    }
                } else {
                    if ui
                        .add(Button::new(t("progress", "minimise")).variant(Variant::Ghost))
                        .clicked()
                    {
                        act = Some("mods.op.minimise");
                    }
                    if ui
                        .add(Button::new(t("progress", "cancel")).variant(Variant::Danger))
                        .clicked()
                    {
                        act = Some("mods.op.cancel");
                    }
                }
            },
        );
    if let Some(id) = act {
        let _ = cx.run(id, &serde_json::Value::Null);
    } else if !open && cx.stores.mods.op.active && !cx.stores.mods.op.minimised {
        let id = if finished {
            "mods.op.dismiss"
        } else {
            "mods.op.minimise"
        };
        let _ = cx.run(id, &serde_json::Value::Null);
    }
}

/// Whether a dialog shows this frame: while open, and for its fade-out after.
fn fading(ctx: &Context, open: bool, until: &mut f64) -> bool {
    let now = ctx.input(|i| i.time);
    if open {
        *until = now + 0.2;
        return true;
    }
    now <= *until
}
