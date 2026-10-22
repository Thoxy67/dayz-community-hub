//! The Steam client itself: find it and start it. The launcher never closes
//! it: SteamCMD runs beside it with its own install directory.

use dz_common::Result;
use std::path::PathBuf;
use std::process::Stdio;

#[cfg(target_os = "windows")]
use crate::detect::query_steam_registry_path;

/// Steam client management (detect, start).
pub struct SteamClient;

impl SteamClient {
    /// Returns the Steam executable name for the current platform.
    pub fn steam_exe() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "steam.exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "steam"
        }
    }

    /// How to run the Steam client: the program, and the arguments that come
    /// before Steam's own. `None` when Steam is nowhere to be found.
    ///
    /// On Windows `steam.exe` lives at `<SteamPath>\steam.exe` (from the
    /// registry) and is *not* on `PATH`. On Linux `steam` is on `PATH` for
    /// most installs, but not for Debian's `/usr/games` in some desktop
    /// sessions, and not at all for Flatpak Steam, which runs through
    /// `flatpak run com.valvesoftware.Steam`.
    pub fn launcher() -> Option<(PathBuf, Vec<String>)> {
        #[cfg(target_os = "windows")]
        {
            // PATH first (rare, but honour it if a user added Steam there).
            if let Ok(p) = which::which("steam.exe") {
                return Some((p, Vec::new()));
            }
            // Registry is the reliable source for non-default install drives.
            if let Some(steam_path) = query_steam_registry_path() {
                let candidate = PathBuf::from(&steam_path).join("steam.exe");
                if candidate.exists() {
                    return Some((candidate, Vec::new()));
                }
            }
            [
                "C:\\Program Files (x86)\\Steam\\steam.exe",
                "C:\\Program Files\\Steam\\steam.exe",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.exists())
            .map(|p| (p, Vec::new()))
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(p) = which::which("steam") {
                return Some((p, Vec::new()));
            }
            for p in [
                "/usr/games/steam",
                "/usr/bin/steam",
                "/usr/local/bin/steam",
                "/snap/bin/steam",
                "/var/lib/snapd/snap/bin/steam",
            ] {
                let p = PathBuf::from(p);
                if p.is_file() {
                    return Some((p, Vec::new()));
                }
            }
            let flatpak_data = std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".var/app/com.valvesoftware.Steam"));
            if flatpak_data.is_some_and(|d| d.is_dir())
                && let Ok(flatpak) = which::which("flatpak")
            {
                return Some((
                    flatpak,
                    vec!["run".into(), "com.valvesoftware.Steam".into()],
                ));
            }
            None
        }
    }

    /// The error for a machine without a Steam client the launcher can find.
    pub fn not_found() -> dz_common::Error {
        dz_common::Error::Other(
            "Steam was not found. Install the Steam client (or start it once), then try again."
                .into(),
        )
    }

    /// Check if the Steam client is currently running: a look at the
    /// process table (shared, at most a second old), without connecting to
    /// Steam. Blocking.
    pub fn is_running() -> bool {
        with_processes(STEAM_MAX_AGE, |system| {
            system.processes().values().any(|process| {
                let name = process.name().to_string_lossy();
                // "steam" — native Linux / macOS / Flatpak entry-point.
                // "steam.exe" — Windows and Wine/Proton on Linux.
                // Exclude "steamwebhelper" — it can outlive a crashed client.
                // Case-insensitive compare avoids allocating a lowercased String
                // for every process in the table on every poll.
                name.eq_ignore_ascii_case("steam") || name.eq_ignore_ascii_case("steam.exe")
            })
        })
    }

    /// Start Steam in silent mode (no friends UI).
    ///
    /// Returns `true` if Steam was already running, `false` if it was just
    /// started (so the caller knows whether to wait for it to become ready).
    pub fn start() -> Result<bool> {
        if Self::is_running() {
            return Ok(true);
        }

        let (program, pre) = Self::launcher().ok_or_else(Self::not_found)?;
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            // On Windows spawn steam.exe directly; no nohup equivalent needed
            // because detached processes persist after the parent exits.
            std::process::Command::new(program)
                .args(&pre)
                .arg("-nofriendsui")
                .arg("-silent")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(dz_common::CREATE_NO_WINDOW)
                .spawn()?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            // On Linux/macOS use nohup so Steam keeps running if the spawner exits.
            let mut cmd = std::process::Command::new("nohup");
            cmd.arg(program)
                .args(&pre)
                .arg("-nofriendsui")
                .arg("-silent")
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            for v in appimage_env() {
                cmd.env_remove(v);
            }
            cmd.spawn()?;
        }

        Ok(false)
    }
}

