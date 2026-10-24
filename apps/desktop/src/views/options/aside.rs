//! The column beside the options: what the machine suggests (the hardware card), what
//! DayZ will be told (the command line) and, on Linux, how it really starts
//! (= `LinuxLaunch.svelte`).

use std::sync::Arc;

use dz_hub::features::launch::SteamLaunchInfoDto;
use dz_hub::features::profile::LaunchOptionDto;
use dz_hub::features::system::SystemSpecsDto;
use egui::text::LayoutJob;
use egui::{
    Align, Color32, CornerRadius, Galley, Layout, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextFormat, Ui, Vec2,
};

use super::catalog::{self, Rec, SteamOptions};
use super::{Act, UiState};
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Copy, Disclosure, Spinner, Tag, TipExt, Tone, Variant, panel};
use crate::parts::{CommandPreview, SectionCard, draw};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// Whether the option `r` names is on with the recommended value.
pub fn rec_done(r: &Rec, opts: &[LaunchOptionDto]) -> bool {
    opts.iter()
        .find(|o| o.key == r.key)
        .is_some_and(|x| x.enabled && r.value.as_ref().is_none_or(|v| x.value.as_ref() == Some(v)))
}

/// `gb(mb)`: one decimal under 10 GB.
fn gb(mb: u64) -> String {
    let g = mb as f64 / 1024.0;
    if mb >= 10240 {
        format!("{g:.0}")
    } else {
        format!("{g:.1}")
    }
}

/// A wrapped paragraph in `px`, `color`.
fn para(ui: &mut Ui, text: &str, px: f32, line_h: f32, color: Color32) {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font(Face::Sans, px),
            color,
            line_height: Some(line_h),
            ..Default::default()
        },
    );
    job.wrap.max_width = ui.available_width();
    let g = paint::layout_top(ui, job);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, color);
}

/// A small mono chip (`rounded-xs px-1.5 py-px font-mono text-3xs`): text, ground, ink,
/// a tick before the text.
pub struct ChipSpec {
    pub text: String,
    pub bg: Color32,
    pub ink: Color32,
    pub tick: bool,
}

/// Chips flowed onto lines (`flex flex-wrap gap-1`).
fn chips(ui: &mut Ui, items: &[ChipSpec]) {
    const GAP: f32 = 4.0;
    let w = ui.available_width();
    let laid: Vec<(Arc<Galley>, Vec2)> = items
        .iter()
        .map(|c| {
            let g = paint::galley(
                ui,
                &c.text,
                font(Face::Mono, size::TEXT_3XS),
                Color32::PLACEHOLDER,
            );
            let iw = if c.tick { 12.0 + 4.0 } else { 0.0 };
            let sz = Vec2::new(6.0 + iw + g.size().x + 6.0, 17.0);
            (g, sz)
        })
        .collect();
    let mut pos = Vec::with_capacity(laid.len());
    let (mut x, mut y, mut line_h) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (_, sz) in &laid {
        if x > 0.0 && x + sz.x > w {
            x = 0.0;
            y += line_h + GAP;
            line_h = 0.0;
        }
        pos.push(Vec2::new(x, y));
        x += sz.x + GAP;
        line_h = line_h.max(sz.y);
    }
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, y + line_h), Sense::hover());
    let p = ui.painter();
    for ((c, (g, sz)), at) in items.iter().zip(&laid).zip(pos) {
        let cr = Rect::from_min_size(r.min + at, *sz);
        p.rect_filled(cr, CornerRadius::same(size::RADIUS_XS), c.bg);
        let mut tx = cr.left() + 6.0;
        if c.tick {
            lucide::CHECK.paint(
                p,
                Rect::from_min_size(Pos2::new(tx, cr.center().y - 6.0), Vec2::splat(12.0)),
                c.ink,
            );
            tx += 16.0;
        }
        paint::text_at(p, g, tx, cr.center().y, c.ink);
    }
}

