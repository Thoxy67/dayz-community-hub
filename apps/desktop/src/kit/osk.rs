//! The on-screen keyboard: a controller types into a text field without a keyboard
//! (a Steam Deck outside Steam's big-screen interface, a pad on a desktop).
//!
//! [`open`] it on the focused field (`crate::pad` does when A picks a text field, the
//! setting says the built-in keyboard); it stays at the foot of the window (at the top
//! when the field is down there) until [`Press::Done`] or [`Press::Close`], the field
//! losing the focus, or a press outside it. The pad drives it ([`press`]): the
//! directions walk the keys, A types the one under the cursor, X erases, Y is Shift
//! (for one letter), LB/RB move the caret. A pointer or a finger types on it too.
//!
//! What it types reaches the field as the keys a keyboard would send, at the start of
//! the next frame ([`begin_frame`]), so any `TextEdit` takes it unchanged. While it is
//! up, a press elsewhere does not take the focus from the field (egui's
//! `surrender_focus_on`, put back when it closes).
//!
//! | for | call |
//! |---|---|
//! | the layout of a language, or digits | [`Layout::for_locale`], [`Layout::Digits`] |
//! | open, close, ask | [`open`], [`close`], [`is_open`] |
//! | the pad's buttons | [`press`] |
//! | each frame | [`begin_frame`] before the views, [`show`] after them |

use egui::{
    Align2, Context, CornerRadius, Event, Id, Key as EKey, Modifiers, Order, Rect, Sense, Stroke,
    SurrenderFocusOn, Ui, Vec2,
};

use super::button::{Button, Size, Variant};
use super::paint;
use crate::icons::{Icon, lucide};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// The keys' arrangement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Qwerty,
    /// German.
    Qwertz,
    /// French.
    Azerty,
    /// A number field's.
    Digits,
}

impl Layout {
    /// The letters of the interface's language (`"fr"`, `"de"`…): AZERTY in French,
    /// QWERTZ in German, else QWERTY.
    pub fn for_locale(code: &str) -> Self {
        match code {
            "fr" => Layout::Azerty,
            "de" => Layout::Qwertz,
            _ => Layout::Qwerty,
        }
    }
}

/// A key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum K {
    /// Types its text (the second when Shift is on).
    Ch(&'static str, &'static str),
    Shift,
    Back,
    Space,
    Left,
    Right,
}

impl K {
    /// How wide, in key widths.
    fn units(self) -> f32 {
        match self {
            K::Space => 3.0,
            K::Shift | K::Back => 1.5,
            _ => 1.0,
        }
    }
}

type Row = Vec<K>;

fn chars(lower: &'static str, upper: &'static str) -> Row {
    // Both strings are ASCII of the same length (the layouts below).
    lower
        .char_indices()
        .filter_map(|(i, _)| Some(K::Ch(lower.get(i..=i)?, upper.get(i..=i)?)))
        .collect()
}

fn rows(layout: Layout) -> Vec<Row> {
    let digits = chars("1234567890", "!?#&()+='\"");
    let foot = vec![
        K::Left,
        K::Ch(".", ","),
        K::Ch("-", "_"),
        K::Ch("@", "%"),
        K::Space,
        K::Ch("/", "\\"),
        K::Ch(":", ";"),
        K::Right,
    ];
    let with_shift = |letters: Row| {
        let mut r = vec![K::Shift];
        r.extend(letters);
        r.push(K::Back);
        r
    };
    match layout {
        Layout::Qwerty => vec![
            digits,
            chars("qwertyuiop", "QWERTYUIOP"),
            chars("asdfghjkl", "ASDFGHJKL"),
            with_shift(chars("zxcvbnm", "ZXCVBNM")),
            foot,
        ],
        Layout::Qwertz => vec![
            digits,
            chars("qwertzuiop", "QWERTZUIOP"),
            chars("asdfghjkl", "ASDFGHJKL"),
            with_shift(chars("yxcvbnm", "YXCVBNM")),
            foot,
        ],
        Layout::Azerty => vec![
            digits,
            chars("azertyuiop", "AZERTYUIOP"),
            chars("qsdfghjklm", "QSDFGHJKLM"),
            with_shift(chars("wxcvbn", "WXCVBN")),
            foot,
        ],
        Layout::Digits => vec![
            chars("789", "789"),
            chars("456", "456"),
            chars("123", "123"),
            vec![K::Ch(".", "."), K::Ch("0", "0"), K::Ch("-", "-")],
            vec![K::Left, K::Back, K::Right],
        ],
    }
}

