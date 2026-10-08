//! The account's friends as the Steam client has them (`ISteamFriends`):
//! who is online, what they play, and on which server. For when there is no
//! Web API key to ask Steam's servers with; the client knows even about
//! friends whose profile is private.

use std::ffi::CStr;
use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

use crate::session::{Session, begin};

/// `k_EFriendFlagImmediate`: regular friends.
const IMMEDIATE: i32 = 0x04;
/// More than `sizeof(FriendGameInfo_t)` (24 bytes) under any packing.
const GAME_INFO_LEN: usize = 32;

/// One friend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub steam_id: u64,
    pub name: String,
    /// `EPersonaState`: 0 offline, 1 online, 2 busy, 3 away, 4 snooze,
    /// 5 looking to trade, 6 looking to play, 7 invisible.
    pub state: i32,
    /// The game they are in, if any.
    pub game: Option<FriendGame>,
}

/// What a friend plays (`FriendGameInfo_t`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendGame {
    pub app_id: u32,
    /// The server they are on, when Steam knows it.
    pub server: Option<FriendServer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendServer {
    pub ip: Ipv4Addr,
    pub game_port: u16,
    /// 0 when unknown (`k_usFriendGameInfoQueryPort_NotInitialized`, ...Error).
    pub query_port: u16,
}

/// Every friend, as the running Steam client has them. Connects to Steam as
/// DayZ for a moment.
///
/// `Err` when no session could be opened: a sentence for the user.
pub(crate) fn friends() -> Result<Vec<Friend>, String> {
    let (_one, api) = begin()?;
    // SAFETY (all calls into `api` below): as in session::download.
    let session = Session::open(api)?;
    session.wait_logged_on()?;
    let iface = unsafe { (api.friends)() };
    if iface.is_null() {
        return Err(
            "This Steam client does not offer the friends interface. Update Steam and try again."
                .into(),
        );
    }
    session.pump(|_, _| {});
    let count = unsafe { (api.friend_count)(iface, IMMEDIATE) }.max(0);
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let id = unsafe { (api.friend_by_index)(iface, i, IMMEDIATE) };
        if id == 0 {
            continue;
        }
        let name = unsafe { (api.friend_name)(iface, id) };
        let name = if name.is_null() {
            String::new()
        } else {
            // SAFETY: a NUL-terminated string Steam keeps until the next call.
            unsafe { CStr::from_ptr(name) }
                .to_string_lossy()
                .into_owned()
        };
        let state = unsafe { (api.friend_state)(iface, id) };
        let mut info = [0u8; GAME_INFO_LEN];
        let playing = unsafe { (api.friend_game)(iface, id, info.as_mut_ptr()) };
        out.push(Friend {
            steam_id: id,
            name,
            state,
            game: if playing { parse_game(&info) } else { None },
        });
    }
    Ok(out)
}

/// A `FriendGameInfo_t`: `CGameID` (u64, the app id in its low 24 bits),
/// the server's IP (host order), game port, query port.
fn parse_game(b: &[u8]) -> Option<FriendGame> {
    let game_id = u64::from_le_bytes(b.get(0..8)?.try_into().ok()?);
    let app_id = (game_id & 0xFF_FFFF) as u32;
    if app_id == 0 {
        return None;
    }
    let ip = Ipv4Addr::from(u32::from_le_bytes(b.get(8..12)?.try_into().ok()?));
    let game_port = u16::from_le_bytes(b.get(12..14)?.try_into().ok()?);
    let query_port = u16::from_le_bytes(b.get(14..16)?.try_into().ok()?);
    // 0xFFFF and 0xFFFE are "not initialized" and "error".
    let query_port = if query_port >= 0xFFFE { 0 } else { query_port };
    let server = (!ip.is_unspecified() && game_port != 0).then_some(FriendServer {
        ip,
        game_port,
        query_port,
    });
    Some(FriendGame { app_id, server })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_friend_on_a_server_reads_its_address() {
        let mut b = [0u8; GAME_INFO_LEN];
        b[0..8].copy_from_slice(&221_100u64.to_le_bytes());
        b[8..12].copy_from_slice(&u32::from(Ipv4Addr::new(51, 81, 8, 81)).to_le_bytes());
        b[12..14].copy_from_slice(&2402u16.to_le_bytes());
        b[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let g = parse_game(&b).unwrap();
        assert_eq!(g.app_id, 221_100);
        let s = g.server.unwrap();
        assert_eq!(
            (s.ip, s.game_port, s.query_port),
            (Ipv4Addr::new(51, 81, 8, 81), 2402, 0)
        );
    }

    #[test]
    fn a_friend_in_a_menu_has_no_server_and_one_in_no_game_no_game() {
        let mut b = [0u8; GAME_INFO_LEN];
        b[0..8].copy_from_slice(&730u64.to_le_bytes());
        assert_eq!(parse_game(&b).unwrap().server, None);
        assert!(parse_game(&[0u8; GAME_INFO_LEN]).is_none());
    }
}
