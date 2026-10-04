//! DayZ Metrics details for the server panel's Stats tab: no key needed.
//!
//! Finding a server on the site takes a search and a few page reads, so the
//! id found for an address is kept for a day; the figures themselves for five
//! minutes. Nothing is asked until a panel opens. The longer views (history
//! over weeks, rank over a month, busiest hours) are asked by the site's id,
//! once the panel has it, and cached by the window.

use std::time::{Duration, Instant};
use tauri::State;

use crate::state::SharedState;

pub use dz_dayzmetrics::HeatCell;
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

/// Player counts of the site's server `id` over `range` ("1d", "7d", "2w", "1m", "all").
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_metrics_history(
    id: u64,
    range: String,
) -> Result<Vec<(i64, f64)>, String> {
    dz_dayzmetrics::history(crate::net::dayzmetrics(), id, &range).await
}

/// The site's daily rank of server `id` over `range` ("7d", "1m", "6m", "all").
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_metrics_rank_history(
    id: u64,
    range: String,
) -> Result<Vec<(String, f64)>, String> {
    dz_dayzmetrics::rank_history(crate::net::dayzmetrics(), id, &range).await
}

/// Average players of the site's server `id` by weekday and UTC hour.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_metrics_heatmap(id: u64) -> Result<Vec<HeatCell>, String> {
    dz_dayzmetrics::heatmap(crate::net::dayzmetrics(), id).await
}
