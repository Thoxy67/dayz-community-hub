use crate::Result;
use std::path::{Path, PathBuf};

const MAX_MAP_COUNT_MIN: u64 = 1024 * 1024; // 1048576

/// Check vm.max_map_count (Linux only; returns Ok(true) on other platforms).
pub fn check_max_map_count() -> Result<CheckResult> {
    #[cfg(target_os = "linux")]
    {
        let current = read_max_map_count()?;
        let ok = current >= MAX_MAP_COUNT_MIN;
        Ok(CheckResult {
            current,
            required: MAX_MAP_COUNT_MIN,
            ok,
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Not applicable on Windows/macOS — report as satisfied.
        Ok(CheckResult {
            current: MAX_MAP_COUNT_MIN,
            required: MAX_MAP_COUNT_MIN,
            ok: true,
        })
    }
}

#[cfg(target_os = "linux")]
fn read_max_map_count() -> Result<u64> {
    let content = std::fs::read_to_string("/proc/sys/vm/max_map_count")?;
    content
        .trim()
        .parse()
        .map_err(|e| crate::errors::Error::Other(format!("Failed to parse max_map_count: {}", e)))
}

pub struct CheckResult {
    pub current: u64,
    pub required: u64,
    pub ok: bool,
}

impl CheckResult {
    pub fn recommendation(&self) -> String {
        if self.ok {
            format!(
                "vm.max_map_count is sufficient ({} >= {})",
                self.current, self.required
            )
        } else {
            format!(
                "vm.max_map_count needs adjustment: {} < {}\n\
                Run: echo 'vm.max_map_count={}' | sudo tee /etc/sysctl.d/50-dayz.conf && sudo sysctl -w vm.max_map_count={}",
                self.current, self.required, self.required, self.required
            )
        }
    }
}

pub fn check_dayz_installed(steam_root: &Path) -> bool {
    let dayz_path = steam_root.join("common").join("DayZ");
    dayz_path.exists() && dayz_path.is_dir()
}

/// Add `<p>` if it ends in `steamapps`, else `<p>/steamapps`, when it exists.
fn push_steamapps(out: &mut Vec<PathBuf>, p: &Path) {
    let sa = if p.file_name().and_then(|n| n.to_str()) == Some("steamapps") {
        p.to_path_buf()
    } else {
        p.join("steamapps")
    };
    if sa.is_dir() && !out.contains(&sa) {
        out.push(sa);
    }
}

/// Extract library `path` values from a `libraryfolders.vdf`.
fn parse_library_folders(vdf: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(vdf) else {
        return Vec::new();
    };
    let re = regex::Regex::new(r#""path"\s*"([^"]+)""#).unwrap();
    re.captures_iter(&text)
        .filter_map(|c| c.get(1))
        // VDF escapes backslashes on Windows ("C:\\foo"); normalise.
        .map(|m| PathBuf::from(m.as_str().replace("\\\\", "\\")))
        .collect()
}

/// Best-effort detection of the DayZ install directory (the folder that
/// contains `DayZ_x64.exe` / `!Workshop`) by scanning Steam libraries.
///
/// `steam_root_hint` is the user's configured Steam path (may be a Steam root
/// or a `steamapps` directory); it is tried first, then common OS locations,
/// then every library listed in `libraryfolders.vdf`.
pub fn detect_dayz_path(steam_root_hint: Option<&str>) -> Option<PathBuf> {
    let mut steamapps: Vec<PathBuf> = Vec::new();

    if let Some(h) = steam_root_hint.filter(|s| !s.trim().is_empty()) {
        push_steamapps(&mut steamapps, Path::new(h));
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if let Some(home) = &home {
        for rel in [
            ".steam/steam",
            ".local/share/Steam",
            ".steam/root",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            "snap/steam/common/.local/share/Steam",
        ] {
            push_steamapps(&mut steamapps, &home.join(rel));
        }
    }
    #[cfg(windows)]
    for p in [
        "C:/Program Files (x86)/Steam",
        "C:/Program Files/Steam",
        "D:/Steam",
    ] {
        push_steamapps(&mut steamapps, Path::new(p));
    }

    // Expand each base via its libraryfolders.vdf (additional drives/libraries).
    let mut all = steamapps.clone();
    for sa in &steamapps {
        for lib in parse_library_folders(&sa.join("libraryfolders.vdf")) {
            push_steamapps(&mut all, &lib);
        }
    }

    for sa in all {
        let dayz = sa.join("common").join("DayZ");
        let installed = dayz.join("DayZ_x64.exe").exists()
            || (dayz.is_dir() && sa.join("appmanifest_221100.acf").exists());
        if installed {
            return Some(dayz);
        }
    }
    None
}

pub fn check_steamcmd_installed() -> bool {
    which::which("steamcmd").is_ok()
}
