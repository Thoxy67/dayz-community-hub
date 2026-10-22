//! Which widget a direction leads to (= `lib/gamepad/spatial.ts`): pure geometry over
//! rectangles, tested without a window. [`super::nav`] gathers the candidates.
//!
//! A candidate counts only if it lies on the side the direction points to. Among those
//! the nearest wins, where "near" is the gap along the direction plus the sideways gap
//! weighted heavier: Down from a button goes to what is under it, not to something a
//! little closer but off to the side.

use egui::Rect;

/// A direction of the D-pad or the left stick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub fn vertical(self) -> bool {
        matches!(self, Dir::Up | Dir::Down)
    }
}

/// Sideways distance costs this much more than distance along the direction.
const SIDEWAYS: f32 = 3.0;

/// The cost of going from `from` to `to` in `dir`; `None` if it is not that way at all.
pub fn cost(from: Rect, to: Rect, dir: Dir) -> Option<f32> {
    // Beyond the edge it leaves by, give or take a quarter of the smaller of the two:
    // something on the same line is beside, not below.
    let slack_y = from.height().min(to.height()) * 0.25;
    let slack_x = from.width().min(to.width()) * 0.25;
    let (along, (lo1, hi1, lo2, hi2)) = match dir {
        Dir::Down => {
            if to.top() < from.bottom() - slack_y {
                return None;
            }
            (
                (to.top() - from.bottom()).max(0.0),
                (from.left(), from.right(), to.left(), to.right()),
            )
        }
        Dir::Up => {
            if to.bottom() > from.top() + slack_y {
                return None;
            }
            (
                (from.top() - to.bottom()).max(0.0),
                (from.left(), from.right(), to.left(), to.right()),
            )
        }
        Dir::Right => {
            if to.left() < from.right() - slack_x {
                return None;
            }
            (
                (to.left() - from.right()).max(0.0),
                (from.top(), from.bottom(), to.top(), to.bottom()),
            )
        }
        Dir::Left => {
            if to.right() > from.left() + slack_x {
                return None;
            }
            (
                (from.left() - to.right()).max(0.0),
                (from.top(), from.bottom(), to.top(), to.bottom()),
            )
        }
    };
    let gap = (lo1.max(lo2) - hi1.min(hi2)).max(0.0);
    // Among those in line, the one whose middle is nearest breaks ties.
    let offset = ((lo1 + hi1) / 2.0 - (lo2 + hi2) / 2.0).abs();
    let k = along + gap * SIDEWAYS + offset * 0.01;
    k.is_finite().then_some(k)
}

/// The index of the best candidate in `dir`, or `None`.
pub fn best(from: Rect, candidates: &[Rect], dir: Dir) -> Option<usize> {
    let mut at = None;
    let mut low = f32::INFINITY;
    for (i, c) in candidates.iter().enumerate() {
        if let Some(k) = cost(from, *c, dir)
            && k < low
        {
            low = k;
            at = Some(i);
        }
    }
    at
}

/// How many rows a held direction moves at once: one, then three after ten repeats,
/// then ten after twenty-four (`nav.ts`'s `stride`).
pub fn stride(streak: u32) -> i64 {
    if streak > 24 {
        10
    } else if streak > 10 {
        3
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn b(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    #[test]
    fn down_goes_to_what_is_under_not_beside() {
        let from = b(100.0, 100.0, 80.0, 24.0);
        let under = b(100.0, 160.0, 80.0, 24.0);
        let beside_closer = b(300.0, 130.0, 80.0, 24.0);
        assert_eq!(best(from, &[beside_closer, under], Dir::Down), Some(1));
    }

    #[test]
    fn nothing_that_way() {
        let from = b(100.0, 100.0, 80.0, 24.0);
        let above = b(100.0, 40.0, 80.0, 24.0);
        assert_eq!(best(from, &[above], Dir::Down), None);
        assert_eq!(best(from, &[above], Dir::Up), Some(0));
        assert_eq!(best(from, &[], Dir::Left), None);
    }

    #[test]
    fn same_line_is_beside_not_below() {
        // A taller neighbour on the same line, its top a little lower: not "down".
        let from = b(0.0, 100.0, 80.0, 24.0);
        let neighbour = b(100.0, 104.0, 80.0, 30.0);
        assert_eq!(cost(from, neighbour, Dir::Down), None);
        assert!(cost(from, neighbour, Dir::Right).is_some());
    }

    #[test]
    fn right_prefers_in_line_then_nearest_middle() {
        let from = b(0.0, 100.0, 50.0, 20.0);
        let in_line_far = b(200.0, 100.0, 50.0, 20.0);
        let off_line_near = b(70.0, 200.0, 50.0, 20.0);
        assert_eq!(
            best(from, &[off_line_near, in_line_far], Dir::Right),
            Some(1)
        );
        // Two in line at the same distance: the one whose middle is nearer.
        let a = b(100.0, 70.0, 50.0, 40.0);
        let c = b(100.0, 100.0, 50.0, 20.0);
        assert_eq!(best(from, &[a, c], Dir::Right), Some(1));
    }

    #[test]
    fn left_and_up_mirror() {
        let from = b(300.0, 300.0, 50.0, 20.0);
        let left = b(100.0, 300.0, 50.0, 20.0);
        let up = b(300.0, 100.0, 50.0, 20.0);
        assert_eq!(best(from, &[left, up], Dir::Left), Some(0));
        assert_eq!(best(from, &[left, up], Dir::Up), Some(1));
    }

    #[test]
    fn non_finite_rects_never_win() {
        let from = b(0.0, 0.0, 10.0, 10.0);
        let nan = Rect::from_min_max(pos2(f32::NAN, 20.0), pos2(f32::NAN, 30.0));
        assert_eq!(best(from, &[nan], Dir::Down), None);
    }

    #[test]
    fn a_held_direction_speeds_up() {
        assert_eq!(stride(0), 1);
        assert_eq!(stride(10), 1);
        assert_eq!(stride(11), 3);
        assert_eq!(stride(24), 3);
        assert_eq!(stride(25), 10);
        assert_eq!(stride(u32::MAX), 10);
    }
}
