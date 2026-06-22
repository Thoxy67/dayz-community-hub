use dayz_community_hub_core::{a2s_query, api};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::State;

use crate::dto::*;
use crate::state::{A2S_CACHE_TTL_SECS, SharedState};
use crate::utils::error::ResultExt;

/// Query A2S live info for a server.
///
/// New signature: accepts query_port directly and optional game_port.
/// Frontend is responsible for resolving ports from server list.
#[tauri::command]
pub(crate) async fn query_a2s(
    ip: String,
    query_port: i64,
    game_port: Option<i64>,
    state: State<'_, SharedState>,
) -> Result<Arc<A2sDetailsDto>, String> {
    let cache_key = format!("{}:{}", ip, query_port);

    // Single read-lock acquisition: peek the cache and (on a miss) resolve the
    // server's mod list in the same critical section. The (ip, query_port) → idx
    // index makes the lookup O(1) instead of an 18k-element linear scan, which
    // used to dominate the cold-start verify wave.
    let mods_dto = {
        let state_read = state.read().await;
        if let Some((cached, fetched_at)) = state_read.a2s_cache.peek(&cache_key)
            && fetched_at.elapsed() < Duration::from_secs(A2S_CACHE_TTL_SECS)
        {
            return Ok(cached.clone());
        }
        state_read
            .find_by_query_port(&ip, query_port)
            .or_else(|| game_port.and_then(|gp| state_read.find_flexible(&ip, gp)))
            .map(|s| {
                s.mods
                    .iter()
                    .map(|m| ModDto {
                        name: m.name.clone(),
                        steam_workshop_id: m.steam_workshop_id,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    let query_server = api::Server {
        endpoint: dayz_community_hub_core::Endpoint {
            ip: ip.clone(),
            port: query_port,
        },
        ..Default::default()
    };

    // Query info, players, and rules concurrently
    let qs_info = query_server.clone();
    let qs_players = query_server.clone();
    let qs_rules = query_server;
    let (info_res, players_res, rules_res) = tokio::join!(
        async move { a2s_query::query_server_info(&qs_info).await.cmd_err() },
        async move { a2s_query::query_player_info(&qs_players).await.cmd_err() },
        async move { a2s_query::query_rules(&qs_rules).await.cmd_err() }
    );

    let info = info_res?;

    let players_list = match players_res {
        Ok(pl) => pl
            .into_iter()
            .filter(|p| !p.name.is_empty())
            .map(|p| A2sPlayerDto {
                name: p.name.clone(),
                score: p.score,
                duration: p.duration,
            })
            .collect(),
        Err(_) => vec![],
    };

    // Extract mods and rules from A2S rules query
    let (mods_from_a2s, rules_list) = match rules_res {
        Ok(rules) => {
            let mods = a2s_query::extract_mods_from_rules(&rules);
            let non_mod_rules: Vec<A2sRuleDto> = rules
                .iter()
                .filter(|r| r.name != "mod" && r.name != "creator_dlc" && !r.name.is_empty())
                .map(|r| A2sRuleDto {
                    name: r.name.clone(),
                    value: r.value.clone(),
                })
                .collect();
            (mods, non_mod_rules)
        }
        Err(_) => (vec![], vec![]),
    };

    // Resolve game port: prefer provided, then A2S extended info
    let resolved_game_port = game_port.or_else(|| info.extended_server_info.port.map(|p| p as i64));

    // Compute before moving fields out of `info` below.
    let players = a2s_query::human_player_count(&info);

    let result = Arc::new(A2sDetailsDto {
        server_name: info.name,
        game: info.game,
        players,
        max_players: info.max_players,
        bots: info.bots,
        map: info.map,
        version: info.version,
        players_list,
        mods: mods_dto,
        mods_from_a2s,
        rules: rules_list,
        query_port,
        game_port: resolved_game_port,
    });

    // Cache the result (write lock). Storing the Arc is a refcount bump, not a
    // deep clone of the players/rules payload.
    {
        let mut state_write = state.write().await;
        state_write
            .a2s_cache
            .put(cache_key, (Arc::clone(&result), Instant::now()));
    }

    Ok(result)
}
