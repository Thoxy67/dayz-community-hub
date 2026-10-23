//! The account's Steam friends: who is online, who plays DayZ, and on which
//! server, for the Friends view.
//!
//! With a Web API key and a Steam id in the profile, Steam's servers are
//! asked (GetFriendList, GetPlayerSummaries): no Steam session, avatars, game
//! names; a friend's server shows only when their profile is public. Without
//! a key, or when that fails (a private friends list), the running Steam
//! client is asked instead (`dz_steamworks::friends`): Steam shows DayZ
//! running for a moment, game names come from the Store.
//!
//! The interface asks on a cadence while it matters (every 20 to 120 s, see
//! `lib/stores/friends.svelte.ts`). Asks closer together than `MIN_AGE`
//! share one answer, and the friend list itself is kept `LIST_AGE`, so a
//! refresh costs one GetPlayerSummaries call per hundred friends: far under
//! the Web API's 100 000 calls a day per key
//! (<https://steamcommunity.com/dev/apiterms>).

use crate::Hub;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::error::{HttpResultExt, send_ok};
use crate::state::{AppState, SharedState};

const DAYZ: u32 = 221_100;
/// Ids per GetPlayerSummaries call (Steam's limit).
const SUMMARIES_PAGE: usize = 100;
/// GetPlayerSummaries calls in flight at once.
const SUMMARIES_IN_FLIGHT: usize = 4;
/// Asks closer together than this get the last answer: the cadence, a
/// manual Refresh and the window's return can coincide.
const MIN_AGE: Duration = Duration::from_secs(10);
/// The friend list (who your friends are) is asked again after this; what
/// they are doing is asked every time.
const LIST_AGE: Duration = Duration::from_secs(10 * 60);

/// The last answer, behind an async lock: a second ask while one is in
/// flight waits for it and gets its answer instead of asking Steam again.
static LAST: LazyLock<tokio::sync::Mutex<Option<(Instant, FriendsDto)>>> =
    LazyLock::new(Default::default);
/// A friend list's ids: whose, when asked, the ids.
type KeptIds = (String, Instant, Vec<String>);
/// The friend list's ids, by the Steam id they belong to.
static IDS: LazyLock<Mutex<Option<KeptIds>>> = LazyLock::new(Default::default);

/// What a friend is doing.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Serialize, Clone, Debug)]
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

#[derive(Serialize, Clone, Debug)]
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
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FriendsSource {
    WebApi,
    SteamClient,
}

#[derive(Serialize, Clone, Debug)]
pub struct FriendsDto {
    pub source: Option<FriendsSource>,
    /// Why there are none, when there are none: a sentence.
    pub error: Option<String>,
    /// In DayZ first, then in another game, online, offline; by name within.
    pub friends: Vec<FriendDto>,
}

