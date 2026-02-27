use dayz_community_hub_core::{a2s_query, api};
use tauri::State;

use crate::dto::*;
use crate::state::SharedState;

/// Query A2S live info for a server.
#[tauri::command]
pub(crate) async fn query_a2s(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<A2sDetailsDto, String> {
    let (query_addr, mods_dto, list_game_port) = {
        let state = state.lock().await;
        let found = state
            .servers
            .iter()
            .find(|s| s.endpoint.ip == ip && (s.endpoint.port == port || s.game_port == port));
        match found {
            Some(s) => {
                let addr = format!("{}:{}", s.endpoint.ip, s.endpoint.port);
                let mods = s
                    .mods
                    .iter()
                    .map(|m| ModDto {
                        name: m.name.clone(),
                        steam_workshop_id: m.steam_workshop_id,
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "[query_a2s] found in list → query addr={addr} game_port={}",
                    s.game_port
                );
                (addr, mods, Some(s.game_port))
            }
            None => {
                let candidate_addr = if port == 2302 {
                    format!("{}:27016", ip)
                } else {
                    format!("{}:{}", ip, port)
                };
                eprintln!(
                    "[query_a2s] not in list → query addr={candidate_addr} (port={port}, using default query-port heuristic)"
                );
                (candidate_addr, vec![], None)
            }
        }
    };

    let resolved_port = query_addr
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(port);
    let query_server = api::Server {
        endpoint: dayz_community_hub_core::Endpoint {
            ip: ip.clone(),
            port: resolved_port,
        },
        ..Default::default()
    };

    eprintln!("[query_a2s] querying info + players concurrently …");
    let qs_info = query_server.clone();
    let qs_players = query_server;
    let (info_res, players_res) = tokio::join!(
        async move {
            a2s_query::query_server_info(&qs_info)
                .await
                .map_err(|e| e.to_string())
        },
        async move {
            a2s_query::query_player_info(&qs_players)
                .await
                .map_err(|e| e.to_string())
        }
    );
    let info = info_res?;
    eprintln!(
        "[query_a2s] info ok: name={:?} players={}/{}",
        info.name, info.players, info.max_players
    );
    let players_list = match players_res {
        Ok(pl) => {
            eprintln!("[query_a2s] players ok: {} entries", pl.len());
            pl.into_iter()
                .filter(|p| !p.name.is_empty())
                .map(|p| A2sPlayerDto {
                    name: p.name.clone(),
                    score: p.score,
                    duration: p.duration,
                })
                .collect()
        }
        Err(e) => {
            eprintln!("[query_a2s] players failed (ok for DayZ): {e}");
            vec![]
        }
    };

    let query_port = query_addr
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(port);

    let game_port = list_game_port.or_else(|| info.extended_server_info.port.map(|p| p as i64));

    eprintln!("[query_a2s] resolved game_port={:?}", game_port);

    Ok(A2sDetailsDto {
        server_name: info.name,
        game: info.game,
        players: info.players,
        max_players: info.max_players,
        map: info.map,
        version: info.version,
        players_list,
        mods: mods_dto,
        query_port,
        game_port,
    })
}
