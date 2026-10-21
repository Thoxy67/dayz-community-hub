//! The interface's own preferences: how the window is laid out and dressed, not what
//! the launcher does (the launcher's settings live in the profile, in the hub).
//!
//! The Svelte interface kept them in `localStorage` keys (`dzch.prefs`, `dzch.theme`,
//! `dzch.locale`, `dzch.padGlyphs`, `dzch.palette.recent`); here they are one JSON file in
//! the hub's data directory ([`FILE_NAME`]), its top-level fields named after those keys.
//! Nothing is migrated from the old WebView's storage: a first run starts from defaults.
//!
//! Reads happen once, before the first frame. Every change marks the file dirty
//! ([`Prefs::edit`]); [`Prefs::poll`] writes it 250 ms after the last change (dragging a
//! pane's edge changes it on every pointer move) on a writer thread, never on the UI
//! thread. [`Prefs::flush`] writes what is pending at exit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::theme::{Raw, Scheme};

/// The file, in the hub's data directory.
pub const FILE_NAME: &str = "interface.json";

/// Writes wait this long after the last change.
const SAVE_DELAY: Duration = Duration::from_millis(250);

/// Controller mode: follow the last input used, or always, or never.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PadMode {
    #[default]
    Auto,
    Always,
    Never,
}

/// Which maker's button labels to draw: the pad's own, or one picked in the settings.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GlyphChoice {
    #[default]
    Auto,
    Xbox,
    PlayStation,
    Nintendo,
    SteamDeck,
}

/// `dzch.prefs`: the window's layout.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct UiPrefs {
    /// Sizes of the resizable panes, by id, in points.
    pub panes: BTreeMap<String, f32>,
    /// The navigation rail shows icons only.
    pub rail_collapsed: bool,
    /// The last-played strip was put away for this server ("ip:port").
    pub dismissed_rejoin: Option<String>,
    pub pad_mode: PadMode,
    /// How much larger the interface is drawn while a controller drives it.
    pub pad_scale: f32,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            panes: BTreeMap::new(),
            rail_collapsed: false,
            dismissed_rejoin: None,
            pad_mode: PadMode::Auto,
            pad_scale: 1.15,
        }
    }
}

/// One of the player's own themes. `id` always starts with `custom:`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CustomTheme {
    pub id: String,
    pub name: String,
    pub scheme: Scheme,
    /// The twelve colours.
    pub tokens: Raw,
    /// The preset it was started from, which "reset" goes back to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
}

/// Which preset "follow the system" wears for each OS scheme.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SystemPair {
    pub dark: String,
    pub light: String,
}

impl Default for SystemPair {
    fn default() -> Self {
        Self {
            dark: crate::theme::presets::DEFAULT_DARK.to_string(),
            light: crate::theme::presets::DEFAULT_LIGHT.to_string(),
        }
    }
}

/// `dzch.theme`: the theme choice (no window frame: egui draws a square one).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct StoredTheme {
    /// A preset id, a custom theme's id, or `None` to follow the OS.
    pub preset: Option<String>,
    pub customs: Vec<CustomTheme>,
    pub pair: SystemPair,
}

/// Where the window was and how big, for the next start.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct WindowState {
    /// Outer position, in points.
    pub pos: Option<[f32; 2]>,
    /// Inner size, in points.
    pub size: [f32; 2],
    pub maximized: bool,
}

/// Everything in the file. Unknown top-level keys (a view's own) are kept in `other`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Stored {
    #[serde(rename = "dzch.prefs")]
    pub ui: UiPrefs,
    #[serde(rename = "dzch.theme")]
    pub theme: StoredTheme,
    /// The language chosen, as its code; `None` follows the OS.
    #[serde(rename = "dzch.locale")]
    pub locale: Option<String>,
    #[serde(rename = "dzch.padGlyphs")]
    pub pad_glyphs: GlyphChoice,
    /// The command palette's recent picks, newest first.
    #[serde(rename = "dzch.palette.recent")]
    pub palette_recent: Vec<String>,
    #[serde(rename = "dzch.window")]
    pub window: Option<WindowState>,
    /// The interface scale (egui's zoom factor), 1 = as designed.
    #[serde(rename = "dzch.scale")]
    pub scale: f32,
    /// No motion: views appear, toasts and menus do not slide.
    #[serde(rename = "dzch.reducedMotion")]
    pub reduced_motion: bool,
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
}

