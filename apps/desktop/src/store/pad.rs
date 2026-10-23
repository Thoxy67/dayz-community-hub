//! Whether a controller or the pointer is driving the window, and the pads connected
//! (= `lib/gamepad/state.svelte.ts`). `mode` follows the last input used ("auto"), or is
//! held by the setting. In gamepad mode the interface is drawn larger
//! (`prefs.ui.pad_scale`); the focus navigation is `crate::pad`'s, its state kept here.

use dz_hub::features::gamepad::{GamepadStatus, PadAction, PadInfo, PadKind};
use dz_hub::{Hub, HubEvent};

use crate::prefs::{GlyphChoice, PadMode};
use crate::rt::{self, Task};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Pointer,
    Gamepad,
}

pub struct Pad {
    hub: Hub,
    last: InputMode,
    /// Pads could be opened at all.
    pub available: bool,
    pub pads: Vec<PadInfo>,
    /// Inside Steam's big-screen interface: its on-screen keyboard is there.
    pub steam_ui: bool,
    /// On a Steam Deck (its firmware, or Steam's Game Mode, says so).
    pub steam_deck: bool,
    /// Inside gamescope's session (`crate::platform::deck`).
    pub game_mode: bool,
    /// The last pointer input was a finger on a touch screen (a mouse undoes it).
    pub touch: bool,
    /// When the last touch was (egui time): the pointer egui moves for a finger
    /// may still glide a moment after it is lifted.
    touch_at: f64,
    /// The last action a pad sent and when (egui time), for the settings' live test.
    pub last_action: Option<(PadAction, f64)>,
    /// Pad actions not yet handled by the focus navigation, oldest first, each with
    /// whether it is a held repeat.
    pub queue: Vec<(PadAction, bool)>,
    /// The focus navigation's state (`crate::pad::nav`).
    pub nav: crate::pad::PadNav,
    status: Option<Task<GamepadStatus>>,
}

impl Pad {
    pub fn new(hub: Hub) -> Self {
        Self {
            hub,
            last: InputMode::Pointer,
            available: false,
            pads: Vec::new(),
            steam_ui: false,
            steam_deck: false,
            game_mode: false,
            touch: false,
            touch_at: f64::NEG_INFINITY,
            last_action: None,
            queue: Vec::new(),
            nav: crate::pad::PadNav::default(),
            status: None,
        }
    }

    pub fn start(&mut self) {
        let hub = self.hub.clone();
        self.status = Some(rt::spawn(async move { hub.gamepad_status().await }));
    }

    /// Who drives: the setting's choice, else the last input used.
    pub fn mode(&self, setting: PadMode) -> InputMode {
        match setting {
            PadMode::Always => InputMode::Gamepad,
            PadMode::Never => InputMode::Pointer,
            PadMode::Auto => self.last,
        }
    }

    /// The labels to draw: the setting's, else the first pad's. On a Deck, Steam Input
    /// hands the window a virtual pad with Microsoft's vendor id (an Xbox one to
    /// gilrs) or a generic one: the buttons under the player's thumbs are the
    /// Deck's. A generic pad inside Steam's big-screen interface is most likely a
    /// Deck too.
    pub fn kind(&self, glyphs: GlyphChoice) -> PadKind {
        pick_kind(
            glyphs,
            self.pads.first().map(|p| p.kind),
            self.steam_deck,
            self.steam_ui,
        )
    }

    pub fn used_pad(&mut self) {
        self.last = InputMode::Gamepad;
    }

    pub fn used_pointer(&mut self) {
        self.last = InputMode::Pointer;
        self.touch = false;
    }

    /// A touch on the screen: not a mouse, so the input mode (and with it the pad
    /// density and zoom) stays as it is; hit areas grow (`size::TOUCH_REACH`).
    pub fn used_touch(&mut self, now: f64) {
        self.touch = true;
        self.touch_at = now;
    }