/// "This computer": the hardware and the flags it suggests.
pub fn hardware(ui: &mut Ui, st: &UiState, opts: &[LaunchOptionDto], acts: &mut Vec<Act>) {
    let tk = ui.tokens();
    SectionCard::new(t("options", "hardware"))
        .icon(lucide::CPU)
        .show(
            ui,
            |_| {},
            |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let Some(specs) = st.specs.value.as_ref() else {
                    if st.specs.loading() {
                        loading(ui, &tk);
                    }
                    return;
                };
                figures(ui, &tk, specs);
                let recs = &st.recs;
                let applied = !recs.is_empty() && recs.iter().all(|r| rec_done(r, opts));
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);
                        let ram = (specs.total_memory_mb as f64 / 1024.0).round() as u64;
                        para(
                            ui,
                            &tf(
                                "options",
                                "recommendDesc",
                                &[("cores", &specs.physical_cores), ("ram", &ram)],
                            ),
                            size::TEXT_2XS,
                            15.0,
                            tk.fg_muted,
                        );
                        let items: Vec<ChipSpec> = recs
                            .iter()
                            .map(|r| {
                                let done = rec_done(r, opts);
                                ChipSpec {
                                    text: catalog::flag_text(r.key, r.value.as_deref()),
                                    bg: if done { alpha(tk.ok, 0.12) } else { tk.raised },
                                    ink: if done { tk.ok } else { tk.fg_muted },
                                    tick: done,
                                }
                            })
                            .collect();
                        chips(ui, &items);
                        ui.add_space(-4.0);
                        para(
                            ui,
                            t("options", "recommendedHint"),
                            size::TEXT_3XS,
                            13.75,
                            tk.fg_faint,
                        );
                        ui.add_space(-6.0);
                        let w = ui.available_width();
                        let b = if applied {
                            Button::new(t("options", "recommendApplied"))
                                .icon(lucide::CHECK)
                                .enabled(false)
                        } else {
                            Button::new(t("options", "recommendApply"))
                                .icon(lucide::WAND_SPARKLES)
                                .variant(Variant::Accent)
                        };
                        if ui.add(b.width(w)).clicked() {
                            acts.push(Act::ApplyRecommended);
                        }
                    });
            },
        );
}

fn loading(ui: &mut Ui, tk: &Tokens) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add(Spinner::new().size(size::ICON_SM).color(tk.fg_faint));
                ui.label(
                    egui::RichText::new(t("options", "recommendDetecting"))
                        .font(font(Face::Sans, size::TEXT_2XS))
                        .color(tk.fg_faint),
                );
            });
        });
}

/// Cores, threads, memory: three cells with rules between and under.
fn figures(ui: &mut Ui, tk: &Tokens, s: &SystemSpecsDto) {
    let w = ui.available_width();
    let h = 50.0;
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let p = ui.painter();
    p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, tk.border));
    let cells = [
        (t("options", "cores"), s.physical_cores.to_string()),
        (t("options", "threads"), s.logical_cores.to_string()),
        (
            t("options", "memory"),
            format!("{} GB", gb(s.total_memory_mb)),
        ),
    ];
    let cw = w / 3.0;
    for (i, (k, v)) in cells.iter().enumerate() {
        let x = r.left() + cw * i as f32;
        if i > 0 {
            p.vline(x, r.y_range(), Stroke::new(1.0, alpha(tk.border, 0.6)));
        }
        let vg = paint::galley(
            ui,
            v,
            font(Face::DisplayBold, size::TEXT_XL),
            Color32::PLACEHOLDER,
        );
        p.galley(Pos2::new(x + 12.0, r.top() + 6.0), vg, tk.fg);
        let kg = paint::galley_trunc(
            ui,
            &k.to_uppercase(),
            font(Face::Sans, size::TEXT_3XS),
            Color32::PLACEHOLDER,
            cw - 24.0,
        );
        draw(p, &kg, x + 12.0, r.bottom() - 13.0, tk.fg_faint);
    }
}

/// "Command line": what DayZ receives from this page.
pub fn command(ui: &mut Ui, flags: &[String]) {
    CommandPreview::new(
        t("options", "commandLine"),
        flags,
        t("options", "commandLineEmpty"),
        t("options", "copyCommand"),
    )
    .program("DayZ_x64")
    .hint(t("options", "commandLineHint"))
    .show(ui);
}

