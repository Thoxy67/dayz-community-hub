//! `Wizard.svelte`: the aside (the mark, the steps, the profile import), the pane (the
//! progress line, the step's header and body, the footer), the welcome, appearance and
//! done steps, and Enter.

use egui::{
    Align, Color32, CornerRadius, Id, Key, Layout, Order, Rect, Role, Sense, Stroke, Ui, UiBuilder,
    Vec2, WidgetInfo, pos2, vec2,
};
use serde_json::{Value, json};

use super::steps::{self, para, plate_box};
use super::{Acts, Step, Wizard};
use crate::app::{Ctx, reduced_motion};
use crate::i18n::{self, format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha, tracked};
use crate::kit::{Button, Size, Spinner, Variant, topo};
use crate::parts::{LanguagePicker, SectionCard, ThemePick, ThemeSwatches, draw, ph_trunc};
use crate::theme::presets;
use crate::theme::{Scheme, Tokens, TokensExt, font, fonts::Face, motion, size};

/// The aside (`w-72`).
const ASIDE_W: f32 = 288.0;
/// The content column (`max-w-2xl px-8`).
const COLUMN_W: f32 = 672.0;
const COLUMN_PAD: f32 = 32.0;
/// The footer: `py-3` around a large button, a rule over it.
const FOOTER_H: f32 = 12.0 + size::CONTROL_LG + 12.0 + 1.0;
/// How far the body rises as the wizard opens (`slide-up`).
const RISE_PX: f32 = 6.0;

pub(super) fn show(ctx: &egui::Context, cx: &mut Ctx, w: &mut Wizard, acts: &mut Acts) {
    let full = ctx.content_rect();
    let rect = Rect::from_min_max(pos2(full.left(), full.top() + size::TITLEBAR), full.max);
    if !rect.is_positive() {
        return;
    }
    let now = ctx.input(|i| i.time);
    let reduced = reduced_motion(ctx);
    let opened = *w.opened_at.get_or_insert(now);
    let since = (now - opened) as f32;
    // `fade-in` (120 ms) for the whole, `slide-up` (180 ms) for the body.
    let fade = if reduced {
        1.0
    } else {
        (since / motion::FAST).clamp(0.0, 1.0)
    };
    let rise = if reduced {
        1.0
    } else {
        egui::emath::easing::cubic_out((since / motion::BASE).clamp(0.0, 1.0))
    };
    if rise < 1.0 || fade < 1.0 {
        ctx.request_repaint();
    }
    egui::Area::new(Id::unique("dzch.setup"))
        .order(Order::Middle)
        .fixed_pos(rect.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(rect);
            if fade < 1.0 {
                ui.set_opacity(fade);
            }
            // The whole region answers the pointer: nothing behind it does.
            let _ = ui.allocate_rect(rect, Sense::CLICK);
            let tk = ui.tokens();
            ui.painter().rect_filled(rect, 0, tk.bg);
            let aside_r =
                Rect::from_min_size(rect.min, vec2(ASIDE_W.min(rect.width()), rect.height()));
            let pane_r = Rect::from_min_max(pos2(aside_r.right(), rect.top()), rect.max);
            aside(ui, w, aside_r, acts);
            pane(ui, cx, w, pane_r, rise, acts);
        });
    keys(ctx, cx, w, acts);
}

/// Enter anywhere but on a button advances (or, on the last step, goes to the servers).
fn keys(ctx: &egui::Context, cx: &Ctx, _w: &Wizard, acts: &mut Acts) {
    let blocked = cx.stores.dialogs.open()
        || egui::Popup::is_any_open(ctx)
        || ctx.memory(|m| m.top_modal_layer().is_some());
    if blocked {
        return;
    }
    let enter = ctx.input(|i| i.key_pressed(Key::Enter) && i.modifiers.is_none());
    if !enter {
        return;
    }
    // A focused button takes Enter itself; a text field gave its focus up on Enter.
    let focused = ctx.memory(|m| m.focused()).is_some();
    if focused && !ctx.egui_wants_keyboard_input() {
        return;
    }
    acts.push(("setup.next", Value::Null));
}