    /// The pointer moved or pressed at `now`: the mouse, unless a finger was on the
    /// screen just before ([`TOUCH_GRACE_S`]).
    pub fn used_pointer_at(&mut self, now: f64) {
        if is_mouse(now, self.touch_at) {
            self.used_pointer();
        }
    }

    /// A pad action as if a pad had sent it (the control channel, `ui_shot`, the
    /// `pad.*` commands). The next frame acts on it.
    pub fn inject(&mut self, action: PadAction, repeat: bool) {
        self.last_action = Some((action, now_secs()));
        if self.queue.len() < 64 {
            self.queue.push((action, repeat));
        }
    }

    /// The input mode follows a pad once the navigation acts on its action (not when
    /// the setting says Never).
    pub fn on_event(&mut self, ev: &HubEvent) {
        match ev {
            HubEvent::GamepadPads(p) => {
                self.available = true;
                self.pads = p.0.clone();
            }
            HubEvent::GamepadInput(i) => self.inject(i.action, i.repeat),
            _ => {}
        }
    }

    pub fn poll(&mut self) -> bool {
        match self.status.as_mut().and_then(Task::poll) {
            Some(s) => {
                self.status = None;
                self.available = s.available;
                self.pads = s.pads;
                self.steam_ui = s.steam_ui;
                self.steam_deck = s.steam_deck;
                self.game_mode = s.game_mode;
                // Game Mode on a Steam Deck: there is no mouse to start with.
                if s.steam_ui {
                    self.used_pad();
                }
                true
            }
            None => false,
        }
    }
}

/// How long after a touch the pointer's movement is still the finger's.
const TOUCH_GRACE_S: f64 = 0.6;

/// Pointer input at `now` is the mouse's, the last touch having been at `touch_at`.
fn is_mouse(now: f64, touch_at: f64) -> bool {
    now - touch_at > TOUCH_GRACE_S
}

/// [`Pad::kind`] from the setting, the first pad's kind and where the window runs.
fn pick_kind(
    glyphs: GlyphChoice,
    first: Option<PadKind>,
    steam_deck: bool,
    steam_ui: bool,
) -> PadKind {
    match glyphs {
        GlyphChoice::Xbox => PadKind::Xbox,
        GlyphChoice::PlayStation => PadKind::PlayStation,
        GlyphChoice::Nintendo => PadKind::Nintendo,
        GlyphChoice::SteamDeck => PadKind::SteamDeck,
        GlyphChoice::Auto => match first {
            Some(PadKind::Xbox | PadKind::Generic) | None if steam_deck => PadKind::SteamDeck,
            Some(k) if k != PadKind::Generic => k,
            _ if steam_ui => PadKind::SteamDeck,
            _ => PadKind::Xbox,
        },
    }
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Steam Input's virtual pad says Microsoft: on a Deck the glyphs stay the Deck's.
    #[test]
    fn a_deck_draws_its_own_buttons() {
        let auto = GlyphChoice::Auto;
        assert_eq!(
            pick_kind(auto, Some(PadKind::Xbox), true, false),
            PadKind::SteamDeck
        );
        assert_eq!(pick_kind(auto, None, true, false), PadKind::SteamDeck);
        assert_eq!(
            pick_kind(auto, Some(PadKind::Xbox), false, false),
            PadKind::Xbox
        );
        assert_eq!(
            pick_kind(auto, Some(PadKind::Generic), false, true),
            PadKind::SteamDeck
        );
        assert_eq!(
            pick_kind(auto, Some(PadKind::PlayStation), true, true),
            PadKind::PlayStation
        );
        assert_eq!(pick_kind(auto, None, false, false), PadKind::Xbox);
        assert_eq!(
            pick_kind(GlyphChoice::Xbox, None, true, true),
            PadKind::Xbox
        );
    }

    /// A finger's pointer gliding on after the touch is no mouse; later movement is.
    #[test]
    fn a_touch_is_not_a_mouse() {
        assert!(is_mouse(10.0, f64::NEG_INFINITY));
        assert!(!is_mouse(10.2, 10.0));
        assert!(is_mouse(11.0, 10.0));
    }
}