/// The full Linux command, as copied.
pub fn linux_command(info: &SteamLaunchInfoDto, flags: &[String]) -> String {
    info.launcher
        .iter()
        .chain(&info.applaunch)
        .chain(flags)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ")
}

/// How Steam is installed, in words.
fn steam_kind(info: &SteamLaunchInfoDto) -> &'static str {
    match info.launcher.first() {
        None => t("options", "linuxSteamMissing"),
        Some(_) if info.launcher.iter().any(|a| a == "com.valvesoftware.Steam") => {
            t("options", "linuxSteamFlatpak")
        }
        Some(first) if first.contains("snap") => t("options", "linuxSteamSnap"),
        Some(_) => t("options", "linuxSteamNative"),
    }
}

/// `label-stencil text-fg-faint`.
fn stencil(ui: &mut Ui, text: &str, tk: &Tokens) {
    let g = panel::stencil(ui, text, Color32::PLACEHOLDER, Some(ui.available_width()));
    let (r, _) = ui.allocate_exact_size(Vec2::new(g.size().x, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, tk.fg_faint);
}

/// A `<code>` block: `rounded-sm border bg-bg px-2 py-1.5 font-mono text-3xs`, text flowed.
fn code_block(ui: &mut Ui, tk: &Tokens, job: LayoutJob) {
    let w = ui.available_width();
    let mut job = job;
    job.wrap.max_width = w - 16.0;
    let g: Arc<Galley> = paint::layout_top(ui, job);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y + 12.0), Sense::hover());
    ui.painter().rect(
        r,
        CornerRadius::same(size::RADIUS_SM),
        tk.bg,
        Stroke::new(1.0, tk.border),
        StrokeKind::Inside,
    );
    ui.painter()
        .galley(Pos2::new(r.left() + 8.0, r.top() + 6.0), g, tk.fg_muted);
}

fn mono_fmt(c: Color32) -> TextFormat {
    TextFormat {
        font_id: font(Face::Mono, size::TEXT_3XS),
        color: c,
        line_height: Some(16.0),
        ..Default::default()
    }
}

/// "Launch on Linux".
pub fn linux(
    ui: &mut Ui,
    info: &SteamLaunchInfoDto,
    steam: Option<&SteamOptions>,
    flags: &[String],
    copied: bool,
    acts: &mut Vec<Act>,
) {
    let tk = ui.tokens();
    SectionCard::new(t("options", "linuxTitle"))
        .icon(lucide::SQUARE_TERMINAL)
        .show(
            ui,
            |_| {},
            |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 12.0);
                        para(
                            ui,
                            t("options", "linuxDesc"),
                            size::TEXT_2XS,
                            15.0,
                            tk.fg_muted,
                        );
                        facts(ui, &tk, info);
                        command_block(ui, &tk, info, flags, copied, acts);
                        steam_options(ui, &tk, info, steam);
                    });
            },
        );
}

