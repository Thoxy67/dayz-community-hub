//! The steps that ask something (= `GameStep`, `DownloadsStep`, `SteamcmdStep`,
//! `AccountStep`, `ServicesStep`), and the few shapes they share: the status box that says
//! where things stand, the notes under a field, the paragraphs.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, Color32, CornerRadius, FontId, Galley, Id, Layout, Rect, RichText, Role, Sense, Stroke,
    Ui, UiBuilder, Vec2, WidgetInfo, pos2, vec2,
};
use serde_json::{Value, json};

use super::{Acts, Wizard};
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Field, Input, InputKind, Size, Spinner, TipExt, Variant};
use crate::parts::{PathInput, SecretInput, SectionCard, draw, ph_trunc};
use crate::theme::{TokensExt, font, fonts::Face, size};
use dz_hub::features::steamworks::ModDownloaderDto;

/// SteamCMD from the package managers (`SteamcmdStep.svelte`).
const DISTROS: [(&str, &str); 3] = [
    ("Arch / Manjaro", "yay -S steamcmd"),
    ("Debian / Ubuntu", "sudo apt install steamcmd"),
    ("Fedora / RHEL", "sudo dnf install steamcmd"),
];

const API_KEY_URL: &str = "https://steamcommunity.com/dev/apikey";

// ── text ─────────────────────────────────────────────────────────────────────

fn job(
    ui: &Ui,
    text: &str,
    font: FontId,
    line_h: Option<f32>,
    width: f32,
    tracking: f32,
    max_rows: usize,
) -> Arc<Galley> {
    let spacing = font.size * tracking;
    let mut j = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color: Color32::PLACEHOLDER,
            extra_letter_spacing: spacing,
            line_height: line_h,
            ..Default::default()
        },
    );
    j.wrap = TextWrapping {
        max_width: width.max(1.0),
        max_rows,
        break_anywhere: max_rows == 1,
        overflow_character: (max_rows == 1).then_some('…'),
    };
    paint::layout_top(ui, j)
}

/// A paragraph wrapped at `width`, lines `line_h` apart, coloured when drawn.
pub fn para(ui: &Ui, text: &str, font: FontId, line_h: f32, width: f32) -> Arc<Galley> {
    job(ui, text, font, Some(line_h), width, 0.0, usize::MAX)
}

/// `title-display`: the display face, capitals, tracked 0.02 em, `leading` 0.95.
pub fn display(ui: &Ui, text: &str, px: f32, width: f32) -> Arc<Galley> {
    job(
        ui,
        &text.to_uppercase(),
        font(Face::DisplayBold, px),
        Some(px * 0.95),
        width,
        0.02,
        usize::MAX,
    )
}

/// One italic line.
pub fn italic(ui: &Ui, text: &str, px: f32) -> Arc<Galley> {
    let mut j = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font(Face::Sans, px),
            color: Color32::PLACEHOLDER,
            italics: true,
            ..Default::default()
        },
    );
    j.wrap.max_rows = 1;
    ui.painter().layout_job(j)
}

/// A small paragraph across the width (`text-2xs`).
pub fn note_text(ui: &mut Ui, text: &str, color: Color32) {
    let width = ui.available_width();
    let g = para(
        ui,
        text,
        font(Face::Sans, size::TEXT_2XS),
        size::TEXT_2XS * 1.4,
        width,
    );
    let (r, _) = ui.allocate_exact_size(vec2(width, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, color);
}

/// Where a note's rule is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Top,
    Bottom,
}

