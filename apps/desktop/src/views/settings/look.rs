//! Appearance (follow the system and its pair, every preset drawn in its own colours,
//! the player's own themes, the editor for the one worn) and language.

use std::sync::OnceLock;

use egui::{Align, Color32, CornerRadius, Layout, Pos2, Rect, Sense, Ui, Vec2};
use serde_json::{Value, json};

use super::{SettingsView, block, boxed, line, rule, words};
use crate::app::Ctx;
use crate::i18n::{self, t};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{
    Button, Field, IconButton, Input, Segment, Segmented, Select, SelectOption, Side, Switch, Tag,
    TipExt, Tone, Variant,
};
use crate::parts::{ColorToken, LanguagePicker, SettingsSection, preset_name, theme_preview};
use crate::store::theme::{TOKEN_NAMES, raw_get};
use crate::theme::presets::{PRESETS, Preset};
use crate::theme::{Scheme, Tokens, TokensExt, font, fonts::Face, size};

/// Every preset's tokens, derived once.
fn preset_tokens() -> &'static [Tokens] {
    static ALL: OnceLock<Vec<Tokens>> = OnceLock::new();
    ALL.get_or_init(|| {
        PRESETS
            .iter()
            .map(|p| Tokens::derive(&p.raw, p.scheme))
            .collect()
    })
}

/// A colour's name in the editor (`settings.token*`).
fn token_label(name: &str) -> &'static str {
    let key = match name {
        "bg" => "tokenBg",
        "panel" => "tokenPanel",
        "raised" => "tokenRaised",
        "fg" => "tokenFg",
        "accent" => "tokenAccent",
        "accent-fg" => "tokenAccentFg",
        "ok" => "tokenOk",
        "warn" => "tokenWarn",
        "err" => "tokenErr",
        "info" => "tokenInfo",
        "map" => "tokenMap",
        _ => "tokenMods",
    };
    t("settings", key)
}

/// One theme card to draw.
struct Card<'a> {
    id: &'a str,
    name: &'a str,
    sub: &'a str,
    tokens: Tokens,
}

/// The cards in an auto-fill grid (`minmax(9.5rem, 1fr)`, gap 8); the one clicked.
fn grid(ui: &mut Ui, cards: &[Card<'_>], worn: Option<&str>) -> Option<String> {
    let tk = ui.tokens();
    let w = ui.available_width();
    let gap = 8.0;
    let cols = (((w + gap) / (152.0 + gap)).floor() as usize).clamp(1, 12);
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    // The name's line and the description's two (`line-clamp-2`), in the density worn:
    // the pad density's text is taller.
    let name_font = font(Face::SansMedium, size::TEXT_2XS);
    let sub_font = font(Face::Sans, size::TEXT_3XS);
    let name_h = crate::parts::ph(ui, "A", name_font.clone())
        .size()
        .y
        .max(14.0);
    let sub_h = crate::parts::ph(ui, "A\nA", sub_font.clone())
        .size()
        .y
        .max(26.0);
    let ch = 6.0 + 72.0 + 6.0 + name_h + sub_h + 4.0;
    let rows = cards.len().div_ceil(cols);
    let height = rows as f32 * ch + rows.saturating_sub(1) as f32 * gap;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
    let mut out = None;
    for (i, c) in cards.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let r = Rect::from_min_size(
            Pos2::new(
                rect.left() + col as f32 * (cw + gap),
                rect.top() + row as f32 * (ch + gap),
            ),
            Vec2::new(cw, ch),
        );
        if !ui.is_rect_visible(r) {
            continue;
        }
        let b = ui.interact(r, resp.id.with(c.id), Sense::click());
        let on = worn == Some(c.id);
        let h = paint::ease(ui, b.id.with("hover"), b.hovered());
        let p = ui.painter();
        if on {
            p.add(paint::glow(&tk).as_shape(r, CornerRadius::same(size::RADIUS_MD)));
            // The halo only outside (a CSS shadow is not drawn under its box): the
            // panel's ground over it, then the tint, or the words drown in the glow.
            p.rect_filled(r, CornerRadius::same(size::RADIUS_MD), tk.bg);
            paint::plate(p, r, size::RADIUS_MD, alpha(tk.accent, 0.08), tk.accent);
        } else {
            paint::plate(
                p,
                r,
                size::RADIUS_MD,
                alpha(tk.raised, 0.4 * h),
                paint::lerp(tk.border, tk.border_strong, h),
            );
        }
        if paint::keyboard_focus(ui, &b) {
            paint::focus_ring(p, r, size::RADIUS_MD, &tk);
        }
        let inner = r.shrink(6.0);
        let pr = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 72.0));
        theme_preview(p, pr, &c.tokens, true);
        let ty = pr.bottom() + 6.0 + name_h / 2.0;
        let check = if on { 14.0 } else { 0.0 };
        let ng = crate::parts::ph_trunc(ui, c.name, name_font.clone(), inner.width() - 4.0 - check);
        crate::parts::draw(p, &ng, inner.left() + 2.0, ty, tk.fg);
        if on {
            lucide::CHECK.paint(
                p,
                Rect::from_min_size(Pos2::new(inner.right() - 14.0, ty - 6.0), Vec2::splat(12.0)),
                tk.accent,
            );
        }
        let sub = paint::galley_wrap(
            ui,
            c.sub,
            sub_font.clone(),
            tk.fg_faint,
            inner.width() - 4.0,
        );
        // Two lines at most (`line-clamp-2`).
        let clip = Rect::from_min_size(
            Pos2::new(inner.left() + 2.0, ty + name_h / 2.0 + 1.0),
            Vec2::new(inner.width(), sub_h),
        );
        ui.painter()
            .with_clip_rect(clip.intersect(ui.clip_rect()))
            .galley(clip.min, sub, tk.fg_faint);
        b.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, c.name));
        if b.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            out = Some(c.id.to_string());
        }
    }
    out
}

