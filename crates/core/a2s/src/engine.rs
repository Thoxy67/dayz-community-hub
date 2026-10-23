//! Many servers asked over one socket: what a scan of the whole list, a
//! listing's queries and the checks after a refresh go through.
//!
//! One unconnected UDP socket per address family carries every request,
//! sent at a paced rate rather than in bursts, and each reply is routed by
//! the address it came from to the request waiting for it. A server's
//! challenge (`0x41`) is answered on the same socket, for the same entry,
//! within the request's one deadline; every deadline sits in one queue, read
//! by one timer, instead of a timer per request.
//!
//! A2S answers carry no request id, so an answer is only as good as the
//! guess of which request it answers. One request to an address is out at
//! a time, an answer of the other kind is ignored, and an address whose
//! request timed out stays quiet for half that timeout (less if the late
//! answer comes): what arrives meanwhile is dropped rather than taken, with
//! a far too short round trip, by the next request there.
//!
//! The official DayZ launcher, captured on the owner's machine, queries the
//! same way: one socket for every A2S_INFO, about 85 to 160 a second, a few
//! in flight, no retries, and about 0.4% of the answers lost. Before this, a
//! socket was bound and connected for each query and again for its
//! challenge (two sockets and about sixteen system calls a server), and a
//! burst of them lost enough replies that every listing ran a slower second
//! pass for what had not answered.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::io::{self, Cursor};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::pin;
use std::time::Duration;

use async_a2s::info::Info;
use dz_common::{Error, Result};
use futures_util::{Stream, StreamExt};
use rustc_hash::FxHashMap;
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep_until};

use crate::InfoAnswer;
use crate::exchange::{A2S_INFO_REQUEST, HEADER, MAX_CHALLENGES, S2A_INFO, S2C_CHALLENGE};
use crate::rules::{self, DayzRules, Reassembly};

/// The receive buffer asked of the system for each socket, so the answers
/// of a busy second wait there rather than being dropped. Best effort:
/// Linux caps it at `net.core.rmem_max` (often 208 KB), Windows grants it.
const RECV_BUFFER: usize = 4 << 20;
/// Datagrams read in one go before sends and deadlines get a turn.
const DRAIN_MAX: usize = 256;

/// How fast an [`Engine`] sends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pace {
    /// New requests sent a second. A challenge's resend is not counted: it
    /// answers the server at once, within the request's deadline.
    pub rate: u32,
    /// Requests that may go out back to back after a quiet moment.
    pub burst: u32,
    /// Requests waiting for their answer at most; the rest wait their turn.
    pub in_flight: usize,
}

impl Default for Pace {
    /// 250 servers a second, a little over the official launcher's top
    /// pace, and in flight at most what that rate sends in two seconds,
    /// the longest a scan usually waits.
    fn default() -> Self {
        Self {
            rate: 250,
            burst: 8,
            in_flight: 500,
        }
    }
}

/// A handle on the sockets and the task that drives them. Cheap to clone;
/// the task ends once every handle is gone and the last answer is in.
#[derive(Clone)]
pub struct Engine {
    tx: mpsc::UnboundedSender<Request>,
    pace: Pace,
}

