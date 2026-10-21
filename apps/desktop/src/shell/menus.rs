//! The title bar's two menus (= `LanguageMenu.svelte`, `ThemeMenu.svelte`): the language,
//! by its flag; the theme, every preset with its swatch, worn for as long as the pointer
//! rests on it (150 ms), "follow the system" and its dark/light pair, the player's own
//! themes, and a way to the editor.

use egui::{Color32, CornerRadius, Popup, Rect, Response, Sense, Stroke, Ui, Vec2, pos2};
use serde_json::json;

use crate::app::Ctx;
use crate::i18n::{self, Locale, t};
use crate::icons::{Icon, flag, lucide};
use crate::kit::{Align, Side, TipExt, menu, paint};
use crate::parts::preset_name;
use crate::theme::presets::{self, PRESETS};
use crate::theme::{self, Raw, Scheme, Tokens, TokensExt, font, fonts::Face, size};
use crate::views::ViewId;

/// How long the pointer rests on a theme before it is worn.
const PREVIEW_AFTER: f64 = 0.15;
/// The theme menu's width (`w-64`) less its padding.
const THEME_W: f32 = 256.0 - 8.0;
/// The language menu's rows (`min-w-40`).
const LANG_W: f32 = 168.0;

/// What the menus keep between frames: the theme being rested on, and the one worn
/// for it.
#[derive(Default)]
pub(crate) struct MenuState {
    hover: Option<(String, f64)>,
    previewing: Option<String>,
}

/// The flag of a language (`circle-flags`: English is the British one).
fn flag_of(l: Locale) -> Option<Icon> {
    flag::by_code(match l {
        Locale::En => "gb",
        other => other.code(),
    })
}

/// A title-bar trigger (`grid h-titlebar w-9 place-items-center`, lit on hover and
/// while its menu is open).
fn trigger(ui: &mut Ui, rect: Rect, salt: &str, label: &str) -> (Response, f32) {
    let tk = ui.tokens();
    let resp = ui.interact(rect, ui.id().with(salt), Sense::click());
    // Where a command (`theme.menu`) finds the menu to open.
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::new(("dzch.shell.menu", salt)),
            Popup::default_response_id(&resp),
        )
    });
    let open = menu::is_open(&resp);
    let label_s = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label_s));
    let h = paint::ease(ui, resp.id, resp.hovered() || open);
    ui.painter().rect_filled(
        rect,
        CornerRadius::ZERO,
        paint::lerp(Color32::TRANSPARENT, tk.raised, h),
    );
    let resp = if open {
        resp
    } else {
        resp.tip_kbd(label, "", Side::Bottom)
    };
    (resp, h)
}

pub(super) fn language_menu(ui: &mut Ui, cx: &mut Ctx, rect: Rect) {
    let label = t("shell", "langChange");
    let (resp, _) = trigger(ui, rect, "lang", label);
    let current = i18n::locale();
    if let Some(f) = flag_of(current) {
        f.paint(
            ui.painter(),
            Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
            Color32::WHITE,
        );
    }
    let mut pick = None;
    menu::dropdown(&resp, Side::Bottom, Align::End, |ui| {
        let tk = ui.tokens();
        for l in Locale::ALL {
            let (r, item) = ui.allocate_exact_size(Vec2::new(LANG_W, size::ROW), Sense::click());
            let lit = item.hovered();
            let p = ui.painter();
            if lit {
                p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.raised);
            }
            let cy = r.center().y;
            let mut x = r.left() + 6.0;
            if let Some(f) = flag_of(l) {
                f.paint(
                    p,
                    Rect::from_min_size(pos2(x, cy - 8.0), Vec2::splat(16.0)),
                    Color32::WHITE,
                );
            }
            x += 16.0 + 8.0;
            let ink = if lit { tk.fg } else { tk.fg_muted };
            let g = paint::galley(ui, l.label(), font(Face::Sans, size::TEXT_XS), ink);
            paint::text_at(p, &g, x, cy, ink);
            let mut right = r.right() - 6.0;
            if l == current {
                right -= size::ICON_SM;
                lucide::CHECK.paint(
                    p,
                    Rect::from_min_size(pos2(right, cy - 7.0), Vec2::splat(size::ICON_SM)),
                    tk.accent,
                );
                right -= 8.0;
            }
            let code = paint::tracked(
                ui,
                l.code(),
                font(Face::Mono, size::TEXT_3XS),
                tk.fg_faint,
                0.0,
                true,
                None,
            );
            paint::text_at(p, &code, right - code.size().x, cy, tk.fg_faint);
            if item.clicked() {
                pick = Some(l);
            }
        }
    });
    if let Some(l) = pick {
        let _ = cx.run("i18n.set_locale", &json!({ "locale": l.code() }));
    }
}

/// The five colours a swatch shows: ground, panel, accent, ok, err.
fn swatch_colors(raw: &Raw) -> [Color32; 5] {
    [
        theme::rgb(raw.bg),
        theme::rgb(raw.panel),
        theme::rgb(raw.accent),
        theme::rgb(raw.ok),
        theme::rgb(raw.err),
    ]
}