/// A note inside a card (`flex items-start gap-2 px-3 py-2 text-2xs`, a rule at one side).
fn note(ui: &mut Ui, icon: Option<(Icon, Color32)>, text: &str, color: Color32, rule: Rule) {
    let tk = ui.tokens();
    let width = ui.available_width();
    let lead = if icon.is_some() {
        size::ICON_SM + 8.0
    } else {
        0.0
    };
    let g = para(
        ui,
        text,
        font(Face::Sans, size::TEXT_2XS),
        size::TEXT_2XS * 1.375,
        width - 24.0 - lead,
    );
    let (r, _) = ui.allocate_exact_size(vec2(width, g.size().y + 16.0), Sense::hover());
    let p = ui.painter();
    let y = if rule == Rule::Top {
        r.top() + 0.5
    } else {
        r.bottom() - 0.5
    };
    p.hline(r.x_range(), y, Stroke::new(1.0, alpha(tk.border, 0.6)));
    if let Some((i, c)) = icon {
        i.paint(
            p,
            Rect::from_min_size(
                pos2(r.left() + 12.0, r.top() + 9.0),
                Vec2::splat(size::ICON_SM),
            ),
            c,
        );
    }
    p.galley(pos2(r.left() + 12.0 + lead, r.top() + 8.0), g, color);
}

// ── boxes ────────────────────────────────────────────────────────────────────

/// A rounded box with a border (`rounded-md border px-3`, `py` given), `content_h` tall
/// inside; `f` draws into the inner rectangle.
pub fn plate_box(
    ui: &mut Ui,
    fill: Color32,
    border: Color32,
    pad_y: f32,
    content_h: f32,
    f: impl FnOnce(&mut Ui, Rect),
) {
    let width = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(width, content_h + 2.0 * pad_y + 2.0), Sense::hover());
    paint::plate(ui.painter(), r, size::RADIUS_MD, fill, border);
    f(ui, r.shrink2(vec2(13.0, pad_y + 1.0)));
}

/// How a status box looks.
#[derive(Clone, Copy)]
enum Tone {
    Busy,
    Ok,
    Warn,
}

/// What leads a status box.
enum Lead {
    Spinner,
    Icon(Icon, Color32),
}

/// One line of a status box.
struct Line {
    text: String,
    face: Face,
    px: f32,
    color: Color32,
    wrap: bool,
    icon: Option<Icon>,
    selectable: bool,
}

impl Line {
    fn new(text: impl Into<String>, face: Face, px: f32, color: Color32) -> Self {
        Self {
            text: text.into(),
            face,
            px,
            color,
            wrap: false,
            icon: None,
            selectable: false,
        }
    }

    fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }

    fn selectable(mut self) -> Self {
        self.selectable = true;
        self
    }

    fn line_h(&self) -> f32 {
        self.px * if self.px >= size::TEXT_XS { 1.45 } else { 1.4 }
    }
}

/// The width of a small ghost button with an icon.
fn xs_width(ui: &Ui, label: &str) -> f32 {
    paint::galley(
        ui,
        label,
        font(Face::SansMedium, size::TEXT_2XS),
        Color32::WHITE,
    )
    .size()
    .x + 12.0
        + 6.0
        + 12.0
        + 2.0
}

