use dayz_community_hub_core::{api::Server, ctl::DayzCtl};
use lru::LruCache;
use rustc_hash::FxHashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

use crate::dto::{A2sDetailsDto, BattleMetricsDto};

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

// ─── Cache configuration ──────────────────────────────────────────────────────

/// Max entries in the A2S cache
const A2S_CACHE_SIZE: usize = 200;
/// A2S cache TTL in seconds
pub(crate) const A2S_CACHE_TTL_SECS: u64 = 30;

/// Max entries in the BattleMetrics cache
const BM_CACHE_SIZE: usize = 50;
/// BattleMetrics cache TTL in seconds
pub(crate) const BM_CACHE_TTL_SECS: u64 = 300;

// ─── Shared state ─────────────────────────────────────────────────────────────

pub struct AppState {
    pub ctl: DayzCtl,
    pub servers: Vec<Server>,
    /// O(1) lookup index: (ip, query_port) → index into `servers`.
    /// Rebuilt every time `servers` is replaced.  Replaces the previous
    /// linear scan inside hot commands like `query_a2s` (called 500+ times
    /// during the cold-start full-server verify wave).
    pub index_by_query_port: FxHashMap<(String, i64), usize>,
    /// O(1) lookup index: (ip, game_port) → index into `servers`.
    pub index_by_game_port: FxHashMap<(String, i64), usize>,
    /// Unix timestamp (seconds) when `servers` was last populated.
    /// Set on initial cache load and on every refresh.
    pub servers_fetched_at: u64,
    /// Cached Steam avatar as a data: URI. Populated by `fetch_steam_avatar`.
    pub cached_avatar: Option<String>,
    /// mod_id → remote time_updated from Steam Workshop API.
    /// Uses FxHashMap for ~30% faster lookups (not DoS-resistant, fine for local app).
    pub mod_update_cache: FxHashMap<u64, i64>,
    /// A2S response cache: "ip:port" → (dto, fetched_at).
    /// Avoids redundant A2S queries for the same server.
    pub a2s_cache: LruCache<String, (A2sDetailsDto, std::time::Instant)>,
    /// BattleMetrics response cache: "ip:port" → (dto, fetched_at).
    /// Avoids two HTTP round-trips on every panel open for the same server.
    pub bm_cache: LruCache<String, (BattleMetricsDto, std::time::Instant)>,
    /// Channel to send input (password / Steam Guard code) to the running
    /// steamcmd PTY.  Set when a mod operation starts, cleared when it finishes.
    pub pty_input_tx: Option<dayz_community_hub_core::steamcmd::PtyInputTx>,
    /// Abort handle for the running mod operation task.  Calling `.abort()`
    /// cancels the tokio task which drops the PTY pair, killing steamcmd.
    pub mod_op_abort: Option<tokio::task::AbortHandle>,
    /// Abort handle for background ping task (for cancellation on refresh).
    pub ping_abort: Option<tokio::task::AbortHandle>,
    /// True when ping scanning is paused.
    pub ping_paused: bool,
}

impl AppState {
    pub fn new(ctl: DayzCtl) -> Self {
        Self {
            ctl,
            servers: vec![],
            index_by_query_port: FxHashMap::default(),
            index_by_game_port: FxHashMap::default(),
            servers_fetched_at: 0,
            cached_avatar: None,
            mod_update_cache: FxHashMap::default(),
            a2s_cache: LruCache::new(NonZeroUsize::new(A2S_CACHE_SIZE).unwrap()),
            bm_cache: LruCache::new(NonZeroUsize::new(BM_CACHE_SIZE).unwrap()),
            pty_input_tx: None,
            mod_op_abort: None,
            ping_abort: None,
            ping_paused: false,
        }
    }
}

impl AppState {
    /// Replace the server list and rebuild the lookup indexes in one shot.
    /// Always call this when assigning to `self.servers` — direct assignment
    /// will leave the indexes stale and quietly break `query_a2s`,
    /// `find_server`, etc.
    pub fn set_servers(&mut self, servers: Vec<Server>) {
        let n = servers.len();
        let mut by_query =
            FxHashMap::with_capacity_and_hasher(n, Default::default());
        let mut by_game =
            FxHashMap::with_capacity_and_hasher(n, Default::default());
        for (i, s) in servers.iter().enumerate() {
            // First occurrence wins (matches dedup_servers behavior).
            by_query
                .entry((s.endpoint.ip.clone(), s.endpoint.port))
                .or_insert(i);
            by_game
                .entry((s.endpoint.ip.clone(), s.game_port))
                .or_insert(i);
        }
        self.servers = servers;
        self.index_by_query_port = by_query;
        self.index_by_game_port = by_game;
    }

    /// O(1) exact lookup by query port.
    pub fn find_by_query_port(&self, ip: &str, query_port: i64) -> Option<&Server> {
        // String allocation here is needed because the HashMap key is
        // `(String, i64)`; for hot paths the cost is one alloc per call, which
        // dominates the previous 18k-element scan by orders of magnitude.
        self.index_by_query_port
            .get(&(ip.to_owned(), query_port))
            .and_then(|&i| self.servers.get(i))
    }

    /// O(1) flexible lookup: prefers query port match, falls back to game port.
    pub fn find_flexible(&self, ip: &str, port: i64) -> Option<&Server> {
        self.find_by_query_port(ip, port).or_else(|| {
            self.index_by_game_port
                .get(&(ip.to_owned(), port))
                .and_then(|&i| self.servers.get(i))
        })
    }
}

/// Shared state using RwLock for better read concurrency.
/// Use `.read().await` for read-only access, `.write().await` for mutations.
pub(crate) type SharedState = Arc<RwLock<AppState>>;

/// Cached ping result with full metadata.
#[derive(Clone, Debug)]
pub struct CachedPingResult {
    pub ms: u32,
    pub players: Option<u8>,
    pub max_players: Option<u8>,
    pub failed: bool,
}

/// Ping results live in their own lock so the ~50 concurrent ping tasks never
/// contend with unrelated IPC commands that need `AppState`.
/// Uses FxHashMap for ~30% faster lookups on the hot path.
pub(crate) type PingCache = Arc<RwLock<FxHashMap<String, CachedPingResult>>>;
