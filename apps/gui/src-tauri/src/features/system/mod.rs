//! The machine the app runs on.

mod gpu;

use serde::Serialize;

pub use gpu::GpuDto;

/// Hardware specs used to recommend optimal DayZ launch options.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct SystemSpecsDto {
    /// Logical CPU count (threads).
    pub logical_cores: u32,
    /// Physical CPU core count (falls back to logical when unknown).
    pub physical_cores: u32,
    /// Total system RAM in megabytes.
    pub total_memory_mb: u64,
    /// The processor's name ("AMD Ryzen 9 3950X 16-Core Processor").
    pub cpu_name: Option<String>,
    /// The system and its version ("Windows 11 Pro 23H2", "CachyOS Linux").
    pub os: Option<String>,
    /// The graphics cards, the most video memory first.
    pub gpus: Vec<GpuDto>,
}

/// Detect the machine's CPU/RAM specs so the UI can recommend tuned DayZ launch
/// options (`-cpuCount`, `-exThreads`, `-maxMem`, …).
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_system_specs() -> Result<SystemSpecsDto, String> {
    // The machine does not change while the app runs, and finding the cards
    // may start `nvidia-smi` or `lspci`: worked out once, on first ask.
    static SPECS: tokio::sync::OnceCell<SystemSpecsDto> = tokio::sync::OnceCell::const_new();
    let specs = SPECS
        .get_or_try_init(|| tokio::task::spawn_blocking(detect))
        .await
        .map_err(|e| e.to_string())?;
    Ok(specs.clone())
}

/// Blocking: it reads files or the registry and may start helper programs.
fn detect() -> SystemSpecsDto {
    // Logical cores: cheap and reliable via std, no sysinfo refresh needed.
    let logical_cores = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1) as u32;

    // Physical cores is a static query; memory needs a (fast) memory refresh.
    let physical_cores = sysinfo::System::physical_core_count()
        .map(|c| c as u32)
        .unwrap_or(logical_cores)
        .max(1);

    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
    let total_memory_mb = sys.total_memory() / (1024 * 1024); // bytes → MB
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|b| !b.is_empty());
    let os = sysinfo::System::long_os_version();
    let gpus = gpu::gpus();

    SystemSpecsDto {
        logical_cores: logical_cores.max(1),
        physical_cores,
        total_memory_mb,
        cpu_name,
        os,
        gpus,
    }
}
