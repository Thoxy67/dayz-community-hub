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
    /// On a Steam Deck, as Steam's Game Mode says.
    pub steam_deck: bool,
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

    /// The labels to draw: the setting's, else the first pad's (a generic pad inside
    /// Steam's big-screen interface is most likely a Deck).
    pub fn kind(&self, glyphs: GlyphChoice) -> PadKind {
        match glyphs {
            GlyphChoice::Xbox => PadKind::Xbox,
            GlyphChoice::PlayStation => PadKind::PlayStation,
            GlyphChoice::Nintendo => PadKind::Nintendo,
            GlyphChoice::SteamDeck => PadKind::SteamDeck,
            GlyphChoice::Auto => match self.pads.first().map(|p| p.kind) {
                Some(k) if k != PadKind::Generic => k,
                _ if self.steam_deck || self.steam_ui => PadKind::SteamDeck,
                _ => PadKind::Xbox,
            },
        }
    }

    pub fn used_pad(&mut self) {
        self.last = InputMode::Gamepad;
    }

    pub fn used_pointer(&mut self) {
        self.last = InputMode::Pointer;
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

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
