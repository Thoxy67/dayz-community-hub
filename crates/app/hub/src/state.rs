//! What the commands share: the controller, the server list and its index,
//! the response caches, and the running mod operation.

use dz_api::{Server, ServerList};
use dz_game::DayzCtl;
use lru::LruCache;
use rustc_hash::FxHashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;

use crate::error::ResultExt;
use crate::features::a2s::A2sDetailsDto;
use crate::features::browser::ServerSnapshot;
use crate::features::metrics::ServerMetricsDto;

/// Shared state: `.read().await` to look, `.write().await` to change.
pub(crate) type SharedState = Arc<RwLock<AppState>>;

/// Max entries in the A2S cache.
const A2S_CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(200).unwrap();
/// Max entries in the DayZ Metrics caches (figures, and ids found).
const DM_CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(100).unwrap();

pub struct AppState {
    pub ctl: DayzCtl,
    /// The server list, deduplicated. Behind an `Arc` so a command can hand it
    /// to the window without cloning 18 000 servers or holding the lock while
    /// it is serialized.
    pub servers: Arc<ServerList>,
    /// `servers` as the browser reads it, built with it; its index answers
    /// the lookups below (one index per list, not one here and one there).
    snapshot: Arc<ServerSnapshot>,
    /// Moves with every [`AppState::profile_changed`]: a snapshot built off
    /// the lock from an older profile is brought up to date as it is put in.
    profile_epoch: AtomicU64,
    /// Cached Steam avatar as a `data:` URI.
    pub cached_avatar: Option<String>,
    /// mod id → remote `time_updated` from the Steam Workshop API.
    pub mod_update_cache: FxHashMap<u64, i64>,
    /// mod id → `local_updated` of each mod on disk, as last read; `None`
    /// until the disk is read. The browser tells from it whether a
    /// server's mods are ready.
    pub mods_on_disk: Option<FxHashMap<u64, i64>>,
    /// The last read of the mods on disk, and when: `check_mod_updates`
    /// uses it instead of reading the disk again while it is recent.
    /// Cleared by whatever changes the mods on disk.
    pub mod_scan: Option<(Instant, Arc<Vec<dz_game::mods::InstalledMod>>)>,
    /// A2S responses by "ip:query_port". The DTO is behind an `Arc` so a
    /// cache hit is a refcount bump, not a deep clone of players and rules.
    pub a2s_cache: LruCache<String, (Arc<A2sDetailsDto>, Instant)>,
    /// DayZ Metrics figures by "ip:game_port:query_port".
    pub dm_cache: LruCache<String, (ServerMetricsDto, Instant)>,
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
            snapshot: Arc::new(ServerSnapshot::empty()),
            profile_epoch: AtomicU64::new(0),
            cached_avatar: None,
            mod_update_cache: FxHashMap::default(),
            mods_on_disk: None,
            mod_scan: None,
            a2s_cache: LruCache::new(A2S_CACHE_SIZE),
            dm_cache: LruCache::new(DM_CACHE_SIZE),
            pty_input_tx: None,
            mod_op_abort: None,
            profile_writer: Arc::default(),
        }
    }

    /// Replace the server list, building its snapshot here (under the
    /// caller's lock). For a list replaced while the app runs,
    /// [`replace_servers`] builds it off the lock.
    pub fn set_servers(&mut self, servers: Arc<ServerList>) {
        let ready = ReadyList::build(self, servers);
        self.install(ready);
    }

    /// Put in a list whose snapshot is built: pointer swaps, plus the
    /// profile's flags written again if the profile changed meanwhile.
    pub(crate) fn install(&mut self, ready: ReadyList) {
        let ReadyList { list, snap, epoch } = ready;
        if epoch != self.profile_epoch.load(Ordering::Acquire) {
            snap.apply_profile(&crate::features::browser::profile_view(self));
        }
        crate::features::servers::sources::counted(&list);
        self.servers = list;
        self.snapshot = Arc::clone(&snap);
        crate::features::browser::live::store().publish(snap);
    }

    /// The profile's favourites or exclusions, or the mods on disk, changed:
    /// the browser's flags follow.
    pub fn profile_changed(&self) {
        self.profile_epoch.fetch_add(1, Ordering::AcqRel);
        crate::features::browser::profile_changed(&self.snapshot, self);
    }

    /// A fingerprint of everything the browser's flags are read from: the
    /// favourites, the excluded IPs, the mods on disk and their updates. Equal
    /// fingerprints, equal flags (barring a hash collision), so a profile
    /// change that leaves it alone (settings, history, launch options) skips
    /// the pass over every server. The maps are folded order-independently.
    pub(crate) fn flags_fingerprint(&self) -> u64 {
        use std::hash::{BuildHasher, Hash, Hasher};
        let build = rustc_hash::FxBuildHasher;
        let p = self.ctl.profile();
        let mut h = build.build_hasher();
        for f in &p.favorites {
            (f.ip.as_str(), f.port).hash(&mut h);
        }
        0xFFu8.hash(&mut h);
        p.excluded_ips.hash(&mut h);
        let fold = |map: &FxHashMap<u64, i64>| {
            map.iter().fold(map.len() as u64, |acc, e| {
                acc.wrapping_add(build.hash_one(e))
            })
        };
        self.mods_on_disk.as_ref().map(fold).hash(&mut h);
        fold(&self.mod_update_cache).hash(&mut h);
        h.finish()
    }

    /// The server whose query port is `port`.
    pub fn find_by_query_port(&self, ip: &str, port: i64) -> Option<&Server> {
        let i = self.snapshot.find_by_query_port(ip, port)?;
        self.snapshot.list().result.get(i as usize).map(|s| &**s)
    }

    /// The server whose query port, else game port, is `port`.
    pub fn find_flexible(&self, ip: &str, port: i64) -> Option<&Server> {
        let i = self.snapshot.find(ip, port)?;
        self.snapshot.list().result.get(i as usize).map(|s| &**s)
    }
}

