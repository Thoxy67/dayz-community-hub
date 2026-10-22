//! `ui/toast`: things that happened, said briefly and then gone.
//!
//! Anyone with the `Context` says something (`toast::ok(ctx, "Saved")`); the window draws
//! them once with [`host`]. The newest four, at the window's bottom right (`right-3
//! bottom-9`, 20rem wide), each on the floating plate with a 2px edge in its tone, its
//! icon, the words, an optional action and a dismiss button. Four seconds, seven with an
//! action. They slide in from the right edge (8px, `--duration-base`), leave the same way
//! (160ms), and the others close the gap rather than jump.
//!
//! Never the same thing twice: saying what a toast on screen already says (same words,
//! tone and button) keeps that one up for longer instead. Steps of one story (a launch:
//! starting, waiting for Steam, started) share a topic ([`say_topic`]): each replaces
//! the one before in the same toast.

use std::sync::Arc;

use egui::{Color32, Context, CornerRadius, Id, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::paint;
use crate::icons::{Icon, lucide};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, motion, size};

/// A toast's tone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastTone {
    Ok,
    Warn,
    Err,
    #[default]
    Neutral,
}

impl ToastTone {
    fn look(self, t: &Tokens) -> (Icon, Color32, Color32) {
        match self {
            ToastTone::Ok => (lucide::CHECK, t.ok, t.ok),
            ToastTone::Warn => (lucide::TRIANGLE_ALERT, t.warn, t.warn),
            ToastTone::Err => (lucide::CIRCLE_X, t.err, t.err),
            ToastTone::Neutral => (lucide::INFO, t.fg_muted, t.border_strong),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Toast {
    id: u64,
    tone: ToastTone,
    text: String,
    /// (label, command id) of the button on it.
    action: Option<(String, String)>,
    /// Steps of one story replace each other ([`say_topic`]).
    topic: Option<&'static str>,
    born: f64,
    /// When its time on screen started: its birth, or the last time it was said again.
    since: f64,
    leaving: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Toasts {
    pub(crate) list: Vec<Toast>,
    next: u64,
}

const LIFE: f64 = 4.0;
const LIFE_ACTION: f64 = 7.0;
const OUT: f64 = 0.16;
const KEEP: usize = 4;

pub(crate) fn store_id() -> Id {
    Id::new("kit::toasts")
}

/// Says something. Returns its id, for [`dismiss`].
pub fn say(ctx: &Context, text: impl Into<String>, tone: ToastTone) -> u64 {
    push(ctx, text.into(), tone, None, None)
}

/// Says the next step of a story: replaces the toast on screen with the same `topic`.
pub fn say_topic(
    ctx: &Context,
    topic: &'static str,
    text: impl Into<String>,
    tone: ToastTone,
) -> u64 {
    push(ctx, text.into(), tone, None, Some(topic))
}

/// Says something with a button on it; pressing it makes [`host`] return `command`
/// (a command id from the registry).
pub fn say_with_action(
    ctx: &Context,
    text: impl Into<String>,
    tone: ToastTone,
    label: impl Into<String>,
    command: impl Into<String>,
) -> u64 {
    push(
        ctx,
        text.into(),
        tone,
        Some((label.into(), command.into())),
        None,
    )
}

pub fn ok(ctx: &Context, text: impl Into<String>) -> u64 {
    say(ctx, text, ToastTone::Ok)
}

pub fn warn(ctx: &Context, text: impl Into<String>) -> u64 {
    say(ctx, text, ToastTone::Warn)
}

pub fn err(ctx: &Context, text: impl Into<String>) -> u64 {
    say(ctx, text, ToastTone::Err)
}

/// Takes a toast down (it slides out).
pub fn dismiss(ctx: &Context, id: u64) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Toasts>(store_id());
        if let Some(t) = s
            .list
            .iter_mut()
            .find(|t| t.id == id && t.leaving.is_none())
        {
            t.leaving = Some(now);
        }
    });
    ctx.request_repaint();
}

fn push(
    ctx: &Context,
    text: String,
    tone: ToastTone,
    action: Option<(String, String)>,
    topic: Option<&'static str>,
) -> u64 {
    let now = ctx.input(|i| i.time);
    let id = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Toasts>(store_id());
        // Already said (or the same story): that toast says it, from now.
        if let Some(t) = s.list.iter_mut().find(|t| {
            t.leaving.is_none()
                && (topic.is_some() && t.topic == topic
                    || (t.text == text && t.tone == tone && t.action == action))
        }) {
            t.text = text;
            t.tone = tone;
            t.action = action;
            t.since = now;
            return t.id;
        }
        s.next = s.next.wrapping_add(1);
        let id = s.next;
        // The newest four: a burst should not paper over the window.
        let live = s.list.iter().filter(|t| t.leaving.is_none()).count();
        if live >= KEEP {
            let mut extra = live + 1 - KEEP;
            for t in s.list.iter_mut().filter(|t| t.leaving.is_none()) {
                if extra == 0 {
                    break;
                }
                t.leaving = Some(now - OUT);
                extra -= 1;
            }
        }
        s.list.push(Toast {
            id,
            tone,
            text,
            action,
            topic,
            born: now,
            since: now,
            leaving: None,
        });
        id
    });
    ctx.request_repaint();
    id
}