impl Engine {
    /// Start an engine on the current Tokio runtime. Its sockets are opened
    /// at the first request of their address family.
    pub fn new(pace: Pace) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(Driver::new(pace, rx).run());
        Self { tx, pace }
    }

    /// Its task has ended (the runtime it ran on shut down): a new engine is
    /// needed.
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

    /// Requests to keep handed to the engine for its pace to be the limit
    /// when each may wait `timeout`: what it sends in that time, within its
    /// in-flight ceiling.
    pub fn window(&self, timeout: Duration) -> usize {
        let sent = f64::from(self.pace.rate) * timeout.as_secs_f64();
        (sent.ceil() as usize).clamp(1, self.pace.in_flight.max(1))
    }

    /// A2S_INFO from `addr`, the challenge and the answer both within
    /// `timeout` from the moment it is sent. No retry.
    pub async fn info(&self, addr: SocketAddr, timeout: Duration) -> Result<InfoAnswer> {
        let reply = self.ask(addr, Kind::Info, None, timeout).await?;
        let info = Info::from_cursor(Cursor::new(reply.payload))
            .map_err(|e| Error::A2sQuery(format!("A2S info answer from {addr}: {e}")))?;
        Ok(InfoAnswer {
            info,
            latency: reply.latency,
            challenge: reply.challenge,
        })
    }

    /// A2S_RULES from `addr`, sent with `challenge` (the one its A2S_INFO
    /// got, see [`InfoAnswer::challenge`]) or else `00000000`, which most
    /// DayZ servers answer at once (72% in the official launcher's capture);
    /// a server that wants another challenge gets it asked. Within `timeout`.
    pub async fn rules(
        &self,
        addr: SocketAddr,
        challenge: Option<[u8; 4]>,
        timeout: Duration,
    ) -> Result<DayzRules> {
        let reply = self.ask(addr, Kind::Rules, challenge, timeout).await?;
        rules::parse_rules(&reply.payload)
    }

    /// A2S_INFO from every target, each within `timeout`, the answers as
    /// they come. A [`window`](Self::window) of targets is handed on at a
    /// time, so a target waits here, not in the engine.
    pub fn infos<K, S>(
        &self,
        targets: S,
        timeout: Duration,
    ) -> impl Stream<Item = (K, Result<InfoAnswer>)> + use<K, S>
    where
        S: Stream<Item = (K, SocketAddr)>,
    {
        let engine = self.clone();
        let window = self.window(timeout);
        targets
            .map(move |(key, addr)| {
                let engine = engine.clone();
                async move { (key, engine.info(addr, timeout).await) }
            })
            .buffer_unordered(window)
    }

    async fn ask(
        &self,
        addr: SocketAddr,
        kind: Kind,
        challenge: Option<[u8; 4]>,
        timeout: Duration,
    ) -> Result<Reply> {
        let what = kind.what();
        let err = |e: &str| Error::A2sQuery(format!("A2S {what} query to {addr}: {e}"));
        let (reply, answer) = oneshot::channel();
        self.tx
            .send(Request {
                addr,
                kind,
                challenge,
                timeout,
                reply,
            })
            .map_err(|_| err("the query engine has stopped"))?;
        answer
            .await
            .map_err(|_| err("the query engine has stopped"))?
            .map_err(|e| err(&e))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Info,
    Rules,
}

impl Kind {
    fn what(self) -> &'static str {
        match self {
            Kind::Info => "info",
            Kind::Rules => "rules",
        }
    }

    /// The byte its answer starts with.
    fn answer(self) -> u8 {
        match self {
            Kind::Info => S2A_INFO,
            Kind::Rules => rules::S2A_RULES,
        }
    }

    /// The request as sent: A2S_INFO carries a challenge only once it has
    /// one, A2S_RULES always does.
    fn write(self, challenge: Option<[u8; 4]>, out: &mut Vec<u8>) {
        out.clear();
        out.extend_from_slice(&HEADER);
        match self {
            Kind::Info => {
                out.extend_from_slice(A2S_INFO_REQUEST);
                if let Some(c) = challenge {
                    out.extend_from_slice(&c);
                }
            }
            Kind::Rules => {
                out.push(rules::A2S_RULES);
                out.extend_from_slice(&challenge.unwrap_or([0; 4]));
            }
        }
    }
}

struct Request {
    addr: SocketAddr,
    kind: Kind,
    challenge: Option<[u8; 4]>,
    timeout: Duration,
    reply: oneshot::Sender<std::result::Result<Reply, String>>,
}

/// A whole answer, from its kind byte, with the first round trip's time and
/// the challenge it was asked with.
struct Reply {
    payload: Vec<u8>,
    latency: Duration,
    challenge: Option<[u8; 4]>,
}

/// A request sent and not answered yet.
struct Pending {
    /// Which request this is, so a deadline left by an earlier request to
    /// the same address does not end this one.
    id: u64,
    kind: Kind,
    challenge: Option<[u8; 4]>,
    challenges: usize,
    sent: Instant,
    timeout: Duration,
    first_trip: Option<Duration>,
    answer: Reassembly,
    reply: oneshot::Sender<std::result::Result<Reply, String>>,
}

