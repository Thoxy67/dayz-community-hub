//! From a pad's buttons and stick to the window's actions, with no knowledge
//! of gilrs: what is held, which direction the stick points, and when a held
//! direction repeats. Pure, so the timing is tested without a controller.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// What the window is asked to do. Named after what they do, not after the
/// buttons, so the interface reads the same whatever the pad.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PadAction {
    Up,
    Down,
    Left,
    Right,
    /// A (Xbox), Cross (PlayStation): the bottom face button.
    Accept,
    /// B, Circle: the right face button.
    Back,
    /// X, Square: the left face button.
    Primary,
    /// Y, Triangle: the top face button.
    Secondary,
    /// Start, Options, ≡.
    Menu,
    /// Back/View, Share/Create, ⧉.
    View,
    /// LB, L1.
    PrevTab,
    /// RB, R1.
    NextTab,
    /// LT, L2.
    PageUp,
    /// RT, R2.
    PageDown,
    /// The right stick pushed up: scroll what is being read.
    ScrollUp,
    /// The right stick pushed down.
    ScrollDown,
    /// The left stick pressed in (L3).
    LeftStick,
    /// The right stick pressed in (R3).
    RightStick,
}

impl PadAction {
    /// Held down, it fires again and again: moving and paging do, the rest
    /// fire once per press.
    fn repeats(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::Left
                | Self::Right
                | Self::PageUp
                | Self::PageDown
                | Self::ScrollUp
                | Self::ScrollDown
        )
    }
}

/// A button, by position. The glue maps the pad library's buttons onto these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    South,
    East,
    West,
    North,
    Start,
    Select,
    Lb,
    Rb,
    Lt,
    Rt,
    DUp,
    DDown,
    DLeft,
    DRight,
    L3,
    R3,
}

impl Btn {
    fn action(self) -> PadAction {
        match self {
            Self::South => PadAction::Accept,
            Self::East => PadAction::Back,
            Self::West => PadAction::Primary,
            Self::North => PadAction::Secondary,
            Self::Start => PadAction::Menu,
            Self::Select => PadAction::View,
            Self::Lb => PadAction::PrevTab,
            Self::Rb => PadAction::NextTab,
            Self::Lt => PadAction::PageUp,
            Self::Rt => PadAction::PageDown,
            Self::DUp => PadAction::Up,
            Self::DDown => PadAction::Down,
            Self::DLeft => PadAction::Left,
            Self::DRight => PadAction::Right,
            Self::L3 => PadAction::LeftStick,
            Self::R3 => PadAction::RightStick,
        }
    }
}

/// How long a held direction waits before it repeats, then how often.
pub const REPEAT_DELAY: Duration = Duration::from_millis(350);
pub const REPEAT_EVERY: Duration = Duration::from_millis(90);
/// The stick counts as pushed past this, and as let go below the second: the
/// gap keeps a stick resting near the edge from stuttering.
const STICK_ON: f32 = 0.55;
const STICK_OFF: f32 = 0.35;

/// One action to send: the action and whether it is a repeat of a held one.
pub type Fired = (PadAction, bool);

/// What one pad is doing.
#[derive(Debug, Default)]
pub struct PadState {
    /// Which repeating action is held, and when it fires next. The last one
    /// pressed wins, as a keyboard's key repeat does.
    held: Option<(PadAction, Instant)>,
    /// The direction the stick points, if it is pushed far enough.
    stick: Option<PadAction>,
    /// The right stick, up or down only: it scrolls.
    scroll: Option<PadAction>,
    /// Buttons of repeating actions still down, most recent last, so letting
    /// go of one hands the repeat back to the other.
    down: Vec<PadAction>,
}

impl PadState {
    /// A button went down or up.
    pub fn button(&mut self, b: Btn, pressed: bool, now: Instant) -> Option<Fired> {
        let a = b.action();
        if !a.repeats() {
            return pressed.then_some((a, false));
        }
        self.down.retain(|x| *x != a);
        if pressed {
            self.down.push(a);
            self.held = Some((a, now + REPEAT_DELAY));
            Some((a, false))
        } else {
            self.rehold(now);
            None
        }
    }