fn row_units(r: &Row) -> f32 {
    r.iter().map(|k| k.units()).sum()
}

fn widest(rows: &[Row]) -> f32 {
    rows.iter().map(row_units).fold(1.0, f32::max)
}

/// The centre of key `col` of `row`, in key widths from the left of the widest row
/// (rows are centred).
fn center_x(rows: &[Row], row: usize, col: usize) -> f32 {
    let Some(r) = rows.get(row) else {
        return 0.0;
    };
    let mut x = (widest(rows) - row_units(r)) / 2.0;
    for (i, k) in r.iter().enumerate() {
        if i == col {
            return x + k.units() / 2.0;
        }
        x += k.units();
    }
    x
}

/// The key of `row` nearest `x` (key widths); between two, the right one (the rows of
/// a keyboard stagger to the right going down).
fn nearest(rows: &[Row], row: usize, x: f32) -> usize {
    let n = rows.get(row).map_or(0, Vec::len);
    (0..n)
        .rev()
        .min_by(|a, b| {
            (center_x(rows, row, *a) - x)
                .abs()
                .total_cmp(&(center_x(rows, row, *b) - x).abs())
        })
        .unwrap_or(0)
}

/// A button of the pad, for the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Up,
    Down,
    Left,
    Right,
    /// A: the key under the cursor.
    Type,
    /// X.
    Backspace,
    /// Y.
    Shift,
    /// LB.
    CaretLeft,
    /// RB.
    CaretRight,
    /// Menu: done typing.
    Done,
    /// B: put away.
    Close,
}

#[derive(Clone, Debug)]
struct State {
    target: Id,
    layout: Layout,
    shift: bool,
    row: usize,
    col: usize,
    /// What reaches the field at the start of the next frame.
    pending: Vec<Event>,
    /// egui's `surrender_focus_on` before it opened.
    surrender: SurrenderFocusOn,
}

fn state_id() -> Id {
    Id::unique("dzch.kit.osk")
}

fn load(ctx: &Context) -> Option<State> {
    ctx.data(|d| d.get_temp::<State>(state_id()))
}

fn save(ctx: &Context, s: State) {
    ctx.data_mut(|d| d.insert_temp(state_id(), s));
}

/// Whether it is up.
pub fn is_open(ctx: &Context) -> bool {
    load(ctx).is_some()
}

/// Open it on the text field `target` (which has the focus), on `layout`'s keys.
pub fn open(ctx: &Context, target: Id, layout: Layout) {
    let surrender = match load(ctx) {
        Some(s) => s.surrender,
        None => ctx.options(|o| o.input_options.surrender_focus_on),
    };
    ctx.options_mut(|o| o.input_options.surrender_focus_on = SurrenderFocusOn::Never);
    ctx.memory_mut(|m| m.request_focus(target));
    // On the digits' 5 or the letters' first row, where the thumb starts.
    let (row, col) = if layout == Layout::Digits {
        (1, 1)
    } else {
        (1, 0)
    };
    save(
        ctx,
        State {
            target,
            layout,
            shift: false,
            row,
            col,
            pending: Vec::new(),
            surrender,
        },
    );
    ctx.request_repaint();
}

/// Put it away (the field keeps the focus).
pub fn close(ctx: &Context) {
    if let Some(s) = load(ctx) {
        ctx.options_mut(|o| o.input_options.surrender_focus_on = s.surrender);
        ctx.data_mut(|d| d.remove::<State>(state_id()));
        ctx.request_repaint();
    }
}

fn key_event(k: EKey) -> [Event; 2] {
    [true, false].map(|pressed| Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    })
}