// ── the aside ────────────────────────────────────────────────────────────────

fn aside(ui: &mut Ui, w: &Wizard, r: Rect, acts: &mut Acts) {
    let tk = ui.tokens();
    let p = ui.painter_at(r);
    topo::paint(&p, r, &tk, 0.55);
    p.vline(r.right() - 0.5, r.y_range(), Stroke::new(1.0, tk.border));

    // The mark and the name (`px-6 pt-7`, `size-10`, `mt-3 title-display text-2xl`).
    let x = r.left() + 24.0;
    let logo = Rect::from_min_size(pos2(x, r.top() + 28.0), Vec2::splat(40.0));
    crate::shell::logo(ui, logo);
    let f = font(Face::DisplayBold, size::TEXT_2XL);
    let room = r.width() - 48.0;
    let line = size::TEXT_2XL;
    let mut y = logo.bottom() + 12.0;
    for (text, ink) in [("DayZ", tk.fg), ("Community Hub", tk.accent)] {
        let g = tracked(
            ui,
            text,
            f.clone(),
            Color32::PLACEHOLDER,
            0.02,
            true,
            Some(room),
        );
        draw(&p, &g, x, y + line / 2.0, ink);
        y += line;
    }

    // The steps (`mt-8 px-4 gap-0.5`, each `px-2 py-2 gap-3`).
    y += 32.0;
    let cur = w.index();
    for (i, s) in Step::ALL.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left() + 16.0, y), vec2(r.width() - 32.0, 40.0));
        y += 42.0;
        step_row(ui, &tk, w, *s, i, cur, row, acts);
    }

    // The way in from another computer (`mt-auto border-t px-6 py-4`).
    let hint = para(
        ui,
        t("setup", "importHint"),
        font(Face::Sans, size::TEXT_2XS),
        size::TEXT_2XS * 1.375,
        room,
    );
    let foot_h = 16.0 + hint.size().y + 8.0 + size::CONTROL_SM + 16.0;
    let top = (r.bottom() - foot_h).max(y);
    p.hline(
        r.x_range(),
        top + 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.7)),
    );
    p.galley(pos2(x, top + 16.0), hint.clone(), tk.fg_faint);
    let b = Rect::from_min_size(
        pos2(x, top + 16.0 + hint.size().y + 8.0),
        vec2(room, size::CONTROL_SM),
    );
    let mut bui = ui.new_child(
        UiBuilder::new()
            .max_rect(b)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let import = Button::new(t("setup", "importProfile"))
        .icon(lucide::FILE_UP)
        .size(Size::Xs);
    if bui.add(import).clicked() {
        acts.push(("profile.import", Value::Null));
    }
}

