//! Live details about one server, fetched on demand and cached (=
//! `stores/server-data.svelte.ts`): A2S (who is on, the rules, the mods it reports) and
//! DayZ Metrics (rank, schedules, player counts). Requests for the same key share one
//! flight, the caches are bounded, and an error is kept beside the last good value.

use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dz_hub::Hub;
use dz_hub::features::a2s::A2sDetailsDto;
use dz_hub::features::browser::ServerRow;
use dz_hub::features::metrics::{HeatCell, ServerMetricsDto};
use rustc_hash::FxHashMap;

use super::servers::Servers;
use crate::rt::{self, Task};

const A2S_TTL: Duration = Duration::from_secs(30);
const MAX_A2S: usize = 200;
const METRICS_TTL: Duration = Duration::from_secs(300);
const MAX_METRICS: usize = 100;
/// The longer views change slowly: a week's history, a month's rank, the busy hours.
const LONG_TTL: Duration = Duration::from_secs(900);
const MAX_LONG: usize = 30;

/// One cached answer.
#[derive(Debug)]
pub struct Entry<T> {
    pub data: Option<Arc<T>>,
    pub loading: bool,
    pub error: Option<String>,
    pub fetched_at: Option<Instant>,
}

impl<T> Default for Entry<T> {
    fn default() -> Self {
        Self {
            data: None,
            loading: false,
            error: None,
            fetched_at: None,
        }
    }
}

impl<T> Clone for Entry<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            loading: self.loading,
            error: self.error.clone(),
            fetched_at: self.fetched_at,
        }
    }
}

impl<T> Entry<T> {
    fn fresh(&self, ttl: Duration) -> bool {
        self.data.is_some() && self.fetched_at.is_some_and(|t| t.elapsed() < ttl)
    }
}

/// A bounded cache with one flight per key.
struct Cache<K, T> {
    map: FxHashMap<K, Entry<T>>,
    flights: Vec<(K, Task<Result<T, String>>)>,
    ttl: Duration,
    max: usize,
}

impl<K: Eq + Hash + Clone, T: Send + 'static> Cache<K, T> {
    fn new(ttl: Duration, max: usize) -> Self {
        Self {
            map: FxHashMap::default(),
            flights: Vec::new(),
            ttl,
            max,
        }
    }

    fn get(&self, k: &K) -> Option<&Entry<T>> {
        self.map.get(k)
    }

    /// The cached value while fresh (unless `force`), else one shared flight.
    fn fetch<F>(&mut self, k: K, force: bool, fut: impl FnOnce() -> F)
    where
        F: std::future::Future<Output = Result<T, String>> + Send + 'static,
    {
        if !force && self.map.get(&k).is_some_and(|e| e.fresh(self.ttl)) {
            return;
        }
        if self.flights.iter().any(|(f, _)| *f == k) {
            return;
        }
        let e = self.map.entry(k.clone()).or_default();
        e.loading = true;
        e.error = None;
        self.flights.push((k, rt::spawn(fut())));
    }

    fn poll(&mut self) -> bool {
        let mut done = Vec::new();
        self.flights.retain_mut(|(k, t)| match t.poll() {
            Some(r) => {
                done.push((k.clone(), Some(r)));
                false
            }
            None if !t.is_pending() => {
                done.push((k.clone(), None));
                false
            }
            None => true,
        });
        let changed = !done.is_empty();
        for (k, r) in done {
            let e = self.map.entry(k).or_default();
            e.loading = false;
            match r {
                Some(Ok(v)) => {
                    e.data = Some(Arc::new(v));
                    e.error = None;
                    e.fetched_at = Some(Instant::now());
                }
                Some(Err(err)) => e.error = Some(err),
                None => {}
            }
        }
        if changed {
            evict(&mut self.map, self.max);
        }
        changed
    }
}