/// Variables an AppImage's AppRun sets for the launcher itself. Steam, or a
/// file manager, started from the launcher must not inherit them: they point
/// into the AppImage's mount, which goes away when the launcher closes, and
/// load the AppImage's GTK modules into programs built against the system's.
/// Empty outside an AppImage.
pub fn appimage_env() -> &'static [&'static str] {
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_some() {
        return &[
            "APPDIR",
            "APPIMAGE",
            "ARGV0",
            "OWD",
            "LD_LIBRARY_PATH",
            "GDK_BACKEND",
            "GDK_PIXBUF_MODULE_FILE",
            "GDK_PIXBUF_MODULEDIR",
            "GIO_MODULE_DIR",
            "GIO_EXTRA_MODULES",
            "GSETTINGS_SCHEMA_DIR",
            "GTK_PATH",
            "GTK_EXE_PREFIX",
            "GTK_DATA_PREFIX",
            "GTK_IM_MODULE_FILE",
            "PYTHONHOME",
            "PYTHONPATH",
            "PERLLIB",
            "QT_PLUGIN_PATH",
        ];
    }
    &[]
}

/// The game's own processes, beside the Steam client that starts them: the
/// launcher Steam starts, the BattlEye one and the game itself. On Windows
/// their names say so. Under Proton they do not: the game shows as
/// `enfMain` (the engine's main thread), and only the program at the head of
/// its command line (`S:\common\DayZ\DayZ_x64.exe`) gives it away.
pub struct DayzGame;

impl DayzGame {
    /// Process names that are DayZ, compared without case.
    const NAMES: &'static [&'static str] = &[
        "dayz_x64.exe",
        "dayz_be.exe",
        "dayz.exe",
        "dayzlauncher.exe",
        "dayzdiag_x64.exe",
    ];

    fn is_dayz(name: &std::ffi::OsStr) -> bool {
        let name = name.to_string_lossy();
        Self::NAMES.iter().any(|n| name.eq_ignore_ascii_case(n))
    }

    /// The process is DayZ: by its name, or by the program its command line
    /// starts with (a Windows path under Proton, a Unix one otherwise).
    fn is_game(p: &sysinfo::Process) -> bool {
        Self::is_dayz(p.name())
            || p.cmd().first().is_some_and(|first| {
                let first = first.to_string_lossy();
                let base = first.rsplit(['/', '\\']).next().unwrap_or(&first);
                Self::is_dayz(std::ffi::OsStr::new(base))
            })
    }

    /// DayZ is running. Blocking (a look at the shared process table, at
    /// most two seconds old: the window, the play tracker and BattlEye's
    /// repair all ask, and share one scan).
    pub fn is_running() -> bool {
        with_processes(GAME_MAX_AGE, |system| {
            system.processes().values().any(Self::is_game)
        })
    }

    /// The game's processes, from a table read now.
    fn game_pids() -> Vec<sysinfo::Pid> {
        with_processes(std::time::Duration::ZERO, |system| {
            system
                .processes()
                .iter()
                .filter(|(_, p)| Self::is_game(p))
                .map(|(pid, _)| *pid)
                .collect()
        })
    }

    /// Close DayZ: asked to quit first where the system allows it, then
    /// killed if it is still there a moment later. Returns how many processes
    /// were stopped. Blocking.
    pub fn kill() -> usize {
        let targets = Self::game_pids();
        if targets.is_empty() {
            return 0;
        }
        // SIGTERM on Unix lets Wine and the game close cleanly; Windows has
        // no such request, so `kill_with` returns None and it is killed below.
        with_processes(std::time::Duration::MAX, |system| {
            for pid in &targets {
                if let Some(p) = system.process(*pid) {
                    let _ = p.kill_with(sysinfo::Signal::Term);
                }
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(1500));
        with_processes(std::time::Duration::ZERO, |system| {
            for pid in &targets {
                if let Some(p) = system.process(*pid)
                    && Self::is_game(p)
                {
                    p.kill();
                }
            }
        });
        targets.len()
    }
}

// ── the process table ───────────────────────────────────────────────────

/// How old the table may be when asked whether Steam runs: launching asks
/// every second while Steam starts.
const STEAM_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(1);
/// How old it may be when asked whether DayZ runs.
const GAME_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(2);

/// One process table for the whole app, read again only when a caller finds
/// it older than it allows. Callers at the same moment wait for one read
/// instead of each building a table of its own; and a table kept between
/// reads only reads what is new (a command line is read once per process).
struct Table {
    system: sysinfo::System,
    read_at: Option<std::time::Instant>,
}

static TABLE: std::sync::LazyLock<std::sync::Mutex<Table>> = std::sync::LazyLock::new(|| {
    std::sync::Mutex::new(Table {
        system: sysinfo::System::new(),
        read_at: None,
    })
});

/// What to read of each process. Names only on Windows, where they suffice
/// (reading more opens every process in the table, a hundred milliseconds
/// and more per read). On Unix the command lines too, for Proton's games
/// (see [`DayzGame`]); read once per process.
fn refresh_kind() -> sysinfo::ProcessRefreshKind {
    let kind = sysinfo::ProcessRefreshKind::nothing();
    #[cfg(not(windows))]
    let kind = kind.with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);
    kind
}