#[allow(clippy::too_many_arguments)]
fn step_row(
    ui: &mut Ui,
    tk: &Tokens,
    w: &Wizard,
    s: Step,
    i: usize,
    cur: usize,
    row: Rect,
    acts: &mut Acts,
) {
    let reachable = w.reachable(i);
    let label = t("setup", s.label_key());
    let sense = if reachable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(row, Id::unique(("dzch.setup.step", i)), sense);
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, reachable, label));
    let on = i == cur;
    let done = i < cur;
    let hover = paint::ease(ui, resp.id.with("hover"), reachable && resp.hovered());
    let p = ui.painter();
    let fill = if on {
        tk.raised
    } else {
        alpha(tk.raised, 0.5 * hover)
    };
    p.rect_filled(row, CornerRadius::same(size::RADIUS_SM), fill);
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, row, size::RADIUS_SM, tk);
    }
    // Steps ahead are dimmed (`opacity-60`).
    let dim = |c: Color32| if i > cur { c.gamma_multiply(0.6) } else { c };
    let cy = row.center().y;
    let circle = Rect::from_center_size(pos2(row.left() + 8.0 + 12.0, cy), Vec2::splat(24.0));
    let (fill, border, ink) = if on {
        (tk.accent, tk.accent, tk.accent_fg)
    } else if done {
        (alpha(tk.ok, 0.15), alpha(tk.ok, 0.5), tk.ok)
    } else {
        (Color32::TRANSPARENT, tk.border_strong, tk.fg_faint)
    };
    p.circle(
        circle.center(),
        11.5,
        dim(fill),
        Stroke::new(1.0, dim(border)),
    );
    if done {
        lucide::CHECK.paint(
            p,
            Rect::from_center_size(circle.center(), Vec2::splat(14.0)),
            dim(ink),
        );
    } else {
        let g = paint::galley(
            ui,
            &(i + 1).to_string(),
            font(Face::Mono, size::TEXT_2XS),
            Color32::PLACEHOLDER,
        );
        paint::text_center(p, &g, circle, dim(ink));
    }
    let lx = circle.right() + 12.0;
    let mut room = row.right() - 8.0 - lx;
    if s == Step::Services {
        let opt = steps::italic(ui, t("setup", "optionalStep"), size::TEXT_3XS);
        let ox = row.right() - 8.0 - opt.size().x;
        p.galley(
            pos2(ox, cy - opt.size().y / 2.0),
            opt.clone(),
            dim(tk.fg_faint),
        );
        room = ox - 8.0 - lx;
    }
    let face = if on { Face::SansSemibold } else { Face::Sans };
    let g = ph_trunc(ui, label, font(face, size::TEXT_SM), room);
    draw(p, &g, lx, cy, dim(if on { tk.fg } else { tk.fg_muted }));
    if reachable {
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() && !on {
            acts.push(("setup.go_step", json!({ "step": s.id() })));
        }
    }
}

// ── the pane ─────────────────────────────────────────────────────────────────

fn pane(ui: &mut Ui, cx: &mut Ctx, w: &mut Wizard, r: Rect, rise: f32, acts: &mut Acts) {
    let tk = ui.tokens();
    let total = Step::ALL.len();
    let idx = w.index();

    // The progress line (`h-0.5`), eased to the step.
    let bar = Rect::from_min_size(r.min, vec2(r.width(), 2.0));
    let target = (idx + 1) as f32 / total as f32;
    let shown = if reduced_motion(ui.ctx()) {
        target
    } else {
        ui.ctx()
            .animate_value_with_time(Id::unique("dzch.setup.progress"), target, motion::SLOW)
    };
    let p = ui.painter();
    p.rect_filled(bar, 0, tk.raised);
    p.rect_filled(
        Rect::from_min_size(bar.min, vec2(bar.width() * shown.clamp(0.0, 1.0), 2.0)),
        0,
        tk.accent,
    );

    // The footer.
    let foot = Rect::from_min_max(
        pos2(r.left(), (r.bottom() - FOOTER_H).max(bar.bottom())),
        r.max,
    );
    p.rect_filled(foot, 0, tk.panel);
    p.hline(
        foot.x_range(),
        foot.top() + 0.5,
        Stroke::new(1.0, tk.border),
    );
    footer(ui, cx, w, foot.shrink2(vec2(COLUMN_PAD, 12.0)), acts);

    // The step in hand.
    let body = Rect::from_min_max(pos2(r.left(), bar.bottom()), pos2(r.right(), foot.top()));
    let mut bui = ui.new_child(
        UiBuilder::new()
            .id_salt("dzch.setup.body")
            .max_rect(body)
            .layout(Layout::top_down(Align::Min)),
    );
    bui.set_clip_rect(body.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical()
        .id_salt(("dzch.setup.scroll", w.step.id()))
        .auto_shrink([false, false])
        .show(&mut bui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let avail = ui.available_width();
            let col = avail.min(COLUMN_W);
            let inner = (col - 2.0 * COLUMN_PAD).max(120.0);
            let left = ui.cursor().left() + (avail - col) / 2.0 + COLUMN_PAD;
            let top = ui.cursor().top() + 40.0;
            let mut c = ui.new_child(
                UiBuilder::new()
                    .id_salt("dzch.setup.column")
                    .max_rect(Rect::from_min_size(
                        pos2(left, top),
                        vec2(inner, f32::INFINITY),
                    ))
                    .layout(Layout::top_down(Align::Min)),
            );
            c.spacing_mut().item_spacing = Vec2::ZERO;
            c.set_width(inner);
            header(&mut c, w);
            c.add_space(28.0 + (1.0 - rise) * RISE_PX);
            c.scope(|ui| {
                if rise < 1.0 {
                    ui.set_opacity(rise);
                }
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                match w.step {
                    Step::Welcome => welcome(ui, cx, acts),
                    Step::Game => steps::game(ui, w, acts),
                    Step::Downloads => steps::downloads(ui, w, acts),
                    Step::Account => steps::account(ui, w),
                    Step::Services => steps::services(ui, w),
                    Step::Appearance => appearance(ui, cx, acts),
                    Step::Done => done(ui, cx, w),
                }
            });
            let h = c.min_rect().bottom() - ui.cursor().top();
            ui.allocate_space(vec2(avail, h.max(0.0) + 32.0));
        });
}

