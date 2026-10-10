//! The account's Steam friends: who is online, who plays DayZ, and on which
//! server, for the Friends view.
//!
//! With a Web API key and a Steam id in the profile, Steam's servers are
//! asked (GetFriendList, GetPlayerSummaries): no Steam session, avatars, game
//! names; a friend's server shows only when their profile is public. Without
//! a key, or when that fails (a private friends list), the running Steam
//! client is asked instead (`dz_steamworks::friends`): Steam shows DayZ
//! running for a moment, game names come from the Store.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{HttpResultExt, send_ok};
use crate::state::{AppState, SharedState};

const DAYZ: u32 = 221_100;
/// Ids per GetPlayerSummaries call (Steam's limit).
const SUMMARIES_PAGE: usize = 100;
/// GetPlayerSummaries calls in flight at once.
const SUMMARIES_IN_FLIGHT: usize = 4;

/// What a friend is doing.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum FriendStatus {
    Offline,
    Online,
    Busy,
    Away,
    Snooze,
}

impl FriendStatus {
    /// From `EPersonaState`; "looking to trade/play" read as online,
    /// invisible as offline.
    fn from_persona(state: i64) -> Self {
        match state {
            1 | 5 | 6 => Self::Online,
            2 => Self::Busy,
            3 => Self::Away,
            4 => Self::Snooze,
            _ => Self::Offline,
        }
    }
}

/// The server a friend is on.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct FriendServerDto {
    pub ip: String,
    /// The port to join with: the listed server's query port when it is in
    /// the list, else the game port Steam gave.
    pub port: u16,
    /// From the server list, when the server is in it.
    pub name: Option<String>,
    pub map: Option<String>,
    pub players: Option<i64>,
    pub max_players: Option<i64>,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct FriendDto {
    pub steam_id: String,
    pub name: String,
    pub status: FriendStatus,
    /// Steam's avatar picture (from the Web API only).
    pub avatar: Option<String>,
    /// The game they are in.
    pub game: Option<String>,
    pub in_dayz: bool,
    pub server: Option<FriendServerDto>,
    /// When they were last online, Unix seconds (from the Web API only).
    pub last_online: Option<i64>,
}

/// Where the friends came from.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum FriendsSource {
    WebApi,
    SteamClient,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct FriendsDto {
    pub source: Option<FriendsSource>,
    /// Why there are none, when there are none: a sentence.
    pub error: Option<String>,
    /// In DayZ first, then in another game, online, offline; by name within.
    pub friends: Vec<FriendDto>,
}

/// The account's friends and what they are doing.
#[tauri::command]
#[specta::specta]
pub(crate) async fn steam_friends(state: State<'_, SharedState>) -> Result<FriendsDto, String> {
    let web = crate::features::steam::access::web(&state).await;
    let mut web_error = None;
    if let (Some(key), Some(id)) = (&web.key, &web.steam_id) {
        match from_web(key, id).await {
            Ok(friends) => return Ok(finish(&state, friends, FriendsSource::WebApi).await),
            Err(e) => web_error = Some(e),
        }
    }
    match tauri::async_runtime::spawn_blocking(dz_steamworks::friends).await {
        Ok(Ok(friends)) => {
            let friends = from_client(friends).await;
            Ok(finish(&state, friends, FriendsSource::SteamClient).await)
        }
        Ok(Err(e)) => Ok(FriendsDto {
            source: None,
            error: Some(match web_error {
                Some(w) => format!("{w} {e}"),
                None => e,
            }),
            friends: Vec::new(),
        }),
        Err(e) => Err(e.to_string()),
    }
}

/// A friend before their server is looked up in the list.
struct Raw {
    dto: FriendDto,
    /// ip, game port (or unknown), query port (or 0).
    addr: Option<(String, u16, u16)>,
}

