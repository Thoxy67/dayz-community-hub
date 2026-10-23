//! One socket talking to one server: requests, the challenges DayZ guards
//! them with, and split answers, each request within one deadline.
//!
//! A2S_INFO is asked here too, rather than by `async-a2s`, for two reasons.
//! Its challenge round trip waited a whole timeout again for the answer, so
//! a server that answered the challenge and dropped the rest held a slot for
//! twice the time asked: here the challenge and the answer share one
//! deadline. And the challenge A2S_INFO got is good for A2S_RULES on the
//! same socket (measured on 38 DayZ servers: 37 of 37 answered the rules at
//! once, the challenge still good 10 s later, gone after 150 s), so a
//! modded server's rules cost one round trip instead of two.

use std::io::Cursor;
use std::time::Duration;

use async_a2s::info::Info;
use dz_common::{Error, Result};
use tokio::net::UdpSocket;
use tokio::time::{Instant, sleep};

use crate::rules::{self, DayzRules};

const HEADER: [u8; 4] = [0xFF; 4];
/// A2S_INFO's body: the kind byte and its C string.
const A2S_INFO_REQUEST: &[u8] = b"TSource Engine Query\0";
const S2A_INFO: u8 = 0x49;
const S2C_CHALLENGE: u8 = 0x41;
/// A server that keeps answering with a new challenge is not going to answer.
const MAX_CHALLENGES: usize = 3;

/// A socket connected to one server, and the last challenge it handed out.
pub(crate) struct Exchange {
    socket: UdpSocket,
    addr: String,
    challenge: Option<[u8; 4]>,
    /// One receive buffer for every answer, challenges included.
    buf: Vec<u8>,
}

impl Exchange {
    pub(crate) async fn open(addr: &str) -> Result<Self> {
        let err = |e: std::io::Error| Error::A2sQuery(format!("A2S query to {addr}: {e}"));
        let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(err)?;
        socket.connect(addr).await.map_err(err)?;
        Ok(Self {
            socket,
            addr: addr.to_owned(),
            challenge: None,
            buf: vec![0u8; 65_535],
        })
    }

    /// Send `request` (after the `FF FF FF FF` header) and return the whole
    /// answer from its `answer` byte, with the first round trip's time. The
    /// challenge known is sent along (`FF FF FF FF` when none is and
    /// `challenged`, nothing for a request that may go without one); a new
    /// challenge is followed. A late answer to an earlier A2S_INFO on this
    /// socket is skipped.
    pub(crate) async fn ask(
        &mut self,
        what: &str,
        request: &[u8],
        challenged: bool,
        answer: u8,
        deadline: Instant,
    ) -> Result<(Vec<u8>, Duration)> {
        let addr = &self.addr;
        let err = |e: String| Error::A2sQuery(format!("A2S {what} query to {addr}: {e}"));
        let mut packet = Vec::with_capacity(HEADER.len() + request.len() + 4);
        let mut first_trip = None;
        for _ in 0..MAX_CHALLENGES {
            packet.clear();
            packet.extend_from_slice(&HEADER);
            packet.extend_from_slice(request);
            match self.challenge {
                Some(c) => packet.extend_from_slice(&c),
                None if challenged => packet.extend_from_slice(&[0xFF; 4]),
                None => {}
            }
            let sent = Instant::now();
            self.socket
                .send(&packet)
                .await
                .map_err(|e| err(e.to_string()))?;
            let payload = loop {
                let p = rules::receive(&self.socket, &mut self.buf, deadline)
                    .await
                    .map_err(err)?;
                if p.first() == Some(&S2A_INFO) && answer != S2A_INFO {
                    continue;
                }
                break p;
            };
            let trip = *first_trip.get_or_insert_with(|| sent.elapsed());
            match payload.first() {
                Some(&S2C_CHALLENGE) if payload.len() >= 5 => {
                    let mut c = [0u8; 4];
                    c.copy_from_slice(&payload[1..5]);
                    self.challenge = Some(c);
                }
                Some(&b) if b == answer => return Ok((payload, trip)),
                Some(b) => return Err(err(format!("unexpected answer {b:#04x}"))),
                None => return Err(err("empty answer".into())),
            }
        }
        Err(err("the server keeps sending new challenges".into()))
    }
}

/// What A2S_INFO answered, with the socket and challenge it was asked on.
pub struct InfoAnswer {
    pub info: Info,
    /// The first round trip: the challenge's, for a server that sends one.
    pub latency: Duration,
    exchange: Exchange,
}

impl InfoAnswer {
    /// The answer as a ping.
    pub fn ping(&self) -> crate::Ping {
        crate::ping_of(&self.info, self.latency)
    }