/// Steam, the compatibility tool, the prefix: a two-column list.
fn facts(ui: &mut Ui, tk: &Tokens, info: &SteamLaunchInfoDto) {
    let w = ui.available_width();
    let lw = 104.0;
    let vw = (w - lw - 12.0).max(40.0);
    let row = |ui: &mut Ui, label: &str, value: &mut dyn FnMut(&mut Ui)| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);
            ui.allocate_ui_with_layout(Vec2::new(lw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(lw);
                ui.add_space(1.0);
                para(ui, label, size::TEXT_2XS, 15.0, tk.fg_faint);
            });
            ui.allocate_ui_with_layout(Vec2::new(vw, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(vw);
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 2.0);
                value(ui);
            });
        });
    };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);
        row(ui, t("options", "linuxSteam"), &mut |ui| {
            para(ui, steam_kind(info), size::TEXT_XS, 16.0, tk.fg);
            let program = info.launcher.join(" ");
            if !program.is_empty() {
                let g = paint::galley_trunc(
                    ui,
                    &program,
                    font(Face::Sans, size::TEXT_3XS),
                    Color32::PLACEHOLDER,
                    ui.available_width(),
                );
                let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(r.min, g, tk.fg_faint);
                resp.tip(&program);
            }
        });
        row(
            ui,
            t("options", "linuxProton"),
            &mut |ui| match &info.compat_tool {
                Some(tool) => {
                    // The name, then its tag beside it when both fit, else under it.
                    let (word, tone) = if info.compat_tool_default {
                        (t("options", "linuxProtonDefault"), Tone::Neutral)
                    } else {
                        (t("options", "linuxProtonChosen"), Tone::Accent)
                    };
                    let w = ui.available_width();
                    let tg = paint::galley_wrap(
                        ui,
                        tool,
                        font(Face::Sans, size::TEXT_XS),
                        Color32::PLACEHOLDER,
                        w,
                    );
                    let word_w = paint::galley(
                        ui,
                        word,
                        font(Face::Sans, size::TEXT_2XS),
                        Color32::PLACEHOLDER,
                    )
                    .size()
                    .x + 18.0;
                    let beside = tg.rows.len() == 1 && tg.size().x + 6.0 + word_w <= w;
                    let put = |ui: &mut Ui| {
                        let (r, _) = ui.allocate_exact_size(
                            Vec2::new(tg.size().x, tg.size().y.max(16.0)),
                            Sense::hover(),
                        );
                        ui.painter().galley(
                            Pos2::new(r.left(), r.center().y - tg.size().y / 2.0),
                            tg.clone(),
                            tk.fg,
                        );
                    };
                    if beside {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            put(ui);
                            ui.add(Tag::new(word).tone(tone));
                        });
                    } else {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        put(ui);
                        ui.add(Tag::new(word).tone(tone));
                    }
                }
                None => para(
                    ui,
                    t("options", "linuxProtonNone"),
                    size::TEXT_XS,
                    16.0,
                    tk.warn,
                ),
            },
        );
        row(
            ui,
            t("options", "linuxPrefix"),
            &mut |ui| match &info.prefix {
                Some(prefix) => {
                    ui.add(
                        Copy::new(prefix)
                            .labels(t("common", "copy"), t("common", "copied"))
                            .wrap()
                            .color(ui.tokens().fg_muted),
                    );
                    para(
                        ui,
                        t("options", "linuxPrefixHint"),
                        size::TEXT_3XS,
                        14.0,
                        tk.fg_faint,
                    );
                }
                None => para(
                    ui,
                    t("options", "linuxPrefixNone"),
                    size::TEXT_2XS,
                    15.0,
                    tk.fg_faint,
                ),
            },
        );
    });
}

/// "Command the launcher runs": program, Steam's arguments, this page's flags.
fn command_block(
    ui: &mut Ui,
    tk: &Tokens,
    info: &SteamLaunchInfoDto,
    flags: &[String],
    copied: bool,
    acts: &mut Vec<Act>,
) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL_SM), Sense::hover());
        let g = panel::stencil(
            ui,
            t("options", "linuxCommand"),
            Color32::PLACEHOLDER,
            Some(w - 30.0),
        );
        draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
        let br = Rect::from_min_size(
            Pos2::new(r.right() - size::CONTROL_SM, r.top()),
            Vec2::splat(size::CONTROL_SM),
        );
        let b = ui
            .interact(br, ui.scope_id().with("linux-copy"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let hv = paint::ease(ui, b.id.with("hover"), b.hovered());
        if paint::keyboard_focus(ui, &b) {
            paint::focus_ring(ui.painter(), br, size::RADIUS_SM, tk);
        }
        if hv > 0.0 {
            ui.painter().rect_filled(
                br,
                CornerRadius::same(size::RADIUS_SM),
                alpha(tk.raised, hv),
            );
        }
        let ink = if copied {
            tk.ok
        } else {
            paint::lerp(tk.fg_faint, tk.fg, hv)
        };
        let icon = if copied { lucide::CHECK } else { lucide::COPY };
        icon.paint(
            ui.painter(),
            Rect::from_center_size(br.center(), Vec2::splat(14.0)),
            ink,
        );
        if b.tip(t("options", "copyCommand")).clicked() {
            acts.push(Act::CopyLinux);
        }

        let mut job = LayoutJob::default();
        let program = info.launcher.join(" ");
        job.append(
            if program.is_empty() {
                "steam"
            } else {
                &program
            },
            0.0,
            mono_fmt(tk.fg_faint),
        );
        // A word goes to the next line whole: its spaces do not break.
        let nb = |s: &str| s.replace(' ', "\u{a0}");
        for a in &info.applaunch {
            let c = if a == "-malloc=system" {
                tk.accent
            } else {
                tk.fg
            };
            job.append(" ", 0.0, mono_fmt(tk.fg_muted));
            job.append(&nb(a), 0.0, mono_fmt(c));
        }
        for f in flags {
            job.append(" ", 0.0, mono_fmt(tk.fg_muted));
            job.append(&nb(f), 0.0, mono_fmt(tk.info));
        }
        code_block(ui, tk, job);
        para(
            ui,
            &format!(
                "{} {}",
                t("options", "linuxMalloc"),
                t("options", "linuxCommandHint")
            ),
            size::TEXT_3XS,
            14.0,
            tk.fg_faint,
        );
    });
}

