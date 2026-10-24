//! The Steam Deck and Steam's Game Mode (`dz_common::deck`: the firmware, gamescope's
//! variables), and what changes under them:
//!
//! - [`open`] hands a web link to Steam's own browser in Game Mode
//!   (`steam://openurl/…`: no other browser can show over gamescope) and refuses a
//!   folder there (no file manager); elsewhere it is the system's opener
//!   ([`detached`], with the child environment cleaned on Linux).
//! - the title bar keeps only its close button and the window no resize edges
//!   (`shell::titlebar`, `shell::window`); closing quits (`platform::life`).
//! - [`cmd_add_to_steam`]: the launcher as a non-Steam game in Steam's library
//!   (`dz_hub::features::steam::shortcut`), the way to reach it from Game Mode.

use std::ffi::OsStr;

use dz_common::deck::Deck;

use crate::app::Ctx;
use crate::i18n::{t, tf};

/// Read the machine and the session now (at start-up, off the frames) and keep them.
pub fn warm() -> Deck {
    dz_common::deck::deck()
}

/// Inside gamescope's session (a Deck's Game Mode).
pub fn game_mode() -> bool {
    dz_common::deck::deck().game_mode
}

/// On a Steam Deck.
pub fn on_deck() -> bool {
    dz_common::deck::deck().hardware
}

/// A folder can be shown: not in Game Mode, where there is no file manager. The
/// open-folder buttons are drawn only while this holds.
pub fn folders() -> bool {
    !game_mode()
}

/// [`folders`], as a command's `enabled`.
pub fn can_open_folders(_: &Ctx) -> bool {
    folders()
}

/// What [`open`] hands the system for `target` in or out of Game Mode: `Some(what)`
/// to open, `None` when Game Mode has nothing to show it with (a folder, a file).
pub fn route(target: &str, game_mode: bool) -> Option<String> {
    if !game_mode {
        return Some(target.to_string());
    }
    let lower = target.get(..8).unwrap_or(target).to_ascii_lowercase();
    if lower.starts_with("steam://") {
        Some(target.to_string())
    } else if lower.starts_with("https://") || lower.starts_with("http://") {
        Some(format!("steam://openurl/{target}"))
    } else {
        None
    }
}

/// Open a link or a folder as the session allows (see the module). A folder in Game
/// Mode is an `Unsupported` error: its buttons are hidden there, a command that still
/// asks gets that answer.
pub fn open(target: impl AsRef<OsStr>) -> std::io::Result<()> {
    let target = target.as_ref().to_string_lossy();
    match route(&target, game_mode()) {
        Some(what) => detached(&what),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            t("pad", "gameModeNoFolders"),
        )),
    }
}

/// Hand `what` (a link, a folder) to the system's opener, which outlives the
/// launcher. On Linux the opener (`xdg-open`, then Steam or the browser) starts
/// without the AppImage's variables nor those a Steam session set for the launcher
/// (its overlay preload, its game id), as `dz_steamcmd::scrub_child_env` does for
/// every other child; the process is reaped off the frames.
pub fn detached(what: &str) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Stdio;
        let mut last = None;
        for mut cmd in open::commands(what) {
            dz_steamcmd::scrub_child_env(&mut cmd);
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            match dz_steamcmd::spawn_detached(&mut cmd) {
                Ok(()) => return Ok(()),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| std::io::Error::other("no opener")))
    }
    #[cfg(not(target_os = "linux"))]
    {
        open::that_detached(what)
    }
}

/// The window's own flags the Steam shortcut passes on: the data folder and the
/// instance, never the control port or `--mock`.
fn own_flags(own: Vec<std::ffi::OsString>) -> String {
    use dz_hub::features::steam::shortcut::launch_arg;
    let mut out = Vec::new();
    let mut it = own.into_iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--control") => {
                it.next();
            }
            Some("--mock") => {}
            _ => out.push(launch_arg(&a.to_string_lossy())),
        }
    }
    out.join(" ")
}

/// `steam.add_shortcut` can run: not in Game Mode, where Steam always runs and would
/// write its own list back over the change.
pub fn can_add_to_steam(_: &Ctx) -> bool {
    !game_mode()
}

/// `steam.add_shortcut`: add the launcher to Steam's library (Steam closed); a toast
/// says it is there, or why not.
pub fn cmd_add_to_steam(cx: &mut Ctx, _: &serde_json::Value) {
    use dz_hub::features::steam::shortcut::{AddToSteamError, AddToSteamRequest};
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            cx.stores
                .bus
                .err(tf("pad", "addToSteamFailed", &[("error", &e)]));
            return;
        }
    };
    // An AppImage runs from a mount that goes with it: the image itself is started.
    #[cfg(target_os = "linux")]
    let exe = std::env::var_os("APPIMAGE")
        .filter(|a| !a.is_empty())
        .map_or(exe, std::path::PathBuf::from);
    let req = AddToSteamRequest {
        name: "DayZ Community Hub".to_string(),
        exe,
        launch_options: own_flags(dz_hub::features::updater::restart_args()),
        icon_png: super::window::PNG,
    };
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    crate::rt::fire(async move {
        match hub.add_to_steam(req).await {
            Ok(a) if a.updated => bus.ok(t("pad", "addToSteamUpdated")),
            Ok(_) => bus.ok(t("pad", "addToSteamDone")),
            Err(e) => {
                let why = match e {
                    AddToSteamError::SteamRunning => t("pad", "addToSteamRunning").to_string(),
                    AddToSteamError::NoSteam => t("pad", "addToSteamNoSteam").to_string(),
                    AddToSteamError::NoAccount => t("pad", "addToSteamNoAccount").to_string(),
                    AddToSteamError::Failed(e) => e,
                };
                bus.err(tf("pad", "addToSteamFailed", &[("error", &why)]));
            }
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn game_mode_sends_links_to_steam_and_has_no_folders() {
        assert_eq!(
            route("https://x.org/a?b", false).unwrap(),
            "https://x.org/a?b"
        );
        assert_eq!(route("/home/deck", false).unwrap(), "/home/deck");
        assert_eq!(
            route("https://x.org/a?b", true).unwrap(),
            "steam://openurl/https://x.org/a?b"
        );
        assert_eq!(
            route("HTTP://x.org", true).unwrap(),
            "steam://openurl/HTTP://x.org"
        );
        assert_eq!(
            route("steam://open/keyboard", true).unwrap(),
            "steam://open/keyboard"
        );
        assert!(route("/home/deck/mods", true).is_none());
        assert!(route("file:///home", true).is_none());
        assert!(route("", true).is_none());
    }

    #[test]
    fn the_shortcut_passes_the_windows_own_flags_only() {
        let own = |v: &[&str]| v.iter().map(std::ffi::OsString::from).collect();
        assert_eq!(
            own_flags(own(&[
                "--data-dir",
                "/home/deck/My Data",
                "--control",
                "4000",
                "--mock"
            ])),
            "--data-dir \"/home/deck/My Data\""
        );
        assert_eq!(own_flags(own(&[])), "");
    }
}