    /// The server's A2S_RULES, asked on the same socket with the challenge
    /// A2S_INFO got, within `timeout`. If that fails (the challenge expired,
    /// a datagram was lost), asked once more on a fresh socket after a short
    /// pause, as [`rules::query_dayz_rules`] would.
    pub async fn rules(mut self, timeout: Duration) -> Result<DayzRules> {
        let first = self
            .exchange
            .ask(
                "rules",
                &[rules::A2S_RULES],
                true,
                rules::S2A_RULES,
                Instant::now() + timeout,
            )
            .await;
        let payload = match first {
            Ok((payload, _)) => payload,
            Err(_) => {
                sleep(Duration::from_millis(300)).await;
                rules::fetch_once(
                    &self.exchange.addr,
                    rules::A2S_RULES,
                    rules::S2A_RULES,
                    timeout,
                )
                .await?
            }
        };
        rules::parse_rules(&payload)
    }
}

/// A2S_INFO from `addr` (`"ip:port"`), the challenge and the answer both
/// within `timeout`. No retry: a caller that wants another try asks again.
pub async fn info_within(addr: &str, timeout: Duration) -> Result<InfoAnswer> {
    let deadline = Instant::now() + timeout;
    let mut exchange = Exchange::open(addr).await?;
    let (payload, latency) = exchange
        .ask("info", A2S_INFO_REQUEST, false, S2A_INFO, deadline)
        .await?;
    let info = Info::from_cursor(Cursor::new(payload))
        .map_err(|e| Error::A2sQuery(format!("A2S info answer from {addr}: {e}")))?;
    Ok(InfoAnswer {
        info,
        latency,
        exchange,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// A DayZ server's A2S_INFO answer, from its `0x49` byte.
    fn info_payload() -> Vec<u8> {
        let mut p = vec![S2A_INFO, 17];
        for s in ["Test server", "chernarusplus", "dayz", "DayZ"] {
            p.extend_from_slice(s.as_bytes());
            p.push(0);
        }
        p.extend_from_slice(&0u16.to_le_bytes());
        p.extend_from_slice(&[3, 60, 0, b'd', b'w', 0, 1]);
        p.extend_from_slice(b"1.26.0\0");
        p.push(0x80 | 0x20);
        p.extend_from_slice(&2302u16.to_le_bytes());
        p.extend_from_slice(b"battleye,mod\0");
        p
    }

    /// A server that challenges every request, answers A2S_INFO and the
    /// rules, and counts the requests it got.
    async fn challenging_server() -> (String, tokio::task::JoinHandle<Vec<Vec<u8>>>) {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap().to_string();
        let task = tokio::spawn(async move {
            let challenge = [1, 2, 3, 4];
            let mut got = Vec::new();
            let mut buf = [0u8; 1500];
            while got.len() < 3 {
                let (n, from) = socket.recv_from(&mut buf).await.unwrap();
                let req = buf[..n].to_vec();
                got.push(req.clone());
                let mut out = HEADER.to_vec();
                if !req.ends_with(&challenge) {
                    out.push(S2C_CHALLENGE);
                    out.extend_from_slice(&challenge);
                } else if req[4] == b'T' {
                    out.extend_from_slice(&info_payload());
                } else {
                    // Rules: two plain ones, no mods block.
                    out.extend_from_slice(&[rules::S2A_RULES, 2, 0]);
                    out.extend_from_slice(b"dedicated\x001\x00platform\x00win\x00");
                }
                socket.send_to(&out, from).await.unwrap();
            }
            got
        });
        (addr, task)
    }

    #[tokio::test]
    async fn the_rules_reuse_the_challenge_info_got() {
        let (addr, server) = challenging_server().await;
        let answer = info_within(&addr, Duration::from_secs(2)).await.unwrap();
        assert_eq!(answer.info.name, "Test server");
        assert_eq!(answer.info.extended_server_info.port, Some(2302));
        assert_eq!(answer.ping().players, 3);
        let rules = answer.rules(Duration::from_secs(2)).await.unwrap();
        assert_eq!(rules.get("platform"), Some("win"));
        let got = server.await.unwrap();
        // Info, info with the challenge, then the rules with it at once.
        assert_eq!(got.len(), 3);
        assert_eq!(&got[0][4..], A2S_INFO_REQUEST);
        assert_eq!(got[2][4], rules::A2S_RULES);
        assert!(got[2].ends_with(&[1, 2, 3, 4]));
    }

    #[tokio::test]
    async fn a_server_that_drops_the_answer_holds_one_timeout_not_two() {
        // Answers the challenge, never the info.
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            let (_, from) = socket.recv_from(&mut buf).await.unwrap();
            let mut out = HEADER.to_vec();
            out.extend_from_slice(&[S2C_CHALLENGE, 9, 9, 9, 9]);
            socket.send_to(&out, from).await.unwrap();
            // Keep the socket open, silent.
            let _ = socket.recv_from(&mut buf).await;
        });
        let t = std::time::Instant::now();
        assert!(
            info_within(&addr, Duration::from_millis(300))
                .await
                .is_err()
        );
        assert!(
            t.elapsed() < Duration::from_millis(550),
            "{:?}",
            t.elapsed()
        );
    }
}
