//! The seasonal effects over the window (`kit::season` draws them): winter's snow,
//! falling and resting on the status bar; Halloween's fog above it and the spider that
//! comes down the rail when the pointer lingers there (the title's candle glow is the
//! title bar's own).
//!
//! Cheap and polite: drawn on a layer of their own that takes no input, under the
//! dialogs and popups (dimmed by their veil like the rest). They move only while the
//! window has focus, is visible and DayZ is not running, at about 30 frames a second
//! (`kit::paint::animate`); with reduced motion they hold one still frame (no flakes
//! falling, the fog still, the candle steady, the spider up); the "Seasonal effects"
//! setting off leaves only the colours. A theme without effects asks for no frame.

use egui::{Context, Id, LayerId, Order, Pos2, Rect};

use crate::app::{Ctx, reduced_motion};
use crate::kit::paint;
use crate::kit::season::{self, Fx};
use crate::theme::{Ambient, TokensExt};

/// How long the pointer stays over the rail before the spider comes down.
const SPIDER_WAIT: f64 = 0.5;
/// How long it takes to come down, and to climb back.
const SPIDER_DOWN: f32 = 1.6;
const SPIDER_UP: f32 = 1.0;
/// How far it comes down.
const SPIDER_DROP: f32 = 110.0;

/// The window's regions this frame, as the panels took them.
#[derive(Clone, Copy, Debug)]
pub struct Regions {
    pub window: Rect,
    pub rail: Rect,
    pub status: Rect,
    /// The pad hints' bar, in pad mode.
    pub hints: Option<Rect>,
}

/// Before the regions are drawn: whether the effects show and move this frame, and the
/// effect clock stepped.
pub fn begin(ctx: &Context, cx: &Ctx) {
    let mut f = season::fx(ctx);
    let before = f;
    let now = ctx.input(|i| i.time);
    f.on = f.ambient != Ambient::None
        && cx.prefs.get().ui.seasonal_effects
        && cx.stores.app.initialized
        && !super::wizard_covers(cx);
    let (focused, visible) = ctx.input(|i| {
        let v = i.viewport();
        (v.focused.unwrap_or(i.focused), v.minimized != Some(true))
    });
    f.still = reduced_motion(ctx);
    f.moving = f.on && !f.still && focused && visible && !cx.stores.game.running;
    if f.moving {
        f.t += (now - f.last).clamp(0.0, 0.1);
    }
    f.last = now;
    if f.still || !f.on {
        f.drop = 0.0;
        f.hover_since = None;
    }
    if f != before {
        season::set_fx(ctx, f);
    }
    if f.moving {
        paint::animate(ctx);
    }
}

/// After the regions: the effects over them.
pub fn paint(ctx: &Context, r: Regions) {
    let mut f = season::fx(ctx);
    if !f.on {
        return;
    }
    let tk = ctx.tokens();
    let layer = LayerId::new(Order::Middle, Id::new("dzch.season"));
    let p = ctx.layer_painter(layer).with_clip_rect(r.window);
    // The edge things rest on: the status bar's, or in pad mode the pad hints' beside
    // the rail (snow on the hints' glyphs would cover them).
    let floor: Vec<(egui::Rangef, f32)> = match r.hints {
        Some(h) => vec![
            (egui::Rangef::new(r.window.left(), h.left()), r.status.top()),
            (h.x_range(), h.top()),
        ],
        None => vec![(r.window.x_range(), r.status.top())],
    };
    match f.ambient {
        Ambient::Winter => {
            for (xs, y) in &floor {
                season::snow_cap(&p, *xs, *y, &tk);
            }
            if !f.still {
                // Down to the snow on the bars, never past it.
                let field =
                    Rect::from_min_max(r.window.min, Pos2::new(r.window.right(), r.status.top()));
                let rest = |x: f32| {
                    floor
                        .iter()
                        .find(|(xs, _)| xs.contains(x))
                        .map_or(r.status.top(), |(_, y)| *y)
                };
                season::flakes(&p, field, f.t, &tk, rest);
            }
        }
        Ambient::Halloween => {
            let bottom = r.hints.map_or(r.status.top(), |h| h.top());
            let h = (r.window.height() * 0.34).min(260.0);
            let band = Rect::from_min_max(
                Pos2::new(r.window.left(), bottom - h),
                Pos2::new(r.window.right(), bottom),
            );
            season::fog(&p, band, f.t, &tk);
            // Cobwebs in the view's header corners, under the title bar (not in the rail).
            let top = r.rail.top();
            season::cobweb(
                &p,
                Pos2::new(r.rail.right(), top),
                56.0,
                egui::vec2(1.0, 1.0),
                &tk,
            );
            season::cobweb(
                &p,
                Pos2::new(r.window.right(), top),
                70.0,
                egui::vec2(-1.0, 1.0),
                &tk,
            );
            spider(ctx, &p, &mut f, r.rail, &tk);
            season::set_fx(ctx, f);
        }
        Ambient::None => {}
    }
}

/// Behind the title's words (`size` from `x`, centred on `cy`): Halloween's candle.
pub fn title(ui: &egui::Ui, x: f32, cy: f32, size: egui::Vec2) {
    let f = season::fx(ui.ctx());
    if f.on && f.ambient == Ambient::Halloween {
        let r = Rect::from_min_size(Pos2::new(x, cy - size.y / 2.0), size);
        season::candle(ui.painter(), r, &f, &ui.tokens());
    }
}

/// The spider at the rail's top right: down after the pointer has lingered anywhere over
/// the rail (its ground as much as its entries), back up when it leaves.
fn spider(ctx: &Context, p: &egui::Painter, f: &mut Fx, rail: Rect, tk: &crate::theme::Tokens) {
    let now = f.last;
    if f.moving {
        let over = ctx
            .pointer_hover_pos()
            .is_some_and(|pos| rail.contains(pos));
        if over {
            let since = *f.hover_since.get_or_insert(now);
            let waited = now - since;
            if waited < SPIDER_WAIT {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                    (SPIDER_WAIT - waited).max(0.01),
                ));
            }
        } else {
            f.hover_since = None;
        }
        let down = f.hover_since.is_some_and(|s| now - s >= SPIDER_WAIT);
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        f.drop = if down {
            (f.drop + dt / SPIDER_DOWN).min(1.0)
        } else {
            (f.drop - dt / SPIDER_UP).max(0.0)
        };
    }
    let eased = 1.0 - (1.0 - f.drop).powi(3);
    let drop = eased * SPIDER_DROP.min((rail.height() - 40.0).max(0.0));
    let sway = if f.still {
        0.0
    } else {
        (f.t * 1.3).sin() as f32 * 1.5 * eased
    };
    // Whole inside the rail, at its edge: beside the places' counts, not over them.
    let top = Pos2::new(rail.right() - 11.0, rail.top());
    season::spider(p, top, drop, sway, tk);
}
