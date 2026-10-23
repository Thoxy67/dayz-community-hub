//! "Add to Steam": the launcher as a non-Steam game in the account's library,
//! so that a Steam Deck's Game Mode (or Big Picture) starts it with the pad
//! configured by Steam Input. The file is Steam's `shortcuts.vdf`
//! (`dz_steamcmd::shortcuts`); the icon is written into the data folder.
//!
//! Steam keeps the list in memory and writes it back over the file when it
//! exits: the shortcut is only written while Steam is closed
//! ([`AddToSteamError::SteamRunning`]), and shows the next time Steam starts.

use std::path::PathBuf;

use serde::Serialize;

use crate::Hub;

/// What the window asks for: the launcher as it runs now.
#[derive(Clone, Debug)]
pub struct AddToSteamRequest {
    pub name: String,
    /// The program to start: the AppImage, or the executable.
    pub exe: PathBuf,
    /// Arguments after the program, ready for Steam's launch options.
    pub launch_options: String,
    /// The icon, as PNG bytes; written to `<data dir>/steam-icon.png`.
    pub icon_png: &'static [u8],
}

/// Where the shortcut went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct AddedToSteam {
    /// The shortcut's id in Steam (art and `steam://rungameid` links use it).
    pub id: u32,
    /// The launcher was in the library already: its entry was brought up to date.
    pub updated: bool,
}

/// Why nothing was written; the window says it in its words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum AddToSteamError {
    /// Steam runs: it would write its own list back over the file.
    SteamRunning,
    /// No Steam install found.
    NoSteam,
    /// Steam was never logged in, or several accounts and none was last.
    NoAccount,
    /// The system's own error.
    Failed(String),
}

/// Quote a launch option as Steam's command line wants it (a space or a quote
/// inside one would split or end it): a shell's double quotes on Linux, the
/// Windows command line's rules on Windows.
pub fn launch_arg(s: &str) -> String {
    quote_arg(s, cfg!(windows))
}

/// [`launch_arg`] for either system (tests check both).
fn quote_arg(s: &str, windows: bool) -> String {
    let plain = |c: char| !(c.is_whitespace() || c == '"' || (c == '\\' && !windows));
    if !s.is_empty() && s.chars().all(plain) {
        return s.to_string();
    }
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    if windows {
        // Backslashes are themselves unless a quote follows: then each is doubled
        // and the quote escaped (the closing quote counts too).
        let mut run = 0usize;
        for c in s.chars().filter(|c| !c.is_control()) {
            match c {
                '\\' => run += 1,
                '"' => {
                    q.extend(std::iter::repeat_n('\\', run * 2 + 1));
                    q.push('"');
                    run = 0;
                }
                _ => {
                    q.extend(std::iter::repeat_n('\\', run));
                    q.push(c);
                    run = 0;
                }
            }
        }
        q.extend(std::iter::repeat_n('\\', run * 2));
    } else {
        for c in s.chars() {
            if c == '"' || c == '\\' {
                q.push('\\');
            }
            if !c.is_control() {
                q.push(c);
            }
        }
    }
    q.push('"');
    q
}

impl Hub {
    /// Add the launcher to Steam's library (see the module). Never from the
    /// invented world: Steam is the real one.
    pub async fn add_to_steam(
        &self,
        req: AddToSteamRequest,
    ) -> Result<AddedToSteam, AddToSteamError> {
        mocked!(self => Ok(AddedToSteam { id: 0x8000_0001, updated: false }));
        let data = crate::paths::data_dir();
        tokio::task::spawn_blocking(move || {
            use dz_steamcmd::shortcuts::{self, Added, Shortcut};
            if dz_steamcmd::SteamClient::is_running() {
                return Err(AddToSteamError::SteamRunning);
            }
            let steam = dz_steamcmd::steam_install_dir().ok_or(AddToSteamError::NoSteam)?;
            let account = shortcuts::account(&steam).ok_or(AddToSteamError::NoAccount)?;
            let icon = data.join("steam-icon.png");
            let icon = std::fs::create_dir_all(&data)
                .and_then(|()| std::fs::write(&icon, req.icon_png))
                .ok()
                .map(|()| icon);
            let start_dir = req
                .exe
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| data.clone());
            let s = Shortcut {
                name: req.name,
                exe: req.exe,
                start_dir,
                icon,
                launch_options: req.launch_options,
            };
            match shortcuts::add(&steam, account, &s) {
                Ok(Added::New(id)) => Ok(AddedToSteam { id, updated: false }),
                Ok(Added::Updated(id)) => Ok(AddedToSteam { id, updated: true }),
                Err(e) => Err(AddToSteamError::Failed(e)),
            }
        })
        .await
        .map_err(|e| AddToSteamError::Failed(format!("task: {e}")))?
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::quote_arg;

    #[test]
    fn launch_options_stay_whole() {
        let unix = |s| quote_arg(s, false);
        assert_eq!(unix("--instance"), "--instance");
        assert_eq!(unix("/home/deck/My Games"), "\"/home/deck/My Games\"");
        assert_eq!(unix(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(unix(""), "\"\"");
    }

    #[test]
    fn windows_paths_keep_their_backslashes() {
        let win = |s| quote_arg(s, true);
        assert_eq!(win(r"C:\Users\me\d"), r"C:\Users\me\d");
        assert_eq!(win(r"\\server\share"), r"\\server\share");
        assert_eq!(win(r"C:\My Games\"), r#""C:\My Games\\""#);
        assert_eq!(win(r#"a"b\c"#), r#""a\"b\c""#);
        assert_eq!(win(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(win(""), "\"\"");
    }
}