/// Where things stand (`flex items-center gap-3 rounded-md border px-3 py-2.5`): a spinner
/// or an icon, lines of text, and a button at the right.
fn status_box(
    ui: &mut Ui,
    tone: Tone,
    lead: Lead,
    lines: &[Line],
    action: Option<(&str, &'static str)>,
    acts: &mut Acts,
) {
    let tk = ui.tokens();
    let (fill, border) = match tone {
        Tone::Busy => (alpha(tk.raised, 0.4), tk.border),
        Tone::Ok => (alpha(tk.ok, 0.08), alpha(tk.ok, 0.4)),
        Tone::Warn => (alpha(tk.warn, 0.08), alpha(tk.warn, 0.4)),
    };
    let width = ui.available_width();
    let bw = action.map(|(l, _)| xs_width(ui, l));
    let text_w = (width - 26.0 - size::ICON_LG - 12.0 - bw.map_or(0.0, |b| b + 12.0)).max(20.0);
    let galleys: Vec<(Arc<Galley>, f32)> = lines
        .iter()
        .map(|l| {
            let lead = if l.icon.is_some() { 16.0 } else { 0.0 };
            let f = font(l.face, l.px);
            let rows = if l.wrap { usize::MAX } else { 1 };
            let g = job(ui, &l.text, f, Some(l.line_h()), text_w - lead, 0.0, rows);
            let gap = if l.icon.is_some() { 2.0 } else { 0.0 };
            (g, gap)
        })
        .collect();
    let text_h: f32 = galleys.iter().map(|(g, gap)| g.size().y + gap).sum();
    let content_h =
        text_h
            .max(size::ICON_LG)
            .max(if bw.is_some() { size::CONTROL_SM } else { 0.0 });
    plate_box(ui, fill, border, 10.0, content_h, |ui, inner| {
        let cy = inner.center().y;
        let lr = Rect::from_min_size(pos2(inner.left(), cy - 10.0), Vec2::splat(size::ICON_LG));
        match lead {
            Lead::Spinner => Spinner::paint(ui, lr, tk.accent),
            Lead::Icon(i, c) => i.paint(ui.painter(), lr, c),
        }
        let x = lr.right() + 12.0;
        let mut y = cy - text_h / 2.0;
        for (l, (g, gap)) in lines.iter().zip(galleys) {
            y += gap;
            let mut gx = x;
            if let Some(i) = l.icon {
                i.paint(
                    ui.painter(),
                    Rect::from_center_size(pos2(x + 6.0, y + g.size().y / 2.0), Vec2::splat(12.0)),
                    l.color,
                );
                gx += 16.0;
            }
            if l.selectable {
                let r = Rect::from_min_size(pos2(gx, y), vec2(text_w, g.size().y));
                let mut c = ui.new_child(
                    UiBuilder::new()
                        .max_rect(r)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                c.add(
                    egui::Label::new(
                        RichText::new(&l.text)
                            .font(font(l.face, l.px))
                            .color(l.color),
                    )
                    .truncate()
                    .selectable(true),
                );
            } else {
                ui.painter().galley(pos2(gx, y), g.clone(), l.color);
            }
            y += g.size().y;
        }
        if let (Some((label, cmd)), Some(bw)) = (action, bw) {
            let r = Rect::from_min_size(
                pos2(inner.right() - bw, cy - size::CONTROL_SM / 2.0),
                vec2(bw, size::CONTROL_SM),
            );
            let mut c = ui.new_child(
                UiBuilder::new()
                    .max_rect(r)
                    .layout(Layout::right_to_left(Align::Center)),
            );
            let b = Button::new(label)
                .icon(lucide::REFRESH_CW)
                .variant(Variant::Ghost)
                .size(Size::Xs);
            if c.add(b).clicked() {
                acts.push((cmd, Value::Null));
            }
        }
    });
}

// ── the game ─────────────────────────────────────────────────────────────────

/// Where DayZ is: found on its own in any Steam library on any drive, or in the library
/// the player picks. Not finding it does not block the setup.
pub fn game(ui: &mut Ui, w: &mut Wizard, acts: &mut Acts) {
    let tk = ui.tokens();
    let g = w.game.clone();
    match &g {
        Some(g) if !w.finding_game() => match &g.dayz_dir {
            Some(dir) => {
                let mut lines = vec![
                    Line::new(
                        t("setup", "gameFound"),
                        Face::SansSemibold,
                        size::TEXT_XS,
                        tk.ok,
                    ),
                    Line::new(dir.clone(), Face::Mono, size::TEXT_2XS, tk.fg_muted).selectable(),
                ];
                if g.workshop_mods > 0 {
                    let n = format::num(f64::from(g.workshop_mods));
                    lines.push(
                        Line::new(
                            tf("setup", "gameMods", &[("count", &n)]),
                            Face::Sans,
                            size::TEXT_2XS,
                            tk.mods,
                        )
                        .icon(lucide::PUZZLE),
                    );
                }
                status_box(
                    ui,
                    Tone::Ok,
                    Lead::Icon(lucide::CIRCLE_CHECK, tk.ok),
                    &lines,
                    Some((t("setup", "rescan"), "setup.rescan_game")),
                    acts,
                );
            }
            None => status_box(
                ui,
                Tone::Warn,
                Lead::Icon(lucide::TRIANGLE_ALERT, tk.warn),
                &[
                    Line::new(
                        t("setup", "gameNotFound"),
                        Face::SansSemibold,
                        size::TEXT_XS,
                        tk.warn,
                    ),
                    Line::new(
                        t("setup", "gameNotFoundHint"),
                        Face::Sans,
                        size::TEXT_2XS,
                        tk.fg_muted,
                    )
                    .wrap(),
                ],
                Some((t("setup", "rescan"), "setup.rescan_game")),
                acts,
            ),
        },
        _ => status_box(
            ui,
            Tone::Busy,
            Lead::Spinner,
            &[Line::new(
                t("setup", "gameFinding"),
                Face::Sans,
                size::TEXT_XS,
                tk.fg_muted,
            )],
            None,
            acts,
        ),
    }
    ui.add_space(12.0);
    let example = if w.windows() {
        tf("setup", "windowsPathHint", &[("path", &"D:\\SteamLibrary")])
    } else {
        tf(
            "setup",
            "linuxPathHint",
            &[("path", &"/mnt/games/SteamLibrary")],
        )
    };
    let hint = format!("{} {example}", t("setup", "gameLibraryHint"));
    let placeholder = g
        .as_ref()
        .and_then(|g| g.steamapps.clone())
        .unwrap_or_else(|| t("setup", "autoDetect").to_string());
    SectionCard::new(t("setup", "gameLibrary"))
        .icon(lucide::FOLDER_SEARCH)
        .show(
            ui,
            |_| {},
            |ui| {
                Field::new(t("setup", "steamRoot"))
                    .hint(&hint)
                    .stacked()
                    .last()
                    .show(ui, |ui| {
                        let r = PathInput::new(&mut w.steam_root)
                            .placeholder(&placeholder)
                            .id_salt("dzch.setup.root")
                            .show(ui);
                        if r.browse {
                            acts.push(("setup.pick_steam_library", Value::Null));
                        }
                    });
            },
        );
}

// ── downloads ────────────────────────────────────────────────────────────────

/// How mods will download: SteamCMD (its own tool, a Steam login once) or the Steam client
/// (nothing to set up, Steam open). What the choice needs follows it on the same page.
pub fn downloads(ui: &mut Ui, w: &mut Wizard, acts: &mut Acts) {
    let tk = ui.tokens();
    choices(ui, w, acts);
    ui.add_space(12.0);
    if !w.via_steam() {
        steamcmd(ui, w, acts);
        return;
    }
    match &w.steam {
        Some(st) if !w.checking_steam() => {
            let again = Some((t("setup", "checkAgain"), "setup.check_steam"));
            if st.library && st.steam_running {
                status_box(
                    ui,
                    Tone::Ok,
                    Lead::Icon(lucide::CIRCLE_CHECK, tk.ok),
                    &[Line::new(t("setup", "steamOk"), Face::Sans, size::TEXT_XS, tk.ok).wrap()],
                    again,
                    acts,
                );
            } else {
                let text = if st.library {
                    t("setup", "steamClosed").to_string()
                } else {
                    let e = st.error.clone().unwrap_or_default();
                    tf("setup", "steamLibFail", &[("error", &e)])
                };
                status_box(
                    ui,
                    Tone::Warn,
                    Lead::Icon(lucide::CIRCLE_X, tk.warn),
                    &[Line::new(text, Face::Sans, size::TEXT_XS, tk.warn).wrap()],
                    again,
                    acts,
                );
            }
        }
        _ => status_box(ui, Tone::Busy, Lead::Spinner, &[], None, acts),
    }
}

/// The two radio cards (`grid grid-cols-2 gap-2`).
fn choices(ui: &mut Ui, w: &Wizard, acts: &mut Acts) {
    let tk = ui.tokens();
    let width = ui.available_width();
    let gap = 8.0;
    let cw = ((width - gap) / 2.0).max(80.0);
    let all = [
        (
            ModDownloaderDto::Steamcmd,
            "steamcmd",
            "SteamCMD",
            t("setup", "dlSteamcmdDesc"),
            lucide::SQUARE_TERMINAL,
        ),
        (
            ModDownloaderDto::Steamworks,
            "steamworks",
            t("setup", "dlSteam"),
            t("setup", "dlSteamDesc"),
            lucide::GAMEPAD_2,
        ),
    ];
    let descs: Vec<_> = all
        .iter()
        .map(|c| {
            para(
                ui,
                c.3,
                font(Face::Sans, size::TEXT_2XS),
                size::TEXT_2XS * 1.375,
                cw - 26.0,
            )
        })
        .collect();
    let desc_h = descs.iter().map(|g| g.size().y).fold(0.0_f32, f32::max);
    let head = size::TEXT_SM * 1.5;
    let (row, _) = ui.allocate_exact_size(
        vec2(width, 13.0 + head + 6.0 + desc_h + 13.0),
        Sense::hover(),
    );
    for (k, ((value, id, title, _, icon), desc)) in all.iter().zip(descs).enumerate() {
        let r = Rect::from_min_size(
            pos2(row.left() + k as f32 * (cw + gap), row.top()),
            vec2(cw, row.height()),
        );
        let on = w.downloader == *value;
        let resp = ui.interact(r, Id::unique(("dzch.setup.downloader", k)), Sense::click());
        resp.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, on, *title));
        let h = paint::ease(ui, resp.id.with("hover"), resp.hovered());
        let p = ui.painter();
        if on {
            paint::plate(p, r, size::RADIUS_MD, alpha(tk.accent, 0.08), tk.accent);
        } else {
            paint::plate(
                p,
                r,
                size::RADIUS_MD,
                paint::lerp(tk.panel, alpha(tk.raised, 0.4), h),
                paint::lerp(tk.border, tk.border_strong, h),
            );
        }
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(p, r, size::RADIUS_MD, &tk);
        }
        let cy = r.top() + 13.0 + head / 2.0;
        let x = r.left() + 13.0;
        icon.paint(
            p,
            Rect::from_center_size(pos2(x + 8.0, cy), Vec2::splat(size::ICON)),
            if on { tk.accent } else { tk.fg_faint },
        );
        let dot = pos2(r.right() - 13.0 - 8.0, cy);
        p.circle_stroke(
            dot,
            7.5,
            Stroke::new(1.0, if on { tk.accent } else { tk.border_strong }),
        );
        if on {
            p.circle_filled(dot, 4.0, tk.accent);
        }
        let g = ph_trunc(
            ui,
            title,
            font(Face::SansSemibold, size::TEXT_SM),
            (dot.x - 16.0 - x - 24.0).max(8.0),
        );
        draw(p, &g, x + 24.0, cy, if on { tk.fg } else { tk.fg_muted });
        p.galley(pos2(x, r.top() + 13.0 + head + 6.0), desc, tk.fg_muted);
        if resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            && !on
        {
            acts.push(("setup.choose_downloader", json!({ "downloader": id })));
        }
    }
}

