//! The title bar (= `TitleBar.svelte`): the identity and where we are, the live figures
//! centred on the window, the search pill (Ctrl K), the player, the language and theme
//! menus, the window's buttons. Dragging anything that is not a control moves the window;
//! a double-click maximises it.

use std::sync::Arc;

use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};

use super::menus::{self, MenuState};
use crate::app::Ctx;
use crate::i18n::{format, t};
use crate::icons::lucide;
use crate::kit::{Brand, Kbd, Side, TipExt, paint};
use crate::parts::rolling;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// Below this window width the figures go (`max-[1180px]:hidden`).
const FIGURES_MIN: f32 = 1180.0;
/// The Steam avatar's side (`size-5`).
const AVATAR: f32 = 20.0;
/// The window buttons' width (`w-11`).
const WIN_BTN: f32 = 44.0;

/// A press on the title bar's free area: the first of a pair, or the second (a double-click).
enum Press {
    First,
    Second,
}

/// The double-click window, as the platforms' defaults go.
const DOUBLE_PRESS_S: f64 = 0.4;
const DOUBLE_PRESS_PX: f32 = 6.0;

/// The window moves on the *press*, as `TitleBar.svelte`'s `mousedown` did, not once
/// egui's drag threshold is crossed: on an unfocused window the compositor takes the
/// focus click, and a drag that waited for motion never started. The window manager owns
/// the pointer from then on, so egui sees no release and cannot tell a double-click:
/// a second press close in time and place is one, and maximises instead.
fn drag_press(ui: &Ui, drag: &egui::Response) -> Option<Press> {
    let (pressed, pos, time) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.press_origin(),
            i.time,
        )
    });
    // With a click sense, egui gives the press to the topmost control: a press on a
    // title-bar button is the button's (`hovered` would still be true here).
    if !pressed || !drag.is_pointer_button_down_on() {
        return None;
    }
    let pos = pos?;
    // A press on the window's edge is a resize (`window::resize_edges`), not a move.
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    if !maximized && super::window::direction(ui.ctx().content_rect(), pos).is_some() {
        return None;
    }
    let id = drag.id.with("last-press");
    let last: Option<(f64, egui::Pos2)> = ui.data(|d| d.get_temp(id));
    let second =
        last.is_some_and(|(t, p)| time - t <= DOUBLE_PRESS_S && p.distance(pos) <= DOUBLE_PRESS_PX);
    ui.data_mut(|d| {
        if second {
            d.remove::<(f64, egui::Pos2)>(id);
        } else {
            d.insert_temp(id, (time, pos));
        }
    });
    Some(if second { Press::Second } else { Press::First })
}