/// `Step N of 7`, the title, what the step is for.
fn header(ui: &mut Ui, w: &Wizard) {
    let tk = ui.tokens();
    let width = ui.available_width();
    let n = w.index() + 1;
    let total = Step::ALL.len();
    let label = tf("setup", "stepOf", &[("n", &n), ("total", &total)]);
    let g = crate::kit::panel::stencil(ui, &label, Color32::PLACEHOLDER, Some(width));
    let (r, _) = ui.allocate_exact_size(vec2(width, size::TEXT_2XS), Sense::hover());
    draw(ui.painter(), &g, r.left(), r.center().y, tk.accent);

    ui.add_space(8.0);
    let title = steps::display(ui, title(w), size::TEXT_3XL, width);
    let (r, _) = ui.allocate_exact_size(vec2(width, title.size().y), Sense::hover());
    // egui sets the capitals lower in a tight line than the browser does.
    ui.painter().galley(r.min - vec2(0.0, 6.0), title, tk.fg);

    ui.add_space(10.0);
    let d = para(
        ui,
        &description(w),
        font(Face::Sans, size::TEXT_SM),
        size::TEXT_SM * 1.625,
        width.min(576.0),
    );
    let (r, _) = ui.allocate_exact_size(vec2(width, d.size().y), Sense::hover());
    ui.painter().galley(r.min, d, tk.fg_muted);
}

fn title(w: &Wizard) -> &'static str {
    let key = match w.step {
        Step::Welcome => "welcomeTitle",
        Step::Game => "gameTitle",
        Step::Downloads => "downloadsTitle",
        Step::Account => "configTitle",
        Step::Services => "stepServices",
        Step::Appearance => "appearanceTitle",
        Step::Done => "doneTitle",
    };
    t("setup", key)
}

fn description(w: &Wizard) -> String {
    let key = match w.step {
        Step::Welcome => "welcomeSubtitle",
        Step::Game => "gameDesc",
        Step::Downloads => "downloadsDesc",
        Step::Account if w.via_steam() => "accountDescSteam",
        Step::Account => "accountDesc",
        Step::Services => "servicesDesc",
        Step::Appearance => "appearanceDesc",
        Step::Done => {
            return tf("setup", "doneDesc", &[("button", &t("setup", "goServers"))]);
        }
    };
    t("setup", key).to_string()
}