/// A server list with its snapshot built, ready for [`AppState::install`].
pub(crate) struct ReadyList {
    list: Arc<ServerList>,
    snap: Arc<ServerSnapshot>,
    /// The profile epoch the snapshot's flags were read at.
    epoch: u64,
}

impl ReadyList {
    /// Build `list`'s snapshot with `s`'s profile (the heavy part: the
    /// index, lowercased names, ranks).
    fn build(s: &AppState, list: Arc<ServerList>) -> Self {
        let epoch = s.profile_epoch.load(Ordering::Acquire);
        let view = crate::features::browser::profile_view(s);
        let snap = Arc::new(ServerSnapshot::build(Arc::clone(&list), &view));
        Self { list, snap, epoch }
    }
}

/// Replace the server list while the app runs: the profile's flags are read
/// under the read lock, the snapshot (index, names, ranks: milliseconds for
/// thousands of servers) is built on the blocking pool with no lock held,
/// and only the pointer swaps happen under the write lock.
pub(crate) async fn replace_servers(state: &SharedState, list: Arc<ServerList>) {
    let (view, epoch) = {
        let s = state.read().await;
        (
            crate::features::browser::profile_view(&s),
            s.profile_epoch.load(Ordering::Acquire),
        )
    };
    let ready = {
        let list = Arc::clone(&list);
        tokio::task::spawn_blocking(move || {
            let snap = Arc::new(ServerSnapshot::build(Arc::clone(&list), &view));
            ReadyList { list, snap, epoch }
        })
        .await
    };
    let mut s = state.write().await;
    match ready {
        Ok(ready) => s.install(ready),
        // The build task failed (it does not, short of a bug): build it
        // here rather than lose the list.
        Err(_) => s.set_servers(list),
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
        let flags = s.flags_fingerprint();
        let out = f(&mut s)?;
        // Favorites and excluded IPs show in the browser; most changes (settings,
        // history, launch options) touch neither, and cost the browser nothing.
        if s.flags_fingerprint() != flags {
            s.profile_changed();
        }
        let snapshot = s.profile_writer.stamp(s.ctl.profile_snapshot().cmd_err()?);
        (out, snapshot, Arc::clone(&s.profile_writer))
    };
    writer.write(snapshot).await.cmd_err()?;
    Ok(out)
}