// ── SteamCMD ─────────────────────────────────────────────────────────────────

fn steamcmd(ui: &mut Ui, w: &mut Wizard, acts: &mut Acts) {
    let tk = ui.tokens();
    let windows = w.windows();
    if w.detecting() {
        status_box(
            ui,
            Tone::Busy,
            Lead::Spinner,
            &[Line::new(
                t("setup", "steamcmdDetecting"),
                Face::Sans,
                size::TEXT_XS,
                tk.fg_muted,
            )],
            None,
            acts,
        );
    } else if w.found() {
        let path = w
            .status
            .as_ref()
            .and_then(|s| s.path.clone())
            .unwrap_or_default();
        status_box(
            ui,
            Tone::Ok,
            Lead::Icon(lucide::CIRCLE_CHECK, tk.ok),
            &[
                Line::new(
                    t("setup", "steamcmdDetected"),
                    Face::SansSemibold,
                    size::TEXT_XS,
                    tk.ok,
                ),
                Line::new(path, Face::Mono, size::TEXT_2XS, tk.fg_muted).selectable(),
            ],
            None,
            acts,
        );
    } else {
        let os = if windows { "Windows" } else { "Linux" };
        status_box(
            ui,
            Tone::Warn,
            Lead::Icon(lucide::TRIANGLE_ALERT, tk.warn),
            &[
                Line::new(
                    t("setup", "steamcmdNotFound"),
                    Face::SansSemibold,
                    size::TEXT_XS,
                    tk.warn,
                ),
                Line::new(
                    tf("setup", "platform", &[("name", &os)]),
                    Face::Sans,
                    size::TEXT_2XS,
                    tk.fg_faint,
                ),
            ],
            Some((t("setup", "rescan"), "setup.rescan_steamcmd")),
            acts,
        );
    }

    if !w.found() && !w.detecting() {
        ui.add_space(12.0);
        if windows {
            install_windows(ui, w, acts);
        } else {
            install_linux(ui, w, acts);
        }
    }

    // Paths, for anyone whose install is not where it is looked for.
    ui.add_space(12.0);
    let placeholder = w
        .status
        .as_ref()
        .and_then(|s| s.path.clone())
        .unwrap_or_else(|| t("setup", "autoDetect").to_string());
    SectionCard::new(t("setup", "steamcmdPath"))
        .description(t("setup", "steamcmdOverride"))
        .icon(lucide::FOLDER_COG)
        .show(
            ui,
            |_| {},
            |ui| {
                Field::new(t("setup", "steamcmdPath"))
                    .hint(t("setup", "leaveBlank"))
                    .stacked()
                    .last()
                    .show(ui, |ui| {
                        let r = PathInput::new(&mut w.steamcmd_path)
                            .placeholder(&placeholder)
                            .id_salt("dzch.setup.steamcmd")
                            .show(ui);
                        if r.browse {
                            acts.push(("setup.pick_steamcmd", Value::Null));
                        }
                    });
            },
        );
}

