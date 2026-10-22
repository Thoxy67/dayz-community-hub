//! The window as the system shows it: its icon, and where it was, how big and whether
//! maximised, from one start to the next.
//!
//! - **Geometry.** eframe's persistence saves the window (outer position in physical
//!   pixels, inner size, maximised) in `window.ron` beside the launcher's data (a
//!   temporary directory in mock mode), off the UI thread and only when it changed
//!   (`HubApp::auto_save_interval`, and on exit). At the next start eframe restores it
//!   before the window exists, with the size clamped to the largest monitor and, on
//!   Windows, the position moved onto a connected monitor (a window last seen on a
//!   monitor since unplugged would open where nobody can see it). [`sanitize`] first drops
//!   a file holding values that would hurt (not finite, absurd, or the `-32000` Windows
//!   gives a minimised window), and [`restored`] corrects what eframe hands back: never
//!   full screen, at least the minimum size, and a maximised window gets back the size it
//!   had before it was maximised (kept in the preferences, `dzch.window`), so restoring
//!   it does not leave a screen-sized window. A minimised window is never saved
//!   (`HubApp::save` keeps the last value). Wayland compositors place windows
//!   themselves: there is no position to save.
//! - **Icon.** [`icon`] is the title-bar / taskbar / Alt+Tab icon (`WM_SETICON` on
//!   Windows, `_NET_WM_ICON` on X11); the Windows executable also carries `icon.ico`
//!   (`build.rs`), which Explorer and the registrations show.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{IconData, ViewportBuilder};

use crate::prefs::WindowState;

/// eframe's file, in the data directory.
const FILE_NAME: &str = "window.ron";
/// eframe's key for the window in that file.
const WINDOW_KEY: &str = "window";
/// Coordinates beyond this (pixels or points) are corrupt.
const MAX_COORD: f64 = 100_000.0;
/// Windows parks a minimised window at (-32000, -32000); anything this far out is that.
const MINIMISED_POS: f64 = 30_000.0;

/// The 256 px app icon (the Tauri app's `icons/128x128@2x.png`).
const PNG: &[u8] = include_bytes!("../../assets/app/icon-256.png");

/// The window's icon (an empty one if the bundled PNG cannot be read, which a test
/// rules out).
pub fn icon() -> IconData {
    eframe::icon_data::from_png_bytes(PNG).unwrap_or_default()
}

/// `window.ron` in `dir`.
pub fn path_in(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Remove the file at `path` if its window would be wrong to restore. Blocking: call
/// it before the window opens. Never fails (an error is printed).
pub fn sanitize(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    if let Err(why) = check(&text) {
        eprintln!("window: ignoring {}: {why}", path.display());
        if let Err(e) = std::fs::remove_file(path) {
            eprintln!("window: cannot remove {}: {e}", path.display());
        }
    }
}

/// Whether the file at `path` holds a position eframe will restore; then the window
/// must not also be centred (eframe centres after restoring).
pub fn has_saved_position(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| window(&t))
        .and_then(|w| field(&w, "outer_position_pixels").cloned())
        .is_some_and(|p| matches!(p, ron::Value::Option(Some(_))))
}

/// The window eframe restores, corrected before it is created (eframe's
/// `NativeOptions::window_builder`). `saved` is the preferences' geometry.
pub fn restored(builder: ViewportBuilder, saved: Option<WindowState>) -> ViewportBuilder {
    let mut b = builder.with_fullscreen(false);
    // Saved maximised, eframe's size is the maximised one: restoring would leave a window
    // as big as the screen. The size before maximising, if smaller (eframe's is already
    // clamped to the monitor).
    if b.maximized == Some(true)
        && let (Some(size), Some(w)) = (b.inner_size, saved)
    {
        let before = egui::vec2(w.size[0], w.size[1]);
        if before.x.is_finite() && before.y.is_finite() {
            b.inner_size = Some(before.min(size));
        }
    }
    // Windows saves a minimised window's size (eframe clamps it to 64 pt); the minimum
    // size is not applied to a restored one. Grow it about its centre.
    if let (Some(size), Some(min)) = (b.inner_size, b.min_inner_size) {
        let grown = size.max(min);
        if grown != size {
            b.position = b.position.map(|p| p - (grown - size) / 2.0);
            b.inner_size = Some(grown);
        }
    }
    b
}

/// The window entry of eframe's file, if it has one that parses.
fn window(text: &str) -> Option<ron::Value> {
    let kv = ron::from_str::<HashMap<String, String>>(text).ok()?;
    ron::from_str::<ron::Value>(kv.get(WINDOW_KEY)?).ok()
}