/// Let the oldest answers go once there are more than `max` (never one loading).
fn evict<K: Eq + Hash + Clone, T>(map: &mut FxHashMap<K, Entry<T>>, max: usize) {
    if map.len() <= max {
        return;
    }
    let mut old: Vec<(Option<Instant>, K)> = map
        .iter()
        .filter(|(_, e)| !e.loading)
        .map(|(k, e)| (e.fetched_at, k.clone()))
        .collect();
    old.sort_by_key(|(t, _)| *t);
    let excess = map.len() - max;
    for (_, k) in old.into_iter().take(excess) {
        map.remove(&k);
    }
}

/// An A2S flight: the key it was asked under, the key its answer is kept under, the row
/// it resolved, and the answer.
type A2sAnswer = (
    String,
    String,
    Option<ServerRow>,
    Result<Arc<A2sDetailsDto>, String>,
);

pub struct ServerData {
    hub: Hub,
    a2s: FxHashMap<String, Entry<A2sDetailsDto>>,
    a2s_flights: Vec<(String, Task<A2sAnswer>)>,
    metrics: Cache<String, ServerMetricsDto>,
    history: Cache<String, Vec<(i64, f64)>>,
    rank: Cache<u64, Vec<(String, f64)>>,
    heat: Cache<u64, Vec<HeatCell>>,
}

impl ServerData {
    pub fn new(hub: Hub) -> Self {
        Self {
            hub,
            a2s: FxHashMap::default(),
            a2s_flights: Vec::new(),
            metrics: Cache::new(METRICS_TTL, MAX_METRICS),
            history: Cache::new(LONG_TTL, MAX_LONG),
            rank: Cache::new(LONG_TTL, MAX_LONG),
            heat: Cache::new(LONG_TTL, MAX_LONG),
        }
    }

    /// The key a server's A2S data is kept under: its query port when known.
    pub fn key(servers: &mut Servers, ip: &str, port: i64) -> String {
        let qp = servers.find(ip, port).map_or(port, |r| r.query_port);
        format!("{ip}:{qp}")
    }

    pub fn a2s(&self, servers: &mut Servers, ip: &str, port: i64) -> Entry<A2sDetailsDto> {
        self.a2s
            .get(&Self::key(servers, ip, port))
            .cloned()
            .unwrap_or_default()
    }

    pub fn has_fresh_a2s(&self, servers: &mut Servers, ip: &str, port: i64) -> bool {
        self.a2s
            .get(&Self::key(servers, ip, port))
            .is_some_and(|e| e.fresh(A2S_TTL))
    }

    /// Ask the server itself; concurrent callers share one query. The address may carry
    /// the game port: the row is resolved first, so the query goes to the query port.
    pub fn refresh_a2s(&mut self, servers: &mut Servers, ip: &str, port: i64) {
        let asked = Self::key(servers, ip, port);
        if self.a2s_flights.iter().any(|(k, _)| *k == asked) {
            return;
        }
        let e = self.a2s.entry(asked.clone()).or_default();
        e.loading = true;
        e.error = None;
        let known = servers.find(ip, port);
        let hub = self.hub.clone();
        let ip = ip.to_string();
        let asked2 = asked.clone();
        let task = rt::spawn(async move {
            let row = match known {
                Some(r) => Some((*r).clone()),
                None => super::servers::lookup_one(&hub, &ip, port).await,
            };
            let qp = row.as_ref().map_or(port, |r| r.query_port);
            let gp = row.as_ref().map(|r| r.game_port);
            let key = format!("{ip}:{qp}");
            let r = hub.query_a2s(ip, qp, gp).await;
            (asked2, key, row, r)
        });
        self.a2s_flights.push((asked, task));
    }

    /// Players right now: from A2S when it has answered, else the list.
    pub fn players(&self, servers: &mut Servers, ip: &str, port: i64) -> super::servers::Live {
        let key = Self::key(servers, ip, port);
        if let Some(d) = self.a2s.get(&key).and_then(|e| e.data.as_ref()) {
            return super::servers::Live {
                players: i64::from(d.players),
                max: i64::from(d.max_players),
                bots: u32::from(d.bots),
            };
        }
        servers
            .find(ip, port)
            .map(|r| Servers::count(&r))
            .unwrap_or_default()
    }

