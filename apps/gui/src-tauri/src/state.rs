//! What the commands share: the controller, the server list and its index,
//! the response caches, and the running mod operation.

use dz_api::{Server, ServerIndex, ServerList};
use dz_game::DayzCtl;
use lru::LruCache;
use rustc_hash::FxHashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tokio::sync::RwLock;

use crate::error::ResultExt;
use crate::features::a2s::A2sDetailsDto;
use crate::features::battlemetrics::BattleMetricsDto;

/// Shared state: `.read().await` to look, `.write().await` to change.
pub(crate) type SharedState = Arc<RwLock<AppState>>;

/// Max entries in the A2S cache.
const A2S_CACHE_SIZE: usize = 200;
/// Max entries in the BattleMetrics cache.
const BM_CACHE_SIZE: usize = 50;

pub struct AppState {
    pub ctl: DayzCtl,
    /// The server list, deduplicated. Behind an `Arc` so a command can hand it
    /// to the window without cloning 18 000 servers or holding the lock while
    /// it is serialized.
    pub servers: Arc<ServerList>,
    /// O(1) lookups into `servers`; rebuilt with it by [`AppState::set_servers`].
    index: ServerIndex,
    /// Cached Steam avatar as a `data:` URI.
    pub cached_avatar: Option<String>,
    /// mod id → remote `time_updated` from the Steam Workshop API.
    pub mod_update_cache: FxHashMap<u64, i64>,
    /// A2S responses by "ip:query_port". The DTO is behind an `Arc` so a
    /// cache hit is a refcount bump, not a deep clone of players and rules.
    pub a2s_cache: LruCache<String, (Arc<A2sDetailsDto>, Instant)>,
    /// BattleMetrics responses by "ip:port:query_port".
    pub bm_cache: LruCache<String, (BattleMetricsDto, Instant)>,
    /// Input (password / Steam Guard code) for the running steamcmd PTY.
    pub pty_input_tx: Option<dz_steamcmd::PtyInputTx>,
    /// Abort handle of the running mod operation: aborting drops the PTY
    /// pair, which kills steamcmd.
    pub mod_op_abort: Option<tokio::task::AbortHandle>,
    /// Orders profile writes so they happen outside the lock.
    pub profile_writer: Arc<dz_profile::Writer>,
}

impl AppState {
    pub fn new(ctl: DayzCtl) -> Self {
        Self {
            ctl,
            servers: Arc::default(),
            index: ServerIndex::default(),
            cached_avatar: None,
            mod_update_cache: FxHashMap::default(),
            a2s_cache: LruCache::new(NonZeroUsize::new(A2S_CACHE_SIZE).unwrap()),
            bm_cache: LruCache::new(NonZeroUsize::new(BM_CACHE_SIZE).unwrap()),
            pty_input_tx: None,
            mod_op_abort: None,
            profile_writer: Arc::default(),
        }
    }

    /// Replace the server list and rebuild its index in one step.
    pub fn set_servers(&mut self, servers: Arc<ServerList>) {
        self.index = ServerIndex::build(&servers.result);
        self.servers = servers;
    }

    /// The server whose query port is `port`.
    pub fn find_by_query_port(&self, ip: &str, port: i64) -> Option<&Server> {
        self.index
            .by_query_port(ip, port)
            .and_then(|i| self.servers.result.get(i))
    }

    /// The server whose query port, else game port, is `port`.
    pub fn find_flexible(&self, ip: &str, port: i64) -> Option<&Server> {
        self.index
            .flexible(ip, port)
            .and_then(|i| self.servers.result.get(i))
    }
}

/// Change the profile under the write lock, then write it to disk once the
/// lock is released, so a slow disk never holds up other commands.
pub(crate) async fn mutate_profile<T>(
    state: &SharedState,
    f: impl FnOnce(&mut AppState) -> Result<T, String>,
) -> Result<T, String> {
    let (out, snapshot, writer) = {
        let mut s = state.write().await;
        let out = f(&mut s)?;
        let snapshot = s.profile_writer.stamp(s.ctl.profile_snapshot().cmd_err()?);
        (out, snapshot, Arc::clone(&s.profile_writer))
    };
    // Favorites and excluded IPs show in the browser.
    crate::features::browser::live::store().touch();
    writer.write(snapshot).await.cmd_err()?;
    Ok(out)
}

/// One HTTP client for the hosts whose certificates fail validation
/// (DayZ's CDN) and for Steam/BattleMetrics lookups, reused so its
/// connection pool stays warm.
pub(crate) fn insecure_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
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