/// What key `k` sends (and Shift turned off after a letter).
fn hit(s: &mut State, k: K) {
    if s.pending.len() > 64 {
        return;
    }
    match k {
        K::Ch(lo, up) => {
            s.pending
                .push(Event::Text(if s.shift { up } else { lo }.to_string()));
            s.shift = false;
        }
        K::Space => s.pending.push(Event::Text(" ".to_string())),
        K::Shift => s.shift = !s.shift,
        K::Back => s.pending.extend(key_event(EKey::Backspace)),
        K::Left => s.pending.extend(key_event(EKey::ArrowLeft)),
        K::Right => s.pending.extend(key_event(EKey::ArrowRight)),
    }
}

/// One step of the pad's cursor or typing (tested without a window).
fn step(s: &mut State, p: Press) -> bool {
    let rows = rows(s.layout);
    let n_rows = rows.len();
    let len = |r: usize| rows.get(r).map_or(1, Vec::len).max(1);
    s.row = s.row.min(n_rows.saturating_sub(1));
    s.col = s.col.min(len(s.row) - 1);
    match p {
        Press::Up | Press::Down => {
            let x = center_x(&rows, s.row, s.col);
            s.row = if p == Press::Up {
                s.row.checked_sub(1).unwrap_or(n_rows.saturating_sub(1))
            } else if s.row + 1 >= n_rows {
                0
            } else {
                s.row + 1
            };
            s.col = nearest(&rows, s.row, x);
        }
        Press::Left => s.col = s.col.checked_sub(1).unwrap_or(len(s.row) - 1),
        Press::Right => s.col = (s.col + 1) % len(s.row),
        Press::Type => {
            if let Some(k) = rows.get(s.row).and_then(|r| r.get(s.col)).copied() {
                hit(s, k);
            }
        }
        Press::Backspace => hit(s, K::Back),
        Press::Shift => hit(s, K::Shift),
        Press::CaretLeft => hit(s, K::Left),
        Press::CaretRight => hit(s, K::Right),
        Press::Done | Press::Close => return false,
    }
    true
}

/// A pad button while it is up: false when it is not (the press is not its).
pub fn press(ctx: &Context, p: Press) -> bool {
    let Some(mut s) = load(ctx) else {
        return false;
    };
    if step(&mut s, p) {
        save(ctx, s);
    } else {
        close(ctx);
    }
    ctx.request_repaint();
    true
}

/// Before the views draw: what was typed reaches the field (which gets the focus back
/// if a press took it); the keyboard closes when its field is gone or another widget
/// took the focus.
pub fn begin_frame(ctx: &Context) {
    let Some(mut s) = load(ctx) else {
        return;
    };
    let drawn = ctx.viewport(|v| v.prev_pass.widgets.get(s.target).is_some());
    let focused = ctx.memory(|m| m.focused());
    if !drawn || focused.is_some_and(|f| f != s.target) {
        close(ctx);
        return;
    }
    if focused.is_none() {
        ctx.memory_mut(|m| m.request_focus(s.target));
    }
    if !s.pending.is_empty() {
        let events = std::mem::take(&mut s.pending);
        ctx.input_mut(|i| i.events.extend(events));
        save(ctx, s);
    }
}

/// The words it shows.
#[derive(Clone, Copy, Debug, Default)]
pub struct Labels<'a> {
    pub done: &'a str,
    pub shift: &'a str,
    pub backspace: &'a str,
    pub space: &'a str,
}