enum Slot {
    Unopened,
    Open(UdpSocket),
    /// The family has no socket here (no IPv6, say): why.
    Failed(String),
}

impl Slot {
    async fn readable(&self) -> io::Result<()> {
        match self {
            Slot::Open(s) => s.readable().await,
            _ => std::future::pending().await,
        }
    }
}

enum Wake {
    Readable(bool),
    Request(Option<Request>),
    Timer,
}

struct Driver {
    pace: Pace,
    rx: mpsc::UnboundedReceiver<Request>,
    /// Some handle may still send requests.
    open: bool,
    queue: VecDeque<Request>,
    /// Requests to an address with one in flight: replies are routed by
    /// address, so they go one at a time.
    parked: FxHashMap<SocketAddr, VecDeque<Request>>,
    pending: FxHashMap<SocketAddr, Pending>,
    /// Addresses whose request timed out, by the id of their quiet spell:
    /// nothing is sent there and what they send is dropped until it ends.
    quiet: FxHashMap<SocketAddr, u64>,
    /// Every pending request's deadline and every quiet spell's end, the
    /// soonest on top; an entry whose request or spell has ended is dropped
    /// when it comes up.
    deadlines: BinaryHeap<Reverse<(Instant, u64, SocketAddr)>>,
    next_id: u64,
    tokens: f64,
    refilled: Instant,
    v4: Slot,
    v6: Slot,
    buf: Vec<u8>,
    packet: Vec<u8>,
}

impl Driver {
    fn new(pace: Pace, rx: mpsc::UnboundedReceiver<Request>) -> Self {
        Self {
            pace,
            rx,
            open: true,
            queue: VecDeque::new(),
            parked: FxHashMap::default(),
            pending: FxHashMap::default(),
            quiet: FxHashMap::default(),
            deadlines: BinaryHeap::new(),
            next_id: 0,
            tokens: f64::from(pace.burst.max(1)),
            refilled: Instant::now(),
            v4: Slot::Unopened,
            v6: Slot::Unopened,
            buf: vec![0u8; 65_535],
            packet: Vec::with_capacity(64),
        }
    }

    async fn run(mut self) {
        let mut timer = pin!(sleep_until(Instant::now()));
        loop {
            self.send_due();
            if !self.open
                && self.queue.is_empty()
                && self.pending.is_empty()
                && self.parked.is_empty()
            {
                return;
            }
            let wake_at = self.next_wake();
            if let Some(at) = wake_at {
                timer.as_mut().reset(at);
            }
            let wake = tokio::select! {
                biased;
                _ = self.v4.readable() => Wake::Readable(false),
                _ = self.v6.readable() => Wake::Readable(true),
                r = self.rx.recv(), if self.open => Wake::Request(r),
                () = &mut timer, if wake_at.is_some() => Wake::Timer,
            };
            match wake {
                Wake::Readable(v6) => self.drain(v6),
                Wake::Request(Some(r)) => {
                    self.queue.push_back(r);
                    while let Ok(r) = self.rx.try_recv() {
                        self.queue.push_back(r);
                    }
                }
                Wake::Request(None) => self.open = false,
                Wake::Timer => {}
            }
            self.expire(Instant::now());
        }
    }

    /// Send what the pace allows from the queue.
    fn send_due(&mut self) {
        let now = Instant::now();
        let rate = f64::from(self.pace.rate.max(1));
        let elapsed = now.saturating_duration_since(self.refilled).as_secs_f64();
        self.tokens = (self.tokens + elapsed * rate).min(f64::from(self.pace.burst.max(1)));
        self.refilled = now;
        while self.tokens >= 1.0 && self.pending.len() < self.pace.in_flight {
            let Some(req) = self.queue.pop_front() else {
                break;
            };
            // Its caller gave up (a scan stopped): nothing to send.
            if req.reply.is_closed() {
                continue;
            }
            if self.pending.contains_key(&req.addr) || self.quiet.contains_key(&req.addr) {
                self.parked.entry(req.addr).or_default().push_back(req);
                continue;
            }
            req.kind.write(req.challenge, &mut self.packet);
            let sent = Instant::now();
            if let Err(e) = self.send_to(req.addr) {
                let _ = req.reply.send(Err(e));
                continue;
            }
            self.tokens -= 1.0;
            self.next_id += 1;
            let id = self.next_id;
            self.deadlines
                .push(Reverse((sent + req.timeout, id, req.addr)));
            self.pending.insert(
                req.addr,
                Pending {
                    id,
                    kind: req.kind,
                    challenge: req.challenge,
                    challenges: 0,
                    sent,
                    timeout: req.timeout,
                    first_trip: None,
                    answer: Reassembly::default(),
                    reply: req.reply,
                },
            );
        }
    }

