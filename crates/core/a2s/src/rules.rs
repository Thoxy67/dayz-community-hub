//! A2S_RULES as DayZ answers it, read byte for byte.
//!
//! DayZ lists a few plain rules (`allowedBuild`, `dedicated`, `island`,
//! `language`, `platform`, `requiredBuild`, `requiredVersion`, `timeLeft`)
//! and, under names made of two control bytes (chunk index, chunk count), a
//! binary block in Bohemia's "Protocol 3" format that carries the mods with
//! their Workshop ids. `async-a2s` hands back only the mods' names, and its
//! Protocol 3 reader takes a DayZ server with 0 to 3 mods for an Arma 3
//! header, so the rules are fetched and read here. See `docs/a2s-dayz.md`.

use std::time::Duration;

use dz_common::{Error, Result};
use tokio::net::UdpSocket;
use tokio::time::{Instant, sleep, timeout_at};

const HEADER: [u8; 4] = [0xFF; 4];
const SPLIT: [u8; 4] = [0xFE, 0xFF, 0xFF, 0xFF];
const A2S_RULES: u8 = 0x56;
const S2C_CHALLENGE: u8 = 0x41;
const S2A_RULES: u8 = 0x45;
/// A server that keeps answering with a new challenge is not going to answer.
const MAX_CHALLENGES: usize = 3;

/// One mod a server runs, as it states it in its rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A2sMod {
    /// Steam Workshop id; `None` only if the server sent none.
    pub id: Option<u64>,
    pub name: String,
    /// Bohemia's hash of the mod's files.
    pub hash: u32,
}

/// Everything DayZ's A2S_RULES answer holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DayzRules {
    /// The plain rules, in the order the server sent them.
    pub rules: Vec<(String, String)>,
    /// The mods from the binary block, in load order.
    pub mods: Vec<A2sMod>,
    /// The names of the signing keys (`.bikey`) the server accepts.
    pub signatures: Vec<String>,
    /// A line official servers fill ("Official DayZ game server").
    pub description: Option<String>,
    /// The binary block was there but could not be read (truncated, or a
    /// layout this code does not know): `mods` is then empty, not "no mods".
    pub mods_unreadable: bool,
}

impl DayzRules {
    /// A plain rule's value.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Fetch A2S_RULES from `addr` (`"ip:port"`) and return the reassembled
/// answer, starting at its `0x45` byte. Each attempt waits at most `timeout`;
/// a silent server is asked once more after a short pause, since a single lost
/// UDP datagram (of up to a dozen for a heavily modded server) is common.
pub async fn fetch_rules(addr: &str, timeout: Duration) -> Result<Vec<u8>> {
    let mut last = Error::A2sQuery(format!("A2S rules query to {addr} was never sent"));
    for attempt in 0..2u32 {
        if attempt > 0 {
            sleep(Duration::from_millis(300)).await;
        }
        match fetch_once(addr, timeout).await {
            Ok(payload) => return Ok(payload),
            Err(e) => last = e,
        }
    }
    Err(last)
}

async fn fetch_once(addr: &str, timeout: Duration) -> Result<Vec<u8>> {
    let err = |what: String| Error::A2sQuery(format!("A2S rules query to {addr}: {what}"));
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| err(e.to_string()))?;
    socket.connect(addr).await.map_err(|e| err(e.to_string()))?;
    let deadline = Instant::now() + timeout;

    let mut challenge = [0xFF; 4];
    for _ in 0..MAX_CHALLENGES {
        let mut request = HEADER.to_vec();
        request.push(A2S_RULES);
        request.extend_from_slice(&challenge);
        socket
            .send(&request)
            .await
            .map_err(|e| err(e.to_string()))?;

        let payload = receive(&socket, deadline).await.map_err(err)?;
        match payload.first() {
            Some(&S2C_CHALLENGE) if payload.len() >= 5 => challenge.copy_from_slice(&payload[1..5]),
            Some(&S2A_RULES) => return Ok(payload),
            Some(b) => return Err(err(format!("unexpected answer {b:#04x}"))),
            None => return Err(err("empty answer".into())),
        }
    }
    Err(err("the server keeps sending new challenges".into()))
}

