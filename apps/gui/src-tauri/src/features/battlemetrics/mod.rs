//! BattleMetrics details for the server panel, cached for five minutes.

use std::time::{Duration, Instant};
use tauri::State;

use crate::state::{SharedState, insecure_client};

pub use dz_battlemetrics::BattleMetricsServer as BattleMetricsDto;

const BM_CACHE_TTL: Duration = Duration::from_secs(300);

/// Find a server on BattleMetrics by IP and ports, falling back to its name.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_battlemetrics_server(
    ip: String,
    port: i64,
    query_port: i64,
    name: String,
    state: State<'_, SharedState>,
) -> Result<BattleMetricsDto, String> {
    let key = format!("{ip}:{port}:{query_port}");

    let token = {
        let s = state.read().await;
        if let Some((cached, fetched_at)) = s.bm_cache.peek(&key)
            && fetched_at.elapsed() < BM_CACHE_TTL
        {
            return Ok(cached.clone());
        }
        s.ctl
            .profile()
            .battlemetrics_api_key
            .clone()
            .ok_or_else(|| "No BattleMetrics API key configured".to_string())?
    };

    let result =
        dz_battlemetrics::lookup(insecure_client(), &token, &ip, port, query_port, &name).await?;

    state
        .write()
        .await
        .bm_cache
        .put(key, (result.clone(), Instant::now()));
    Ok(result)
}