/// A group's heading (`font-mono text-2xs tracking-[0.08em] uppercase`).
fn group(ui: &mut Ui, label: &str, ic: Icon) {
    let tk = ui.tokens();
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(size::PAD);
        ui.spacing_mut().item_spacing.x = 6.0;
        super::icon(ui, ic, size::ICON_SM, tk.fg_faint);
        let g = paint::tracked(
            ui,
            label,
            font(Face::Mono, size::TEXT_2XS),
            tk.fg_faint,
            0.08,
            true,
            None,
        );
        let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(r.min, g, tk.fg_faint);
    });
    ui.add_space(4.0);
}

/// `px-pad` around `body`.
fn padded<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.add_space(size::PAD);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - size::PAD);
            body(ui)
        })
        .inner
    })
    .inner
}

impl SettingsView {
    pub(super) fn appearance(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let selected = cx.stores.theme.selected(cx.prefs).map(str::to_string);
        // A seasonal theme worn over the player's is the one marked: it is what shows.
        let seasonal = cx.stores.theme.seasonal(cx.prefs);
        let worn = seasonal
            .map(str::to_string)
            .or_else(|| cx.prefs.get().theme.preset.clone());
        let custom = cx.stores.theme.custom(cx.prefs).cloned();
        let worn_name = match (seasonal, &selected, &custom) {
            (Some(id), _, _) => crate::theme::presets::by_id(id)
                .map_or_else(|| id.to_string(), |p| preset_name(p).to_string()),
            (None, None, _) => t("theme", "followSystem").to_string(),
            (None, _, Some(c)) => c.name.clone(),
            (None, Some(id), None) => crate::theme::presets::by_id(id)
                .map_or_else(|| id.clone(), |p| preset_name(p).to_string()),
        };
        SettingsSection::new(t("settings", "sectionAppearance"))
            .description(t("theme", "followSystemHint"))
            .icon(lucide::PALETTE)
            .show(
                ui,
                |ui| {
                    ui.add(Tag::new(&worn_name).tone(Tone::Accent));
                },
                |ui| {
                    // The interface's size (also Ctrl+ / Ctrl−), kept for the next start.
                    let mut pct = (cx.prefs.get().scale * 100.0).round() as i32;
                    Field::new(t("settings", "uiScale"))
                        .hint(t("settings", "uiScaleHint"))
                        .show(ui, |ui| {
                            let opts = [
                                Segment::new(75, "75 %"),
                                Segment::new(90, "90 %"),
                                Segment::new(100, "100 %"),
                                Segment::new(110, "110 %"),
                                Segment::new(125, "125 %"),
                                Segment::new(150, "150 %"),
                            ];
                            if ui.add(Segmented::new(&mut pct, &opts)).changed() {
                                let _ = cx.run(
                                    "ui.set_scale",
                                    &json!({ "scale": f64::from(pct) / 100.0 }),
                                );
                            }
                        });
                    seasons(ui, cx);
                    // Follow the system, and which dark and light preset that means.
                    block(ui, 10, false, None, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(16.0, 8.0);
                            if follow_button(ui, selected.is_none()) {
                                let _ = cx.run("theme.follow_system", &Value::Null);
                            }
                            let os = cx.stores.theme.os.unwrap_or(Scheme::Dark);
                            for scheme in [Scheme::Dark, Scheme::Light] {
                                // A group is one piece for the wrapping row: measured
                                // first, it starts a new line when it would run past
                                // the card's edge.
                                let word = match scheme {
                                    Scheme::Dark => "whenDark",
                                    Scheme::Light => "whenLight",
                                };
                                let label_w = paint::galley(
                                    ui,
                                    t("theme", word),
                                    font(Face::Sans, size::TEXT_2XS),
                                    tk.fg_faint,
                                )
                                .size()
                                .x;
                                let need = size::ICON_SM + 6.0 + label_w + 6.0 + 160.0;
                                if ui.available_size_before_wrap().x < need {
                                    ui.end_row();
                                }
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    let (ic, word, key) = match scheme {
                                        Scheme::Dark => (lucide::MOON, "whenDark", "dark"),
                                        Scheme::Light => (lucide::SUN, "whenLight", "light"),
                                    };
                                    let ink = if os == scheme { tk.accent } else { tk.fg_faint };
                                    super::icon(ui, ic, size::ICON_SM, ink);
                                    line(
                                        ui,
                                        t("theme", word),
                                        Face::Sans,
                                        size::TEXT_2XS,
                                        tk.fg_faint,
                                    );
                                    let pair = &cx.prefs.get().theme.pair;
                                    let cur = match scheme {
                                        Scheme::Dark => pair.dark.as_str(),
                                        Scheme::Light => pair.light.as_str(),
                                    };
                                    let mut v: &'static str =
                                        crate::theme::presets::by_id(cur).map_or("", |p| p.id);
                                    let opts: Vec<SelectOption<'_, &'static str>> = PRESETS
                                        .iter()
                                        .filter(|p| p.scheme == scheme)
                                        .map(|p| SelectOption::new(p.id, preset_name(p)))
                                        .collect();
                                    if ui
                                        .add(
                                            Select::new(("theme-pair", key), &mut v, &opts)
                                                .width(160.0)
                                                .label(t("theme", word)),
                                        )
                                        .changed()
                                    {
                                        let _ = cx.run(
                                            "theme.set_pair",
                                            &json!({ "scheme": key, "id": v }),
                                        );
                                    }
                                });
                            }
                        });
                    });

                    // Every preset, dark then light.
                    rule(ui);
                    let all = preset_tokens();
                    for (scheme, word, ic) in [
                        (Scheme::Dark, "dark", lucide::MOON),
                        (Scheme::Light, "light", lucide::SUN),
                    ] {
                        group(ui, t("theme", word), ic);
                        let cards: Vec<Card<'_>> = PRESETS
                            .iter()
                            .zip(all)
                            .filter(|(p, _)| p.scheme == scheme)
                            .map(|(p, tokens): (&Preset, &Tokens)| Card {
                                id: p.id,
                                name: preset_name(p),
                                sub: t("theme", p.blurb),
                                tokens: *tokens,
                            })
                            .collect();
                        if let Some(id) = padded(ui, |ui| grid(ui, &cards, worn.as_deref())) {
                            let _ = cx.run("theme.use_preset", &json!({ "id": id }));
                        }
                    }
                    ui.add_space(10.0);

                    // The player's own themes.
                    rule(ui);
                    ui.add_space(10.0);
                    padded(ui, |ui| {
                        ui.horizontal(|ui| {
                            let dup = t("theme", "duplicate");
                            let imp = t("theme", "import");
                            ui.allocate_ui_with_layout(
                                Vec2::new((ui.available_width() - 220.0).max(80.0), size::CONTROL),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    let g = paint::tracked(
                                        ui,
                                        t("theme", "yourThemes"),
                                        font(Face::DisplayBold, size::TEXT_LG),
                                        tk.fg,
                                        0.02,
                                        true,
                                        None,
                                    );
                                    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                                    ui.painter().galley(r.min, g, tk.fg);
                                    words(
                                        ui,
                                        t("settings", "yourThemeHint"),
                                        Face::Sans,
                                        size::TEXT_2XS,
                                        tk.fg_faint,
                                    );
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                if ui
                                    .add(
                                        Button::new(imp)
                                            .icon(lucide::UPLOAD)
                                            .variant(Variant::Ghost),
                                    )
                                    .clicked()
                                {
                                    let _ = cx.run("theme.import_file", &Value::Null);
                                }
                                if custom.is_none()
                                    && ui
                                        .add(
                                            Button::new(dup)
                                                .icon(lucide::COPY)
                                                .variant(Variant::Accent),
                                        )
                                        .clicked()
                                {
                                    let _ = cx.run("theme.duplicate", &Value::Null);
                                }
                            });
                        });
                        ui.add_space(6.0);
                        let customs = &cx.prefs.get().theme.customs;
                        if customs.is_empty() {
                            words(
                                ui,
                                t("theme", "yourThemesEmpty"),
                                Face::Sans,
                                size::TEXT_2XS,
                                tk.fg_muted,
                            );
                        } else {
                            let cards: Vec<Card<'_>> = customs
                                .iter()
                                .map(|c| Card {
                                    id: &c.id,
                                    name: &c.name,
                                    sub: t(
                                        "theme",
                                        if c.scheme == Scheme::Dark {
                                            "dark"
                                        } else {
                                            "light"
                                        },
                                    ),
                                    tokens: Tokens::derive(&c.tokens, c.scheme),
                                })
                                .collect();
                            if let Some(id) = grid(ui, &cards, worn.as_deref()) {
                                let _ = cx.run("theme.use_preset", &json!({ "id": id }));
                            }
                        }
                    });
                    ui.add_space(10.0);

                    if let Some(c) = &custom {
                        rule(ui);
                        self.editor(ui, cx, c);
                    }
                },
            );
    }

    /// The worn custom theme, open for editing.
    fn editor(&mut self, ui: &mut Ui, cx: &mut Ctx, c: &crate::prefs::CustomTheme) {
        let tk = ui.tokens();
        block(ui, 8, false, None, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // The name, renamed when the field is left.
                let draft = match &mut self.theme_name {
                    Some((id, n)) if *id == c.id => n,
                    slot => &mut slot.insert((c.id.clone(), c.name.clone())).1,
                };
                let r = ui.add(
                    Input::new(draft)
                        .placeholder(t("theme", "untitled"))
                        .width(176.0)
                        .id_salt("theme-name"),
                );
                if r.lost_focus() {
                    let name = draft.clone();
                    let _ = cx.run("theme.rename", &json!({ "id": c.id, "name": name }));
                }
                if !r.has_focus() && *draft != c.name {
                    draft.clone_from(&c.name);
                }
                let mut scheme = c.scheme;
                let opts = [
                    Segment::new(Scheme::Dark, t("theme", "dark")).icon(lucide::MOON),
                    Segment::new(Scheme::Light, t("theme", "light")).icon(lucide::SUN),
                ];
                if ui.add(Segmented::new(&mut scheme, &opts)).changed() {
                    let s = if scheme == Scheme::Dark {
                        "dark"
                    } else {
                        "light"
                    };
                    let _ = cx.run("theme.set_scheme", &json!({ "scheme": s }));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if ui
                        .add(
                            IconButton::new(lucide::TRASH_2, t("theme", "remove"))
                                .danger_hover()
                                .side(Side::Bottom),
                        )
                        .clicked()
                    {
                        let _ = cx.run("theme.remove_custom", &json!({ "id": c.id }));
                    }
                    let json_label = format!("{} (.json)", t("theme", "export"));
                    if ui
                        .add(IconButton::new(lucide::DOWNLOAD, &json_label).side(Side::Bottom))
                        .clicked()
                    {
                        let _ = cx.run("theme.export_file", &json!({ "id": c.id }));
                    }
                    if ui
                        .add(
                            IconButton::new(lucide::CLIPBOARD_COPY, t("theme", "export"))
                                .side(Side::Bottom),
                        )
                        .clicked()
                    {
                        let _ = cx.run("theme.export_clipboard", &json!({ "id": c.id }));
                    }
                    if c.base.is_some()
                        && ui
                            .add(
                                IconButton::new(lucide::ROTATE_CCW, t("theme", "resetHint"))
                                    .side(Side::Bottom),
                            )
                            .clicked()
                    {
                        let _ = cx.run("theme.reset_custom", &json!({ "id": c.id }));
                    }
                    if ui
                        .add(Button::new(t("theme", "derive")).icon(lucide::WAND_SPARKLES))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .tip_kbd(t("theme", "deriveHint"), "", Side::Bottom)
                        .clicked()
                    {
                        let _ = cx.run("theme.derive", &Value::Null);
                    }
                });
            });
        });
        rule(ui);
        // The twelve colours, two by two, 1px apart.
        let w = ui.available_width();
        let cw = (w - 1.0) / 2.0;
        let rows = TOKEN_NAMES.len().div_ceil(2);
        let ch = 40.0;
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(w, rows as f32 * ch + (rows as f32 - 1.0)),
            Sense::hover(),
        );
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, alpha(tk.border, 0.6));
        for (i, name) in TOKEN_NAMES.iter().enumerate() {
            let Some(mut v) = raw_get(&c.tokens, name) else {
                continue;
            };
            let r = Rect::from_min_size(
                Pos2::new(
                    rect.left() + (i % 2) as f32 * (cw + 1.0),
                    rect.top() + (i / 2) as f32 * (ch + 1.0),
                ),
                Vec2::new(cw, ch),
            );
            let mut cui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(r)
                    .id_salt(("theme-token", *name))
                    .layout(Layout::top_down(Align::Min)),
            );
            if cui
                .add(ColorToken::new(token_label(name), &mut v))
                .changed()
            {
                cx.stores.theme.set_token(cx.prefs, name, v);
            }
        }
    }

    pub(super) fn language(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        SettingsSection::new(t("settings", "sectionLanguage"))
            .description(t("settings", "languageHint"))
            .icon(lucide::LANGUAGES)
            .show(
                ui,
                |_| {},
                |ui| {
                    block(ui, 10, false, None, |ui| {
                        if let Some(l) = LanguagePicker::new(i18n::locale()).inline().show(ui) {
                            let _ = cx.run("i18n.set_locale", &json!({ "locale": l.code() }));
                        }
                    });
                },
            );
    }
}

