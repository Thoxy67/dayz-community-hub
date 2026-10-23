//! DayZ Metrics details for the server panel's Stats tab: no key needed.
//!
//! Finding a server on the site takes a search and a few page reads, so the
//! id found for an address is kept on disk for a week (it is checked against
//! the address before use); the figures themselves for five minutes. Nothing is asked until a panel opens. The longer views (history
//! over weeks, rank over a month, busiest hours) are asked by the site's id,
//! once the panel has it, and cached by the window.

use crate::Hub;
use dz_dayzmetrics::KnownIds;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub use dz_dayzmetrics::HeatCell;
pub use dz_dayzmetrics::ServerMetrics as ServerMetricsDto;

const METRICS_TTL: Duration = Duration::from_secs(300);
/// How long a found id is used before it is searched for again.
const ID_TTL_SECS: i64 = 7 * 24 * 3600;

/// The ids found, read from disk at the first lookup of the run.
static KNOWN: Mutex<Option<KnownIds>> = Mutex::new(None);

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The id kept for `key`, reading the file first if this run has not.
async fn known_id(key: String) -> Option<dz_dayzmetrics::Resolved> {
    tokio::task::spawn_blocking(move || {
        let mut known = KNOWN.lock().ok()?;
        let ids =
            known.get_or_insert_with(|| KnownIds::load(&crate::paths::dayzmetrics_ids_path()));
        ids.get(&key, ID_TTL_SECS, now_secs())
    })
    .await
    .ok()
    .flatten()
}

/// Keep `r` for `key`, on disk when it changed anything.
fn remember_id(key: String, r: dz_dayzmetrics::Resolved) {
    tokio::task::spawn_blocking(move || {
        let Ok(mut known) = KNOWN.lock() else { return };
        let ids =
            known.get_or_insert_with(|| KnownIds::load(&crate::paths::dayzmetrics_ids_path()));
        if ids.put(&key, r, now_secs())
            && let Err(e) = ids.save(&crate::paths::dayzmetrics_ids_path())
        {
            eprintln!("[metrics] ids not saved: {e}");
        }
    });
}

impl Hub {
    /// What DayZ Metrics knows about the server at `ip`, by game or query port.
    /// `name`, as the list has it, finds the server quickly on the site.
    pub async fn fetch_server_metrics(
        &self,
        ip: String,
        game_port: u32,
        query_port: u32,
        name: String,
    ) -> Result<ServerMetricsDto, String> {
        mocked!(self, w => Ok(w.metrics(&ip)));
        let state = self.state()?;
        let key = format!("{ip}:{game_port}:{query_port}");

        {
            let s = state.read().await;
            if let Some((cached, at)) = s.dm_cache.peek(&key)
                && at.elapsed() < METRICS_TTL
            {
                return Ok(cached.clone());
            }
        }
        let known = known_id(key.clone()).await;

        let (resolved, metrics) = dz_dayzmetrics::lookup(
            crate::net::dayzmetrics(),
            &ip,
            game_port,
            query_port,
            &name,
            known,
        )
        .await?;

        remember_id(key.clone(), resolved);
        let mut s = state.write().await;
        s.dm_cache.put(key, (metrics.clone(), Instant::now()));
        Ok(metrics)
    }

    /// Player counts of the site's server `id` over `range` ("1d", "7d", "2w", "1m", "all").
    pub async fn fetch_metrics_history(
        &self,
        id: u64,
        range: String,
    ) -> Result<Vec<(i64, f64)>, String> {
        mocked!(self => Ok(crate::mock::fixtures::metrics_history(&range)));
        dz_dayzmetrics::history(crate::net::dayzmetrics(), id, &range).await
    }

    /// The site's daily rank of server `id` over `range` ("7d", "1m", "6m", "all").
    pub async fn fetch_metrics_rank_history(
        &self,
        id: u64,
        range: String,
    ) -> Result<Vec<(String, f64)>, String> {
        mocked!(self => Ok(crate::mock::fixtures::metrics_rank_history()));
        dz_dayzmetrics::rank_history(crate::net::dayzmetrics(), id, &range).await
    }

    /// Average players of the site's server `id` by weekday and UTC hour.
    pub async fn fetch_metrics_heatmap(&self, id: u64) -> Result<Vec<HeatCell>, String> {
        mocked!(self => Ok(crate::mock::fixtures::metrics_heatmap()));
        dz_dayzmetrics::heatmap(crate::net::dayzmetrics(), id).await
    }
}
