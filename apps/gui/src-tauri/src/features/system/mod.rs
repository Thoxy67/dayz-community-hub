//! The machine the app runs on.

use serde::Serialize;

/// Hardware specs used to recommend optimal DayZ launch options.
#[derive(Serialize, Clone, Debug)]
pub struct SystemSpecsDto {
    /// Logical CPU count (threads).
    pub logical_cores: u32,
    /// Physical CPU core count (falls back to logical when unknown).
    pub physical_cores: u32,
    /// Total system RAM in megabytes.
    pub total_memory_mb: u64,
}

/// Detect the machine's CPU/RAM specs so the UI can recommend tuned DayZ launch
/// options (`-cpuCount`, `-exThreads`, `-maxMem`, …).
#[tauri::command]
pub(crate) async fn get_system_specs() -> Result<SystemSpecsDto, String> {
    // Logical cores: cheap and reliable via std, no sysinfo refresh needed.
    let logical_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1) as u32;

    // Physical cores is a static query; memory needs a (fast) memory refresh.
    let physical_cores = sysinfo::System::physical_core_count()
        .map(|c| c as u32)
        .unwrap_or(logical_cores)
        .max(1);

    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let total_memory_mb = sys.total_memory() / (1024 * 1024); // bytes → MB

    Ok(SystemSpecsDto {
        logical_cores: logical_cores.max(1),
        physical_cores,
        total_memory_mb,
    })
}