/// `flex h-4 rounded-xs border`: five 8 px bands. Returns its width.
fn swatch(ui: &Ui, x: f32, cy: f32, colors: &[Color32; 5], tk: &Tokens) -> f32 {
    let r = Rect::from_min_size(pos2(x, cy - 8.0), Vec2::new(5.0 * 8.0 + 2.0, 16.0));
    let p = ui.painter();
    for (i, c) in colors.iter().enumerate() {
        let band = Rect::from_min_size(
            pos2(r.left() + 1.0 + i as f32 * 8.0, r.top() + 1.0),
            Vec2::new(8.0, 14.0),
        );
        p.rect_filled(band, CornerRadius::ZERO, *c);
    }
    p.rect_stroke(
        r,
        CornerRadius::same(size::RADIUS_XS),
        Stroke::new(1.0, tk.border),
        egui::StrokeKind::Inside,
    );
    r.width()
}

/// One theme row (`h-control px-1.5 gap-2 text-xs`): a swatch, the name, a tick.
fn theme_row(ui: &mut Ui, width: f32, colors: &[Color32; 5], name: &str, on: bool) -> Response {
    let tk = ui.tokens();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(width, size::CONTROL), Sense::click());
    let lit = resp.hovered();
    let p = ui.painter();
    if lit {
        p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.raised);
    }
    let cy = r.center().y;
    let x = r.left() + 6.0;
    let sw = swatch(ui, x, cy, colors, &tk);
    let mut right = r.right() - 6.0;
    if on {
        right -= size::ICON_SM;
        lucide::CHECK.paint(
            ui.painter(),
            Rect::from_min_size(pos2(right, cy - 7.0), Vec2::splat(size::ICON_SM)),
            tk.accent,
        );
        right -= 8.0;
    }
    let ink = if lit { tk.fg } else { tk.fg_muted };
    let tx = x + sw + 8.0;
    let g = paint::galley_trunc(
        ui,
        name,
        font(Face::Sans, size::TEXT_XS),
        ink,
        (right - tx).max(0.0),
    );
    paint::text_at(ui.painter(), &g, tx, cy, ink);
    resp
}

/// A plain row: an icon, the label, a tick.
fn icon_row(ui: &mut Ui, icon: Icon, label: &str, on: bool) -> Response {
    let tk = ui.tokens();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(THEME_W, size::CONTROL), Sense::click());
    let lit = resp.hovered();
    let p = ui.painter();
    if lit {
        p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.raised);
    }
    let ink = if lit { tk.fg } else { tk.fg_muted };
    let cy = r.center().y;
    let x = r.left() + 6.0;
    icon.paint(
        p,
        Rect::from_min_size(pos2(x, cy - size::ICON / 2.0), Vec2::splat(size::ICON)),
        ink,
    );
    let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_XS), ink);
    paint::text_at(ui.painter(), &g, x + size::ICON + 8.0, cy, ink);
    if on {
        lucide::CHECK.paint(
            ui.painter(),
            Rect::from_min_size(
                pos2(r.right() - 6.0 - size::ICON_SM, cy - 7.0),
                Vec2::splat(size::ICON_SM),
            ),
            tk.accent,
        );
    }
    resp
}

/// "When dark: Chernarus ‹": the item opening the list of one scheme's presets.
fn pair_choice(ui: &mut Ui, cx: &mut Ctx, scheme: Scheme) {
    let tk = ui.tokens();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(THEME_W, size::CONTROL), Sense::click());
    let sub_id = egui::containers::menu::SubMenu::id_from_widget_id(resp.id);
    let open = Popup::is_id_open(ui.ctx(), sub_id);
    let lit = resp.hovered() || open;
    let pair = &cx.prefs.get().theme.pair;
    let current = match scheme {
        Scheme::Dark => pair.dark.clone(),
        Scheme::Light => pair.light.clone(),
    };
    {
        let p = ui.painter();
        if lit {
            p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.raised);
        }
        let ink = if lit { tk.fg } else { tk.fg_muted };
        let cy = r.center().y;
        let mut x = r.left() + 28.0;
        let (icon, label) = match scheme {
            Scheme::Dark => (lucide::MOON, t("theme", "whenDark")),
            Scheme::Light => (lucide::SUN, t("theme", "whenLight")),
        };
        icon.paint(
            p,
            Rect::from_min_size(pos2(x, cy - 7.0), Vec2::splat(size::ICON_SM)),
            ink,
        );
        x += size::ICON_SM + 8.0;
        let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_XS), ink);
        paint::text_at(p, &g, x, cy, ink);
        x += g.size().x + 8.0;
        let right = r.right() - 6.0 - size::ICON_SM;
        lucide::CHEVRON_LEFT.paint(
            p,
            Rect::from_min_size(pos2(right, cy - 7.0), Vec2::splat(size::ICON_SM)),
            tk.fg_faint,
        );
        let name: &str = match presets::by_id(&current) {
            Some(p) => preset_name(p),
            None => &current,
        };
        let ng = paint::galley_trunc(
            ui,
            name,
            font(Face::Sans, size::TEXT_XS),
            tk.fg_faint,
            (right - 8.0 - x).max(0.0),
        );
        paint::text_at(
            ui.painter(),
            &ng,
            right - 8.0 - ng.size().x,
            cy,
            tk.fg_faint,
        );
    }
    let mut pick = None;
    egui::containers::menu::SubMenu::new().show(ui, &resp, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for p in PRESETS.iter().filter(|p| p.scheme == scheme) {
            let colors = swatch_colors(&p.raw);
            if theme_row(ui, 208.0 - 8.0, &colors, preset_name(p), p.id == current).clicked() {
                pick = Some(p.id);
            }
        }
    });
    if let Some(id) = pick {
        let s = match scheme {
            Scheme::Dark => "dark",
            Scheme::Light => "light",
        };
        let _ = cx.run("theme.set_pair", &json!({ "scheme": s, "id": id }));
    }
}