/// Draws the toasts; call once per frame, last, from the window. Returns the command id
/// of an action pressed this frame.
pub fn host(ctx: &Context, dismiss_label: &str) -> Option<String> {
    let now = ctx.input(|i| i.time);
    let list: Vec<Toast> = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Toasts>(store_id());
        for t in &mut s.list {
            let life = if t.action.is_some() {
                LIFE_ACTION
            } else {
                LIFE
            };
            if t.leaving.is_none() && now - t.since >= life {
                t.leaving = Some(now);
            }
        }
        s.list.retain(|t| t.leaving.is_none_or(|l| now - l < OUT));
        s.list.clone()
    });
    if list.is_empty() {
        return None;
    }
    let t = ctx.tokens();
    let screen = ctx.content_rect();
    let w = 320.0_f32.min(screen.width() - 24.0).max(120.0);
    let text_w = w - 8.0 - 6.0 - size::ICON - 8.0 - 8.0 - 16.0;
    let mut fired = None;
    let mut next_wake = f64::INFINITY;
    let mut bottom = screen.bottom() - 36.0;
    for toast in list.iter().rev() {
        let id = Id::new(("kit::toast", toast.id));
        let action_w = toast.action.as_ref().map_or(0.0, |(l, _)| {
            ctx.fonts_mut(|f| {
                f.layout_no_wrap(l.clone(), font(Face::SansMedium, size::TEXT_2XS), t.accent)
                    .size()
                    .x
            }) + 12.0
                + 8.0
        });
        let g = ctx.fonts_mut(|f| {
            f.layout(
                toast.text.clone(),
                font(Face::Sans, size::TEXT_XS),
                t.fg,
                (text_w - action_w).max(40.0),
            )
        });
        let h = g.size().y.max(size::ICON + 2.0) + 12.0;
        let target = bottom - h;
        let y = ctx.animate_value_with_time(id.with("y"), target, motion::BASE);
        bottom = target - 6.0;
        let (alpha, dx) = match toast.leaving {
            Some(l) => {
                let k = ((now - l) / OUT).clamp(0.0, 1.0) as f32;
                (1.0 - k, 8.0 * k)
            }
            None => {
                let k = ((now - toast.born) / f64::from(motion::BASE)).clamp(0.0, 1.0) as f32;
                let e = egui::emath::easing::cubic_out(k);
                (e, 8.0 * (1.0 - e))
            }
        };
        let life = if toast.action.is_some() {
            LIFE_ACTION
        } else {
            LIFE
        };
        next_wake = next_wake.min(match toast.leaving {
            Some(_) => 0.0,
            None if now - toast.born < f64::from(motion::BASE) => 0.0,
            None => toast.since + life - now,
        });
        let rect = Rect::from_min_size(
            egui::pos2(screen.right() - 12.0 - w + dx, y),
            Vec2::new(w, h),
        );
        let g = Arc::clone(&g);
        egui::Area::new(id)
            .order(egui::Order::Tooltip)
            .fixed_pos(rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                ui.multiply_opacity(alpha);
                let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
                let (icon, ink, edge) = toast.tone.look(&t);
                let p = ui.painter();
                p.add(paint::shadow_pop().as_shape(r, CornerRadius::same(size::RADIUS_MD)));
                p.rect(
                    r,
                    CornerRadius::same(size::RADIUS_MD),
                    t.overlay,
                    Stroke::new(1.0, t.border),
                    StrokeKind::Inside,
                );
                // The 2px edge in the tone, on the left.
                let edge_r = Rect::from_min_max(r.min, egui::pos2(r.left() + 2.0, r.bottom()));
                p.rect_filled(
                    edge_r,
                    CornerRadius {
                        nw: size::RADIUS_MD,
                        sw: size::RADIUS_MD,
                        ne: 0,
                        se: 0,
                    },
                    edge,
                );
                let top = r.top() + 6.0;
                icon.paint(
                    p,
                    Rect::from_min_size(
                        egui::pos2(r.left() + 8.0, top + 2.0),
                        Vec2::splat(size::ICON),
                    ),
                    ink,
                );
                let tx = r.left() + 8.0 + size::ICON + 8.0;
                p.galley(egui::pos2(tx, top + 1.0), Arc::clone(&g), t.fg);
                // Dismiss.
                let xr = Rect::from_min_size(
                    egui::pos2(r.right() - 6.0 - 16.0, top + 2.0),
                    Vec2::splat(16.0),
                );
                let xb = ui.interact(xr, id.with("x"), Sense::click());
                xb.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, dismiss_label)
                });
                let hx = paint::ease(ui, xb.id, xb.hovered());
                let p = ui.painter();
                p.rect_filled(
                    xr,
                    CornerRadius::same(size::RADIUS_XS),
                    paint::lerp(Color32::TRANSPARENT, t.raised, hx),
                );
                lucide::X.paint(
                    p,
                    Rect::from_center_size(xr.center(), Vec2::splat(size::ICON_SM)),
                    paint::lerp(t.fg_faint, t.fg, hx),
                );
                if xb.clicked() {
                    dismiss(ui.ctx(), toast.id);
                }
                if let Some((label, cmd)) = &toast.action {
                    let lg =
                        paint::galley(ui, label, font(Face::SansMedium, size::TEXT_2XS), t.accent);
                    let ar = Rect::from_min_size(
                        egui::pos2(xr.left() - 8.0 - lg.size().x - 12.0, top),
                        Vec2::new(lg.size().x + 12.0, 20.0),
                    );
                    let ab = ui.interact(ar, id.with("act"), Sense::click());
                    let ha = paint::ease(ui, ab.id, ab.hovered());
                    let p = ui.painter();
                    p.rect_filled(
                        ar,
                        CornerRadius::same(size::RADIUS_SM),
                        paint::lerp(Color32::TRANSPARENT, t.raised, ha),
                    );
                    paint::text_center(p, &lg, ar, t.accent);
                    if ab.clicked() {
                        fired = Some(cmd.clone());
                        dismiss(ui.ctx(), toast.id);
                    }
                }
            });
    }
    if next_wake <= 0.0 {
        ctx.request_repaint();
    } else if next_wake.is_finite() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(next_wake.min(60.0)));
    }
    fired
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(ctx: &Context) -> Vec<String> {
        ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Toasts>(store_id())
                .list
                .iter()
                .filter(|t| t.leaving.is_none())
                .map(|t| t.text.clone())
                .collect()
        })
    }

    #[test]
    fn the_same_thing_is_said_once_and_a_story_in_one_toast() {
        let ctx = Context::default();
        ok(&ctx, "Copied");
        ok(&ctx, "Copied");
        err(&ctx, "Copied");
        assert_eq!(
            live(&ctx),
            ["Copied", "Copied"],
            "same words, other tone: two"
        );
        say_topic(&ctx, "launch", "Starting", ToastTone::Neutral);
        say_topic(&ctx, "launch", "Waiting for Steam", ToastTone::Neutral);
        say_topic(&ctx, "launch", "Started", ToastTone::Ok);
        assert_eq!(live(&ctx), ["Copied", "Copied", "Started"]);
    }
}