    /// The left stick moved; `y` is up.
    pub fn stick(&mut self, x: f32, y: f32, now: Instant) -> Option<Fired> {
        let (ax, ay) = (x.abs(), y.abs());
        let reach = ax.max(ay);
        let dir = if reach < STICK_OFF || (self.stick.is_none() && reach < STICK_ON) {
            None
        } else if ax > ay {
            Some(if x > 0.0 {
                PadAction::Right
            } else {
                PadAction::Left
            })
        } else {
            Some(if y > 0.0 {
                PadAction::Up
            } else {
                PadAction::Down
            })
        };
        if dir == self.stick {
            return None;
        }
        let was = std::mem::replace(&mut self.stick, dir);
        if let Some(a) = was {
            self.down.retain(|x| *x != a);
        }
        match dir {
            Some(a) => {
                self.down.retain(|x| *x != a);
                self.down.push(a);
                self.held = Some((a, now + REPEAT_DELAY));
                Some((a, false))
            }
            None => {
                self.rehold(now);
                None
            }
        }
    }

    /// The right stick moved; `y` is up. Only its vertical reach counts.
    pub fn right_stick(&mut self, y: f32, now: Instant) -> Option<Fired> {
        let reach = y.abs();
        let dir = if reach < STICK_OFF || (self.scroll.is_none() && reach < STICK_ON) {
            None
        } else if y > 0.0 {
            Some(PadAction::ScrollUp)
        } else {
            Some(PadAction::ScrollDown)
        };
        if dir == self.scroll {
            return None;
        }
        if let Some(a) = std::mem::replace(&mut self.scroll, dir) {
            self.down.retain(|x| *x != a);
        }
        match dir {
            Some(a) => {
                self.down.push(a);
                self.held = Some((a, now + REPEAT_DELAY));
                Some((a, false))
            }
            None => {
                self.rehold(now);
                None
            }
        }
    }

    /// Whatever is still held after a release takes the repeat, from a fresh delay.
    fn rehold(&mut self, now: Instant) {
        self.held = self.down.last().map(|a| (*a, now + REPEAT_DELAY));
    }

    /// The held action fires again if its time has come.
    pub fn tick(&mut self, now: Instant) -> Option<Fired> {
        let (a, at) = self.held?;
        if now < at {
            return None;
        }
        // Late wake-ups do not burst: the next one is a full interval away.
        self.held = Some((a, now + REPEAT_EVERY));
        Some((a, true))
    }

    /// When `tick` has something to do next, if anything is held.
    pub fn deadline(&self) -> Option<Instant> {
        self.held.map(|(_, at)| at)
    }

    /// Forget everything held: the window lost focus, the pad went away.
    pub fn release_all(&mut self) {
        *self = Self::default();
    }
}

/// What kind of pad, for the glyphs the window draws.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PadKind {
    Xbox,
    PlayStation,
    Nintendo,
    SteamDeck,
    Generic,
}

