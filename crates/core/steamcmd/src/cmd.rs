//! The steamcmd install this app drives, and where it puts things.
//!
//! SteamCMD is a Steam installation of its own. Pointed at a Steam client's
//! library (`+force_install_dir`), it records itself there: its workshop
//! manifest and library identity overwrite the client's, and the client may
//! then drop that library. So it downloads into a directory of the
//! launcher's (`content_dir`), and the Steam client's libraries are only
//! ever read.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::DAYZ_GAME_ID;

pub struct SteamCmd {
    pub(crate) steamcmd_path: PathBuf,
    /// The Steam client's `steamapps` directory that holds DayZ. Read only.
    pub(crate) steam_root: PathBuf,
    /// SteamCMD's own install directory (`+force_install_dir`).
    pub(crate) content_dir: PathBuf,
    pub(crate) login: String,
    /// A password saved by an earlier version, typed once at SteamCMD's
    /// `password:` prompt so that login is cached; never on the command line.
    pub(crate) saved_password: Option<String>,
    /// `HOME` for the SteamCMD process (Linux). With the real one, SteamCMD
    /// follows `~/.steam/root` into the Steam client's directory and keeps
    /// its config and logs there; its own `HOME` gives it a Steam install of
    /// its own.
    pub(crate) home_dir: Option<PathBuf>,
    pub(crate) game_id: u32,
}

impl SteamCmd {
    pub fn new(
        steamcmd_path: impl AsRef<Path>,
        steam_root: impl AsRef<Path>,
        content_dir: impl AsRef<Path>,
        login: Option<String>,
    ) -> Self {
        let login = login.unwrap_or_else(|| "anonymous".to_string());
        Self {
            steamcmd_path: steamcmd_path.as_ref().to_path_buf(),
            steam_root: steam_root.as_ref().to_path_buf(),
            content_dir: content_dir.as_ref().to_path_buf(),
            login,
            saved_password: None,
            home_dir: None,
            game_id: DAYZ_GAME_ID,
        }
    }

    /// A password saved by an earlier version: answered once at the prompt.
    pub fn with_saved_password(mut self, password: Option<String>) -> Self {
        self.saved_password = password.filter(|p| !p.is_empty());
        self
    }

    /// The `HOME` SteamCMD runs with (ignored on Windows, where SteamCMD
    /// keeps everything beside its executable).
    pub fn with_home(mut self, home: Option<PathBuf>) -> Self {
        self.home_dir = home;
        self
    }

    pub fn login(&self) -> &str {
        &self.login
    }

    pub fn steam_root(&self) -> &Path {
        &self.steam_root
    }

    /// SteamCMD's own install directory.
    pub fn content_dir(&self) -> &Path {
        &self.content_dir
    }

    /// Returns true if the login is non-anonymous (required for workshop downloads).
    pub fn has_real_login(&self) -> bool {
        !self.login.is_empty() && self.login != "anonymous"
    }

    /// Where SteamCMD puts workshop items: `<content_dir>/steamapps/workshop/content/221100/`.
    pub fn workshop_path(&self) -> PathBuf {
        workshop_content(&self.content_dir.join("steamapps"), self.game_id)
    }

    /// Path to the DayZ game directory: `steamapps/common/DayZ`
    pub fn dayz_path(&self) -> PathBuf {
        self.steam_root.join("common").join("DayZ")
    }

    /// The command line for a session: install into `content_dir`, log in by
    /// name only (a password is typed at the prompt, never passed as an
    /// argument), download `items`, quit. `validate` re-checks every file of
    /// every item, which is slow: only for an explicit repair.
    pub(crate) fn session_args(&self, items: &[u64], validate: bool) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "+@ShutdownOnFailedCommand".into(),
            "0".into(),
            "+force_install_dir".into(),
            self.content_dir.as_os_str().into(),
            "+login".into(),
            (&self.login).into(),
        ];
        for id in items {
            args.push("+workshop_download_item".into());
            args.push(self.game_id.to_string().into());
            args.push(id.to_string().into());
            if validate {
                args.push("validate".into());
            }
        }
        args.push("+quit".into());
        args
    }

    /// An earlier version made `<steamcmd>/steamapps` (Windows) a junction
    /// to the Steam client's library, so SteamCMD wrote its manifests into
    /// it. Undo that: only the link goes, never what it points to.
    pub(crate) fn detach_steam_library_link(&self) {
        let Some(dir) = self.steamcmd_path.parent() else {
            return;
        };
        let link = dir.join("steamapps");
        if is_link(&link) {
            // On a symlink `remove_file`, on a junction `remove_dir`: both
            // remove the link itself.
            let _ = std::fs::remove_file(&link).or_else(|_| std::fs::remove_dir(&link));
        }
    }
}