/// "Launch options in Steam", taken apart.
fn steam_options(
    ui: &mut Ui,
    tk: &Tokens,
    info: &SteamLaunchInfoDto,
    steam: Option<&SteamOptions>,
) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().hline(
        r.x_range(),
        r.top() + 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.6)),
    );
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 6.0);
        ui.add_space(-2.0);
        stencil(ui, t("options", "linuxSteamOptions"), tk);
        match steam {
            Some(s) => {
                if !s.env.is_empty() {
                    para(
                        ui,
                        t("options", "linuxEnv"),
                        size::TEXT_2XS,
                        15.0,
                        tk.fg_faint,
                    );
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for (k, v) in &s.env {
                            let mut job = LayoutJob::default();
                            job.append(k, 0.0, mono_fmt(tk.accent));
                            job.append("=", 4.0, mono_fmt(tk.fg_faint));
                            job.append(v, 4.0, mono_fmt(tk.fg));
                            job.wrap.max_width = ui.available_width();
                            job.wrap.break_anywhere = true;
                            let g = paint::layout_top(ui, job);
                            let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                            ui.painter().galley(r.min, g, tk.fg);
                        }
                    });
                }
                if !s.wrappers.is_empty() {
                    para(
                        ui,
                        t("options", "linuxWrappers"),
                        size::TEXT_2XS,
                        15.0,
                        tk.fg_faint,
                    );
                    let items: Vec<ChipSpec> = s
                        .wrappers
                        .iter()
                        .map(|x| ChipSpec {
                            text: x.clone(),
                            bg: tk.raised,
                            ink: tk.fg,
                            tick: false,
                        })
                        .collect();
                    chips(ui, &items);
                }
                if !s.args.is_empty() {
                    para(
                        ui,
                        t("options", "linuxArgs"),
                        size::TEXT_2XS,
                        15.0,
                        tk.fg_faint,
                    );
                    let items: Vec<ChipSpec> = s
                        .args
                        .iter()
                        .map(|x| ChipSpec {
                            text: x.clone(),
                            bg: tk.raised,
                            ink: tk.info,
                            tick: false,
                        })
                        .collect();
                    chips(ui, &items);
                    if !s.has_command {
                        para(
                            ui,
                            t("options", "linuxNoCommand"),
                            size::TEXT_3XS,
                            14.0,
                            tk.fg_faint,
                        );
                    }
                }
                let raw = info.launch_options.as_deref().unwrap_or("");
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    Disclosure::new(super::RAW_ID, t("options", "linuxRaw")).show(ui, |ui| {
                        let mut job = LayoutJob::default();
                        job.append(raw, 0.0, mono_fmt(tk.fg_muted));
                        job.wrap.break_anywhere = true;
                        code_block(ui, tk, job);
                    });
                });
            }
            None => para(
                ui,
                t("options", "linuxNoOptions"),
                size::TEXT_2XS,
                15.0,
                tk.fg_faint,
            ),
        }
        para(
            ui,
            t("options", "linuxSteamOptionsHint"),
            size::TEXT_3XS,
            14.0,
            tk.fg_faint,
        );
    });
}
