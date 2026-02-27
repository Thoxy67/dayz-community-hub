use dayz_community_hub_core::{api::Server, ctl::DayzCtl};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, RwLock};

use crate::dto::BattleMetricsDto;

// ─── Shared HTTP client (insecure) ────────────────────────────────────────────
//
// DayZ's CDN and news API use certificates that fail standard validation.
// We build one client once and reuse it across all fetch_image /
// fetch_steam_avatar / fetch_news calls to keep the connection pool alive.

static INSECURE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub(crate) fn insecure_client() -> &'static reqwest::Client {
    INSECURE_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0",
            )
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build insecure HTTP client")
    })
}

// ─── Shared state ─────────────────────────────────────────────────────────────

pub struct AppState {
    pub ctl: DayzCtl,
    pub servers: Vec<Server>,
    /// Cached Steam avatar as a data: URI. Populated by `fetch_steam_avatar`.
    pub cached_avatar: Option<String>,
    /// mod_id → remote time_updated from Steam Workshop API.
    pub mod_update_cache: std::collections::HashMap<u64, i64>,
    /// BattleMetrics response cache: "ip:port" → (dto, fetched_at).
    /// Avoids two HTTP round-trips on every panel open for the same server.
    pub bm_cache: std::collections::HashMap<String, (BattleMetricsDto, std::time::Instant)>,
    /// Channel to send input (password / Steam Guard code) to the running
    /// steamcmd PTY.  Set when a mod operation starts, cleared when it finishes.
    pub pty_input_tx: Option<dayz_community_hub_core::steamcmd::PtyInputTx>,
    /// Abort handle for the running mod operation task.  Calling `.abort()`
    /// cancels the tokio task which drops the PTY pair, killing steamcmd.
    pub mod_op_abort: Option<tokio::task::AbortHandle>,
}

pub(crate) type SharedState = Arc<Mutex<AppState>>;

/// Ping results live in their own lock so the ~50 concurrent ping tasks never
/// contend with unrelated IPC commands that need `AppState`.
pub(crate) type PingCache = Arc<RwLock<std::collections::HashMap<String, u32>>>;