/// Back, `n / 7`, and the way forward (or, on the last step, the ways out).
fn footer(ui: &mut Ui, cx: &Ctx, w: &Wizard, r: Rect, acts: &mut Acts) {
    let tk = ui.tokens();
    let mut fu = ui.new_child(
        UiBuilder::new()
            .id_salt("dzch.setup.footer")
            .max_rect(r)
            .layout(Layout::left_to_right(Align::Center)),
    );
    fu.spacing_mut().item_spacing.x = 8.0;
    if w.index() > 0 {
        let back = Button::new(t("setup", "back"))
            .icon(lucide::ARROW_LEFT)
            .variant(Variant::Ghost);
        if fu.add(back).clicked() {
            acts.push(("setup.back", Value::Null));
        }
    }
    fu.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if w.step == Step::Done {
            let go = Button::new(t("setup", "goServers"))
                .icon(lucide::PLAY)
                .variant(Variant::Play)
                .size(Size::Lg);
            if ui.add(go).clicked() {
                acts.push(("setup.finish", json!({ "view": "servers" })));
            }
            if !cx.stores.profile.favorites.is_empty() {
                let fav = Button::new(t("setup", "goFavorites"))
                    .icon(lucide::HEART)
                    .variant(Variant::Ghost);
                if ui.add(fav).clicked() {
                    acts.push(("setup.finish", json!({ "view": "favorites" })));
                }
            }
            let mods = Button::new(t("setup", "goMods"))
                .icon(lucide::PUZZLE)
                .variant(Variant::Ghost);
            if ui.add(mods).clicked() {
                acts.push(("setup.finish", json!({ "view": "mods" })));
            }
        } else {
            let label = if w.step == Step::Welcome {
                t("setup", "getStarted")
            } else {
                t("setup", "next")
            };
            let next = Button::new(label)
                .trailing(lucide::ARROW_RIGHT)
                .variant(Variant::Accent)
                .size(Size::Lg)
                .enabled(w.can_advance());
            if ui.add(next).clicked() {
                acts.push(("setup.next", Value::Null));
            }
        }
        let n = format!("{} / {}", w.index() + 1, Step::ALL.len());
        let g = paint::galley(ui, &n, font(Face::Mono, size::TEXT_3XS), tk.fg_faint);
        let (cr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(cr.min, g, tk.fg_faint);
    });
}

// ── welcome ──────────────────────────────────────────────────────────────────

fn welcome(ui: &mut Ui, cx: &Ctx, acts: &mut Acts) {
    let tk = ui.tokens();
    let width = ui.available_width();
    let feats: [(Icon, &str, Color32); 4] = [
        (lucide::SERVER, "welcomeFeat1", tk.info),
        (lucide::PUZZLE, "welcomeFeat2", tk.mods),
        (lucide::STAR, "welcomeFeat3", tk.warn),
        (lucide::PLAY, "welcomeFeat4", tk.accent),
    ];
    // `grid grid-cols-2 gap-2`: cards of a row as tall as the tallest.
    let gap = 8.0;
    let cw = ((width - gap) / 2.0).max(60.0);
    let text_w = (cw - 24.0 - 32.0 - 12.0).max(20.0);
    let f = font(Face::Sans, size::TEXT_XS);
    for pair in feats.chunks(2) {
        let galleys: Vec<_> = pair
            .iter()
            .map(|(_, k, _)| para(ui, t("setup", k), f.clone(), size::TEXT_XS * 1.375, text_w))
            .collect();
        let inner = galleys.iter().map(|g| g.size().y).fold(32.0_f32, f32::max);
        let (row, _) = ui.allocate_exact_size(vec2(width, inner + 24.0), Sense::hover());
        for (k, ((icon, _, tone), g)) in pair.iter().zip(galleys).enumerate() {
            let card = Rect::from_min_size(
                pos2(row.left() + k as f32 * (cw + gap), row.top()),
                vec2(cw, row.height()),
            );
            let p = ui.painter();
            paint::plate(p, card, size::RADIUS_MD, tk.panel, tk.border);
            // The tile and the words on the card's centre line, whatever the row's height.
            let tile = Rect::from_min_size(
                pos2(card.left() + 12.0, card.center().y - 16.0),
                Vec2::splat(32.0),
            );
            p.rect_filled(tile, CornerRadius::same(size::RADIUS_SM), tk.raised);
            icon.paint(
                p,
                Rect::from_center_size(tile.center(), Vec2::splat(size::ICON)),
                *tone,
            );
            let y = card.center().y - g.size().y / 2.0;
            p.galley(pos2(tile.right() + 12.0, y), g, tk.fg);
        }
        ui.add_space(gap);
    }
    ui.add_space(12.0 - gap);
    SectionCard::new(t("setup", "language"))
        .icon(lucide::LANGUAGES)
        .padded()
        .show(
            ui,
            |_| {},
            |ui| {
                if let Some(l) = LanguagePicker::new(i18n::locale()).show(ui) {
                    acts.push(("i18n.set_locale", json!({ "locale": l.code() })));
                }
            },
        );
    let _ = cx;
    ui.add_space(16.0);
    steps::note_text(ui, t("setup", "welcomeHint"), tk.fg_faint);
}

