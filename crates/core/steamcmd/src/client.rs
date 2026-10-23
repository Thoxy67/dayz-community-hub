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

    /// Check if the Steam client is currently running, without connecting
    /// to it. What Steam records of its own process is asked first (its pid
    /// file, its registry key: microseconds); a look at the process table
    /// (shared, at most a second old) settles what that record cannot.
    /// Blocking.
    pub fn is_running() -> bool {
        if Self::known_running() == Some(true) {
            return true;
        }
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

    /// Whether Steam runs, from the record it keeps of its own process (what
    /// `SteamAPI_IsSteamRunning` reads), checked against the live process:
    /// the record outlives a crash. `None` when the record cannot tell (no
    /// record; Flatpak's, whose pid is the sandbox's own). Blocking, but no
    /// process scan.
    pub fn known_running() -> Option<bool> {
        #[cfg(target_os = "windows")]
        {
            use dz_common::win::{HKEY_CURRENT_USER, reg_u64};
            let pid = reg_u64(HKEY_CURRENT_USER, ACTIVE_PROCESS, "pid").ok()??;
            let pid = u32::try_from(pid).ok().filter(|p| *p != 0)?;
            Some(process_named(pid, "steam.exe"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let home = PathBuf::from(std::env::var_os("HOME")?);
            let mut recorded = false;
            for file in [".steam/steam.pid", "snap/steam/common/.steam/steam.pid"] {
                let Ok(text) = std::fs::read_to_string(home.join(file)) else {
                    continue;
                };
                recorded = true;
                if let Ok(pid) = text.trim().parse::<u32>()
                    && pid_is_steam(pid)
                {
                    return Some(true);
                }
            }
            if home
                .join(".var/app/com.valvesoftware.Steam/.steam/steam.pid")
                .is_file()
            {
                return None;
            }
            recorded.then_some(false)
        }
    }

    /// Steam is logged in, so it takes a launch at once: Windows records the
    /// account in use (`ActiveProcess\ActiveUser`, 0 until a login). `None`
    /// where nothing says so (Linux).
    pub fn is_ready() -> Option<bool> {
        #[cfg(target_os = "windows")]
        {
            use dz_common::win::{HKEY_CURRENT_USER, reg_u64};
            reg_u64(HKEY_CURRENT_USER, ACTIVE_PROCESS, "ActiveUser")
                .ok()
                .flatten()
                .map(|user| user != 0)
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
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
            spawn_detached(&mut cmd)?;
        }

        Ok(false)
    }
}

/// Where Steam on Windows records its running process (`pid`) and the
/// account logged in (`ActiveUser`).
#[cfg(target_os = "windows")]
const ACTIVE_PROCESS: &str = r"Software\Valve\Steam\ActiveProcess";

/// The process `pid` is Steam's client: `/proc/<pid>/comm` says `steam`.
#[cfg(not(target_os = "windows"))]
fn pid_is_steam(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|comm| comm.trim() == "steam")
}

/// The process `pid` runs and is named `name` (without case): a refresh of
/// that one process in the shared table.
#[cfg(target_os = "windows")]
fn process_named(pid: u32, name: &str) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    with_some_processes(&[pid], |system| {
        system
            .process(pid)
            .is_some_and(|p| p.name().to_string_lossy().eq_ignore_ascii_case(name))
    })
}