/// Linux: the package managers' commands, and the wait for SteamCMD to appear.
fn install_linux(ui: &mut Ui, w: &Wizard, acts: &mut Acts) {
    let tk = ui.tokens();
    SectionCard::new(t("setup", "steamcmdLinuxHint"))
        .icon(lucide::TERMINAL)
        .show(
            ui,
            |_| {},
            |ui| {
                let width = ui.available_width();
                let copy = t("setup", "copyCommand");
                for (i, (label, cmd)) in DISTROS.iter().enumerate() {
                    let (r, _) =
                        ui.allocate_exact_size(vec2(width, 6.0 + 23.0 + 6.0), Sense::hover());
                    let cy = r.center().y;
                    let p = ui.painter();
                    if i > 0 {
                        p.hline(
                            r.x_range(),
                            r.top() + 0.5,
                            Stroke::new(1.0, alpha(tk.border, 0.5)),
                        );
                    }
                    let lg = ph_trunc(ui, label, font(Face::Sans, size::TEXT_2XS), 112.0);
                    draw(p, &lg, r.left() + 12.0, cy, tk.fg_muted);
                    let bx = r.right() - 12.0 - size::CONTROL_SM;
                    let code = Rect::from_min_max(
                        pos2(r.left() + 12.0 + 112.0 + 12.0, cy - 11.5),
                        pos2(bx - 12.0, cy + 11.5),
                    );
                    p.rect_filled(code, CornerRadius::same(size::RADIUS_SM), tk.bg);
                    let cg = ph_trunc(
                        ui,
                        cmd,
                        font(Face::Mono, size::TEXT_2XS),
                        code.width() - 16.0,
                    );
                    draw(p, &cg, code.left() + 8.0, cy, tk.fg);
                    let b = Rect::from_center_size(
                        pos2(bx + size::CONTROL_SM / 2.0, cy),
                        Vec2::splat(size::CONTROL_SM),
                    );
                    let resp = ui.interact(b, Id::unique(("dzch.setup.copy", i)), Sense::click());
                    resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, copy));
                    let h = paint::ease(ui, resp.id.with("hover"), resp.hovered());
                    let copied = w.copied.as_ref().is_some_and(|(c, _)| c == cmd);
                    let p = ui.painter();
                    p.rect_filled(b, CornerRadius::same(size::RADIUS_SM), alpha(tk.raised, h));
                    let (icon, ink) = if copied {
                        (lucide::CHECK, tk.ok)
                    } else {
                        (lucide::COPY, paint::lerp(tk.fg_faint, tk.fg, h))
                    };
                    icon.paint(
                        p,
                        Rect::from_center_size(b.center(), Vec2::splat(size::ICON_SM)),
                        ink,
                    );
                    if paint::keyboard_focus(ui, &resp) {
                        paint::focus_ring(p, b, size::RADIUS_SM, &tk);
                    }
                    let resp = resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .tip(copy);
                    if resp.clicked() {
                        acts.push(("setup.copy_install_command", json!({ "command": cmd })));
                    }
                }
                // The radar pulses (`animate-pulse`) while SteamCMD is looked for.
                let ctx = ui.ctx().clone();
                let pulse = if crate::app::reduced_motion(&ctx) {
                    1.0
                } else {
                    let now = ctx.input(|i| i.time);
                    ctx.request_repaint_after(std::time::Duration::from_millis(50));
                    0.75 + 0.25 * (now * std::f64::consts::PI).cos() as f32
                };
                note(
                    ui,
                    Some((lucide::RADAR, alpha(tk.accent, pulse))),
                    t("setup", "steamcmdLinuxWaiting"),
                    tk.fg_muted,
                    Rule::Top,
                );
            },
        );
}