pub(super) fn show(ui: &mut Ui, cx: &mut Ctx, menus_state: &mut MenuState) {
    let tk = ui.tokens();
    let rect = ui.max_rect();
    let win_w = ui.ctx().content_rect().width();
    // The drag area first: every control drawn after it takes the pointer before it.
    // Click, not drag: egui hands a drag to the drag-sensing widget under the pointer
    // even below a button, so a press on a title-bar button also moved the window and
    // the button never saw its release. Sensing clicks, the topmost control wins.
    // (Not focusable: the bar is no stop for Tab.)
    let drag = ui.interact(rect, ui.id().with("titlebar-drag"), Sense::CLICK);
    // Game Mode (gamescope) has no window manager: nothing moves, maximises or
    // minimises there, only the close button is drawn.
    let game_mode = crate::platform::deck::game_mode();
    if let Some(press) = drag_press(ui, &drag).filter(|_| !game_mode) {
        match press {
            Press::Second => toggle_maximized(ui.ctx()),
            Press::First => {
                let _ = cx.run("window.drag", &serde_json::Value::Null);
            }
        }
    }
    // (The bottom border is the panel's separator line.)
    let cy = rect.center().y - 0.5;

    // ── identity, then where we are ──
    let mut x = rect.left() + 12.0;
    logo(
        ui,
        Rect::from_center_size(pos2(x + 10.0, cy), Vec2::splat(20.0)),
    );
    x += 20.0 + 10.0;
    let face = font(Face::DisplayBold, 15.0);
    let dayz = paint::tracked(ui, "DayZ ", face.clone(), tk.fg, 0.06, true, None);
    let hub = paint::tracked(ui, "Community Hub", face, tk.accent, 0.06, true, None);
    paint::text_at(ui.painter(), &dayz, x, cy + 0.5, tk.fg);
    x += dayz.size().x;
    paint::text_at(ui.painter(), &hub, x, cy + 0.5, tk.accent);
    // CSS tracking also follows the last letter.
    x += hub.size().x + 15.0 * 0.06 + 10.0;
    ui.painter().vline(
        x + 0.5,
        (cy - 7.0)..=(cy + 7.0),
        Stroke::new(1.0, tk.border_strong),
    );
    x += 1.0 + 10.0;
    // Under the setup wizard the view is hidden: the crumb names the wizard, not it.
    let here = if super::wizard_covers(cx) {
        t("shell", "titlebarSetup")
    } else {
        cx.nav.view.label()
    };
    let left_end = x;

    // ── the right side, from the window's edge inwards ──
    let mut right = rect.right();
    let maximized = cx.stores.app.maximized;
    for (i, (icon, label)) in [
        (lucide::X, t("shell", "windowClose")),
        (
            if maximized {
                lucide::COPY
            } else {
                lucide::SQUARE
            },
            t("shell", "windowMaximize"),
        ),
        (lucide::MINUS, t("shell", "windowMinimize")),
    ]
    .into_iter()
    .enumerate()
    .take(if game_mode { 1 } else { 3 })
    {
        right -= WIN_BTN;
        let r = Rect::from_min_max(
            pos2(right, rect.top()),
            pos2(right + WIN_BTN, rect.bottom() - 1.0),
        );
        let resp = ui.interact(r, ui.id().with(("win", i)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        let h = paint::ease(ui, resp.id, resp.hovered());
        let close = i == 0;
        let (fill, ink) = if close {
            // `hover:bg-err hover:text-white`.
            (
                paint::lerp(Color32::TRANSPARENT, tk.err, h),
                paint::lerp(tk.fg_muted, Color32::WHITE, h),
            )
        } else {
            (
                paint::lerp(Color32::TRANSPARENT, tk.raised, h),
                paint::lerp(tk.fg_muted, tk.fg, h),
            )
        };
        ui.painter().rect_filled(r, CornerRadius::ZERO, fill);
        let px = if i == 1 { 12.0 } else { size::ICON_SM };
        icon.paint(
            ui.painter(),
            Rect::from_center_size(r.center(), Vec2::splat(px)),
            ink,
        );
        super::ring(ui, &resp, r.shrink(3.0), size::RADIUS_SM);
        if resp.clicked() {
            let action = match i {
                0 => "close",
                1 => "maximize",
                _ => "minimize",
            };
            let _ = cx.run(
                "window.minimize_maximize_close",
                &serde_json::json!({ "action": action }),
            );
        }
    }
    right -= 1.0;
    sep(ui, right, rect, &tk);

    // Theme, then language: `w-9` each.
    right -= 36.0;
    let theme_r = Rect::from_min_max(
        pos2(right, rect.top()),
        pos2(right + 36.0, rect.bottom() - 1.0),
    );
    menus::theme_menu(ui, cx, theme_r, menus_state);
    right -= 36.0;
    let lang_r = Rect::from_min_max(
        pos2(right, rect.top()),
        pos2(right + 36.0, rect.bottom() - 1.0),
    );
    menus::language_menu(ui, cx, lang_r);

    // The player: avatar, name, Steam login (`max-w-56 px-2.5 gap-2`).
    right = profile(ui, cx, right, rect, win_w);
    right -= 1.0;
    sep(ui, right, rect, &tk);
    right -= 8.0;
    right = search_pill(ui, cx, right, rect, win_w);

    // The figures only where they clear the search pill (the pad density's wider text
    // can bring it past the centre).
    let figures_on =
        win_w > FIGURES_MIN && rect.center().x + figures_half_width(ui, cx) + 12.0 <= right;
    // Where we are, cut to what is left.
    let max_here = (right.min(if figures_on {
        rect.center().x - figures_half_width(ui, cx)
    } else {
        right
    }) - left_end
        - 12.0)
        .max(0.0);
    let g = super::stencil(ui, here, tk.fg_muted, Some(max_here));
    paint::text_at(ui.painter(), &g, left_end, cy + 0.5, tk.fg_muted);

    // ── the live figures, centred on the window ──
    if figures_on {
        figures(ui, cx, rect);
    }
}

fn toggle_maximized(ctx: &egui::Context) {
    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
}

/// `my-2 w-px bg-border` at `x`.
fn sep(ui: &Ui, x: f32, rect: Rect, tk: &crate::theme::Tokens) {
    ui.painter().vline(
        x + 0.5,
        (rect.top() + 8.0)..=(rect.bottom() - 9.0),
        Stroke::new(1.0, tk.border),
    );
}

/// The three figures: servers, players in game, players on Steam.
struct Figure {
    value: String,
    tip: &'static str,
}

fn figure_values(ui: &Ui, cx: &Ctx) -> [Figure; 3] {
    let s = &cx.stores.servers;
    let servers = s
        .stats
        .as_ref()
        .map(|st| st.server_count as f64)
        .or((s.total > 0).then_some(f64::from(s.total)));
    let ingame = s.stats.as_ref().map(|st| st.total_players as f64);
    let steam = s.steam_players.map(f64::from);
    let id = ui.id().with("figures");
    let roll = |k: &str, v: Option<f64>| match v {
        Some(v) => format::num(rolling(ui, id.with(k), v).round()),
        None => format::NONE.to_string(),
    };
    [
        Figure {
            value: roll("servers", servers),
            tip: t("shell", "titlebarServers"),
        },
        Figure {
            value: roll("ingame", ingame),
            tip: t("shell", "titlebarPlayersIngame"),
        },
        Figure {
            value: roll("steam", steam),
            tip: t("shell", "titlebarPlayersSteam"),
        },
    ]
}

/// The figures laid out, once per pass: the bar's centring and its drawing both ask.
fn figure_galleys(ui: &Ui, cx: &Ctx) -> ([Figure; 3], [Arc<egui::Galley>; 3]) {
    type Laid = (u64, [&'static str; 3], [Arc<egui::Galley>; 3]);
    let id = egui::Id::new("dzch.titlebar.figures-laid");
    let pass = ui.ctx().cumulative_pass_nr();
    let figure = |tip: &'static str| Figure {
        value: String::new(),
        tip,
    };
    if let Some((p, tips, g)) = ui.data(|d| d.get_temp::<Arc<Laid>>(id)).as_deref()
        && *p == pass
    {
        return (tips.map(figure), g.clone());
    }
    let tk = ui.tokens();
    let f = figure_values(ui, cx);
    let mk = |s: &str| paint::galley(ui, s, font(Face::MonoMedium, size::TEXT_2XS), tk.fg);
    let g = [mk(&f[0].value), mk(&f[1].value), mk(&f[2].value)];
    let tips = [f[0].tip, f[1].tip, f[2].tip];
    ui.data_mut(|d| d.insert_temp(id, Arc::new((pass, tips, g.clone()))));
    (f, g)
}

fn figures_half_width(ui: &Ui, cx: &Ctx) -> f32 {
    let (_, g) = figure_galleys(ui, cx);
    let w: f32 = g
        .iter()
        .map(|g| size::ICON_SM + 6.0 + g.size().x)
        .sum::<f32>()
        + 2.0 * 16.0;
    w / 2.0
}

fn figures(ui: &mut Ui, cx: &Ctx, rect: Rect) {
    let tk = ui.tokens();
    let (f, g) = figure_galleys(ui, cx);
    let w: f32 = g
        .iter()
        .map(|g| size::ICON_SM + 6.0 + g.size().x)
        .sum::<f32>()
        + 2.0 * 16.0;
    let cy = rect.center().y - 0.5;
    let mut x = ui.ctx().content_rect().center().x - w / 2.0;
    for (i, (fig, g)) in f.iter().zip(g.iter()).enumerate() {
        let item_w = size::ICON_SM + 6.0 + g.size().x;
        let r = Rect::from_min_size(pos2(x, cy - 8.0), vec2(item_w, 16.0));
        let resp = ui.interact(r, ui.id().with(("figure", i)), Sense::hover());
        let icon_r = Rect::from_center_size(
            pos2(x + size::ICON_SM / 2.0, cy),
            Vec2::splat(size::ICON_SM),
        );
        match i {
            0 => lucide::SERVER.paint(ui.painter(), icon_r, tk.info),
            1 => lucide::USERS.paint(ui.painter(), icon_r, tk.ok),
            _ => Brand::Steam.paint(ui.painter(), icon_r, tk.fg_muted),
        }
        paint::text_at(ui.painter(), g, x + size::ICON_SM + 6.0, cy + 0.5, tk.fg);
        resp.tip_kbd(fig.tip, "", Side::Bottom);
        x += item_w + 16.0;
    }
}

/// The search pill (`my-1.5 mr-2 rounded-sm border bg-panel/60 pl-2 pr-1 gap-2`), its
/// right edge at `right`. Returns its left edge.
fn search_pill(ui: &mut Ui, cx: &mut Ctx, right: f32, rect: Rect, win_w: f32) -> f32 {
    let tk = ui.tokens();
    let words = (win_w >= 1024.0).then(|| format!("{}…", t("shell", "paletteSearch")));
    let wg = words
        .as_deref()
        .map(|w| paint::galley(ui, w, font(Face::Sans, size::TEXT_2XS), tk.fg_faint));
    let kbd_w = Kbd::width(ui, "Ctrl K");
    let w = 8.0 + size::ICON_SM + 8.0 + wg.as_ref().map_or(0.0, |g| g.size().x + 8.0) + kbd_w + 4.0;
    let r = Rect::from_min_max(
        pos2(right - w, rect.top() + 6.0),
        pos2(right, rect.bottom() - 7.0),
    );
    let resp = ui.interact(r, ui.id().with("search-pill"), Sense::click());
    let h = paint::ease(ui, resp.id, resp.hovered());
    let p = ui.painter();
    paint::plate(
        p,
        r,
        size::RADIUS_SM,
        paint::alpha(tk.panel, 0.6),
        paint::lerp(tk.border, tk.border_strong, h),
    );
    let ink = paint::lerp(tk.fg_faint, tk.fg_muted, h);
    let cy = r.center().y;
    let mut x = r.left() + 8.0;
    lucide::SEARCH.paint(
        p,
        Rect::from_min_size(
            pos2(x, cy - size::ICON_SM / 2.0),
            Vec2::splat(size::ICON_SM),
        ),
        ink,
    );
    x += size::ICON_SM + 8.0;
    if let Some(g) = &wg {
        paint::text_at(p, g, x, cy, ink);
        x += g.size().x + 8.0;
    }
    Kbd::paint_at(ui, "Ctrl K", x, cy - 0.5);
    super::ring(ui, &resp, r, size::RADIUS_SM);
    let resp = resp.tip_kbd(t("shell", "paletteOpen"), "", Side::Bottom);
    if resp.clicked() {
        cx.stores.dialogs.shortcuts = false;
        cx.stores.dialogs.palette = true;
    }
    r.left()
}

/// The player (`max-w-56 px-2.5 gap-2 text-xs`), its right edge at `right`. Returns its
/// left edge.
fn profile(ui: &mut Ui, cx: &mut Ctx, right: f32, rect: Rect, win_w: f32) -> f32 {
    let tk = ui.tokens();
    let stats = cx.stores.servers.stats.as_ref();
    let name = stats
        .and_then(|s| s.player_name.as_deref())
        .filter(|n| !n.is_empty());
    let login = stats
        .and_then(|s| s.steam_login.as_deref())
        .filter(|l| !l.is_empty() && win_w >= 1280.0);
    let max_w = 224.0;
    let inner_max = max_w - 20.0 - size::ICON - 8.0;
    let (ng, lg) = match name {
        Some(n) => {
            let ng = paint::galley_trunc(
                ui,
                n,
                font(Face::SansMedium, size::TEXT_XS),
                tk.fg,
                inner_max,
            );
            let lg = login.map(|l| {
                paint::galley_trunc(
                    ui,
                    l,
                    font(Face::Mono, size::TEXT_2XS),
                    tk.fg_faint,
                    (inner_max - ng.size().x - 8.0).max(0.0),
                )
            });
            (ng, lg)
        }
        None => (
            paint::galley_trunc(
                ui,
                t("shell", "titlebarSetupAccount"),
                font(Face::Sans, size::TEXT_XS),
                tk.fg_muted,
                inner_max,
            ),
            None,
        ),
    };
    // The avatar is drawn at `size-5`, the icon at `size-icon`.
    let avatar_key = cx.stores.profile.avatar_key;
    let lead = match cx.stores.profile.avatar_url.as_deref() {
        Some(src)
            if crate::parts::image::texture_keyed(
                ui.ctx(),
                avatar_key,
                src,
                crate::parts::image::side_for(ui.ctx(), Vec2::splat(AVATAR)),
            )
            .is_some() =>
        {
            AVATAR
        }
        _ => size::ICON,
    };
    let w = 10.0 + lead + 8.0 + ng.size().x + lg.as_ref().map_or(0.0, |g| g.size().x + 8.0) + 10.0;
    let r = Rect::from_min_max(
        pos2(right - w, rect.top()),
        pos2(right, rect.bottom() - 1.0),
    );
    let resp = ui.interact(r, ui.id().with("profile"), Sense::click());
    let h = paint::ease(ui, resp.id, resp.hovered());
    let p = ui.painter();
    p.rect_filled(
        r,
        CornerRadius::ZERO,
        paint::lerp(Color32::TRANSPARENT, tk.raised, h),
    );
    let cy = r.center().y;
    let mut x = r.left() + 10.0;
    // The Steam avatar (`size-5 rounded-full ring-1 ring-border-strong`), else the icon.
    let avatar = cx.stores.profile.avatar_url.as_deref().and_then(|src| {
        let a = Rect::from_min_size(pos2(x, cy - AVATAR / 2.0), Vec2::splat(AVATAR));
        crate::parts::image::paint_keyed(p, a, avatar_key, src, CornerRadius::same(255))
            .then_some(a)
    });
    match avatar {
        Some(a) => {
            p.circle_stroke(
                a.center(),
                AVATAR / 2.0 + 0.5,
                Stroke::new(1.0, tk.border_strong),
            );
        }
        None => lucide::USER_ROUND.paint(
            p,
            Rect::from_min_size(pos2(x, cy - size::ICON / 2.0), Vec2::splat(size::ICON)),
            paint::lerp(tk.fg_muted, tk.fg, h),
        ),
    }
    x += lead + 8.0;
    let name_ink = if name.is_some() {
        tk.fg
    } else {
        paint::lerp(tk.fg_muted, tk.fg, h)
    };
    // (`italic` for "set up the account": the faces ship no italic.)
    paint::text_at(p, &ng, x, cy, name_ink);
    x += ng.size().x + 8.0;
    if let Some(lg) = &lg {
        paint::text_at(p, lg, x, cy + 0.5, tk.fg_faint);
    }
    super::ring(ui, &resp, r.shrink(3.0), size::RADIUS_SM);
    let resp = resp.tip_kbd(t("shell", "titlebarEditAccount"), "", Side::Bottom);
    if resp.clicked() {
        let _ = cx.run("settings.open_account", &serde_json::Value::Null);
    }
    r.left()
}

/// The launcher's mark (`logo.svg`: the pin, its compass ticks and the stencil "DZ", drawn
/// white), rasterised once per size and tinted with the theme's ink.
pub(super) fn logo(ui: &Ui, rect: Rect) {
    static SVG: &[u8] = include_bytes!("logo.svg");
    let ctx = ui.ctx();
    let px = (rect.width() * ctx.pixels_per_point())
        .round()
        .clamp(1.0, 256.0) as u32;
    let id = egui::Id::new(("dzch.logo", px));
    let tex: Option<Option<egui::TextureHandle>> = ctx.data(|d| d.get_temp(id));
    let tex = match tex {
        Some(t) => t,
        None => {
            let t = raster(SVG, px)
                .map(|img| ctx.load_texture("dzch-logo", img, egui::TextureOptions::LINEAR));
            ctx.data_mut(|d| d.insert_temp(id, t.clone()));
            t
        }
    };
    if let Some(tex) = tex {
        ui.painter().image(
            tex.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            ui.tokens().fg,
        );
    }
}

fn raster(svg: &[u8], px: u32) -> Option<egui::ColorImage> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
    let s = tree.size();
    let scale = px as f32 / s.width().max(s.height()).max(1.0);
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        pixmap.data(),
    ))
}