// ── appearance ───────────────────────────────────────────────────────────────

fn appearance(ui: &mut Ui, cx: &Ctx, acts: &mut Acts) {
    let selected = cx.stores.theme.selected(cx.prefs).map(str::to_string);
    let pair = &cx.prefs.get().theme.pair;
    let look = |id: &str, scheme: Scheme| {
        presets::by_id(id)
            .filter(|p| p.scheme == scheme)
            .map(|p| Tokens::derive(&p.raw, p.scheme))
            .unwrap_or_else(|| ui.tokens())
    };
    let system = (
        look(&pair.dark, Scheme::Dark),
        look(&pair.light, Scheme::Light),
    );
    SectionCard::new(t("setup", "theme"))
        .icon(lucide::PALETTE)
        .padded()
        .show(
            ui,
            |_| {},
            |ui| {
                let pick =
                    ThemeSwatches::new(selected.as_deref(), system, t("setup", "followSystem"))
                        .columns(5)
                        .show(ui);
                match pick {
                    Some(ThemePick::System) => acts.push(("theme.follow_system", Value::Null)),
                    Some(ThemePick::Preset(id)) => {
                        acts.push(("theme.use_preset", json!({ "id": id })));
                    }
                    None => {}
                }
            },
        );
}

/// The worn theme's own name.
fn theme_name(cx: &Ctx) -> String {
    match cx.stores.theme.selected(cx.prefs) {
        None => t("setup", "followSystem").to_string(),
        Some("custom") => cx
            .stores
            .theme
            .custom(cx.prefs)
            .map(|c| c.name.clone())
            .unwrap_or_default(),
        Some(id) => presets::by_id(id)
            .map(|p| crate::parts::preset_name(p).to_string())
            .unwrap_or_else(|| id.to_string()),
    }
}

// ── done ─────────────────────────────────────────────────────────────────────

/// One line of the summary.
struct Row {
    label: String,
    ok: bool,
    value: String,
    warn: bool,
}