/// A pad's kind from its USB vendor and its name.
pub fn kind_of(vendor: Option<u16>, name: &str) -> PadKind {
    let n = name.to_ascii_lowercase();
    match vendor {
        Some(0x054c) => return PadKind::PlayStation,
        Some(0x057e) => return PadKind::Nintendo,
        Some(0x045e) => return PadKind::Xbox,
        Some(0x28de) if n.contains("deck") => return PadKind::SteamDeck,
        _ => {}
    }
    if ["playstation", "dualsense", "dualshock", "ps4", "ps5"]
        .iter()
        .any(|k| n.contains(k))
    {
        PadKind::PlayStation
    } else if n.contains("steam deck") {
        PadKind::SteamDeck
    } else if n.contains("nintendo") || n.contains("switch") || n.contains("joy-con") {
        PadKind::Nintendo
    } else if n.contains("xbox") || n.contains("x-box") || n.contains("xinput") {
        PadKind::Xbox
    } else {
        PadKind::Generic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn a_press_fires_once_and_a_release_nothing() {
        let (mut p, now) = (PadState::default(), t0());
        assert_eq!(
            p.button(Btn::South, true, now),
            Some((PadAction::Accept, false))
        );
        assert_eq!(p.button(Btn::South, false, now), None);
        assert_eq!(p.deadline(), None);
        assert_eq!(p.tick(now + Duration::from_secs(5)), None);
    }

    #[test]
    fn a_held_direction_waits_then_repeats() {
        let (mut p, now) = (PadState::default(), t0());
        assert_eq!(
            p.button(Btn::DDown, true, now),
            Some((PadAction::Down, false))
        );
        assert_eq!(p.tick(now + REPEAT_DELAY - Duration::from_millis(1)), None);
        let first = now + REPEAT_DELAY;
        assert_eq!(p.tick(first), Some((PadAction::Down, true)));
        assert_eq!(p.deadline(), Some(first + REPEAT_EVERY));
        assert_eq!(p.tick(first + REPEAT_EVERY / 2), None);
        assert_eq!(p.tick(first + REPEAT_EVERY), Some((PadAction::Down, true)));
        p.button(Btn::DDown, false, first);
        assert_eq!(p.deadline(), None);
    }

    #[test]
    fn letting_go_of_the_newer_direction_hands_back_to_the_older() {
        let (mut p, now) = (PadState::default(), t0());
        p.button(Btn::DDown, true, now);
        assert_eq!(
            p.button(Btn::DRight, true, now),
            Some((PadAction::Right, false))
        );
        p.button(Btn::DRight, false, now);
        assert_eq!(p.tick(now + REPEAT_DELAY), Some((PadAction::Down, true)));
    }

    #[test]
    fn the_stick_has_a_deadzone_and_hysteresis() {
        let (mut p, now) = (PadState::default(), t0());
        assert_eq!(p.stick(0.3, 0.1, now), None);
        assert_eq!(p.stick(0.2, 0.7, now), Some((PadAction::Up, false)));
        // Easing off but still past the release point: still held, no new fire.
        assert_eq!(p.stick(0.1, 0.45, now), None);
        assert!(p.deadline().is_some());
        assert_eq!(p.stick(0.0, 0.1, now), None);
        assert_eq!(p.deadline(), None);
        assert_eq!(p.stick(-0.9, -0.2, now), Some((PadAction::Left, false)));
        assert_eq!(p.stick(-0.2, -0.9, now), Some((PadAction::Down, false)));
    }

    #[test]
    fn triggers_page_and_repeat() {
        let (mut p, now) = (PadState::default(), t0());
        assert_eq!(
            p.button(Btn::Rt, true, now),
            Some((PadAction::PageDown, false))
        );
        assert_eq!(
            p.tick(now + REPEAT_DELAY),
            Some((PadAction::PageDown, true))
        );
        assert_eq!(
            p.button(Btn::Lb, true, now),
            Some((PadAction::PrevTab, false))
        );
    }

    #[test]
    fn the_right_stick_scrolls_and_repeats() {
        let mut s = PadState::default();
        let t = t0();
        assert_eq!(s.right_stick(0.2, t), None);
        assert_eq!(s.right_stick(-0.9, t), Some((PadAction::ScrollDown, false)));
        assert_eq!(
            s.tick(t + REPEAT_DELAY),
            Some((PadAction::ScrollDown, true))
        );
        assert_eq!(s.right_stick(0.0, t + REPEAT_DELAY), None);
        assert_eq!(s.deadline(), None);
    }

    #[test]
    fn stick_clicks_fire_once() {
        let mut s = PadState::default();
        let t = t0();
        assert_eq!(
            s.button(Btn::R3, true, t),
            Some((PadAction::RightStick, false))
        );
        assert_eq!(s.button(Btn::R3, false, t), None);
        assert_eq!(s.deadline(), None);
    }

    #[test]
    fn release_all_forgets_what_was_held() {
        let (mut p, now) = (PadState::default(), t0());
        p.button(Btn::DUp, true, now);
        p.release_all();
        assert_eq!(p.tick(now + Duration::from_secs(1)), None);
    }

    #[test]
    fn kinds() {
        assert_eq!(
            kind_of(Some(0x054c), "Wireless Controller"),
            PadKind::PlayStation
        );
        assert_eq!(
            kind_of(None, "DualSense Wireless Controller"),
            PadKind::PlayStation
        );
        assert_eq!(kind_of(Some(0x28de), "Steam Deck"), PadKind::SteamDeck);
        assert_eq!(
            kind_of(Some(0x28de), "Steam Virtual Gamepad"),
            PadKind::Generic
        );
        assert_eq!(kind_of(Some(0x045e), "Controller"), PadKind::Xbox);
        assert_eq!(kind_of(None, "Microsoft X-Box 360 pad"), PadKind::Xbox);
        assert_eq!(kind_of(None, "8BitDo"), PadKind::Generic);
    }
}
