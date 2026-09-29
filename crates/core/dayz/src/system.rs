use crate::Result;
use std::path::Path;

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

pub fn check_steamcmd_installed() -> bool {
    which::which("steamcmd").is_ok()
}