fn done(ui: &mut Ui, cx: &Ctx, w: &Wizard) {
    let tk = ui.tokens();
    let width = ui.available_width();

    // What loaded while the steps were filled in.
    let total = cx.stores.servers.total;
    plate_box(ui, tk.panel, tk.border, 10.0, size::ICON_LG, |ui, r| {
        let cy = r.center().y;
        let ir = Rect::from_center_size(pos2(r.left() + 10.0, cy), Vec2::splat(size::ICON_LG));
        if total > 0 {
            lucide::SERVER.paint(ui.painter(), ir, tk.ok);
            let text = tf(
                "setup",
                "readyServers",
                &[("count", &format::num(f64::from(total)))],
            );
            let g = ph_trunc(
                ui,
                &text,
                font(Face::SansSemibold, size::TEXT_SM),
                r.width() - 32.0,
            );
            draw(ui.painter(), &g, ir.right() + 12.0, cy, tk.fg);
        } else {
            Spinner::paint(ui, ir, tk.accent);
            let g = ph_trunc(
                ui,
                t("setup", "readyLoading"),
                font(Face::Sans, size::TEXT_XS),
                r.width() - 32.0,
            );
            draw(ui.painter(), &g, ir.right() + 12.0, cy, tk.fg_muted);
        }
    });
    ui.add_space(12.0);

    let game_dir = w.game.as_ref().and_then(|g| g.dayz_dir.clone());
    let missing = t("setup", "missing");
    let not_set = t("setup", "notSet");
    let mut rows = vec![Row {
        label: "DayZ".to_string(),
        ok: game_dir.is_some(),
        warn: game_dir.is_none(),
        value: game_dir.unwrap_or_else(|| t("setup", "gameNotFound").to_string()),
    }];
    if w.via_steam() {
        let running = w.steam.as_ref().is_some_and(|s| s.steam_running);
        rows.push(Row {
            label: t("setup", "stepDownloads").to_string(),
            ok: running,
            warn: !running,
            value: t("setup", "dlSteam").to_string(),
        });
    } else {
        let path = if !w.steamcmd_path.is_empty() {
            w.steamcmd_path.clone()
        } else {
            w.status
                .as_ref()
                .and_then(|s| s.path.clone())
                .unwrap_or_else(|| missing.to_string())
        };
        rows.push(Row {
            label: t("setup", "stepDownloads").to_string(),
            ok: w.found(),
            warn: !w.found(),
            value: format!("SteamCMD · {path}"),
        });
    }
    if w.needs_login() {
        let has = !w.steam_login.trim().is_empty();
        rows.push(Row {
            label: t("setup", "username").to_string(),
            ok: has,
            warn: !has,
            value: if w.steam_login.is_empty() {
                missing.to_string()
            } else {
                w.steam_login.clone()
            },
        });
    }
    rows.push(Row {
        label: t("setup", "ingameName").to_string(),
        ok: !w.player.trim().is_empty(),
        warn: false,
        value: if w.player.is_empty() {
            not_set.to_string()
        } else {
            w.player.clone()
        },
    });
    rows.push(Row {
        label: t("setup", "apiKey").to_string(),
        ok: !w.steam_api_key.trim().is_empty(),
        warn: false,
        value: if w.steam_api_key.is_empty() {
            not_set.to_string()
        } else {
            t("setup", "configured").to_string()
        },
    });
    rows.push(Row {
        label: t("setup", "language").to_string(),
        ok: true,
        warn: false,
        value: i18n::locale().label().to_string(),
    });
    rows.push(Row {
        label: t("setup", "theme").to_string(),
        ok: true,
        warn: false,
        value: theme_name(cx),
    });

    SectionCard::new(t("setup", "summary"))
        .icon(lucide::CIRCLE_CHECK)
        .tone(tk.ok)
        .show(
            ui,
            |_| {},
            |ui| {
                let n = rows.len();
                for (i, row) in rows.iter().enumerate() {
                    summary_row(ui, &tk, row, i + 1 == n);
                }
            },
        );
    ui.add_space(16.0);
    steps::note_text(ui, t("setup", "doneSettingsHint"), tk.fg_faint);
    ui.add_space(4.0);
    let about = tf("setup", "doneAboutHint", &[("tab", &t("nav", "about"))]);
    steps::note_text(ui, &about, tk.fg_faint);
    let _ = width;
}

/// `flex items-center gap-3 px-3 py-2`, a rule under all but the last.
fn summary_row(ui: &mut Ui, tk: &Tokens, row: &Row, last: bool) {
    let width = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(width, 8.0 + 16.0 + 8.0), Sense::hover());
    let p = ui.painter();
    let cy = r.center().y;
    let (icon, tone) = if row.warn {
        (lucide::TRIANGLE_ALERT, tk.warn)
    } else if row.ok {
        (lucide::CIRCLE_CHECK, tk.ok)
    } else {
        (lucide::CIRCLE_DASHED, tk.fg_faint)
    };
    let ir = Rect::from_min_size(pos2(r.left() + 12.0, cy - 7.0), Vec2::splat(size::ICON_SM));
    icon.paint(p, ir, tone);
    let lx = ir.right() + 12.0;
    let lg = ph_trunc(ui, &row.label, font(Face::Sans, size::TEXT_XS), 160.0);
    draw(p, &lg, lx, cy, tk.fg_muted);
    let vx = lx + 160.0 + 12.0;
    let vg = ph_trunc(
        ui,
        &row.value,
        font(Face::Mono, size::TEXT_2XS),
        (r.right() - 12.0 - vx).max(8.0),
    );
    let ink = if row.ok {
        tk.fg
    } else if row.warn {
        tk.warn
    } else {
        tk.fg_faint
    };
    draw(p, &vg, vx, cy, ink);
    if !last {
        p.hline(
            r.x_range(),
            r.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.5)),
        );
    }
}
