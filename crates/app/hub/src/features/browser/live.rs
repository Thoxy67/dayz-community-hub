//! What the app has learnt about servers since the list was fetched: pings
//! and live player counts, and the generation the window re-queries on.
//!
//! One process-wide store behind a `std` lock that is only ever held for a
//! lookup or an insert, never across an `.await`, so the ping scan (dozens of
//! writers) never contends with the commands that need the app state.
//!
//! It also holds the current [`ServerSnapshot`]: every write lands in the
//! snapshot's live table too (under the same lock, so a snapshot being
//! published never misses one), where the window's rows read it.

use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, RwLock, RwLockReadGuard};

use super::snapshot::ServerSnapshot;

/// A ping at or above this is shown as a timeout.
pub(crate) const PING_FAILED_MS: u32 = 5_000;

/// The last query of one server.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Live {
    /// Round trip; `None` when only the counts are known (from an A2S
    /// details query), which is "not pinged yet", not a timeout.
    pub ms: Option<u32>,
    pub players: Option<u8>,
    pub max_players: Option<u8>,
    pub bots: Option<u8>,
    pub failed: bool,
}

impl Live {
    pub(crate) fn from_result(r: &dz_common::Result<dz_a2s::Ping>) -> Self {
        match r {
            Ok(p) => Self {
                ms: Some(p.ms),
                players: Some(p.players),
                max_players: Some(p.max_players),
                bots: Some(p.bots),
                failed: false,
            },
            Err(_) => Self {
                ms: Some(dz_a2s::PING_TIMEOUT_SENTINEL),
                players: None,
                max_players: None,
                bots: None,
                failed: true,
            },
        }
    }

    /// Failed, or so slow it counts as a timeout.
    pub(crate) fn ping_failed(&self) -> bool {
        self.failed || self.ms.is_some_and(|ms| ms >= PING_FAILED_MS)
    }
}

/// Live entries by IP, then query port. Keyed by IP alone so a lookup
/// borrows the caller's `&str` instead of formatting "ip:port".
#[derive(Default)]
pub(crate) struct LiveMap {
    by_ip: FxHashMap<Box<str>, Vec<(i64, Live)>>,
}

impl LiveMap {
    pub(crate) fn get(&self, ip: &str, port: i64) -> Option<&Live> {
        self.by_ip
            .get(ip)?
            .iter()
            .find(|(p, _)| *p == port)
            .map(|(_, l)| l)
    }
}

impl LiveMap {
    fn put(&mut self, ip: &str, port: i64, live: Live) {
        if let Some(slots) = self.by_ip.get_mut(ip) {
            match slots.iter_mut().find(|(p, _)| *p == port) {
                Some((_, l)) => *l = live,
                None => slots.push((port, live)),
            }
        } else {
            self.by_ip.insert(ip.into(), vec![(port, live)]);
        }
    }

    fn get_mut(&mut self, ip: &str, port: i64) -> Option<&mut Live> {
        self.by_ip
            .get_mut(ip)?
            .iter_mut()
            .find(|(p, _)| *p == port)
            .map(|(_, l)| l)
    }
}

/// The store: the map, and a generation that moves when what the browser
/// shows may have changed.
pub(crate) struct LiveStore {
    map: RwLock<LiveMap>,
    /// The list as the window reads it. Lock order: `map`, then `snapshot`.
    snapshot: RwLock<Arc<ServerSnapshot>>,
    generation: AtomicU64,
    /// Set by every write; turned into a generation bump by [`LiveStore::tick`],
    /// so a scan landing hundreds of results a second moves the generation
    /// (and invalidates the query cache) at most at the tick rate.
    dirty: AtomicBool,
    /// Poked by every write and [`LiveStore::touch`]: wakes the notifier
    /// that turns them into `servers-changed`.
    changed: tokio::sync::Notify,
}

static STORE: LazyLock<LiveStore> = LazyLock::new(|| LiveStore {
    map: RwLock::new(LiveMap::default()),
    snapshot: RwLock::new(Arc::new(ServerSnapshot::empty())),
    generation: AtomicU64::new(1),
    dirty: AtomicBool::new(false),
    changed: tokio::sync::Notify::const_new(),
});

pub(crate) fn store() -> &'static LiveStore {
    &STORE
}

impl LiveStore {
    pub(crate) fn read(&self) -> RwLockReadGuard<'_, LiveMap> {
        self.map
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The current snapshot (a refcount bump).
    pub(crate) fn snapshot(&self) -> Arc<ServerSnapshot> {
        Arc::clone(
            &self
                .snapshot
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// Make `snap` the current snapshot, its live table filled from what is
    /// known, and move the generation.
    pub(crate) fn publish(&self, snap: Arc<ServerSnapshot>) {
        {
            let map = self.read();
            snap.fill_live(&map);
            *self
                .snapshot
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = snap;
        }
        self.touch();
    }

    /// Record a query's result for `ip:port`.
    pub(crate) fn record(&self, ip: &str, port: i64, live: Live) {
        let mut map = self
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.put(ip, port, live);
        self.snapshot().put_live(ip, port, &live);
        drop(map);
        self.dirty.store(true, Ordering::Release);
        self.changed.notify_one();
    }

    /// Update only the player counts (from an A2S details query), keeping
    /// the ping that is there.
    pub(crate) fn record_counts(
        &self,
        ip: &str,
        port: i64,
        players: u8,
        max_players: u8,
        bots: u8,
    ) {
        let mut map = self
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let live = match map.get_mut(ip, port) {
            Some(l) => {
                l.players = Some(players);
                l.max_players = Some(max_players);
                l.bots = Some(bots);
                l.failed = false;
                *l
            }
            None => {
                let l = Live {
                    ms: None,
                    players: Some(players),
                    max_players: Some(max_players),
                    bots: Some(bots),
                    failed: false,
                };
                map.put(ip, port, l);
                l
            }
        };
        self.snapshot().put_live(ip, port, &live);
        drop(map);
        self.dirty.store(true, Ordering::Release);
        self.changed.notify_one();
    }

    /// Something other than live data changed (the list, the profile): move
    /// the generation now.
    pub(crate) fn touch(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.changed.notify_one();
    }

    /// Notified when the generation may have moved.
    pub(crate) fn changed(&self) -> &tokio::sync::Notify {
        &self.changed
    }

    /// Fold pending writes into the generation. Returns the generation.
    pub(crate) fn tick(&self) -> u64 {
        if self.dirty.swap(false, Ordering::AcqRel) {
            self.generation.fetch_add(1, Ordering::AcqRel) + 1
        } else {
            self.generation.load(Ordering::Acquire)
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}