/// After the views: the keyboard, when it is up. `hints` draws the pad's buttons
/// beside the Done button (the window knows the pad in hand).
pub fn show(ctx: &Context, labels: &Labels<'_>, hints: impl FnOnce(&mut Ui)) {
    let Some(mut s) = load(ctx) else {
        return;
    };
    let t = ctx.tokens();
    let screen = ctx.content_rect();
    let rows = rows(s.layout);
    let units = widest(&rows);
    let gap = 4.0;
    let key_h = size::control_lg();
    let max_w = if s.layout == Layout::Digits {
        key_h * 2.4 * units + gap * units
    } else {
        720.0
    };
    let w = max_w.min(screen.width() - 2.0 * size::PAD).max(160.0);
    // Clear of the field: at the top when the field is in the lower half.
    let field = ctx.viewport(|v| v.prev_pass.widgets.get(s.target).map(|w| w.rect));
    let low = field.is_some_and(|r| r.center().y > screen.center().y);
    let (anchor, offset) = if low {
        (
            Align2::CENTER_TOP,
            Vec2::new(0.0, size::TITLEBAR + size::PAD),
        )
    } else {
        (Align2::CENTER_BOTTOM, Vec2::new(0.0, -size::PAD))
    };
    let mut clicked: Option<(usize, usize, K)> = None;
    let mut done = false;
    let area = egui::Area::new(state_id().with("area"))
        .order(Order::Tooltip)
        .anchor(anchor, offset)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(t.panel)
                .stroke(Stroke::new(1.0, t.border_strong))
                .corner_radius(CornerRadius::same(size::RADIUS_MD))
                .shadow(paint::shadow_pop())
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing = Vec2::new(gap, gap);
                    ui.horizontal(|ui| {
                        ui.set_height(size::control());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let b = ui.add(
                                Button::new(labels.done)
                                    .variant(Variant::Accent)
                                    .size(Size::Sm)
                                    .icon(lucide::CHECK),
                            );
                            if b.clicked() {
                                done = true;
                            }
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), hints);
                        });
                    });
                    let unit = ((w - gap * (units - 1.0)) / units).max(8.0);
                    let width = |k: K| k.units() * unit + (k.units() - 1.0).max(0.0) * gap;
                    for (ri, row) in rows.iter().enumerate() {
                        let rw: f32 = row.iter().map(|k| width(*k)).sum::<f32>()
                            + gap * row.len().saturating_sub(1) as f32;
                        let (line, _) = ui.allocate_exact_size(Vec2::new(w, key_h), Sense::hover());
                        let mut x = line.left() + (w - rw) / 2.0;
                        for (ci, k) in row.iter().enumerate() {
                            let kw = width(*k);
                            let r = Rect::from_min_size(
                                egui::pos2(x, line.top()),
                                Vec2::new(kw, key_h),
                            );
                            x += kw + gap;
                            let id = state_id().with((ri, ci));
                            let resp = ui.interact(r, id, Sense::CLICK);
                            if resp.clicked() {
                                clicked = Some((ri, ci, *k));
                            }
                            let name = match *k {
                                K::Ch(lo, up) => {
                                    if s.shift {
                                        up
                                    } else {
                                        lo
                                    }
                                }
                                K::Shift => labels.shift,
                                K::Back => labels.backspace,
                                K::Space => labels.space,
                                K::Left => "\u{2190}",
                                K::Right => "\u{2192}",
                            };
                            resp.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::Button, true, name)
                            });
                            let look = Look {
                                on: s.row == ri && s.col == ci,
                                lit: matches!(k, K::Shift) && s.shift,
                                hover: resp.hovered(),
                            };
                            key(ui, r, *k, s.shift, look);
                        }
                    }
                });
        });
    // Over everything, the pad's hints drawn again over a dialog's veil included.
    ctx.move_to_top(area.response.layer_id);
    // A press outside the keyboard puts it away (the field keeps the focus).
    let outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !area.response.rect.contains(p))
    });
    if done || outside {
        close(ctx);
        return;
    }
    if let Some((ri, ci, k)) = clicked {
        s.row = ri;
        s.col = ci;
        hit(&mut s, k);
        save(ctx, s);
        ctx.request_repaint();
    }
}

/// How a key is drawn: under the cursor, lit (Shift on), hovered.
#[derive(Clone, Copy)]
struct Look {
    on: bool,
    lit: bool,
    hover: bool,
}

