//! A2S_PLAYER, read here rather than by `async-a2s`: on busy DayZ servers
//! its reader failed ("Invalid PLAYER response header", "Invalid challenge
//! response") on about one server in fifteen, those with the most players
//! among them. The request, the challenge and split answers go through the
//! same code as A2S_RULES.

use std::time::Duration;

use dz_common::{Error, Result};

use crate::rules::fetch;

const A2S_PLAYER: u8 = 0x55;
const S2A_PLAYER: u8 = 0x44;

/// One player on a server.
#[derive(Debug, Clone, PartialEq)]
pub struct A2sPlayer {
    pub name: String,
    /// Kills, for the games that count them; DayZ sends 0.
    pub score: i32,
    /// Seconds since this player connected.
    pub duration: f32,
}

/// Who is on the server at `addr` (`"ip:port"`).
pub async fn query_players(addr: &str, timeout: Duration) -> Result<Vec<A2sPlayer>> {
    parse_players(&fetch(addr, A2S_PLAYER, S2A_PLAYER, timeout).await?)
}

/// Read an A2S_PLAYER answer (from its `0x44` byte). The count is only a
/// hint: a truncated answer yields the players that arrived whole.
pub fn parse_players(payload: &[u8]) -> Result<Vec<A2sPlayer>> {
    let bad = |what: &str| Error::A2sQuery(format!("A2S players answer: {what}"));
    let (&kind, rest) = payload.split_first().ok_or_else(|| bad("empty"))?;
    if kind != S2A_PLAYER {
        return Err(bad("not a players answer"));
    }
    let (&count, mut cursor) = rest.split_first().ok_or_else(|| bad("no count"))?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        // index (u8), name (C string), score (i32), duration (f32).
        let Some(after_index) = cursor.get(1..) else {
            break;
        };
        let Some(nul) = after_index.iter().position(|&b| b == 0) else {
            break;
        };
        let name = String::from_utf8_lossy(&after_index[..nul]).into_owned();
        let Some(tail) = after_index.get(nul + 1..nul + 9) else {
            break;
        };
        out.push(A2sPlayer {
            name,
            score: i32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]),
            duration: f32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]),
        });
        cursor = &after_index[nul + 9..];
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(name: &str, score: i32, duration: f32) -> Vec<u8> {
        let mut b = vec![0u8];
        b.extend_from_slice(name.as_bytes());
        b.push(0);
        b.extend_from_slice(&score.to_le_bytes());
        b.extend_from_slice(&duration.to_le_bytes());
        b
    }

    #[test]
    fn reads_players() {
        let mut p = vec![S2A_PLAYER, 2];
        p.extend(player("Survivor", 0, 61.5));
        p.extend(player("", 3, 7.0));
        let got = parse_players(&p).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].name, "Survivor");
        assert_eq!(got[0].duration, 61.5);
        assert_eq!(got[1].score, 3);
    }

    #[test]
    fn keeps_the_whole_players_of_a_truncated_answer() {
        let mut p = vec![S2A_PLAYER, 3];
        p.extend(player("One", 0, 1.0));
        let mut two = player("Two", 0, 2.0);
        two.truncate(6);
        p.extend(two);
        assert_eq!(parse_players(&p).unwrap().len(), 1);
    }

    #[test]
    fn refuses_another_answer() {
        assert!(parse_players(&[0x45, 0]).is_err());
        assert!(parse_players(&[]).is_err());
    }
}