/// Start a program the launcher does not wait for (Steam, a `steam://`
/// link). On Unix a thread waits for it, so that once it exits it does not
/// stay behind as a zombie until the launcher closes; Windows has none.
pub fn spawn_detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
    let child = cmd.spawn()?;
    #[cfg(unix)]
    {
        let mut child = child;
        let _ = std::thread::Builder::new()
            .name("reap".into())
            .spawn(move || {
                let _ = child.wait();
            });
    }
    #[cfg(not(unix))]
    drop(child);
    Ok(())
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
/// its command line (`S:\common\DayZ\DayZ_x64.exe`) gives it away. That
/// command line is Wine's loader for the first moments of the process, and
/// it is read once per process; so Steam's reaper, which Steam starts every
/// game under (`reaper SteamLaunch AppId=221100 -- …`) and whose command line
/// never changes, also counts as the game while it runs.
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

    /// The command line is Steam's reaper running DayZ.
    fn is_reaper_cmd<S: AsRef<std::ffi::OsStr>>(cmd: &[S]) -> bool {
        let Some((first, rest)) = cmd.split_first() else {
            return false;
        };
        let first = first.as_ref().to_string_lossy();
        let base = first.rsplit('/').next().unwrap_or(&first);
        let app = format!("AppId={}", crate::DAYZ_GAME_ID);
        // Steam's own arguments end at `--`; the game's come after.
        let own = rest.iter().take_while(|a| a.as_ref() != "--");
        let (mut launch, mut id) = (false, false);
        for a in own {
            let a = a.as_ref();
            launch |= a == "SteamLaunch";
            id |= a == app.as_str();
        }
        base == "reaper" && launch && id
    }

    /// The process is the game, or Steam's reaper standing for it.
    fn is_watched(p: &sysinfo::Process) -> bool {
        Self::is_game(p) || (cfg!(not(windows)) && Self::is_reaper_cmd(p.cmd()))
    }

    /// DayZ is running. Blocking (a look at the shared process table, at
    /// most two seconds old: the play tracker, the window and BattlEye's
    /// repair all ask, and share one scan).
    pub fn is_running() -> bool {
        with_processes(GAME_MAX_AGE, |system| {
            system.processes().values().any(Self::is_watched)
        })
    }

    /// The processes that say DayZ runs (the game's, Steam's reaper for
    /// it): what to wait on with [`Self::any_alive`]. From the shared table,
    /// or one read now when `fresh` (to see a process that just started).
    /// Empty when the game is not running. Blocking.
    pub fn running_pids(fresh: bool) -> Vec<u32> {
        let max_age = if fresh {
            std::time::Duration::ZERO
        } else {
            GAME_MAX_AGE
        };
        with_processes(max_age, |system| {
            system
                .processes()
                .iter()
                .filter(|(_, p)| Self::is_watched(p))
                .map(|(pid, _)| pid.as_u32())
                .collect()
        })
    }

    /// One of `pids` (from [`Self::running_pids`]) still runs as DayZ: a
    /// look at those processes alone, not the whole table. A game that hands
    /// over to another process (BattlEye's launcher starting the game) is
    /// missed: once this says no, a full look ([`Self::running_pids`]) says
    /// whether DayZ is really gone. Blocking.
    pub fn any_alive(pids: &[u32]) -> bool {
        let pids: Vec<sysinfo::Pid> = pids.iter().map(|p| sysinfo::Pid::from_u32(*p)).collect();
        with_some_processes(&pids, |system| {
            pids.iter()
                .any(|pid| system.process(*pid).is_some_and(Self::is_watched))
        })
    }

    /// The game's processes, from a table read now with every command line
    /// read again (one Wine has rewritten since it was first read counts).
    fn game_pids() -> Vec<sysinfo::Pid> {
        with_processes_kind(std::time::Duration::ZERO, fresh_refresh_kind(), |system| {
            system
                .processes()
                .iter()
                .filter(|(_, p)| Self::is_game(p))
                .map(|(pid, _)| *pid)
                .collect()
        })
    }

    /// Close DayZ: asked to quit first where the system allows it, then
    /// killed if it is still there a few seconds later. Returns how many
    /// processes were stopped. Blocking.
    pub fn kill() -> usize {
        let targets = Self::game_pids();
        if targets.is_empty() {
            return 0;
        }
        // SIGTERM on Unix lets Wine and the game close cleanly; Windows has
        // no such request (`kill_with` returns None): nothing to wait for.
        let asked = with_processes(std::time::Duration::MAX, |system| {
            let mut asked = false;
            for pid in &targets {
                if let Some(p) = system.process(*pid) {
                    asked |= p.kill_with(sysinfo::Signal::Term).is_some();
                }
            }
            asked
        });
        let alive = || {
            with_some_processes(&targets, |system| {
                targets
                    .iter()
                    .any(|pid| system.process(*pid).is_some_and(Self::is_game))
            })
        };
        if asked {
            let until = std::time::Instant::now() + KILL_GRACE;
            while std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if !alive() {
                    return targets.len();
                }
            }
        }
        with_some_processes(&targets, |system| {
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
/// How long a game asked to quit has before it is killed.
const KILL_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

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
/// (see [`DayzGame`]); read once per process. Never the threads: Linux
/// lists each as a process of its own (five times the entries, each with
/// the game's command line), and the thread group's leader says it all.
fn refresh_kind() -> sysinfo::ProcessRefreshKind {
    let kind = sysinfo::ProcessRefreshKind::nothing().without_tasks();
    #[cfg(not(windows))]
    let kind = kind.with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);
    kind
}

/// [`refresh_kind`], with every command line read again.
fn fresh_refresh_kind() -> sysinfo::ProcessRefreshKind {
    let kind = sysinfo::ProcessRefreshKind::nothing().without_tasks();
    #[cfg(not(windows))]
    let kind = kind.with_cmd(sysinfo::UpdateKind::Always);
    kind
}

/// Run `f` on the process table, read again first if it is `max_age` old or
/// more (never read, or `Duration::ZERO`: always). Blocking.
fn with_processes<R>(max_age: std::time::Duration, f: impl FnOnce(&sysinfo::System) -> R) -> R {
    with_processes_kind(max_age, refresh_kind(), f)
}

fn lock_table() -> std::sync::MutexGuard<'static, Table> {
    TABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// [`with_processes`], reading `kind` of each process when it reads.
fn with_processes_kind<R>(
    max_age: std::time::Duration,
    kind: sysinfo::ProcessRefreshKind,
    f: impl FnOnce(&sysinfo::System) -> R,
) -> R {
    let mut table = lock_table();
    if is_stale(table.read_at, max_age) {
        table
            .system
            .refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, kind);
        table.read_at = Some(std::time::Instant::now());
    }
    f(&table.system)
}