/// `<steamapps>/workshop/content/<game>`.
pub(crate) fn workshop_content(steamapps: &Path, game_id: u32) -> PathBuf {
    steamapps
        .join("workshop")
        .join("content")
        .join(game_id.to_string())
}

/// A symlink, or on Windows an NTFS junction (a reparse point).
fn is_link(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd() -> SteamCmd {
        SteamCmd::new(
            "/usr/bin/steamcmd",
            "/mnt/games/SteamLibrary/steamapps",
            "/home/u/.local/share/dayz-community-hub/steamcmd-content",
            Some("player".into()),
        )
        .with_saved_password(Some("hunter2".into()))
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_password_is_never_an_argument() {
        let args = strings(&cmd().session_args(&[1, 2], false));
        assert!(!args.iter().any(|a| a.contains("hunter2")), "{args:?}");
        let login = args.iter().position(|a| a == "+login").unwrap();
        assert_eq!(args[login + 1], "player");
        assert!(args[login + 2].starts_with('+'), "{args:?}");
    }

    #[test]
    fn it_installs_into_its_own_directory() {
        let c = cmd();
        let args = strings(&c.session_args(&[1], false));
        let at = args.iter().position(|a| a == "+force_install_dir").unwrap();
        assert_eq!(
            args[at + 1],
            "/home/u/.local/share/dayz-community-hub/steamcmd-content"
        );
        assert!(at < args.iter().position(|a| a == "+login").unwrap());
        assert!(!args.iter().any(|a| a.contains("SteamLibrary")), "{args:?}");
        assert_eq!(
            c.workshop_path(),
            Path::new("/home/u/.local/share/dayz-community-hub/steamcmd-content")
                .join("steamapps/workshop/content/221100")
        );
        // The game itself is still the Steam client's.
        assert!(
            c.dayz_path()
                .starts_with("/mnt/games/SteamLibrary/steamapps")
        );
    }

    #[test]
    fn validate_only_on_request() {
        let plain = strings(&cmd().session_args(&[11, 22], false));
        assert!(!plain.iter().any(|a| a == "validate"));
        assert_eq!(plain.last().map(String::as_str), Some("+quit"));
        let repair = strings(&cmd().session_args(&[11, 22], true));
        assert_eq!(repair.iter().filter(|a| *a == "validate").count(), 2);
        let item = repair.iter().position(|a| a == "11").unwrap();
        assert_eq!(repair[item - 1], "221100");
        assert_eq!(repair[item + 1], "validate");
    }

    #[test]
    fn a_login_session_downloads_nothing() {
        let args = strings(&cmd().session_args(&[], false));
        assert!(!args.iter().any(|a| a == "+workshop_download_item"));
        assert_eq!(&args[args.len() - 3..], ["+login", "player", "+quit"]);
    }

    #[cfg(unix)]
    #[test]
    fn only_the_link_to_a_steam_library_goes() {
        let tmp = std::env::temp_dir().join(format!("dzch-detach-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let library = tmp.join("library");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::write(library.join("libraryfolders.vdf"), "x").unwrap();
        let home = tmp.join("steamcmd");
        std::fs::create_dir_all(&home).unwrap();
        std::os::unix::fs::symlink(&library, home.join("steamapps")).unwrap();
        let c = SteamCmd::new(home.join("steamcmd.sh"), &library, tmp.join("c"), None);
        c.detach_steam_library_link();
        assert!(home.join("steamapps").symlink_metadata().is_err());
        assert!(library.join("libraryfolders.vdf").is_file());
        // A real directory is SteamCMD's own: left alone.
        std::fs::create_dir(home.join("steamapps")).unwrap();
        c.detach_steam_library_link();
        assert!(home.join("steamapps").is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