/// Windows: SteamCMD fetched from Valve into the app's folder.
fn install_windows(ui: &mut Ui, w: &Wizard, acts: &mut Acts) {
    let tk = ui.tokens();
    SectionCard::new("SteamCMD")
        .icon(lucide::DOWNLOAD)
        .padded()
        .show(
            ui,
            |_| {},
            |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                note_text(ui, t("setup", "steamcmdWinHint"), tk.fg_muted);
                ui.add_space(10.0);
                let busy = w.downloading();
                let label = if busy {
                    t("setup", "steamcmdDownloading")
                } else {
                    t("setup", "steamcmdInstallAuto")
                };
                let b = Button::new(label)
                    .icon(lucide::DOWNLOAD)
                    .spinner(busy)
                    .variant(Variant::Accent)
                    .width(ui.available_width())
                    .enabled(!busy);
                if ui.add(b).clicked() {
                    acts.push(("setup.download_steamcmd", Value::Null));
                }
                ui.add_space(6.0);
                let where_ = format!(
                    "{} %APPDATA%\\dayz-community-hub\\steamcmd",
                    t("setup", "steamcmdInstallHint")
                );
                let width = ui.available_width();
                let g = para(
                    ui,
                    &where_,
                    font(Face::Sans, size::TEXT_3XS),
                    size::TEXT_3XS * 1.2,
                    width,
                );
                let (r, _) = ui.allocate_exact_size(vec2(width, g.size().y), Sense::hover());
                ui.painter().galley(r.min, g, tk.fg_faint);
                if !w.download_error.is_empty() {
                    ui.add_space(8.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&w.download_error)
                                .font(font(Face::Mono, size::TEXT_2XS))
                                .color(tk.err),
                        )
                        .wrap()
                        .selectable(true),
                    );
                }
            },
        );
}