    pub fn metrics(&self, ip: &str, game_port: u32, query_port: u32) -> Entry<ServerMetricsDto> {
        self.metrics
            .get(&format!("{ip}:{game_port}:{query_port}"))
            .cloned()
            .unwrap_or_default()
    }

    /// DayZ Metrics, from the cache while it is fresh unless `force`. `name` is how the
    /// list calls the server: the site finds it fastest by name.
    pub fn fetch_metrics(
        &mut self,
        ip: &str,
        game_port: u32,
        query_port: u32,
        name: &str,
        force: bool,
    ) {
        let hub = self.hub.clone();
        let (ip2, name) = (ip.to_string(), name.to_string());
        self.metrics.fetch(
            format!("{ip}:{game_port}:{query_port}"),
            force,
            move || async move {
                hub.fetch_server_metrics(ip2, game_port, query_port, name)
                    .await
            },
        );
    }

    /// The player-count history by the site's id (`metrics().data.id`), for `range`
    /// ("1d", "7d", "2w", "1m").
    pub fn history(&self, id: u64, range: &str) -> Entry<Vec<(i64, f64)>> {
        self.history
            .get(&format!("{id}:{range}"))
            .cloned()
            .unwrap_or_default()
    }

    pub fn fetch_history(&mut self, id: u64, range: &str) {
        let hub = self.hub.clone();
        let r = range.to_string();
        self.history
            .fetch(format!("{id}:{range}"), false, move || async move {
                let v = hub.fetch_metrics_history(id, r).await?;
                Ok(v.into_iter().filter(|(_, y)| y.is_finite()).collect())
            });
    }

    pub fn rank_history(&self, id: u64) -> Entry<Vec<(String, f64)>> {
        self.rank.get(&id).cloned().unwrap_or_default()
    }

    pub fn fetch_rank_history(&mut self, id: u64) {
        let hub = self.hub.clone();
        self.rank.fetch(id, false, move || async move {
            let v = hub.fetch_metrics_rank_history(id, "1m".to_string()).await?;
            Ok(v.into_iter().filter(|(_, y)| y.is_finite()).collect())
        });
    }

    pub fn heatmap(&self, id: u64) -> Entry<Vec<HeatCell>> {
        self.heat.get(&id).cloned().unwrap_or_default()
    }

    pub fn fetch_heatmap(&mut self, id: u64) {
        let hub = self.hub.clone();
        self.heat.fetch(id, false, move || async move {
            hub.fetch_metrics_heatmap(id).await
        });
    }

    pub fn poll(&mut self, servers: &mut Servers) -> bool {
        let mut changed = self.metrics.poll();
        changed |= self.history.poll();
        changed |= self.rank.poll();
        changed |= self.heat.poll();
        let mut done = Vec::new();
        self.a2s_flights.retain_mut(|(_, t)| match t.poll() {
            Some(a) => {
                done.push(a);
                false
            }
            None => t.is_pending(),
        });
        for (asked, key, row, r) in done {
            changed = true;
            if let Some(row) = row {
                servers.remember([row]);
            }
            if key != asked
                && let Some(e) = self.a2s.get_mut(&asked)
            {
                e.loading = false;
            }
            let e = self.a2s.entry(key.clone()).or_default();
            e.loading = false;
            match r {
                Ok(d) => {
                    servers.apply_live_count(
                        &key,
                        i64::from(d.players),
                        i64::from(d.max_players),
                        u32::from(d.bots),
                    );
                    servers.a2s_failures.remove(&key);
                    e.data = Some(d);
                    e.error = None;
                    e.fetched_at = Some(Instant::now());
                }
                Err(err) => {
                    e.error = Some(err);
                    servers.a2s_failures.insert(key);
                }
            }
        }
        if changed {
            evict(&mut self.a2s, MAX_A2S);
        }
        changed
    }
}
