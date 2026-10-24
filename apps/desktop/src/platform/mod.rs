//! What the operating system hands the launcher, and what it tells the system about
//! itself (as Tauri's single-instance and deep-link plugins and `dzch_cli` did):
//!
//! - [`deck`]: a Steam Deck and Steam's Game Mode, links through Steam's browser
//!   there, and "Add to Steam".
//! - [`instance`]: one window per user. A second launch hands its arguments (a
//!   `dzch://` link, a `.dzch` file, `--connect`, `--reconnect`) to the first over
//!   127.0.0.1, keyed by a per-user file with a random token, and exits; the first
//!   raises its window and handles them as `HubEvent::CliArgs`.
//! - [`life`]: closing to the tray, what the launcher does when DayZ starts and
//!   stops, and when to notify; [`tray`] the icon and its menu (Linux: ksni, Windows:
//!   tray-icon), [`notify`] the system notifications.
//! - [`register`]: `dzch://` links and `.dzch` files open this executable (Linux: a
//!   handler `.desktop` file, a shared-mime-info package and `xdg-mime`; Windows: HKCU
//!   classes), written at start only when they differ.
//! - [`shortcut`]: a desktop shortcut that joins one server (`--connect`).
//! - [`window`]: the window's icon, and its size, position and maximised state from one
//!   start to the next.
//!
//! Paste-to-connect needs no platform code: egui turns Ctrl+V into `Event::Paste`,
//! which `HubApp` hands to `Ctx::paste` outside text fields.

pub mod deck;
pub mod instance;
pub mod life;
pub mod notify;
pub mod register;
pub mod shortcut;
pub mod tray;
pub mod window;

use std::path::Path;

use dz_hub::CliArgs;

/// Arguments as the window wants them, from where the launch ran: a `file://` URL (a
/// desktop's `%u`) becomes its path, a file named relative to `cwd` becomes absolute.
/// A `dzch://` link is kept as it is.
pub fn normalize(mut args: CliArgs, cwd: &Path) -> CliArgs {
    if let Some(open) = args.open.take() {
        let open = open.trim().to_string();
        let open = file_url_path(&open).unwrap_or(open);
        let open =
            if open.is_empty() || open.starts_with("dzch://") || !Path::new(&open).is_relative() {
                open
            } else {
                cwd.join(&open).to_string_lossy().into_owned()
            };
        args.open = (!open.is_empty()).then_some(open);
    }
    args
}

/// Whether the arguments ask for anything (a bare second launch only raises the window).
pub fn asks_something(args: &CliArgs) -> bool {
    args.open.is_some() || args.connect.is_some() || args.reconnect
}

/// The path of a `file://` URL (`file:///a%20b.dzch`, `file://localhost/…`,
/// `file:///C:/…` on Windows), percent-decoded; `None` for anything else.
fn file_url_path(s: &str) -> Option<String> {
    let rest = s
        .get(..7)
        .filter(|p| p.eq_ignore_ascii_case("file://"))
        .and_then(|_| s.get(7..))?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(rest)?;
    // `/C:/x` is `C:/x` on Windows.
    if let [b'/', drive, b':', ..] = decoded.as_bytes()
        && drive.is_ascii_alphabetic()
    {
        return decoded.get(1..).map(str::to_string);
    }
    Some(decoded)
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        if c == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn open(s: &str) -> Option<String> {
        normalize(
            CliArgs {
                open: Some(s.into()),
                ..CliArgs::none()
            },
            Path::new("/home/me"),
        )
        .open
    }

    #[test]
    fn files_and_links_reach_the_window_as_it_wants_them() {
        assert_eq!(
            open("dzch://1.2.3.4:2302").as_deref(),
            Some("dzch://1.2.3.4:2302")
        );
        assert_eq!(
            open("a.dzch").map(std::path::PathBuf::from),
            Some(Path::new("/home/me").join("a.dzch"))
        );
        assert_eq!(open("/srv/a.dzch").as_deref(), Some("/srv/a.dzch"));
        assert_eq!(
            open("file:///srv/a%20b.dzch").as_deref(),
            Some("/srv/a b.dzch")
        );
        assert_eq!(
            open("FILE://localhost/srv/x.dzch").as_deref(),
            Some("/srv/x.dzch")
        );
        assert_eq!(file_url_path("file:///C:/x%2Ey").as_deref(), Some("C:/x.y"));
        assert_eq!(file_url_path("file:///bad%zz"), None);
        assert_eq!(file_url_path("file:///cut%2"), None);
        assert_eq!(file_url_path("https://x"), None);
        assert_eq!(open("  "), None);
    }

    #[test]
    fn a_bare_launch_asks_nothing() {
        assert!(!asks_something(&CliArgs::none()));
        assert!(asks_something(&CliArgs {
            reconnect: true,
            ..CliArgs::none()
        }));
    }
}