impl Default for Stored {
    fn default() -> Self {
        Self {
            ui: UiPrefs::default(),
            theme: StoredTheme::default(),
            locale: None,
            pad_glyphs: GlyphChoice::Auto,
            palette_recent: Vec::new(),
            window: None,
            scale: 1.0,
            reduced_motion: false,
            other: BTreeMap::new(),
        }
    }
}

impl Stored {
    /// Values a hand-edited or damaged file may hold, brought back into range.
    fn sanitized(mut self) -> Self {
        if !self.scale.is_finite() || !(0.5..=3.0).contains(&self.scale) {
            self.scale = 1.0;
        }
        let ps = self.ui.pad_scale;
        if !ps.is_finite() || !(0.5..=3.0).contains(&ps) {
            self.ui.pad_scale = 1.15;
        }
        self.ui
            .panes
            .retain(|_, v| v.is_finite() && *v >= 0.0 && *v <= 10_000.0);
        self.palette_recent.truncate(64);
        if let Some(w) = &self.window {
            let ok = w
                .size
                .iter()
                .all(|v| v.is_finite() && *v >= 100.0 && *v <= 20_000.0)
                && w.pos
                    .is_none_or(|p| p.iter().all(|v| v.is_finite() && v.abs() <= 50_000.0));
            if !ok {
                self.window = None;
            }
        }
        self
    }
}

/// The preferences, and their file.
pub struct Prefs {
    s: Stored,
    path: Option<PathBuf>,
    dirty_since: Option<Instant>,
    writer: Option<mpsc::Sender<(PathBuf, Vec<u8>)>>,
}

impl Default for Prefs {
    /// Defaults, kept in memory only (tests, a data directory that cannot be found).
    fn default() -> Self {
        Self {
            s: Stored::default(),
            path: None,
            dirty_since: None,
            writer: None,
        }
    }
}

