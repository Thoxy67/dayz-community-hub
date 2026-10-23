//! Whether the launcher runs on a Steam Deck, and inside Steam's Game Mode.
//!
//! - **The hardware**: the firmware names Valve as the board's maker and the
//!   product `Jupiter` (the LCD Deck) or `Galileo` (the OLED one), in
//!   `/sys/class/dmi/id`. Steam's Game Mode also sets `SteamDeck=1` for what it
//!   starts, which counts too (a Deck whose firmware files cannot be read).
//! - **Game Mode**: the session runs in gamescope, Steam's own compositor; it
//!   says so through `XDG_CURRENT_DESKTOP=gamescope` or by handing its clients
//!   `GAMESCOPE_WAYLAND_DISPLAY`. Big Picture on a desktop is not Game Mode:
//!   windows there still have a window manager.
//!
//! Read once (a few bytes from sysfs and the environment) and kept: neither
//! changes while the launcher runs. Anything unreadable reads as "no".

use std::path::Path;
use std::sync::OnceLock;

/// What the machine and the session are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Deck {
    /// A Steam Deck (its firmware says so, or Steam's Game Mode does).
    pub hardware: bool,
    /// Inside gamescope's session (a Deck's Game Mode, or SteamOS's on any
    /// machine): one full-screen window at a time, no window manager, no
    /// file manager, no browser beside Steam's own.
    pub game_mode: bool,
}

/// The machine and the session, read the first time and kept.
pub fn deck() -> Deck {
    static KEPT: OnceLock<Deck> = OnceLock::new();
    *KEPT.get_or_init(|| detect(Path::new("/sys/class/dmi/id"), &|k| std::env::var(k).ok()))
}

/// [`deck`] from a DMI folder and an environment (tests hand their own).
pub fn detect(dmi: &Path, env: &dyn Fn(&str) -> Option<String>) -> Deck {
    let flag = |k: &str| env(k).is_some_and(|v| !v.is_empty() && v != "0");
    let read = |f: &str| {
        std::fs::read_to_string(dmi.join(f))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    let firmware = cfg!(target_os = "linux")
        && read("board_vendor").eq_ignore_ascii_case("valve")
        && matches!(
            read("product_name").to_ascii_lowercase().as_str(),
            "jupiter" | "galileo"
        );
    let game_mode = env("XDG_CURRENT_DESKTOP")
        .is_some_and(|d| d.split(':').any(|p| p.eq_ignore_ascii_case("gamescope")))
        || env("GAMESCOPE_WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
    Deck {
        hardware: firmware || flag("SteamDeck"),
        game_mode,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    /// `detect` on a DMI folder holding `vendor` and `product`, removed after.
    fn on(vendor: &str, product: &str, env: &dyn Fn(&str) -> Option<String>) -> Deck {
        let d = std::env::temp_dir().join(format!(
            "dzch-dmi-{}-{vendor}-{product}",
            std::process::id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("board_vendor"), format!("{vendor}\n")).unwrap();
        std::fs::write(d.join("product_name"), format!("{product}\n")).unwrap();
        let found = detect(&d, env);
        std::fs::remove_dir_all(&d).unwrap();
        found
    }

    #[test]
    fn the_firmware_names_a_deck() {
        let none = env(&[]);
        if cfg!(target_os = "linux") {
            assert!(on("Valve", "Jupiter", &none).hardware);
            assert!(on("Valve", "Galileo", &none).hardware);
        }
        assert!(!on("Valve", "Index", &none).hardware);
        assert!(!on("ASUSTeK", "Jupiter", &none).hardware);
        assert!(!detect(Path::new("/nonexistent/dzch"), &none).hardware);
    }

    #[test]
    fn steam_and_gamescope_say_the_rest() {
        let nowhere = Path::new("/nonexistent/dzch");
        assert!(detect(nowhere, &env(&[("SteamDeck", "1")])).hardware);
        assert!(!detect(nowhere, &env(&[("SteamDeck", "0")])).hardware);
        assert!(detect(nowhere, &env(&[("XDG_CURRENT_DESKTOP", "gamescope")])).game_mode);
        assert!(detect(nowhere, &env(&[("XDG_CURRENT_DESKTOP", "KDE:gamescope")])).game_mode);
        assert!(
            detect(
                nowhere,
                &env(&[("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")])
            )
            .game_mode
        );
        assert!(!detect(nowhere, &env(&[("XDG_CURRENT_DESKTOP", "KDE")])).game_mode);
        assert_eq!(detect(nowhere, &env(&[])), Deck::default());
    }
}