#[cfg(test)]
mod drag_tests {
    use super::{Press, drag_press};
    use egui::{Event, PointerButton, Pos2, Rect, Sense, pos2, vec2};
    use egui_kittest::Harness;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn press(h: &mut Harness<'_>, at: Pos2) {
        let input = h.input_mut();
        input.events.push(Event::PointerMoved(at));
        input.events.push(Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
    }

    fn release(h: &mut Harness<'_>, at: Pos2) {
        h.input_mut().events.push(Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
    }

    /// One press moves the window at once (no drag threshold to cross), and a second press
    /// on the same spot soon after maximises instead.
    #[test]
    fn the_press_itself_moves_and_a_second_maximises() {
        let seen: Rc<RefCell<Vec<&'static str>>> = Rc::default();
        let log = Rc::clone(&seen);
        let mut h = Harness::builder()
            .with_size(vec2(400.0, 200.0))
            // Real frames: the two presses of a double-click fall within its window.
            .with_step_dt(1.0 / 60.0)
            .build_ui(move |ui| {
                let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 34.0));
                let r = ui.interact(rect, ui.id().with("bar"), Sense::click());
                match drag_press(ui, &r) {
                    Some(Press::First) => log.borrow_mut().push("move"),
                    Some(Press::Second) => log.borrow_mut().push("maximise"),
                    None => {}
                }
            });
        let at = pos2(200.0, 17.0);
        h.run();
        press(&mut h, at);
        h.step();
        assert_eq!(
            *seen.borrow(),
            ["move"],
            "the press alone must start the move"
        );
        release(&mut h, at);
        h.step();
        press(&mut h, at);
        h.step();
        assert_eq!(*seen.borrow(), ["move", "maximise"]);
        // Away from the bar: nothing.
        release(&mut h, at);
        h.step();
        press(&mut h, pos2(200.0, 150.0));
        h.step();
        assert_eq!(seen.borrow().len(), 2);
    }

    /// A press on a control over the bar is the control's: it clicks, the window stays put.
    #[test]
    fn a_press_on_a_control_is_not_a_move() {
        let seen: Rc<RefCell<Vec<&'static str>>> = Rc::default();
        let log = Rc::clone(&seen);
        let mut h = Harness::builder()
            .with_size(vec2(400.0, 200.0))
            .with_step_dt(1.0 / 60.0)
            .build_ui(move |ui| {
                let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 34.0));
                let r = ui.interact(rect, ui.id().with("bar"), Sense::click());
                if drag_press(ui, &r).is_some() {
                    log.borrow_mut().push("move");
                }
                let b = Rect::from_min_size(pos2(360.0, 5.0), vec2(30.0, 24.0));
                if ui.place(b, egui::Button::new("x")).clicked() {
                    log.borrow_mut().push("button");
                }
            });
        let at = pos2(375.0, 17.0);
        h.run();
        h.input_mut().events.push(Event::PointerMoved(at));
        h.step();
        press(&mut h, at);
        h.step();
        release(&mut h, at);
        h.step();
        assert_eq!(*seen.borrow(), ["button"]);
    }
}