/// One whole answer: a single datagram, or every fragment of a split one put
/// back in order. The leading `FF FF FF FF` is stripped.
async fn receive(socket: &UdpSocket, deadline: Instant) -> std::result::Result<Vec<u8>, String> {
    let mut buf = vec![0u8; 65_535];
    let mut fragments: Vec<Option<Vec<u8>>> = Vec::new();
    let mut split_id = None;
    loop {
        let n = timeout_at(deadline, socket.recv(&mut buf))
            .await
            .map_err(|_| "timed out".to_string())?
            .map_err(|e| e.to_string())?;
        let packet = &buf[..n];
        if packet.starts_with(&HEADER) {
            return Ok(packet[4..].to_vec());
        }
        // Source split header: -2, id (i32), total (u8), number (u8), size (u16).
        if !packet.starts_with(&SPLIT) || packet.len() < 12 {
            continue;
        }
        let id = i32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]);
        if id < 0 {
            // Bit 31 marks a bzip2 body; DayZ never compresses.
            return Err("compressed answer, not supported".into());
        }
        let (total, number) = (packet[8] as usize, packet[9] as usize);
        if total == 0 || number >= total {
            continue;
        }
        if split_id != Some(id) {
            // A fragment of another (older) answer resets the collection.
            split_id = Some(id);
            fragments = vec![None; total];
        }
        if fragments.len() == total {
            fragments[number] = Some(packet[12..].to_vec());
        }
        if fragments.iter().all(Option::is_some) {
            let joined: Vec<u8> = fragments.into_iter().flatten().flatten().collect();
            return joined
                .strip_prefix(&HEADER)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "split answer without a header".to_string());
        }
    }
}

/// Read an A2S_RULES answer (from its `0x45` byte). The rule count is only a
/// hint: a truncated answer yields the rules that arrived whole.
pub fn parse_rules(payload: &[u8]) -> Result<DayzRules> {
    let bad = |what: &str| Error::A2sQuery(format!("A2S rules answer: {what}"));
    let (&kind, rest) = payload.split_first().ok_or_else(|| bad("empty"))?;
    if kind != S2A_RULES || rest.len() < 2 {
        return Err(bad("not a rules answer"));
    }
    let count = u16::from_le_bytes([rest[0], rest[1]]) as usize;
    let mut cursor = &rest[2..];

    let mut out = DayzRules::default();
    let mut chunks: Vec<(u8, &[u8])> = Vec::new();
    for _ in 0..count {
        let Some((name, after)) = cstring(cursor) else {
            break;
        };
        let Some((value, after)) = cstring(after) else {
            break;
        };
        cursor = after;
        if is_chunk_key(name) {
            chunks.push((name[0], value));
        } else {
            out.rules.push((
                String::from_utf8_lossy(name).into_owned(),
                String::from_utf8_lossy(value).into_owned(),
            ));
        }
    }

    if !chunks.is_empty() {
        chunks.sort_by_key(|&(index, _)| index);
        let whole = chunks
            .iter()
            .enumerate()
            .all(|(i, &(index, _))| index as usize == i + 1);
        let block: Vec<u8> = chunks
            .iter()
            .flat_map(|&(_, v)| v.iter().copied())
            .collect();
        match whole.then(|| read_block(&unescape(&block))).flatten() {
            Some(b) => {
                out.mods = b.mods;
                out.signatures = b.signatures;
                out.description = b.description;
            }
            None => out.mods_unreadable = true,
        }
    }
    Ok(out)
}

