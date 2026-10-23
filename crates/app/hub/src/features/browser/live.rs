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

use dz_api::SocketAddrV4;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};

use super::clock::Clock;
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

/// Live entries by query address, each with when it was recorded. Keyed by
/// the address itself: a lookup or a write builds no key and copies no text.
#[derive(Default)]
pub(crate) struct LiveMap {
    by_addr: FxHashMap<SocketAddrV4, (Live, Instant)>,
    /// The in-game clock each server's last A2S answer gave.
    clocks: FxHashMap<SocketAddrV4, Clock>,
}

impl LiveMap {
    pub(crate) fn get(&self, addr: SocketAddrV4) -> Option<&Live> {
        self.by_addr.get(&addr).map(|(l, _)| l)
    }

    /// The entry for `addr` if it was recorded less than `within` ago.
    pub(crate) fn fresh(&self, addr: SocketAddrV4, within: Duration) -> Option<&Live> {
        self.by_addr
            .get(&addr)
            .filter(|(_, at)| at.elapsed() < within)
            .map(|(l, _)| l)
    }

    /// The clock `addr`'s last A2S answer gave.
    pub(crate) fn clock(&self, addr: SocketAddrV4) -> Option<Clock> {
        self.clocks.get(&addr).copied()
    }

    fn put_clock(&mut self, addr: SocketAddrV4, c: Clock) {
        self.clocks.insert(addr, c);
    }

    pub(crate) fn put(&mut self, addr: SocketAddrV4, live: Live) {
        self.by_addr.insert(addr, (live, Instant::now()));
    }

    /// The entry for `addr`, to change, marked as recorded now.
    fn get_mut(&mut self, addr: SocketAddrV4) -> Option<&mut Live> {
        let (l, at) = self.by_addr.get_mut(&addr)?;
        *at = Instant::now();
        Some(l)
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

static STORE: LazyLock<LiveStore> = LazyLock::new(LiveStore::new);

pub(crate) fn store() -> &'static LiveStore {
    &STORE
}

impl LiveStore {
    fn new() -> Self {
        Self {
            map: RwLock::new(LiveMap::default()),
            snapshot: RwLock::new(Arc::new(ServerSnapshot::empty())),
            generation: AtomicU64::new(1),
            dirty: AtomicBool::new(false),
            changed: tokio::sync::Notify::const_new(),
        }
    }

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

    /// Record a query's result for the query address `addr`.
    pub(crate) fn record(&self, addr: SocketAddrV4, live: Live) {
        let mut map = self
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.put(addr, live);
        self.snapshot().put_live(addr, &live);
        drop(map);
        self.dirty.store(true, Ordering::Release);
        self.changed.notify_one();
    }

    /// Record the in-game clock an A2S answer from the query address `addr`
    /// gave in its keywords (nothing when they hold none).
    pub(crate) fn record_clock(&self, addr: SocketAddrV4, keywords: Option<&str>) {
        let Some(c) = keywords.and_then(|k| Clock::from_keywords(k, dz_common::time::now_secs()))
        else {
            return;
        };
        let mut map = self
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.put_clock(addr, c);
        self.snapshot().put_clock(addr, &c);
        drop(map);
        self.dirty.store(true, Ordering::Release);
        self.changed.notify_one();
    }

    /// Update only the player counts (from an A2S details query), keeping
    /// the ping that is there.
    pub(crate) fn record_counts(&self, addr: SocketAddrV4, players: u8, max_players: u8, bots: u8) {
        let mut map = self
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let live = match map.get_mut(addr) {
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
                map.put(addr, l);
                l
            }
        };
        self.snapshot().put_live(addr, &live);
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::snapshot::ProfileView;
    use super::*;
    use dz_api::{Server, ServerList};

    fn at(port: u16) -> SocketAddrV4 {
        SocketAddrV4::new(dz_api::Ipv4Addr::new(1, 1, 1, 1), port)
    }

    /// A list of one server at query port 2, its clock "08:00" as the list wrote it.
    fn list_snap(seen: u64) -> Arc<ServerSnapshot> {
        let server = Server {
            endpoint: at(2),
            game_port: 1,
            name: "a".into(),
            time: "08:00".into(),
            ..Default::default()
        };
        let list = ServerList {
            status: 0,
            result: vec![server.into()],
        };
        let s = Arc::new(ServerSnapshot::build(
            Arc::new(list),
            &ProfileView::default(),
        ));
        s.set_list_seen(seen);
        s
    }

    #[test]
    fn an_answers_clock_lands_in_the_list_and_outlives_a_rebuild() {
        // A store of its own: the process-wide one is the app's.
        let store = LiveStore::new();
        let now = dz_common::time::now_secs();
        let old = now - 3600;
        store.publish(list_snap(old));
        // Keywords without a clock change nothing.
        store.record_clock(at(2), Some("battleye,mod"));
        store.record_clock(at(2), None);
        assert!(store.read().clock(at(2)).is_none());
        // An hour after the list, at real time: 09:00.
        assert_eq!(store.snapshot().clock_minute(0, now), Some(9 * 60));

        // An answer now: 14:00 at 4× by day.
        store.record_clock(at(2), Some("etm4.000000,entm2.000000,14:00"));
        assert_eq!(store.snapshot().clock_minute(0, now), Some(14 * 60));
        assert_eq!(
            store.snapshot().clock_minute(0, now + 15 * 60),
            Some(15 * 60)
        );

        // The list fetched again but older than the answer: the answer's clock is
        // carried into the new snapshot.
        store.publish(list_snap(old));
        assert_eq!(store.snapshot().clock_minute(0, now), Some(14 * 60));

        // A list newer than the answer (the server restarted since): its time wins,
        // moved on at the rates the answer gave.
        store.publish(list_snap(now + 60));
        let s = store.snapshot();
        assert_eq!(s.clock_minute(0, now + 60), Some(8 * 60));
        assert_eq!(s.clock_minute(0, now + 60 + 15 * 60), Some(9 * 60));
        // The row says the same as the estimate.
        assert_eq!(s.get(0).unwrap().minute_at(now + 60), Some(8 * 60));
    }
}
