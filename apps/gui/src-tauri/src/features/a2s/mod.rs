//! A server's live details over A2S: name, map, players online, rules, the
//! settings DayZ states in its keywords and the mods it announces.

use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::State;

use crate::error::ResultExt;
use crate::features::servers::{ModDto, mods_to_dto};
use crate::state::SharedState;

/// How long a server's details are served from the cache.
const A2S_CACHE_TTL: Duration = Duration::from_secs(30);
/// How long one A2S_RULES attempt waits (it is tried twice).
const RULES_TIMEOUT: Duration = Duration::from_secs(3);
/// The longest the panel waits for a server's info and players. async-a2s
/// retries a silent server twice at 5 s each: 15 s of a panel loading,
/// which reads as never.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(6);

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

/// What DayZ states about itself in A2S_INFO's keywords.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct A2sDayzDto {
    pub battleye: bool,
    /// Third-person view is off.
    pub first_person_only: bool,
    /// A shard of Bohemia's public hive: not a community server, no private hive.
    pub official: bool,
    /// The server keeps its own characters.
    pub private_hive: bool,
    pub whitelisted: bool,
    /// The server needs a DLC map.
    pub dlc: bool,
    pub shard: Option<String>,
    /// Players waiting in the login queue.
    pub login_queue: Option<u32>,
    /// How many times faster than real time the day passes.
    pub time_accel: Option<f32>,
    /// The same, at night.
    pub night_time_accel: Option<f32>,
    /// The in-game clock, `HH:MM`.
    pub game_time: Option<String>,
}

impl From<dz_a2s::DayzInfo> for A2sDayzDto {
    fn from(i: dz_a2s::DayzInfo) -> Self {
        Self {
            battleye: i.battleye,
            first_person_only: i.first_person_only,
            official: i.official(),
            private_hive: i.private_hive,
            whitelisted: i.whitelisted,
            dlc: i.dlc,
            shard: i.shard,
            login_queue: i.login_queue,
            time_accel: i.time_accel,
            night_time_accel: i.night_time_accel,
            game_time: i.game_time,
        }
    }
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
    /// Mod names from the A2S rules, in load order.
    pub mods_from_a2s: Vec<String>,
    /// The mods the server announces over A2S, with their Workshop ids: what
    /// an unlisted server needs to be joined.
    pub mods_a2s: Vec<ModDto>,
    /// The settings from A2S_INFO's keywords; `None` when it sent none.
    pub dayz: Option<A2sDayzDto>,
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
        tokio::time::timeout(ANSWER_TIMEOUT, dz_a2s::query_info(&addr)),
        tokio::time::timeout(ANSWER_TIMEOUT, dz_a2s::query_players(&addr, RULES_TIMEOUT)),
        dz_a2s::query_dayz_rules(&addr, RULES_TIMEOUT),
    );
    let info = info
        .map_err(|_| {
            format!(
                "{addr} did not answer within {} s",
                ANSWER_TIMEOUT.as_secs()
            )
        })?
        .cmd_err()?;
    let players = players.unwrap_or_else(|_| Err(dz_common::Error::A2sQuery("timed out".into())));

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

    let (mods_from_a2s, mods_a2s, rules) = match rules {
        Ok(r) => (
            r.mods.iter().map(|m| m.name.clone()).collect(),
            r.mods
                .iter()
                .filter_map(|m| {
                    Some(ModDto {
                        name: m.name.clone(),
                        steam_workshop_id: i64::try_from(m.id?).ok()?,
                    })
                })
                .collect(),
            r.rules
                .into_iter()
                .filter(|(name, _)| !name.is_empty())
                .map(|(name, value)| A2sRuleDto { name, value })
                .collect(),
        ),
        Err(_) => (vec![], vec![], vec![]),
    };
    let dayz = info
        .extended_server_info
        .keywords
        .as_deref()
        .filter(|k| !k.is_empty())
        .map(|k| dz_a2s::DayzInfo::parse(k).into());

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
        mods_a2s,
        dayz,
        rules,
        query_port,
        game_port,
    });

    crate::features::browser::live::store().record_counts(
        &ip,
        query_port,
        result.players,
        result.max_players,
        result.bots,
    );
    state
        .write()
        .await
        .a2s_cache
        .put(addr, (Arc::clone(&result), Instant::now()));
    Ok(result)
}