// ── the account ──────────────────────────────────────────────────────────────

pub fn account(ui: &mut Ui, w: &mut Wizard) {
    let tk = ui.tokens();
    if w.needs_login() {
        SectionCard::new(t("setup", "steamcmdLogin"))
            .icon(lucide::KEY_ROUND)
            .show(
                ui,
                |_| {},
                |ui| {
                    Field::new(t("setup", "username")).last().show(ui, |ui| {
                        ui.add(
                            Input::new(&mut w.steam_login)
                                .kind(InputKind::Mono)
                                .placeholder(t("setup", "steamUsernamePlaceholder"))
                                .id_salt("dzch.setup.login"),
                        );
                    });
                    if w.steam_login.trim().is_empty() {
                        note(
                            ui,
                            None,
                            t("setup", "steamRequired"),
                            tk.fg_faint,
                            Rule::Top,
                        );
                    } else {
                        note(
                            ui,
                            Some((lucide::SHIELD_CHECK, tk.ok)),
                            t("setup", "loginOnce"),
                            tk.fg_muted,
                            Rule::Top,
                        );
                    }
                },
            );
    } else {
        let width = ui.available_width();
        let g = para(
            ui,
            t("setup", "loginNotNeeded"),
            font(Face::Sans, size::TEXT_2XS),
            size::TEXT_2XS * 1.4,
            width - 26.0 - size::ICON_SM - 8.0,
        );
        plate_box(
            ui,
            alpha(tk.ok, 0.08),
            alpha(tk.ok, 0.3),
            8.0,
            g.size().y,
            |ui, inner| {
                let p = ui.painter();
                lucide::SHIELD_CHECK.paint(
                    p,
                    Rect::from_min_size(inner.min + vec2(0.0, 1.0), Vec2::splat(size::ICON_SM)),
                    tk.ok,
                );
                p.galley(inner.min + vec2(size::ICON_SM + 8.0, 0.0), g, tk.fg_muted);
            },
        );
    }
    ui.add_space(12.0);
    let hint = tf("setup", "ingameHint", &[("flag", &"-name")]);
    let from_steam = w.from_steam;
    SectionCard::new(t("setup", "ingameName"))
        .icon(lucide::GAMEPAD_2)
        .show(
            ui,
            |_| {},
            |ui| {
                let mut field = Field::new(t("setup", "ingameName")).hint(&hint);
                if !from_steam {
                    field = field.last();
                }
                field.show(ui, |ui| {
                    ui.add(
                        Input::new(&mut w.player)
                            .placeholder(t("setup", "ingamePlaceholder"))
                            .id_salt("dzch.setup.name"),
                    );
                });
                if from_steam {
                    note(
                        ui,
                        Some((lucide::CIRCLE_CHECK, tk.ok)),
                        t("setup", "filledFromSteam"),
                        tk.fg_muted,
                        Rule::Top,
                    );
                }
            },
        );
}