/// Look each friend's server up in the list, and sort.
async fn finish(state: &SharedState, raw: Vec<Raw>, source: FriendsSource) -> FriendsDto {
    let s = state.read().await;
    let mut friends: Vec<FriendDto> = raw
        .into_iter()
        .map(|r| {
            let mut dto = r.dto;
            dto.server = r
                .addr
                .map(|(ip, game, query)| server_of(&s, ip, game, query));
            dto
        })
        .collect();
    friends.sort_by_cached_key(|f| {
        let rank = match (f.in_dayz, f.game.is_some(), f.status) {
            (true, ..) => 0,
            (_, true, _) => 1,
            (_, _, FriendStatus::Offline) => 3,
            _ => 2,
        };
        (rank, f.server.is_none(), f.name.to_lowercase())
    });
    FriendsDto {
        source: Some(source),
        error: None,
        friends,
    }
}

fn server_of(s: &AppState, ip: String, game_port: u16, query_port: u16) -> FriendServerDto {
    let listed = (query_port != 0)
        .then(|| s.find_by_query_port(&ip, i64::from(query_port)))
        .flatten()
        .or_else(|| s.find_flexible(&ip, i64::from(game_port)));
    match listed {
        Some(l) => FriendServerDto {
            ip,
            port: u16::try_from(l.endpoint.port).unwrap_or(game_port),
            name: Some(l.name.clone()),
            map: Some(l.map.clone()),
            players: Some(l.players),
            max_players: Some(l.max_players),
        },
        None => FriendServerDto {
            ip,
            port: game_port,
            name: None,
            map: None,
            players: None,
            max_players: None,
        },
    }
}

// ── Steam's Web API ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FriendList {
    friendslist: FriendListInner,
}
#[derive(Deserialize)]
struct FriendListInner {
    #[serde(default)]
    friends: Vec<FriendEntry>,
}
#[derive(Deserialize)]
struct FriendEntry {
    steamid: String,
}

#[derive(Deserialize)]
struct Summaries {
    response: SummariesInner,
}
#[derive(Deserialize)]
struct SummariesInner {
    #[serde(default)]
    players: Vec<Summary>,
}
#[derive(Deserialize)]
struct Summary {
    steamid: String,
    #[serde(default)]
    personaname: String,
    #[serde(default)]
    personastate: i64,
    #[serde(default)]
    avatarmedium: Option<String>,
    #[serde(default)]
    gameid: Option<String>,
    #[serde(default)]
    gameextrainfo: Option<String>,
    #[serde(default)]
    gameserverip: Option<String>,
    #[serde(default)]
    lastlogoff: Option<i64>,
}

async fn from_web(key: &str, id: &str) -> Result<Vec<Raw>, String> {
    let client = crate::net::api();
    // The key goes as a query parameter, never into a string that could
    // end up in an error message.
    let list: FriendList = send_ok(
        client
            .get("https://api.steampowered.com/ISteamUser/GetFriendList/v1/")
            .query(&[("key", key), ("steamid", id), ("relationship", "friend")]),
    )
    .await
    .http_err("Steam friends list")?
    .json()
    .await
    .http_err("Steam friends list")?;
    let ids: Vec<String> = list
        .friendslist
        .friends
        .into_iter()
        .map(|f| f.steamid)
        .collect();
    // The pages at once (a few in flight): a long friends list is several
    // round trips otherwise, and the view waits on the last.
    let pages: Vec<String> = ids.chunks(SUMMARIES_PAGE).map(|p| p.join(",")).collect();
    let pages: Vec<Summaries> = futures_util::stream::iter(pages)
        .map(|page| async move {
            send_ok(
                client
                    .get("https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/")
                    .query(&[("key", key), ("steamids", page.as_str())]),
            )
            .await
            .http_err("Steam profiles")?
            .json::<Summaries>()
            .await
            .http_err("Steam profiles")
        })
        .buffered(SUMMARIES_IN_FLIGHT)
        .try_collect()
        .await?;
    let mut out = Vec::with_capacity(ids.len());
    for s in pages {
        out.extend(s.response.players.into_iter().map(|p| {
            let app: Option<u32> = p.gameid.as_deref().and_then(|g| g.parse().ok());
            let in_dayz = app == Some(DAYZ);
            let addr = p
                .gameserverip
                .as_deref()
                .and_then(|a| a.rsplit_once(':'))
                .and_then(|(ip, port)| Some((ip.to_owned(), port.parse::<u16>().ok()?, 0)))
                .filter(|(ip, port, _)| *port != 0 && ip != "0.0.0.0");
            Raw {
                dto: FriendDto {
                    steam_id: p.steamid,
                    name: p.personaname,
                    status: FriendStatus::from_persona(p.personastate),
                    avatar: p.avatarmedium,
                    game: p.gameextrainfo.or_else(|| app.map(|a| format!("App {a}"))),
                    in_dayz,
                    server: None,
                    last_online: p.lastlogoff,
                },
                addr: if in_dayz { addr } else { None },
            }
        }));
    }
    Ok(out)
}