/// Undo the block's escaping, which keeps NULs out of the rule values:
/// `01 01` is `01`, `01 02` is `00`, `01 03` is `FF`.
fn unescape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut bytes = data.iter().copied().peekable();
    while let Some(b) = bytes.next() {
        let escaped = match (b, bytes.peek()) {
            (0x01, Some(0x01)) => Some(0x01),
            (0x01, Some(0x02)) => Some(0x00),
            (0x01, Some(0x03)) => Some(0xFF),
            _ => None,
        };
        match escaped {
            Some(e) => {
                out.push(e);
                bytes.next();
            }
            None => out.push(b),
        }
    }
    out
}

struct Block {
    mods: Vec<A2sMod>,
    signatures: Vec<String>,
    description: Option<String>,
}

/// DayZ's binary block, unescaped: a version (u32, 2), the mod count (u8),
/// each mod as hash (u32), id width (u8, low nibble) then the id, and a
/// length-prefixed name; then the signatures count (u8) and names; then a
/// length-prefixed description. Everything little-endian.
///
/// Servers cut long answers short, so a block that ends inside the
/// signatures still gives its mods; one that ends inside the mods gives none.
fn read_block(data: &[u8]) -> Option<Block> {
    let mut r = Reader(data);
    let _version = r.u32()?;
    let count = r.u8()?;
    let mut mods = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let hash = r.u32()?;
        let width = (r.u8()? & 0x0F) as usize;
        if width > 8 {
            return None;
        }
        let mut id = [0u8; 8];
        id[..width].copy_from_slice(r.take(width)?);
        let id = u64::from_le_bytes(id);
        let name = r.string()?;
        mods.push(A2sMod {
            id: (id > 0).then_some(id),
            name,
            hash,
        });
    }
    let mut signatures = Vec::new();
    if let Some(count) = r.u8() {
        for _ in 0..count {
            match r.string() {
                Some(s) => signatures.push(s),
                None => break,
            }
        }
    }
    let description = r.string().filter(|d| !d.is_empty());
    Some(Block {
        mods,
        signatures,
        description,
    })
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn string(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        self.take(n)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }
}

/// A binary chunk's name is two bytes, its index (from 1) and the number of
/// chunks. The count may be printable (32 chunks is a space), so only the
/// index is required to be a control byte.
fn is_chunk_key(name: &[u8]) -> bool {
    matches!(name, [index, count] if (1..0x20).contains(index) && index <= count)
}

fn cstring(data: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = data.iter().position(|&b| b == 0)?;
    Some((&data[..end], &data[end + 1..]))
}