/// The seasonal themes (October, December) and their effects, each on or off.
fn seasons(ui: &mut Ui, cx: &mut Ctx) {
    let p = &cx.prefs.get().ui;
    let (themes, effects) = (p.seasonal_themes, p.seasonal_effects);
    for (on, label, hint, switch, cmd) in [
        (
            themes,
            "seasonalThemes",
            "seasonalThemesHint",
            "seasonalThemesSwitch",
            "ui.set_seasonal_themes",
        ),
        (
            effects,
            "seasonalEffects",
            "seasonalEffectsHint",
            "seasonalEffectsSwitch",
            "ui.set_seasonal_effects",
        ),
    ] {
        Field::new(t("settings", label))
            .hint(t("settings", hint))
            .show(ui, |ui| {
                let mut v = on;
                let changed = boxed(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(Switch::new(&mut v, t("settings", switch))).changed()
                });
                if changed {
                    let _ = cx.run(cmd, &json!({ "on": v }));
                }
            });
    }
}

/// "Follow the system" (`h-control rounded-md border px-2`, lit when chosen).
fn follow_button(ui: &mut Ui, on: bool) -> bool {
    let tk = ui.tokens();
    let label = t("theme", "followSystem");
    let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_XS), tk.fg);
    let w = 8.0 + size::ICON_SM + 6.0 + g.size().x + if on { 6.0 + 12.0 } else { 0.0 } + 8.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL), Sense::click());
    let h = paint::ease(ui, resp.id.with("hover"), resp.hovered());
    let p = ui.painter();
    if on {
        p.add(paint::glow(&tk).as_shape(r, CornerRadius::same(size::RADIUS_MD)));
        p.rect_filled(r, CornerRadius::same(size::RADIUS_MD), tk.bg);
        paint::plate(p, r, size::RADIUS_MD, alpha(tk.accent, 0.08), tk.accent);
    } else {
        paint::plate(
            p,
            r,
            size::RADIUS_MD,
            Color32::TRANSPARENT,
            paint::lerp(tk.border, tk.border_strong, h),
        );
    }
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, r, size::RADIUS_MD, &tk);
    }
    let ink = if on {
        tk.fg
    } else {
        paint::lerp(tk.fg_muted, tk.fg, h)
    };
    let mut x = r.left() + 8.0;
    lucide::MONITOR.paint(
        p,
        Rect::from_min_size(
            Pos2::new(x, r.center().y - size::ICON_SM / 2.0),
            Vec2::splat(size::ICON_SM),
        ),
        ink,
    );
    x += size::ICON_SM + 6.0;
    paint::text_at(p, &g, x, r.center().y, ink);
    if on {
        lucide::CHECK.paint(
            p,
            Rect::from_min_size(
                Pos2::new(x + g.size().x + 6.0, r.center().y - 6.0),
                Vec2::splat(12.0),
            ),
            tk.accent,
        );
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