// ── the Steam client ────────────────────────────────────────────────────

async fn from_client(friends: Vec<dz_steamworks::Friend>) -> Vec<Raw> {
    let apps: Vec<u32> = friends
        .iter()
        .filter_map(|f| f.game.as_ref().map(|g| g.app_id))
        .filter(|&a| a != DAYZ)
        .collect();
    let names = app_names(&apps).await;
    friends
        .into_iter()
        .map(|f| {
            let in_dayz = f.game.as_ref().is_some_and(|g| g.app_id == DAYZ);
            let game = f.game.as_ref().map(|g| {
                if g.app_id == DAYZ {
                    "DayZ".to_owned()
                } else {
                    names
                        .get(&g.app_id)
                        .cloned()
                        .unwrap_or_else(|| format!("App {}", g.app_id))
                }
            });
            let addr = f
                .game
                .as_ref()
                .filter(|_| in_dayz)
                .and_then(|g| g.server)
                .map(|s| (s.ip.to_string(), s.game_port, s.query_port));
            Raw {
                dto: FriendDto {
                    steam_id: f.steam_id.to_string(),
                    name: f.name,
                    status: FriendStatus::from_persona(i64::from(f.state)),
                    avatar: None,
                    game,
                    in_dayz,
                    server: None,
                    last_online: None,
                },
                addr,
            }
        })
        .collect()
}

/// Game names by app id, from the Store (no key), asked once per run.
static APP_NAMES: LazyLock<Mutex<HashMap<u32, String>>> = LazyLock::new(Mutex::default);

async fn app_names(apps: &[u32]) -> HashMap<u32, String> {
    let missing: Vec<u32> = {
        let known = APP_NAMES.lock().map(|m| m.clone()).unwrap_or_default();
        let mut m: Vec<u32> = apps
            .iter()
            .copied()
            .filter(|a| !known.contains_key(a))
            .collect();
        m.sort_unstable();
        m.dedup();
        m
    };
    let found = futures_util::future::join_all(missing.into_iter().map(|app| async move {
        let url =
            format!("https://store.steampowered.com/api/appdetails?appids={app}&filters=basic");
        let v: serde_json::Value = send_ok(crate::net::api().get(&url))
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let name = v[app.to_string()]["data"]["name"].as_str()?.to_owned();
        Some((app, name))
    }))
    .await;
    let mut names = APP_NAMES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    names.extend(found.into_iter().flatten());
    names.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persona_states_read_as_statuses() {
        assert_eq!(FriendStatus::from_persona(0), FriendStatus::Offline);
        assert_eq!(FriendStatus::from_persona(6), FriendStatus::Online);
        assert_eq!(FriendStatus::from_persona(3), FriendStatus::Away);
        assert_eq!(FriendStatus::from_persona(7), FriendStatus::Offline);
    }

    #[test]
    fn a_summary_without_a_server_reads() {
        let s: Summaries = serde_json::from_str(
            r#"{"response":{"players":[{"steamid":"1","personaname":"a","personastate":1,
            "gameid":"221100","gameextrainfo":"DayZ","gameserverip":"51.81.8.81:2402"}]}}"#,
        )
        .unwrap();
        assert_eq!(
            s.response.players[0].gameserverip.as_deref(),
            Some("51.81.8.81:2402")
        );
    }
}
