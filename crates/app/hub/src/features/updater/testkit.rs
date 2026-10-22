//! Test helpers: a throwaway minisign key that signs the way the release
//! tooling does (`tauri signer sign`: a prehashed `ED` signature, the trusted
//! comment `timestamp:…\tfile:…`, the whole `.sig` text base64 encoded), and
//! a local HTTP server standing in for the release server. Never the
//! owner's key, never the real server.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base64::Engine;
use blake2::Digest;
use ed25519_dalek::{Signer, SigningKey};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// A minisign key made for one test.
pub(crate) struct TestKey {
    sk: SigningKey,
    id: [u8; 8],
}

impl TestKey {
    pub(crate) fn new(seed: u8) -> Self {
        Self {
            sk: SigningKey::from_bytes(&[seed; 32]),
            id: [seed, 1, 2, 3, 4, 5, 6, 7],
        }
    }

    /// The public key as `tauri.conf.json` holds it: base64 of the two-line
    /// minisign public key file.
    pub(crate) fn pubkey(&self) -> String {
        let mut raw = b"Ed".to_vec();
        raw.extend_from_slice(&self.id);
        raw.extend_from_slice(self.sk.verifying_key().as_bytes());
        let mut id = self.id;
        id.reverse();
        let hex: String = id.iter().map(|b| format!("{b:02X}")).collect();
        let text = format!(
            "untrusted comment: minisign public key: {hex}\n{}\n",
            B64.encode(raw)
        );
        B64.encode(text)
    }

    /// The `.sig` file's text for `data`, base64 encoded as latest.json
    /// carries it. `prehashed` is minisign's default (`ED`, what the Tauri CLI
    /// writes); otherwise the legacy `Ed` form.
    pub(crate) fn sign(&self, data: &[u8], prehashed: bool, trusted: &str) -> String {
        let (alg, sig) = if prehashed {
            let hash = blake2::Blake2b512::digest(data);
            (b"ED", self.sk.sign(&hash))
        } else {
            (b"Ed", self.sk.sign(data))
        };
        let sig = sig.to_bytes();
        let mut line = alg.to_vec();
        line.extend_from_slice(&self.id);
        line.extend_from_slice(&sig);
        let mut global = sig.to_vec();
        global.extend_from_slice(trusted.as_bytes());
        let global = self.sk.sign(&global).to_bytes();
        let text = format!(
            "untrusted comment: signature from tauri secret key\n{}\ntrusted comment: {trusted}\n{}\n",
            B64.encode(line),
            B64.encode(global)
        );
        B64.encode(text)
    }
}

/// The trusted comment `tauri signer sign` writes.
pub(crate) const TAURI_COMMENT: &str = "timestamp:1760318032\tfile:dayz-community-hub.AppImage";

/// Serve `routes` (path, body) on 127.0.0.1 until the test ends; the base
/// URL (`http://127.0.0.1:port`). Anything else is a 404.
pub(crate) async fn serve(routes: Vec<(String, Vec<u8>)>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let routes = std::sync::Arc::new(routes);
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let routes = routes.clone();
            tokio::spawn(async move {
                let mut req = Vec::new();
                let mut buf = [0u8; 1024];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&req);
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let found = routes.iter().find(|(p, _)| *p == path);
                let (status, body): (&str, &[u8]) = match found {
                    Some((_, b)) => ("200 OK", b),
                    None => ("404 Not Found", b"not found"),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes()).await;
                // In pieces, so the download reports progress more than once.
                for chunk in body.chunks(64 * 1024) {
                    if sock.write_all(chunk).await.is_err() {
                        return;
                    }
                }
                let _ = sock.shutdown().await;
            });
        }
    });
    format!("http://{addr}")
}