/// Run `f` on the process table, read again first if it is `max_age` old or
/// more (never read, or `Duration::ZERO`: always). Blocking.
fn with_processes<R>(max_age: std::time::Duration, f: impl FnOnce(&sysinfo::System) -> R) -> R {
    let mut table = TABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if is_stale(table.read_at, max_age) {
        table.system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            refresh_kind(),
        );
        table.read_at = Some(std::time::Instant::now());
    }
    f(&table.system)
}

/// A table read at `read_at` is too old for a caller allowing `max_age`.
fn is_stale(read_at: Option<std::time::Instant>, max_age: std::time::Duration) -> bool {
    read_at.is_none_or(|t| max_age.is_zero() || t.elapsed() >= max_age)
}

#[cfg(test)]
mod tests {
    use super::DayzGame;
    use std::ffi::OsStr;

    /// By hand, with DayZ running: `cargo test -p dz-steamcmd -- --ignored running`.
    #[test]
    #[ignore = "reads this machine's processes"]
    fn running_here() {
        println!("running: {}", DayzGame::is_running());
    }

    #[test]
    fn the_table_is_read_again_only_when_too_old() {
        use super::is_stale;
        use std::time::{Duration, Instant};
        let now = Instant::now();
        assert!(is_stale(None, Duration::MAX), "never read");
        assert!(!is_stale(Some(now), Duration::from_secs(2)));
        assert!(is_stale(Some(now), Duration::ZERO), "asked for a fresh one");
        if let Some(past) = now.checked_sub(Duration::from_secs(3)) {
            assert!(is_stale(Some(past), Duration::from_secs(2)));
        }
    }

    #[test]
    #[ignore = "reads this machine's processes"]
    fn callers_share_one_read() {
        use std::time::{Duration, Instant};
        super::with_processes(Duration::ZERO, |_| ());
        let t = Instant::now();
        for _ in 0..100 {
            DayzGame::is_running();
            super::SteamClient::is_running();
        }
        println!("100 checks on a fresh table: {:?}", t.elapsed());
        let t = Instant::now();
        for _ in 0..10 {
            super::with_processes(Duration::ZERO, |_| ());
        }
        println!("10 reads of a kept table: {:?}", t.elapsed());
        let t = Instant::now();
        for _ in 0..10 {
            let mut s = sysinfo::System::new();
            s.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::All,
                true,
                super::refresh_kind(),
            );
        }
        println!("10 reads of a new table: {:?}", t.elapsed());
    }

    #[test]
    fn only_the_games_processes_count() {
        assert!(DayzGame::is_dayz(OsStr::new("DayZ_x64.exe")));
        assert!(DayzGame::is_dayz(OsStr::new("DAYZ_BE.EXE")));
        assert!(!DayzGame::is_dayz(OsStr::new("dayz-community-hub")));
        assert!(!DayzGame::is_dayz(OsStr::new("DayZServer_x64.exe")));
    }
}