pub(super) fn theme_menu(ui: &mut Ui, cx: &mut Ctx, rect: Rect, st: &mut MenuState) {
    let tk = ui.tokens();
    let label = t("theme", "change");
    let (resp, h) = trigger(ui, rect, "theme", label);
    lucide::PALETTE.paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(size::ICON)),
        paint::lerp(tk.fg_muted, tk.fg, h),
    );
    let worn = cx.prefs.get().theme.preset.clone();
    let customs: Vec<(String, String, [Color32; 5])> = if menu::is_open(&resp) {
        cx.prefs
            .get()
            .theme
            .customs
            .iter()
            .map(|c| (c.id.clone(), c.name.clone(), swatch_colors(&c.tokens)))
            .collect()
    } else {
        Vec::new()
    };
    let max_h = (ui.ctx().content_rect().height() - size::TITLEBAR - 16.0 - 8.0).max(120.0);
    let mut hovered: Option<String> = None;
    let mut pick: Option<Option<String>> = None;
    let mut edit = false;
    let shown = menu::dropdown(&resp, Side::Bottom, Align::End, |ui| {
        ui.set_width(THEME_W);
        egui::ScrollArea::vertical()
            .max_height(max_h)
            .min_scrolled_height(max_h)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                if icon_row(
                    ui,
                    lucide::MONITOR,
                    t("theme", "followSystem"),
                    worn.is_none(),
                )
                .clicked()
                {
                    pick = Some(None);
                }
                pair_choice(ui, cx, Scheme::Dark);
                pair_choice(ui, cx, Scheme::Light);
                for (scheme, title) in [(Scheme::Dark, "dark"), (Scheme::Light, "light")] {
                    menu::separator(ui);
                    menu::label(ui, t("theme", title));
                    for p in PRESETS.iter().filter(|p| p.scheme == scheme) {
                        let colors = swatch_colors(&p.raw);
                        let on = worn.as_deref() == Some(p.id);
                        let r = theme_row(ui, THEME_W, &colors, preset_name(p), on);
                        if r.hovered() {
                            hovered = Some(p.id.to_string());
                        }
                        if r.clicked() {
                            pick = Some(Some(p.id.to_string()));
                        }
                    }
                }
                if !customs.is_empty() {
                    menu::separator(ui);
                    menu::label(ui, t("theme", "custom"));
                    for (id, name, colors) in &customs {
                        let on = worn.as_deref() == Some(id.as_str());
                        let r = theme_row(ui, THEME_W, colors, name, on);
                        if r.hovered() {
                            hovered = Some(id.clone());
                        }
                        if r.clicked() {
                            pick = Some(Some(id.clone()));
                        }
                    }
                }
                menu::separator(ui);
                if icon_row(ui, lucide::PENCIL, t("theme", "editCustom"), false).clicked() {
                    edit = true;
                }
            });
    });
    let open = shown.is_some();
    let now = ui.input(|i| i.time);
    // The hover preview: resting on an entry wears it; leaving, choosing or closing
    // ends it. Never stored.
    if !open || pick.is_some() {
        hovered = None;
    }
    match hovered {
        None => {
            st.hover = None;
            if st.previewing.take().is_some() {
                cx.stores.theme.preview(None);
            }
        }
        Some(id) => {
            let same = st.hover.as_ref().is_some_and(|(h, _)| *h == id);
            if !same {
                st.hover = Some((id, now));
                if st.previewing.take().is_some() {
                    cx.stores.theme.preview(None);
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(PREVIEW_AFTER));
            } else if let Some((h, since)) = &st.hover
                && st.previewing.as_deref() != Some(h.as_str())
            {
                if now - since >= PREVIEW_AFTER {
                    cx.stores.theme.preview(Some(h));
                    st.previewing = Some(h.clone());
                } else {
                    let left = (PREVIEW_AFTER - (now - since)).max(0.0);
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs_f64(left));
                }
            }
        }
    }
    match pick {
        Some(Some(id)) => {
            st.previewing = None;
            let _ = cx.run("theme.use_preset", &json!({ "id": id }));
        }
        Some(None) => {
            st.previewing = None;
            let _ = cx.run("theme.follow_system", &serde_json::Value::Null);
        }
        None => {}
    }
    if edit {
        cx.go_focus(ViewId::Settings, "appearance");
    }
}