impl Prefs {
    /// The file in `dir`.
    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join(FILE_NAME)
    }

    /// Read the file (a missing or broken one gives the defaults). Blocking: call it
    /// before the first frame.
    pub fn load(path: PathBuf) -> Self {
        let s = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Stored>(&b).ok())
            .unwrap_or_default()
            .sanitized();
        Self {
            s,
            path: Some(path),
            dirty_since: None,
            writer: None,
        }
    }

    /// Preferences from a value, kept in memory only.
    pub fn in_memory(s: Stored) -> Self {
        Self {
            s: s.sanitized(),
            ..Self::default()
        }
    }

    pub fn get(&self) -> &Stored {
        &self.s
    }

    /// Change them; the file is written shortly after.
    pub fn edit(&mut self) -> &mut Stored {
        self.dirty_since = Some(Instant::now());
        &mut self.s
    }

    /// Whether a write is waiting.
    pub fn dirty(&self) -> bool {
        self.dirty_since.is_some()
    }

    /// Write the file once changes have settled; call each frame. Asks for a repaint
    /// at the moment the write is due, so nothing waits on the next input.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(since) = self.dirty_since else {
            return;
        };
        let due = since + SAVE_DELAY;
        let now = Instant::now();
        if now < due {
            ctx.request_repaint_after(due - now);
            return;
        }
        self.dirty_since = None;
        self.send_write();
    }

    /// Write what is pending now (at exit). Blocking, on purpose.
    pub fn flush(&mut self) {
        if self.dirty_since.take().is_none() {
            return;
        }
        if let (Some(path), Ok(bytes)) = (&self.path, serde_json::to_vec_pretty(&self.s)) {
            let _ = write_atomic(path, &bytes);
        }
    }

    fn send_write(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&self.s) else {
            return;
        };
        let tx = self.writer.get_or_insert_with(spawn_writer);
        if tx.send((path.clone(), bytes.clone())).is_err() {
            // The writer went away: start another and try once more.
            let tx = spawn_writer();
            let _ = tx.send((path, bytes));
            self.writer = Some(tx);
        }
    }

    // ── conveniences ────────────────────────────────────────────────────────

    /// A pane's size, or `fallback` when it was never resized.
    pub fn pane(&self, id: &str, fallback: f32) -> f32 {
        self.s.ui.panes.get(id).copied().unwrap_or(fallback)
    }

    pub fn set_pane(&mut self, id: &str, px: f32) {
        if !px.is_finite() {
            return;
        }
        let px = px.round();
        if self.s.ui.panes.get(id) != Some(&px) {
            self.edit().ui.panes.insert(id.to_string(), px);
        }
    }

    pub fn reset_pane(&mut self, id: &str) {
        if self.s.ui.panes.contains_key(id) {
            self.edit().ui.panes.remove(id);
        }
    }

    /// A view's own value under a top-level `key` (`"dzch.servers.filters"`…).
    pub fn value<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.s
            .other
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Keep a view's own value under a top-level `key`.
    pub fn set_value<T: Serialize>(&mut self, key: &str, value: &T) {
        if let Ok(v) = serde_json::to_value(value)
            && self.s.other.get(key) != Some(&v)
        {
            self.edit().other.insert(key.to_string(), v);
        }
    }
}

/// One thread writes, in order: a later write never lands before an earlier one.
fn spawn_writer() -> mpsc::Sender<(PathBuf, Vec<u8>)> {
    let (tx, rx) = mpsc::channel::<(PathBuf, Vec<u8>)>();
    let _ = std::thread::Builder::new()
        .name("prefs-writer".into())
        .spawn(move || {
            while let Ok(mut job) = rx.recv() {
                // Only the newest of a burst matters.
                while let Ok(next) = rx.try_recv() {
                    job = next;
                }
                let _ = write_atomic(&job.0, &job.1);
            }
        });
    tx
}

/// Write beside the file, then rename over it: a crash never leaves half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_the_old_local_storage_keys() {
        let v = serde_json::to_value(Stored::default()).unwrap();
        for k in [
            "dzch.prefs",
            "dzch.theme",
            "dzch.locale",
            "dzch.padGlyphs",
            "dzch.palette.recent",
        ] {
            assert!(v.get(k).is_some(), "{k}");
        }
        assert_eq!(v["dzch.prefs"]["railCollapsed"], false);
        assert_eq!(v["dzch.prefs"]["padMode"], "auto");
    }

    #[test]
    fn round_trips_and_keeps_unknown_keys() {
        let mut s = Stored::default();
        s.ui.rail_collapsed = true;
        s.other
            .insert("dzch.view.thing".into(), serde_json::json!({"a": 1}));
        let text = serde_json::to_string(&s).unwrap();
        let back: Stored = serde_json::from_str(&text).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn broken_values_are_sanitized() {
        let s: Stored =
            serde_json::from_str(r#"{"dzch.scale": 99, "dzch.prefs": {"padScale": -1}}"#).unwrap();
        let s = s.sanitized();
        assert_eq!(s.scale, 1.0);
        assert_eq!(s.ui.pad_scale, 1.15);
    }

    #[test]
    fn writes_and_reads_the_file() {
        let dir = std::env::temp_dir().join(format!("dzch-prefs-{}", std::process::id()));
        let path = Prefs::path_in(&dir);
        let mut p = Prefs::load(path.clone());
        p.edit().locale = Some("fr".into());
        p.flush();
        let q = Prefs::load(path);
        assert_eq!(q.get().locale.as_deref(), Some("fr"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