// ── services ─────────────────────────────────────────────────────────────────

pub fn services(ui: &mut Ui, w: &mut Wizard) {
    let tk = ui.tokens();
    SectionCard::new(t("setup", "steamApi"))
        .description(t("setup", "optionalStep"))
        .icon(lucide::ID_CARD)
        .show(
            ui,
            |ui| {
                let b = Button::new(t("setup", "getApiKey"))
                    .icon(lucide::EXTERNAL_LINK)
                    .variant(Variant::Ghost)
                    .size(Size::Xs);
                if ui.add(b).clicked() {
                    let _ = crate::platform::deck::open(API_KEY_URL);
                }
            },
            |ui| {
                note(
                    ui,
                    None,
                    t("setup", "steamApiDesc"),
                    tk.fg_muted,
                    Rule::Bottom,
                );
                Field::new(t("setup", "apiKey")).show(ui, |ui| {
                    ui.add(
                        SecretInput::new(&mut w.steam_api_key)
                            .placeholder(t("setup", "apiKeyPlaceholder"))
                            .id_salt("dzch.setup.api"),
                    );
                });
                Field::new(t("setup", "steamIdLabel"))
                    .last()
                    .show(ui, |ui| {
                        ui.add(
                            Input::new(&mut w.steam_id)
                                .kind(InputKind::Mono)
                                .placeholder(t("setup", "steamIdPlaceholder"))
                                .id_salt("dzch.setup.sid"),
                        );
                    });
            },
        );
}