/// Run `f` on the process table with `pids` read again first, and only
/// them (those gone are dropped from it). Blocking, but a few reads.
fn with_some_processes<R>(pids: &[sysinfo::Pid], f: impl FnOnce(&sysinfo::System) -> R) -> R {
    let mut table = lock_table();
    table.system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(pids),
        true,
        refresh_kind(),
    );
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
    fn steams_reaper_for_dayz_stands_for_the_game() {
        let reaper = |args: &[&str]| DayzGame::is_reaper_cmd(args);
        assert!(reaper(&[
            "/home/u/.local/share/Steam/ubuntu12_32/reaper",
            "SteamLaunch",
            "AppId=221100",
            "--",
            "/home/u/.local/share/Steam/ubuntu12_32/steam-launch-wrapper",
        ]));
        // Another game, or DayZ's id only among the game's own arguments.
        assert!(!reaper(&["/x/reaper", "SteamLaunch", "AppId=730", "--"]));
        assert!(!reaper(&[
            "/x/reaper",
            "SteamLaunch",
            "AppId=730",
            "--",
            "AppId=221100"
        ]));
        assert!(!reaper(&["/usr/bin/sleep", "SteamLaunch", "AppId=221100"]));
        assert!(!DayzGame::is_reaper_cmd::<&str>(&[]));
    }

    #[test]
    #[ignore = "reads this machine's processes"]
    fn steam_here() {
        println!(
            "known: {:?}, running: {}",
            super::SteamClient::known_running(),
            super::SteamClient::is_running()
        );
    }

    #[test]
    fn only_the_games_processes_count() {
        assert!(DayzGame::is_dayz(OsStr::new("DayZ_x64.exe")));
        assert!(DayzGame::is_dayz(OsStr::new("DAYZ_BE.EXE")));
        assert!(!DayzGame::is_dayz(OsStr::new("dayz-community-hub")));
        assert!(!DayzGame::is_dayz(OsStr::new("DayZServer_x64.exe")));
    }
}
