//! A server's live details over A2S: name, map, players online, rules and
//! the mods it announces.

use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::State;

use crate::error::ResultExt;
use crate::features::servers::{ModDto, mods_to_dto};
use crate::state::SharedState;

/// How long a server's details are served from the cache.
const A2S_CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct A2sPlayerDto {
    pub name: String,
    pub score: i32,
    pub duration: f32,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct A2sRuleDto {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct A2sDetailsDto {
    pub server_name: String,
    pub game: String,
    /// Human players (bots subtracted).
    pub players: u8,
    pub max_players: u8,
    /// Bots reported by A2S_INFO. DayZ servers commonly pad this to equal
    /// `players` to fake a full server.
    pub bots: u8,
    pub map: String,
    pub version: String,
    pub players_list: Vec<A2sPlayerDto>,
    /// Mods from the server list; empty for a server that is not listed.
    pub mods: Vec<ModDto>,
    /// Mod names from the A2S rules (fallback for unlisted servers, no workshop IDs).
    pub mods_from_a2s: Vec<String>,
    /// Server rules/cvars other than the mods.
    pub rules: Vec<A2sRuleDto>,
    /// The query port that was used.
    pub query_port: i64,
    /// The game port: from the server list, else from A2S's extended info.
    pub game_port: Option<i64>,
}

/// Query a server's live details (cached for 30 s).
#[tauri::command]
#[specta::specta]
pub(crate) async fn query_a2s(
    ip: String,
    query_port: i64,
    game_port: Option<i64>,
    state: State<'_, SharedState>,
) -> Result<Arc<A2sDetailsDto>, String> {
    let addr = format!("{ip}:{query_port}");

    // One read lock: peek the cache and, on a miss, take the listed mods.
    let mods = {
        let s = state.read().await;
        if let Some((cached, fetched_at)) = s.a2s_cache.peek(&addr)
            && fetched_at.elapsed() < A2S_CACHE_TTL
        {
            return Ok(Arc::clone(cached));
        }
        s.find_by_query_port(&ip, query_port)
            .or_else(|| game_port.and_then(|gp| s.find_flexible(&ip, gp)))
            .map(mods_to_dto)
            .unwrap_or_default()
    };

    // Info, players and rules concurrently.
    let (info, players, rules) = tokio::join!(
        dz_a2s::query_info(&addr),
        dz_a2s::query_players(&addr),
        dz_a2s::query_rules(&addr),
    );
    let info = info.cmd_err()?;

    let players_list = players
        .map(|pl| {
            pl.into_iter()
                .filter(|p| !p.name.is_empty())
                .map(|p| A2sPlayerDto {
                    name: p.name,
                    score: p.score,
                    duration: p.duration,
                })
                .collect()
        })
        .unwrap_or_default();

    let (mods_from_a2s, rules) = match rules {
        Ok(rules) => {
            let mods = dz_a2s::extract_mods_from_rules(&rules);
            let rules = rules
                .into_iter()
                .filter(|r| r.name != "mod" && r.name != "creator_dlc" && !r.name.is_empty())
                .map(|r| A2sRuleDto {
                    name: r.name,
                    value: r.value,
                })
                .collect();
            (mods, rules)
        }
        Err(_) => (vec![], vec![]),
    };

    let game_port = game_port.or_else(|| info.extended_server_info.port.map(i64::from));
    let players = dz_a2s::human_player_count(&info);

    let result = Arc::new(A2sDetailsDto {
        server_name: info.name,
        game: info.game,
        players,
        max_players: info.max_players,
        bots: info.bots,
        map: info.map,
        version: info.version,
        players_list,
        mods,
        mods_from_a2s,
        rules,
        query_port,
        game_port,
    });

    state
        .write()
        .await
        .a2s_cache
        .put(addr, (Arc::clone(&result), Instant::now()));
    Ok(result)
}
