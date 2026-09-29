//! DayZ Metrics details for the server panel's Stats tab: no key needed.
//!
//! Finding a server on the site takes a search and a few page reads, so the
//! id found for an address is kept for a day; the figures themselves for five
//! minutes. Nothing is asked until a panel opens.

use std::time::{Duration, Instant};
use tauri::State;

use crate::state::SharedState;

pub use dz_dayzmetrics::ServerMetrics as ServerMetricsDto;

const METRICS_TTL: Duration = Duration::from_secs(300);
const ID_TTL: Duration = Duration::from_secs(24 * 3600);

/// What DayZ Metrics knows about the server at `ip`, by game or query port.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_server_metrics(
    ip: String,
    game_port: u32,
    query_port: u32,
    state: State<'_, SharedState>,
) -> Result<ServerMetricsDto, String> {
    let key = format!("{ip}:{game_port}:{query_port}");

    let known = {
        let s = state.read().await;
        if let Some((cached, at)) = s.dm_cache.peek(&key)
            && at.elapsed() < METRICS_TTL
        {
            return Ok(cached.clone());
        }
        s.dm_ids
            .peek(&key)
            .filter(|(_, at)| at.elapsed() < ID_TTL)
            .map(|(r, _)| *r)
    };

    let (resolved, metrics) =
        dz_dayzmetrics::lookup(crate::net::dayzmetrics(), &ip, game_port, query_port, known)
            .await?;

    let mut s = state.write().await;
    s.dm_ids.put(key.clone(), (resolved, Instant::now()));
    s.dm_cache.put(key, (metrics.clone(), Instant::now()));
    Ok(metrics)
}