async fn ask(state: &SharedState, client: bool) -> Result<FriendsDto, String> {
    let web = crate::features::steam::access::web(state).await;
    let mut web_error = None;
    if let (Some(key), Some(id)) = (&web.key, &web.steam_id) {
        match from_web(key, id).await {
            Ok(friends) => return Ok(finish(state, friends, FriendsSource::WebApi).await),
            Err(e) => web_error = Some(e),
        }
    }
    if !client {
        return Ok(FriendsDto {
            source: None,
            error: Some(web_error.unwrap_or_else(|| "No Steam Web API key or Steam id.".into())),
            friends: Vec::new(),
        });
    }
    match tokio::task::spawn_blocking(dz_steamworks::friends).await {
        Ok(Ok(friends)) => {
            let friends = from_client(friends).await;
            Ok(finish(state, friends, FriendsSource::SteamClient).await)
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

/// The friend list's ids: kept `LIST_AGE`, else asked of Steam.
async fn friend_ids(key: &str, id: &str) -> Result<Vec<String>, String> {
    let kept = IDS.lock().ok().and_then(|g| {
        g.as_ref()
            .filter(|(of, at, _)| of == id && at.elapsed() < LIST_AGE)
            .map(|(.., ids)| ids.clone())
    });
    if let Some(ids) = kept {
        return Ok(ids);
    }
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
    if let Ok(mut g) = IDS.lock() {
        *g = Some((id.to_owned(), Instant::now(), ids.clone()));
    }
    Ok(ids)
}

async fn from_web(key: &str, id: &str) -> Result<Vec<Raw>, String> {
    let client = crate::net::api();
    let ids = friend_ids(key, id).await?;
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

/// A Store answer about one app: its name, or when asking failed (no Store
/// data for tools and delisted games, a refused request), so it is not asked
/// again on every poll.
#[derive(Clone, Debug)]
enum AppName {
    Known(String),
    Missing(Instant),
}

/// A failed Store lookup is tried again after this long; meanwhile the friend
/// shows "App N".
const APP_NAME_RETRY: Duration = Duration::from_secs(3600);

/// Game names by app id, from the Store (no key), asked once per run.
static APP_NAMES: LazyLock<Mutex<HashMap<u32, AppName>>> = LazyLock::new(Mutex::default);

/// The apps of `apps` to ask the Store about: neither known nor failed lately.
fn to_ask(known: &HashMap<u32, AppName>, apps: &[u32], now: Instant) -> Vec<u32> {
    let mut m: Vec<u32> = apps
        .iter()
        .copied()
        .filter(|a| match known.get(a) {
            Some(AppName::Known(_)) => false,
            Some(AppName::Missing(at)) => now.duration_since(*at) >= APP_NAME_RETRY,
            None => true,
        })
        .collect();
    m.sort_unstable();
    m.dedup();
    m
}

async fn app_names(apps: &[u32]) -> HashMap<u32, String> {
    let missing = {
        let known = APP_NAMES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        to_ask(&known, apps, Instant::now())
    };
    let found = futures_util::future::join_all(missing.into_iter().map(|app| async move {
        let url =
            format!("https://store.steampowered.com/api/appdetails?appids={app}&filters=basic");
        let name = async {
            let v: serde_json::Value = send_ok(crate::net::api().get(&url))
                .await
                .ok()?
                .json()
                .await
                .ok()?;
            Some(v[app.to_string()]["data"]["name"].as_str()?.to_owned())
        }
        .await;
        (
            app,
            name.map_or_else(|| AppName::Missing(Instant::now()), AppName::Known),
        )
    }))
    .await;
    let mut names = APP_NAMES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    names.extend(found);
    apps.iter()
        .filter_map(|a| match names.get(a) {
            Some(AppName::Known(n)) => Some((*a, n.clone())),
            _ => None,
        })
        .collect()
}

impl Hub {
    /// The account's friends and what they are doing. `client`: the running
    /// Steam client may be asked when the Web API cannot answer (it shows DayZ
    /// running for a moment, so only while the Friends view is open).
    pub async fn steam_friends(&self, client: bool) -> Result<FriendsDto, String> {
        mocked!(self, w => w.friends_answer());
        let state = self.state()?;
        let mut last = LAST.lock().await;
        if let Some((at, dto)) = last.as_ref()
            && at.elapsed() < MIN_AGE
            && (client || dto.source == Some(FriendsSource::WebApi))
        {
            return Ok(dto.clone());
        }
        let dto = ask(&state, client).await?;
        // Only answers: "the Web API is not set up" from a background ask must
        // not stand in for the view's own ask a moment later.
        if dto.source.is_some() {
            *last = Some((Instant::now(), dto.clone()));
        }
        Ok(dto)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_store_lookup_waits_before_it_is_asked_again() {
        let now = Instant::now();
        let mut known = HashMap::new();
        known.insert(1, AppName::Known("Game".into()));
        known.insert(2, AppName::Missing(now));
        assert_eq!(to_ask(&known, &[1, 2, 3, 3], now), vec![3]);
        let later = now + APP_NAME_RETRY;
        assert_eq!(to_ask(&known, &[1, 2, 3], later), vec![2, 3]);
    }

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