/// Fetch and read a DayZ server's rules.
pub async fn query_dayz_rules(addr: &str, timeout: Duration) -> Result<DayzRules> {
    parse_rules(&fetch_rules(addr, timeout).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn fixtures() -> Vec<(String, Vec<u8>)> {
        let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
        let mut all: Vec<_> = std::fs::read_dir(dir)
            .expect("fixtures directory")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("rules-") && n.ends_with(".bin"))
            .collect();
        all.sort();
        all.into_iter().map(|n| (n.clone(), fixture(&n))).collect()
    }

    #[test]
    fn every_captured_server_reads_whole() {
        let all = fixtures();
        assert!(!all.is_empty());
        for (name, bytes) in all {
            let r = parse_rules(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!r.mods_unreadable, "{name}: binary block unreadable");
            assert!(r.get("dedicated").is_some(), "{name}: no plain rules");
            for m in &r.mods {
                assert!(
                    m.id.is_some_and(|id| id > 0),
                    "{name}: {m:?} has no workshop id"
                );
                assert!(!m.name.is_empty(), "{name}: {m:?} has no name");
            }
        }
    }

    #[test]
    fn a_modded_server_lists_its_mods_with_ids() {
        let with_mods = fixtures()
            .into_iter()
            .map(|(n, b)| (n, parse_rules(&b).expect("parses")))
            .filter(|(_, r)| !r.mods.is_empty())
            .count();
        assert!(with_mods > 0, "no fixture has mods");
    }

    #[test]
    fn what_each_captured_server_said() {
        // An official server: no mods (which async-a2s misreads), a description.
        let official = parse_rules(&fixture("rules-official.bin")).expect("parses");
        assert!(official.mods.is_empty() && !official.mods_unreadable);
        assert_eq!(
            official.description.as_deref(),
            Some("Official DayZ game server")
        );
        assert_eq!(official.get("platform"), Some("win"));

        // Two mods, the case async-a2s takes for an Arma 3 header.
        let two = parse_rules(&fixture("rules-modded2.bin")).expect("parses");
        assert_eq!(two.mods.len(), 2);
        assert_eq!(two.mods[1].id, Some(1_559_212_036));
        assert_eq!(two.mods[1].name, "Community Framework");
        assert!(two.signatures.iter().any(|s| s == "dayz"));

        // 101 mods over 32 chunks, the last of which the server never sent:
        // the mods are whole, the signatures cut short.
        let many = parse_rules(&fixture("rules-modded101.bin")).expect("parses");
        assert_eq!(many.mods.len(), 101);
        assert_eq!(many.mods[0].name, "SQUAT Deployable Houses");
    }

    #[test]
    fn chunks_are_put_back_in_index_order() {
        // Two chunks sent in reverse order: the block must still start with
        // the signature. Built by hand: a version-2 block holding no mods.
        let mut payload = vec![S2A_RULES, 3, 0];
        payload.extend_from_slice(b"dedicated\x001\x00");
        payload.extend_from_slice(&[2, 2, 0]);
        payload.extend_from_slice(&[0x01, 0x02, 0x00]); // 00 (escaped): no mods
        payload.extend_from_slice(&[1, 2, 0]);
        payload.extend_from_slice(&[0x02, 0x01, 0x02, 0x01, 0x02, 0x01, 0x02, 0x00]);
        let r = parse_rules(&payload).expect("parses");
        assert_eq!(r.get("dedicated"), Some("1"));
        assert!(!r.mods_unreadable);
        assert!(r.mods.is_empty());
    }

    #[test]
    fn a_truncated_answer_keeps_what_arrived() {
        let mut payload = vec![S2A_RULES, 5, 0];
        payload.extend_from_slice(b"island\x00chernarusplus\x00platform\x00wi");
        let r = parse_rules(&payload).expect("parses");
        assert_eq!(r.rules, vec![("island".into(), "chernarusplus".into())]);
    }

    #[test]
    fn not_a_rules_answer() {
        assert!(parse_rules(&[]).is_err());
        assert!(parse_rules(&[0x49, 0, 0]).is_err());
    }

    /// Captures the fixtures once from a few public servers.
    /// Run with: cargo test -p dz-a2s capture -- --ignored --nocapture
    /// Pass the servers as `DZ_A2S_CAPTURE="name=ip:port name=ip:port"`.
    #[tokio::test]
    #[ignore = "queries public servers"]
    async fn capture() {
        let list = std::env::var("DZ_A2S_CAPTURE").expect("DZ_A2S_CAPTURE");
        let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
        std::fs::create_dir_all(&dir).expect("fixtures directory");
        for entry in list.split_whitespace() {
            let (name, addr) = entry.split_once('=').expect("name=ip:port");
            if let Ok(info) = crate::query_info(addr).await {
                let keywords = info.extended_server_info.keywords.unwrap_or_default();
                println!(
                    "{name}: keywords {keywords:?} -> {:?}",
                    crate::DayzInfo::parse(&keywords)
                );
            }
            match fetch_rules(addr, Duration::from_secs(4)).await {
                Ok(bytes) => {
                    let r = parse_rules(&bytes);
                    println!("{name}: {} bytes, {r:?}", bytes.len());
                    std::fs::write(format!("{dir}/rules-{name}.bin"), bytes).expect("write");
                }
                Err(e) => println!("{name}: {e}"),
            }
        }
    }
}