/// `Err` says why the saved window must not be restored. A file eframe cannot read
/// (not RON, no window) is fine: eframe ignores it.
fn check(text: &str) -> Result<(), String> {
    let Some(w) = window(text) else {
        return Ok(());
    };
    for key in [
        "outer_position_pixels",
        "inner_position_pixels",
        "inner_size_points",
    ] {
        let Some(ron::Value::Option(Some(v))) = field(&w, key) else {
            continue;
        };
        for axis in ["x", "y"] {
            let f = field(v, axis).and_then(as_f64);
            match f {
                Some(f) if !f.is_finite() || f.abs() > MAX_COORD => {
                    return Err(format!("{key}.{axis} = {f}"));
                }
                Some(f) if key != "inner_size_points" && f <= -MINIMISED_POS => {
                    return Err(format!("{key}: saved minimised"));
                }
                Some(f) if key == "inner_size_points" && f < 1.0 => {
                    return Err(format!("{key}.{axis} = {f}"));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn as_f64(v: &ron::Value) -> Option<f64> {
    match v {
        ron::Value::Number(n) => Some(n.into_f64()),
        _ => None,
    }
}

fn field<'a>(v: &'a ron::Value, name: &str) -> Option<&'a ron::Value> {
    match v {
        ron::Value::Map(m) => m.get(&ron::Value::String(name.into())),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// eframe's file as it writes it: a map of key → RON string.
    fn file(window: &str) -> String {
        format!("{{\"window\":{window:?}}}")
    }

    const WINDOW: &str = "(inner_position_pixels:Some((x:101.0,y:132.0)),outer_position_pixels:Some((x:100.0,y:100.0)),fullscreen:false,maximized:false,inner_size_points:Some((x:1200.0,y:780.0)))";

    #[test]
    fn the_icon_decodes() {
        let i = icon();
        assert_eq!((i.width, i.height), (256, 256));
        assert_eq!(i.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn a_saved_window_is_kept() {
        assert_eq!(check(&file(WINDOW)), Ok(()));
        assert_eq!(check("not ron at all"), Ok(()));
        assert_eq!(check("{}"), Ok(()));
        // A second monitor left of the main one: negative but real.
        assert_eq!(
            check(&file(&WINDOW.replace("x:100.0", "x:-1820.0"))),
            Ok(())
        );
    }

    #[test]
    fn minimised_or_corrupt_windows_are_dropped() {
        let minimised = WINDOW
            .replace("(x:100.0,y:100.0)", "(x:-32000.0,y:-32000.0)")
            .replace("(x:101.0,y:132.0)", "(x:-32000.0,y:-32000.0)");
        assert!(check(&file(&minimised)).is_err());
        for size in ["(x:NaN,y:780.0)", "(x:1200.0,y:1e30)", "(x:0.0,y:0.0)"] {
            let w = WINDOW.replace("(x:1200.0,y:780.0)", size);
            assert!(check(&file(&w)).is_err(), "{size}");
        }
    }

    #[test]
    fn sanitize_removes_only_a_bad_file() {
        let dir = std::env::temp_dir().join(format!("dzch-window-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = path_in(&dir);
        std::fs::write(&path, file(WINDOW)).unwrap();
        sanitize(&path);
        assert!(path.exists());
        assert!(has_saved_position(&path));
        std::fs::write(&path, file(&WINDOW.replace("x:1200.0", "x:inf"))).unwrap();
        sanitize(&path);
        assert!(!path.exists());
        assert!(!has_saved_position(&path));
        // Wayland: no position saved, so the window is centred.
        std::fs::write(
            &path,
            file("(fullscreen:false,maximized:false,inner_size_points:Some((x:1200.0,y:780.0)))"),
        )
        .unwrap();
        assert!(!has_saved_position(&path));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restored_corrects_what_eframe_hands_back() {
        let saved = Some(WindowState {
            pos: None,
            size: [1100.0, 700.0],
            maximized: true,
        });
        let base = ViewportBuilder::default().with_min_inner_size([900.0, 600.0]);
        // Maximised: the size before maximising, within eframe's.
        let b = restored(
            base.clone()
                .with_inner_size([1920.0, 1040.0])
                .with_maximized(true)
                .with_fullscreen(true),
            saved,
        );
        assert_eq!(b.inner_size, Some(egui::vec2(1100.0, 700.0)));
        assert_eq!(b.fullscreen, Some(false));
        // A strip saved while minimised grows to the minimum about its centre.
        let b = restored(
            base.clone()
                .with_inner_size([160.0, 64.0])
                .with_position([500.0, 400.0]),
            None,
        );
        assert_eq!(b.inner_size, Some(egui::vec2(900.0, 600.0)));
        assert_eq!(b.position, Some(egui::pos2(130.0, 132.0)));
        // A normal window is left as it was.
        let b = restored(base.with_inner_size([1200.0, 780.0]), saved);
        assert_eq!(b.inner_size, Some(egui::vec2(1200.0, 780.0)));
    }
}
