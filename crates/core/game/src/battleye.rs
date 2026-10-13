//! BattlEye, the anti-cheat most servers require.
//!
//! On Windows it is a service, `BEService`, which the game's BattlEye
//! launcher (`DayZ_BE.exe`, which Steam's launch goes through) installs into
//! `Common Files\BattlEye` and starts with the game. When the service is
//! missing, disabled, or its program was removed (an antivirus quarantining
//! it, a cleaner), the game still starts, without BattlEye, and every
//! BattlEye server kicks it. Reinstalling it is what the game's own
//! `BattlEye\Install_BattlEye.bat` does: `DayZ_BE.exe 1 0`, as administrator.
//!
//! Under Proton there is no service: Steam's "Proton BattlEye Runtime" tool
//! (app 1161040) stands in for it, and must be installed.

use std::path::{Path, PathBuf};

use dz_common::{Error, Result};
use serde::Serialize;

/// The Proton BattlEye Runtime's Steam app id.
#[cfg(not(windows))]
const RUNTIME_APP_ID: u32 = 1161040;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BattlEyeState {
    /// Running (Windows: the service is up, so the game has it).
    Running,
    /// Installed, ready to start with the game.
    Ready,
    /// Not installed (Windows: no service, or its program is gone; Proton:
    /// no runtime). BattlEye servers kick the game.
    Missing,
    /// Windows: the service is disabled, so nothing can start it.
    Disabled,
    /// The game's BattlEye launcher is not there (DayZ not found): nothing to
    /// check or repair.
    NoGame,
}

impl BattlEyeState {
    /// The game can join a BattlEye server as things stand.
    pub fn usable(self) -> bool {
        matches!(self, Self::Running | Self::Ready)
    }
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct BattlEyeStatus {
    pub state: BattlEyeState,
    /// BattlEye is a service here (Windows), which runs while the game does:
    /// a game running without it went wrong.
    pub service: bool,
}

/// The game's BattlEye launcher.
fn launcher(dayz: &Path) -> PathBuf {
    dayz.join("DayZ_BE.exe")
}

/// Where BattlEye stands on this machine. `steam_root` is the `steamapps`
/// holding DayZ. Blocking (the service manager, or the Steam libraries).
pub fn status(dayz: &Path, steam_root: &Path) -> BattlEyeStatus {
    BattlEyeStatus {
        state: state(dayz, steam_root),
        service: cfg!(windows),
    }
}

#[cfg(windows)]
fn state(dayz: &Path, _steam_root: &Path) -> BattlEyeState {
    if !launcher(dayz).is_file() {
        return BattlEyeState::NoGame;
    }
    match dz_common::win::service("BEService") {
        Ok(Some(s)) if s.running => BattlEyeState::Running,
        Ok(Some(s)) if s.binary.as_ref().is_some_and(|b| !b.is_file()) => BattlEyeState::Missing,
        Ok(Some(s)) if s.disabled => BattlEyeState::Disabled,
        Ok(Some(_)) => BattlEyeState::Ready,
        Ok(None) => BattlEyeState::Missing,
        // The service manager would not say: assume the launcher will cope
        // rather than stand in the way of a launch.
        Err(_) => BattlEyeState::Ready,
    }
}

#[cfg(not(windows))]
fn state(dayz: &Path, steam_root: &Path) -> BattlEyeState {
    if !launcher(dayz).is_file() {
        return BattlEyeState::NoGame;
    }
    let installed = dz_steamcmd::steam_libraries(steam_root).iter().any(|sa| {
        sa.join(format!("appmanifest_{RUNTIME_APP_ID}.acf"))
            .is_file()
            || sa.join("common").join("Proton BattlEye Runtime").is_dir()
    });
    if installed {
        BattlEyeState::Ready
    } else {
        BattlEyeState::Missing
    }
}

/// Put BattlEye back. On Windows: enable the service and reinstall it
/// through the game's launcher, as administrator (a UAC prompt); waits for
/// it. Under Proton: ask Steam to install the runtime, which it does on its
/// own time. Blocking.
#[cfg(windows)]
pub fn repair(dayz: &Path) -> Result<()> {
    let be = launcher(dayz);
    if !be.is_file() {
        return Err(Error::Config(format!("{} not found", be.display())));
    }
    // One prompt for both: `sc config` fails harmlessly when the service is
    // not installed, and the launcher then installs it. cmd drops the outer
    // quotes of `/c "…"`, keeping the launcher's own.
    let params = format!(
        "/c \"sc config BEService start= demand & \"{}\" 1 0\"",
        be.display()
    );
    let cmd = std::env::var_os("ComSpec")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
    match dz_common::win::run_elevated(&cmd, &params, &dayz.join("BattlEye")) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Err(Error::Other(
            "BattlEye was not repaired: administrator rights were refused".into(),
        )),
        Err(e) => Err(e.into()),
    }
}

#[cfg(not(windows))]
pub fn repair(_dayz: &Path) -> Result<()> {
    let (program, pre) =
        dz_steamcmd::SteamClient::launcher().ok_or_else(dz_steamcmd::SteamClient::not_found)?;
    let mut cmd = std::process::Command::new(program);
    cmd.args(pre)
        .arg(format!("steam://install/{RUNTIME_APP_ID}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    for v in dz_steamcmd::appimage_env() {
        cmd.env_remove(v);
    }
    cmd.spawn()?;
    Ok(())
}

/// Before joining a BattlEye server: make sure BattlEye can start with the
/// game, repairing it if needed. An error says why the game would be kicked,
/// and the launch should not go ahead. Blocking.
pub fn ensure(dayz: &Path, steam_root: &Path) -> Result<()> {
    match state(dayz, steam_root) {
        // Nothing to check without the game: the launch reports it.
        s if s.usable() || s == BattlEyeState::NoGame => return Ok(()),
        _ => {}
    }
    repair(dayz)?;
    if state(dayz, steam_root).usable() {
        return Ok(());
    }
    Err(Error::Other(if cfg!(windows) {
        "This server requires BattlEye, and its service could not be installed. \
         Run BattlEye\\Install_BattlEye.bat in the DayZ folder as administrator."
            .into()
    } else {
        "This server requires BattlEye: Steam is installing the Proton BattlEye Runtime. \
         Join again once it is installed."
            .into()
    }))
}
