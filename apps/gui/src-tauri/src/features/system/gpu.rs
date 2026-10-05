//! The graphics cards, by name and memory, for the About page and the launch
//! options. Every source is optional: a card that cannot be named is still
//! listed by vendor, and memory that cannot be read is left out.

use serde::Serialize;

/// One graphics card.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct GpuDto {
    pub name: String,
    /// Dedicated video memory in megabytes, when the driver says.
    pub vram_mb: Option<u64>,
}

/// The cards on this machine, the most capable first. Blocking: it reads
/// files or the registry and may run `nvidia-smi` or `lspci`.
pub(crate) fn gpus() -> Vec<GpuDto> {
    let mut list = detect();
    list.sort_by_key(|g| std::cmp::Reverse(g.vram_mb.unwrap_or(0)));
    list
}

#[cfg(windows)]
fn detect() -> Vec<GpuDto> {
    use dz_common::win::{HKEY_LOCAL_MACHINE, reg_string, reg_u64};
    // The display adapters' class key: one numbered subkey per adapter, with
    // the driver's name for it and its memory (exact, unlike WMI's AdapterRAM,
    // which stops at 4 GB).
    const CLASS: &str =
        "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let mut out: Vec<GpuDto> = Vec::new();
    for i in 0..16 {
        let key = format!("{CLASS}\\{i:04}");
        let Ok(Some(name)) = reg_string(HKEY_LOCAL_MACHINE, &key, "DriverDesc") else {
            continue;
        };
        // Not cards: the fallback driver, remote and virtual displays.
        let lower = name.to_lowercase();
        if lower.contains("basic display")
            || lower.contains("basic render")
            || lower.contains("remote")
            || lower.contains("virtual")
        {
            continue;
        }
        let vram = reg_u64(HKEY_LOCAL_MACHINE, &key, "HardwareInformation.qwMemorySize")
            .ok()
            .flatten()
            .or_else(|| {
                reg_u64(HKEY_LOCAL_MACHINE, &key, "HardwareInformation.MemorySize")
                    .ok()
                    .flatten()
            })
            .filter(|&b| b > 0)
            .map(|b| b / (1024 * 1024));
        if !out.iter().any(|g| g.name == name) {
            out.push(GpuDto {
                name,
                vram_mb: vram,
            });
        }
    }
    out
}

#[cfg(not(windows))]
fn detect() -> Vec<GpuDto> {
    // NVIDIA's proprietary driver reports nothing under /sys: ask its tool.
    let mut out = nvidia_smi();
    let Ok(cards) = std::fs::read_dir("/sys/class/drm") else {
        return out;
    };
    for card in cards.flatten() {
        let name = card.file_name().to_string_lossy().into_owned();
        // card0, card1…; not their connectors (card1-DP-1) nor render nodes.
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let dev = card.path().join("device");
        let read = |f: &str| std::fs::read_to_string(dev.join(f)).ok();
        let vendor = read("vendor").unwrap_or_default();
        let vendor = vendor.trim();
        // NVIDIA cards are covered by nvidia-smi when its driver is loaded.
        if vendor == "0x10de" && !out.is_empty() {
            continue;
        }
        let slot = std::fs::canonicalize(&dev)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let vram = read("mem_info_vram_total")
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&b| b > 0)
            .map(|b| b / (1024 * 1024));
        let name = slot
            .as_deref()
            .and_then(lspci_name)
            .unwrap_or_else(|| format!("{} GPU", vendor_name(vendor)));
        out.push(GpuDto {
            name,
            vram_mb: vram,
        });
    }
    out
}

#[cfg(not(windows))]
fn vendor_name(id: &str) -> &'static str {
    match id {
        "0x1002" => "AMD",
        "0x10de" => "NVIDIA",
        "0x8086" => "Intel",
        _ => "Unknown",
    }
}

/// "AMD Radeon RX 7900 XT/7900 XTX" from `lspci -vmm -s <slot>`: the short
/// vendor and the marketing name in the device's brackets, or the whole
/// device name when it has none.
#[cfg(not(windows))]
fn lspci_name(slot: &str) -> Option<String> {
    let out = std::process::Command::new("lspci")
        .args(["-vmm", "-s", slot])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(k))
            .map(|v| v.trim().to_string())
    };
    let device = field("Device:")?;
    let vendor = field("Vendor:").unwrap_or_default();
    let short = if vendor.contains("AMD") || vendor.contains("ATI") {
        "AMD"
    } else if vendor.contains("NVIDIA") {
        "NVIDIA"
    } else if vendor.contains("Intel") {
        "Intel"
    } else {
        vendor.split_whitespace().next().unwrap_or("")
    };
    let model = device
        .rsplit_once('[')
        .and_then(|(_, rest)| rest.strip_suffix(']'))
        .unwrap_or(&device);
    Some(format!("{short} {model}").trim().to_string())
}

#[cfg(not(windows))]
fn nvidia_smi() -> Vec<GpuDto> {
    let Ok(out) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (name, mem) = l.rsplit_once(',')?;
            Some(GpuDto {
                name: name.trim().to_string(),
                vram_mb: mem.trim().parse().ok(),
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "reads this machine"]
    fn print_gpus() {
        println!("{:?}", super::gpus());
    }
}