fn key(ui: &Ui, r: Rect, k: K, shift: bool, look: Look) {
    let Look { on, lit, hover } = look;
    let t = ui.tokens();
    let p = ui.painter();
    let fill = if lit {
        paint::alpha(t.accent, 0.25)
    } else if hover {
        t.overlay
    } else {
        t.raised
    };
    paint::plate(p, r, size::RADIUS_SM, fill, t.border);
    if on {
        p.rect_stroke(
            r,
            CornerRadius::same(size::RADIUS_SM),
            Stroke::new(size::ring(), t.accent),
            egui::StrokeKind::Inside,
        );
    }
    let ink = if on { t.fg } else { t.fg_muted };
    let icon = |i: Icon| {
        i.paint(
            p,
            Rect::from_center_size(r.center(), Vec2::splat(size::ICON_LG)),
            ink,
        );
    };
    match k {
        K::Ch(lo, up) => {
            let g = paint::galley(
                ui,
                if shift { up } else { lo },
                font(Face::Sans, size::TEXT_LG),
                t.fg,
            );
            paint::text_center(p, &g, r, t.fg);
        }
        K::Shift => icon(lucide::ARROW_BIG_UP),
        K::Back => icon(lucide::DELETE),
        K::Space => icon(lucide::SPACE),
        K::Left => icon(lucide::ARROW_LEFT),
        K::Right => icon(lucide::ARROW_RIGHT),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn state(layout: Layout) -> State {
        State {
            target: Id::unique("field"),
            layout,
            shift: false,
            row: 1,
            col: 0,
            pending: Vec::new(),
            surrender: SurrenderFocusOn::Clicks,
        }
    }

    fn typed(s: &State) -> String {
        s.pending
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn layouts_follow_the_language() {
        assert_eq!(Layout::for_locale("fr"), Layout::Azerty);
        assert_eq!(Layout::for_locale("de"), Layout::Qwertz);
        assert_eq!(Layout::for_locale("ru"), Layout::Qwerty);
        let r = rows(Layout::Azerty);
        assert_eq!(r.get(1).and_then(|r| r.first()), Some(&K::Ch("a", "A")));
        for l in [
            Layout::Qwerty,
            Layout::Qwertz,
            Layout::Azerty,
            Layout::Digits,
        ] {
            for row in rows(l) {
                assert!(!row.is_empty());
            }
        }
    }

    #[test]
    fn a_types_and_shift_is_for_one_letter() {
        let mut s = state(Layout::Qwerty);
        assert!(step(&mut s, Press::Shift));
        assert!(step(&mut s, Press::Type));
        assert!(step(&mut s, Press::Right));
        assert!(step(&mut s, Press::Type));
        assert_eq!(typed(&s), "Qw");
        assert!(step(&mut s, Press::Backspace));
        assert!(matches!(
            s.pending.last(),
            Some(Event::Key {
                key: EKey::Backspace,
                pressed: false,
                ..
            })
        ));
        assert!(!step(&mut s, Press::Done));
        assert!(!step(&mut s, Press::Close));
    }

    #[test]
    fn the_cursor_wraps_and_keeps_its_column() {
        let mut s = state(Layout::Qwerty);
        step(&mut s, Press::Left);
        assert_eq!(s.col, 9, "left of q is p");
        step(&mut s, Press::Right);
        assert_eq!(s.col, 0);
        // From `t` down: `g` (a tie goes right, as the rows stagger), then `v`.
        s.col = 4;
        step(&mut s, Press::Down);
        assert_eq!(rows(Layout::Qwerty)[2][s.col], K::Ch("g", "G"));
        step(&mut s, Press::Down);
        assert_eq!(rows(Layout::Qwerty)[3][s.col], K::Ch("v", "V"));
        // Up from the top row goes round to the foot.
        s.row = 0;
        step(&mut s, Press::Up);
        assert_eq!(s.row, 4);
        // A cursor out of range (another layout's) is brought back.
        s.row = 9;
        s.col = 99;
        step(&mut s, Press::Type);
        assert!(s.row < 5);
    }

    #[test]
    fn digits_type_numbers() {
        let mut s = state(Layout::Digits);
        s.col = 1;
        step(&mut s, Press::Type);
        step(&mut s, Press::Down);
        step(&mut s, Press::Type);
        assert_eq!(typed(&s), "52");
    }
}
