//! Discord's IPC frames: an opcode and a length, both `u32` little-endian, then
//! that many bytes of JSON.

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The first frame: the protocol's version and the application.
pub(crate) const OP_HANDSHAKE: u32 = 0;
/// A command, or Discord's answer to one (and its events).
pub(crate) const OP_FRAME: u32 = 1;
/// Discord closes the connection (a bad client id, a bad version).
pub(crate) const OP_CLOSE: u32 = 2;
/// Discord asks whether the client is still there.
pub(crate) const OP_PING: u32 = 3;
/// The answer to a ping, with the ping's payload.
pub(crate) const OP_PONG: u32 = 4;

/// The largest frame read: Discord's answers are a few kilobytes; anything
/// bigger is not Discord and ends the connection.
pub(crate) const MAX_FRAME: usize = 256 * 1024;

/// One frame, ready to write.
pub(crate) fn encode(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let len = u32::try_from(body.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// A frame's header: its opcode and its length, refused above [`MAX_FRAME`].
pub(crate) fn header(h: [u8; 8]) -> Result<(u32, usize), String> {
    let [a, b, c, d, e, f, g, i] = h;
    let op = u32::from_le_bytes([a, b, c, d]);
    let len = usize::try_from(u32::from_le_bytes([e, f, g, i])).unwrap_or(usize::MAX);
    if len > MAX_FRAME {
        return Err(format!("Discord sent a frame of {len} bytes"));
    }
    Ok((op, len))
}

/// The first whole frame at the start of `buf`, with the bytes it took, or
/// `None` while it is not all there (the tests' reader: the task reads with [`read`]).
#[cfg(test)]
pub(crate) fn decode(buf: &[u8]) -> Result<Option<(u32, Value, usize)>, String> {
    let Some(h) = buf.get(..8).and_then(|h| <[u8; 8]>::try_from(h).ok()) else {
        return Ok(None);
    };
    let (op, len) = header(h)?;
    let Some(body) = buf.get(8..8 + len) else {
        return Ok(None);
    };
    let value = body_value(body)?;
    Ok(Some((op, value, 8 + len)))
}

fn body_value(body: &[u8]) -> Result<Value, String> {
    if body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(body).map_err(|e| format!("Discord sent bad JSON: {e}"))
}

/// Read one frame.
pub(crate) async fn read<R: AsyncRead + Unpin>(r: &mut R) -> Result<(u32, Value), String> {
    let mut h = [0u8; 8];
    r.read_exact(&mut h)
        .await
        .map_err(|e| format!("Discord: {e}"))?;
    let (op, len) = header(h)?;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)
        .await
        .map_err(|e| format!("Discord: {e}"))?;
    Ok((op, body_value(&body)?))
}

/// Write one frame.
pub(crate) async fn write<W: AsyncWrite + Unpin>(
    w: &mut W,
    op: u32,
    payload: &Value,
) -> Result<(), String> {
    w.write_all(&encode(op, payload))
        .await
        .map_err(|e| format!("Discord: {e}"))?;
    w.flush().await.map_err(|e| format!("Discord: {e}"))
}

/// The handshake for the application `client_id`.
pub(crate) fn handshake(client_id: &str) -> Value {
    json!({ "v": 1, "client_id": client_id })
}

/// Whether Discord's answer to the handshake says it is ready.
pub(crate) fn is_ready(op: u32, v: &Value) -> bool {
    op == OP_FRAME && v.get("evt").and_then(Value::as_str) == Some("READY")
}

/// Whether a frame from Discord says a command was refused (`evt: ERROR`).
pub(crate) fn is_error(v: &Value) -> bool {
    v.get("evt").and_then(Value::as_str) == Some("ERROR")
}

/// Whether a frame from Discord accepts a `SET_ACTIVITY` (its answer, no error).
pub(crate) fn is_activity_set(v: &Value) -> bool {
    v.get("cmd").and_then(Value::as_str) == Some("SET_ACTIVITY") && !is_error(v)
}

/// `SET_ACTIVITY` for this process: the activity, or none to clear it.
pub(crate) fn set_activity(pid: u32, activity: Option<Value>, nonce: u64) -> Value {
    let mut args = json!({ "pid": pid });
    if let (Some(a), Some(o)) = (activity, args.as_object_mut()) {
        o.insert("activity".into(), a);
    }
    json!({ "cmd": "SET_ACTIVITY", "args": args, "nonce": nonce.to_string() })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_opcode_length_then_json() {
        let f = encode(OP_HANDSHAKE, &handshake("123"));
        assert_eq!(&f[..4], &0u32.to_le_bytes());
        let body = br#"{"client_id":"123","v":1}"#;
        assert_eq!(&f[4..8], &(body.len() as u32).to_le_bytes());
        let v: Value = serde_json::from_slice(&f[8..]).unwrap();
        assert_eq!(v, json!({ "v": 1, "client_id": "123" }));
    }

    #[test]
    fn decoding_waits_for_the_whole_frame_and_reports_what_it_took() {
        let mut buf = encode(OP_PING, &json!({ "n": 1 }));
        let whole = buf.len();
        buf.extend(encode(OP_FRAME, &json!({ "evt": "READY" })));
        assert_eq!(decode(&buf[..5]).unwrap(), None);
        assert_eq!(decode(&buf[..whole - 1]).unwrap(), None);
        let (op, v, used) = decode(&buf).unwrap().unwrap();
        assert_eq!((op, used), (OP_PING, whole));
        assert_eq!(v, json!({ "n": 1 }));
        let (op, v, _) = decode(&buf[used..]).unwrap().unwrap();
        assert!(is_ready(op, &v));
    }

    #[test]
    fn hostile_frames_are_refused() {
        let mut h = Vec::new();
        h.extend_from_slice(&1u32.to_le_bytes());
        h.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&h).is_err(), "a length past the limit");
        let mut bad = Vec::new();
        bad.extend_from_slice(&1u32.to_le_bytes());
        bad.extend_from_slice(&3u32.to_le_bytes());
        bad.extend_from_slice(b"{{{");
        assert!(decode(&bad).is_err(), "not JSON");
        let empty = encode(OP_PONG, &Value::Null);
        // `null` is 4 bytes; an empty body reads as null too.
        assert_eq!(decode(&empty).unwrap().unwrap().1, Value::Null);
        let mut none = Vec::new();
        none.extend_from_slice(&OP_CLOSE.to_le_bytes());
        none.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode(&none).unwrap().unwrap(), (OP_CLOSE, Value::Null, 8));
    }

    #[test]
    fn set_activity_carries_the_pid_the_nonce_and_clears_without_activity() {
        let set = set_activity(42, Some(json!({ "details": "x" })), 7);
        assert_eq!(set["cmd"], "SET_ACTIVITY");
        assert_eq!(set["args"]["pid"], 42);
        assert_eq!(set["args"]["activity"]["details"], "x");
        assert_eq!(set["nonce"], "7");
        let clear = set_activity(42, None, 8);
        assert!(clear["args"].get("activity").is_none());
        assert!(!is_ready(OP_FRAME, &json!({ "evt": "ERROR" })));
        assert!(!is_ready(OP_CLOSE, &json!({ "evt": "READY" })));
        let refused = json!({ "cmd": "SET_ACTIVITY", "evt": "ERROR", "data": { "code": 4000 } });
        assert!(is_error(&refused) && !is_activity_set(&refused));
        let taken = json!({ "cmd": "SET_ACTIVITY", "evt": null, "data": {} });
        assert!(!is_error(&taken) && is_activity_set(&taken));
    }

    #[tokio::test]
    async fn frames_round_trip_through_a_stream() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write(&mut a, OP_FRAME, &json!({ "evt": "READY", "data": {} }))
            .await
            .unwrap();
        let (op, v) = read(&mut b).await.unwrap();
        assert!(is_ready(op, &v));
        drop(a);
        assert!(read(&mut b).await.is_err(), "a closed stream ends reading");
    }
}