    /// Send `self.packet` to `addr` on its family's socket, opened first if
    /// need be. A full send buffer drops the datagram, as the network might:
    /// the request's deadline covers it.
    fn send_to(&mut self, addr: SocketAddr) -> std::result::Result<(), String> {
        let slot = if addr.is_ipv6() {
            &mut self.v6
        } else {
            &mut self.v4
        };
        if let Slot::Unopened = slot {
            *slot = match open(addr.is_ipv6()) {
                Ok(s) => Slot::Open(s),
                Err(e) => Slot::Failed(format!("no UDP socket: {e}")),
            };
        }
        match slot {
            // Sent straight to the system: Tokio's own `try_send_to` waits for
            // the reactor to see a fresh socket writable, and would drop the
            // first requests as if the buffer were full.
            Slot::Open(s) => match SockRef::from(&*s).send_to(&self.packet, &addr.into()) {
                Ok(_) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
                Err(e) => Err(e.to_string()),
            },
            Slot::Failed(e) => Err(e.clone()),
            Slot::Unopened => Err("no UDP socket".into()),
        }
    }

    /// When the loop has something to do without being woken: the soonest
    /// deadline, or the next send the pace allows.
    fn next_wake(&mut self) -> Option<Instant> {
        while let Some(Reverse((_, id, addr))) = self.deadlines.peek() {
            if self.pending.get(addr).is_some_and(|p| p.id == *id)
                || self.quiet.get(addr) == Some(id)
            {
                break;
            }
            self.deadlines.pop();
        }
        let deadline = self.deadlines.peek().map(|Reverse((at, _, _))| *at);
        let send =
            (!self.queue.is_empty() && self.pending.len() < self.pace.in_flight).then(|| {
                let missing = (1.0 - self.tokens).max(0.0);
                self.refilled + Duration::from_secs_f64(missing / f64::from(self.pace.rate.max(1)))
            });
        match (deadline, send) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Read every datagram waiting on a socket (up to [`DRAIN_MAX`]).
    fn drain(&mut self, v6: bool) {
        let mut buf = std::mem::take(&mut self.buf);
        for _ in 0..DRAIN_MAX {
            let Slot::Open(socket) = (if v6 { &self.v6 } else { &self.v4 }) else {
                break;
            };
            match socket.try_recv_from(&mut buf) {
                Ok((n, from)) => self.datagram(from, &buf[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                // A port unreachable reported for one server (Windows, if
                // turning it off failed), or a datagram larger than the
                // buffer: the socket still serves the others.
                Err(_) => {}
            }
        }
        self.buf = buf;
    }

    /// One datagram from `from`: part of an answer, a challenge to answer,
    /// or nothing anyone waits for.
    fn datagram(&mut self, from: SocketAddr, packet: &[u8]) {
        let Some(p) = self.pending.get_mut(&from) else {
            // The late answer of a request that timed out: dropped, and
            // nothing more is expected, so the address may be asked again.
            if self.quiet.contains_key(&from) && is_answer(packet) {
                self.quiet.remove(&from);
                self.release(from);
            }
            return;
        };
        let payload = match p.answer.push(packet) {
            None => return,
            Some(Ok(payload)) => payload,
            Some(Err(e)) => return self.finish(from, Err(e)),
        };
        let trip = p.sent.elapsed();
        match payload.first() {
            Some(&S2C_CHALLENGE) if payload.len() >= 5 => {
                p.first_trip.get_or_insert(trip);
                p.challenges += 1;
                if p.challenges >= MAX_CHALLENGES {
                    return self
                        .finish(from, Err("the server keeps sending new challenges".into()));
                }
                let mut c = [0u8; 4];
                c.copy_from_slice(&payload[1..5]);
                p.challenge = Some(c);
                p.answer = Reassembly::default();
                p.kind.write(p.challenge, &mut self.packet);
                if let Err(e) = self.send_to(from) {
                    self.finish(from, Err(e));
                }
            }
            // A late answer of the other kind, to an earlier request there:
            // this one's is still to come, and its round trip not measured.
            Some(&b) if b != p.kind.answer() && (b == S2A_INFO || b == rules::S2A_RULES) => {
                p.answer = Reassembly::default();
            }
            Some(&b) if b == p.kind.answer() => {
                let latency = p.first_trip.unwrap_or(trip);
                let challenge = p.challenge;
                self.finish(
                    from,
                    Ok(Reply {
                        payload,
                        latency,
                        challenge,
                    }),
                );
            }
            Some(b) => {
                let e = format!("unexpected answer {b:#04x}");
                self.finish(from, Err(e));
            }
            None => self.finish(from, Err("empty answer".into())),
        }
    }

    /// End the request to `addr` with `result`, and let the next one to that
    /// address go first.
    fn finish(&mut self, addr: SocketAddr, result: std::result::Result<Reply, String>) {
        if let Some(p) = self.pending.remove(&addr) {
            let _ = p.reply.send(result);
        }
        self.release(addr);
    }

    /// Let the next request parked for `addr` go first.
    fn release(&mut self, addr: SocketAddr) {
        if let Some(waiting) = self.parked.get_mut(&addr) {
            if let Some(next) = waiting.pop_front() {
                self.queue.push_front(next);
            }
            if waiting.is_empty() {
                self.parked.remove(&addr);
            }
        }
    }

    /// End every request whose deadline has passed, its address quiet for
    /// half its timeout, and every quiet spell that is over.
    fn expire(&mut self, now: Instant) {
        while let Some(&Reverse((at, id, addr))) = self.deadlines.peek() {
            if at > now {
                break;
            }
            self.deadlines.pop();
            if self.pending.get(&addr).is_some_and(|p| p.id == id)
                && let Some(p) = self.pending.remove(&addr)
            {
                let _ = p.reply.send(Err("timed out".into()));
                self.next_id += 1;
                self.quiet.insert(addr, self.next_id);
                self.deadlines
                    .push(Reverse((at + p.timeout / 2, self.next_id, addr)));
            } else if self.quiet.get(&addr) == Some(&id) {
                self.quiet.remove(&addr);
                self.release(addr);
            }
        }
    }
}

/// Whether `packet` is a whole answer, single and of either kind.
fn is_answer(packet: &[u8]) -> bool {
    packet.starts_with(&HEADER) && matches!(packet.get(4), Some(&(S2A_INFO | rules::S2A_RULES)))
}

/// An unconnected UDP socket for one family, on any port, with the large
/// receive buffer, and on Windows deaf to ICMP port unreachable.
fn open(v6: bool) -> io::Result<UdpSocket> {
    let (domain, any) = if v6 {
        (Domain::IPV6, SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)))
    } else {
        (Domain::IPV4, SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
    };
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    if v6 {
        socket.set_only_v6(true)?;
    }
    // Best effort: the default still works, only with less room for a burst.
    let _ = socket.set_recv_buffer_size(RECV_BUFFER);
    socket.set_nonblocking(true)?;
    socket.bind(&any.into())?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        // Without it a reset only fails one receive, which `drain` skips.
        let _ = dz_common::win::udp_ignore_unreachable(socket.as_raw_socket());
    }
    UdpSocket::from_std(socket.into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::exchange::tests::info_payload;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// How a fake server answers.
    #[derive(Clone, Copy)]
    enum Fake {
        /// Answers A2S_INFO at once, without a challenge.
        Open,
        /// Challenges every request without the right challenge.
        Challenging,
        /// Answers the rules sent with `00000000` at once.
        RulesAtZero,
        /// Never answers.
        Silent,
    }

    const CHALLENGE: [u8; 4] = [7, 7, 7, 7];

    fn rules_payload() -> Vec<u8> {
        let mut out = vec![rules::S2A_RULES, 2, 0];
        out.extend_from_slice(b"dedicated\x001\x00platform\x00win\x00");
        out
    }

    /// A fake DayZ server on its own port; returns its address and the
    /// number of datagrams it got.
    async fn fake(kind: Fake) -> (SocketAddr, Arc<AtomicUsize>) {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();
        let got = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&got);
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            loop {
                let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                    return;
                };
                count.fetch_add(1, Ordering::SeqCst);
                let req = &buf[..n];
                let mut out = HEADER.to_vec();
                let is_info = req.get(4) == Some(&b'T');
                match kind {
                    Fake::Silent => continue,
                    Fake::Open if is_info => out.extend_from_slice(&info_payload()),
                    Fake::RulesAtZero if !is_info && req.ends_with(&[0; 4]) => {
                        out.extend_from_slice(&rules_payload());
                    }
                    _ if !req.ends_with(&CHALLENGE) => {
                        out.push(S2C_CHALLENGE);
                        out.extend_from_slice(&CHALLENGE);
                    }
                    _ if is_info => out.extend_from_slice(&info_payload()),
                    _ => out.extend_from_slice(&rules_payload()),
                }
                let _ = socket.send_to(&out, from).await;
            }
        });
        (addr, got)
    }

    /// A fake server that answers its n-th A2S_INFO, without a challenge,
    /// `delays[n]` after it came (never past the list); returns its address
    /// and the number of datagrams it got.
    async fn scripted(delays: Vec<Duration>) -> (SocketAddr, Arc<AtomicUsize>) {
        let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let addr = socket.local_addr().unwrap();
        let got = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&got);
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            loop {
                let Ok((_, from)) = socket.recv_from(&mut buf).await else {
                    return;
                };
                let n = count.fetch_add(1, Ordering::SeqCst);
                let Some(&delay) = delays.get(n) else {
                    continue;
                };
                let socket = Arc::clone(&socket);
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    let mut out = HEADER.to_vec();
                    out.extend_from_slice(&info_payload());
                    let _ = socket.send_to(&out, from).await;
                });
            }
        });
        (addr, got)
    }

    fn fast() -> Pace {
        Pace {
            rate: 10_000,
            burst: 64,
            in_flight: 500,
        }
    }

    #[tokio::test]
    async fn many_servers_answer_over_one_socket() {
        let mut servers = Vec::new();
        for i in 0..200 {
            let kind = if i % 2 == 0 {
                Fake::Challenging
            } else {
                Fake::Open
            };
            servers.push(fake(kind).await.0);
        }
        let engine = Engine::new(fast());
        let targets = futures_util::stream::iter(servers.iter().map(|a| (*a, *a)));
        let answers: Vec<_> = engine
            .infos(targets, Duration::from_secs(2))
            .collect()
            .await;
        assert_eq!(answers.len(), 200);
        for (addr, answer) in answers {
            let answer = answer.unwrap_or_else(|e| panic!("{addr}: {e}"));
            assert_eq!(answer.info.name, "Test server");
            assert!(answer.latency < Duration::from_secs(1));
            let challenged = servers.iter().position(|a| *a == addr).unwrap() % 2 == 0;
            assert_eq!(answer.challenge().is_some(), challenged);
        }
    }

    #[tokio::test]
    async fn a_challenge_is_answered_and_the_rules_reuse_it() {
        let (addr, got) = fake(Fake::Challenging).await;
        let engine = Engine::new(fast());
        let info = engine.info(addr, Duration::from_secs(2)).await.unwrap();
        assert_eq!(info.challenge(), Some(CHALLENGE));
        assert_eq!(got.load(Ordering::SeqCst), 2);
        let rules = engine
            .rules(addr, info.challenge(), Duration::from_secs(2))
            .await
            .unwrap();
        assert_eq!(rules.get("platform"), Some("win"));
        // The rules went at once with the challenge A2S_INFO got.
        assert_eq!(got.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn the_rules_start_with_a_zero_challenge_then_follow_the_servers() {
        let engine = Engine::new(fast());
        let (zero, got) = fake(Fake::RulesAtZero).await;
        let rules = engine.rules(zero, None, Duration::from_secs(2)).await;
        assert_eq!(rules.unwrap().get("dedicated"), Some("1"));
        assert_eq!(got.load(Ordering::SeqCst), 1);

        let (strict, got) = fake(Fake::Challenging).await;
        let rules = engine.rules(strict, None, Duration::from_secs(2)).await;
        assert_eq!(rules.unwrap().get("dedicated"), Some("1"));
        assert_eq!(got.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_silent_server_times_out_alone() {
        let engine = Engine::new(fast());
        let (silent, _) = fake(Fake::Silent).await;
        let (open, _) = fake(Fake::Open).await;
        let t = std::time::Instant::now();
        let (quiet, answered) = tokio::join!(
            engine.info(silent, Duration::from_millis(300)),
            engine.info(open, Duration::from_millis(300)),
        );
        assert!(quiet.is_err());
        assert!(answered.is_ok());
        let took = t.elapsed();
        assert!(
            took >= Duration::from_millis(290) && took < Duration::from_millis(600),
            "{took:?}"
        );
    }

    #[tokio::test]
    async fn a_server_that_drops_the_answer_holds_one_timeout_not_two() {
        // Answers the challenge, never the info.
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            let (_, from) = socket.recv_from(&mut buf).await.unwrap();
            let mut out = HEADER.to_vec();
            out.extend_from_slice(&[S2C_CHALLENGE, 9, 9, 9, 9]);
            socket.send_to(&out, from).await.unwrap();
            let _ = socket.recv_from(&mut buf).await;
            std::future::pending::<()>().await;
        });
        let engine = Engine::new(fast());
        let t = std::time::Instant::now();
        assert!(engine.info(addr, Duration::from_millis(300)).await.is_err());
        assert!(
            t.elapsed() < Duration::from_millis(550),
            "{:?}",
            t.elapsed()
        );
    }

    #[tokio::test]
    async fn a_closed_port_does_not_break_the_socket() {
        // A port nothing listens on: Windows reports it as a reset on the
        // next receive, Linux does not report it on an unconnected socket.
        let closed = {
            let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            s.local_addr().unwrap()
        };
        let engine = Engine::new(fast());
        assert!(
            engine
                .info(closed, Duration::from_millis(200))
                .await
                .is_err()
        );
        let (open, _) = fake(Fake::Open).await;
        let (a, b) = tokio::join!(
            engine.info(closed, Duration::from_millis(500)),
            engine.info(open, Duration::from_millis(500)),
        );
        assert!(a.is_err());
        assert_eq!(b.unwrap().info.name, "Test server");
    }

    #[tokio::test]
    async fn the_pace_holds() {
        let mut servers = Vec::new();
        for _ in 0..60 {
            servers.push(fake(Fake::Open).await.0);
        }
        let engine = Engine::new(Pace {
            rate: 100,
            burst: 5,
            in_flight: 500,
        });
        let t = std::time::Instant::now();
        let targets = futures_util::stream::iter(servers.iter().map(|a| ((), *a)));
        let answers: Vec<_> = engine
            .infos(targets, Duration::from_secs(5))
            .collect()
            .await;
        assert!(answers.iter().all(|(_, a)| a.is_ok()));
        // 5 at once, then 55 at 100 a second.
        let took = t.elapsed();
        assert!(
            took >= Duration::from_millis(520) && took < Duration::from_millis(1200),
            "{took:?}"
        );
    }

    #[tokio::test]
    async fn requests_to_one_address_take_turns() {
        let (addr, got) = fake(Fake::Challenging).await;
        let engine = Engine::new(fast());
        let (a, b) = tokio::join!(
            engine.info(addr, Duration::from_secs(2)),
            engine.rules(addr, None, Duration::from_secs(2)),
        );
        assert!(a.is_ok());
        assert_eq!(b.unwrap().get("platform"), Some("win"));
        assert_eq!(got.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn the_in_flight_ceiling_holds() {
        let mut silent = Vec::new();
        for _ in 0..6 {
            silent.push(fake(Fake::Silent).await);
        }
        let engine = Engine::new(Pace {
            rate: 10_000,
            burst: 64,
            in_flight: 2,
        });
        let t = std::time::Instant::now();
        let targets = futures_util::stream::iter(silent.iter().map(|(a, _)| ((), *a)));
        let answers: Vec<_> = targets
            .map(|(_, a)| {
                let engine = engine.clone();
                async move { engine.info(a, Duration::from_millis(200)).await }
            })
            .buffer_unordered(6)
            .collect()
            .await;
        assert!(answers.iter().all(Result::is_err));
        // Two at a time: three rounds of 200 ms.
        let took = t.elapsed();
        assert!(took >= Duration::from_millis(590), "{took:?}");
        assert_eq!(
            silent
                .iter()
                .map(|(_, n)| n.load(Ordering::SeqCst))
                .sum::<usize>(),
            6
        );
    }

    #[tokio::test]
    async fn a_request_its_caller_dropped_is_never_sent() {
        let (silent, _) = fake(Fake::Silent).await;
        let (open, got) = fake(Fake::Open).await;
        let engine = Engine::new(Pace {
            rate: 10_000,
            burst: 64,
            in_flight: 1,
        });
        // The silent server holds the one place in flight while the other
        // request waits its turn, and is given up before it comes.
        let (quiet, dropped) = tokio::join!(
            engine.info(silent, Duration::from_millis(300)),
            tokio::time::timeout(
                Duration::from_millis(50),
                engine.info(open, Duration::from_secs(1))
            ),
        );
        assert!(quiet.is_err());
        assert!(dropped.is_err());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(got.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_earlier_requests_deadline_does_not_end_the_next() {
        // The first answer comes at once, the second past the first
        // request's deadline.
        let (addr, _) = scripted(vec![Duration::ZERO, Duration::from_millis(500)]).await;
        let engine = Engine::new(fast());
        let first = engine.info(addr, Duration::from_millis(300)).await;
        assert!(first.is_ok());
        let second = engine.info(addr, Duration::from_secs(2)).await.unwrap();
        assert!(
            second.latency >= Duration::from_millis(450),
            "{:?}",
            second.latency
        );
    }

    #[tokio::test]
    async fn a_late_answer_is_not_taken_by_the_next_request() {
        // The first answer comes 250 ms late for a 200 ms timeout, while the
        // address is quiet; the second 150 ms after it is asked.
        let (addr, got) =
            scripted(vec![Duration::from_millis(250), Duration::from_millis(150)]).await;
        let engine = Engine::new(fast());
        assert!(engine.info(addr, Duration::from_millis(200)).await.is_err());
        let t = std::time::Instant::now();
        let second = engine.info(addr, Duration::from_secs(2)).await.unwrap();
        // Asked once the late answer was in (about 50 ms on), and measured
        // from then: not the late answer's 50 ms.
        assert!(
            second.latency >= Duration::from_millis(120),
            "{:?}",
            second.latency
        );
        assert!(
            t.elapsed() < Duration::from_millis(400),
            "{:?}",
            t.elapsed()
        );
        assert_eq!(got.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_late_answer_of_the_other_kind_is_ignored() {
        // Answers the rules at once and late, the info only after them.
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            let (_, from) = socket.recv_from(&mut buf).await.unwrap();
            let (_, _) = socket.recv_from(&mut buf).await.unwrap();
            let mut rules = HEADER.to_vec();
            rules.extend_from_slice(&rules_payload());
            socket.send_to(&rules, from).await.unwrap();
            let mut info = HEADER.to_vec();
            info.extend_from_slice(&info_payload());
            socket.send_to(&info, from).await.unwrap();
            std::future::pending::<()>().await;
        });
        let engine = Engine::new(fast());
        let (rules, info) = tokio::join!(
            engine.rules(addr, None, Duration::from_millis(100)),
            engine.info(addr, Duration::from_secs(2)),
        );
        assert!(rules.is_err());
        assert_eq!(info.unwrap().info.name, "Test server");
    }

    #[tokio::test]
    async fn the_window_is_what_the_pace_sends_in_one_timeout() {
        let engine = Engine::new(Pace::default());
        assert_eq!(engine.window(Duration::from_secs(1)), 250);
        assert_eq!(engine.window(Duration::from_secs(5)), 500);
        assert_eq!(engine.window(Duration::ZERO), 1);
    }
}
