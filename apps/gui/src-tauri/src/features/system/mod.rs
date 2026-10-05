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
    // Files, the registry, maybe a helper program: off the runtime thread.
    let gpus = tokio::task::spawn_blocking(gpu::gpus)
        .await
        .unwrap_or_default();

    Ok(SystemSpecsDto {
        logical_cores: logical_cores.max(1),
        physical_cores,
        total_memory_mb,
        cpu_name,
        os,
        gpus,
    })
}
