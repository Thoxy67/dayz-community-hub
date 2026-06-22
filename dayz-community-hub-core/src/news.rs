use crate::Result;
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

/// TTL for the news cache in seconds (1 hour, matching the bash script's default).
pub const NEWS_CACHE_TTL_SECS: u64 = 3600;

/// A single DayZ news article.
#[derive(Debug, Clone, Deserialize)]
pub struct Article {
    pub id: u64,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content: Option<String>,
    pub version: Option<String>,
    /// Hero image filename (e.g. "0OGKO3k.jpeg") served from /app-static/uploads/article/{id}/
    pub image: Option<String>,
    /// ISO 8601 publish date from the API, e.g. "2026-02-05T00:30:00.000Z"
    pub published_at: Option<String>,
    #[serde(rename = "ArticleCategory")]
    pub category: Option<ArticleCategory>,
    #[serde(rename = "Author")]
    pub author: Option<ArticleAuthor>,
}

// Regexes for HTML stripping — compiled once.
static RE_BLOCK: OnceLock<Regex> = OnceLock::new();
static RE_TAG: OnceLock<Regex> = OnceLock::new();
static RE_ENTITY: OnceLock<Regex> = OnceLock::new();
static RE_BLANK: OnceLock<Regex> = OnceLock::new();

impl Article {
    /// Return the raw HTML content with `<app-picture>` custom elements replaced
    /// by standard `<img>` tags pointing to dayz.com's CDN at 640 px webp.
    pub fn content_html(&self) -> String {
        let html = match self.content.as_deref() {
            Some(h) if !h.is_empty() => h,
            _ => return String::new(),
        };
        // Match the full opening tag so we can extract all attributes.
        static RE_PIC: OnceLock<Regex> = OnceLock::new();
        static RE_CODE: OnceLock<Regex> = OnceLock::new();
        static RE_SIZES: OnceLock<Regex> = OnceLock::new();
        static RE_FMTS: OnceLock<Regex> = OnceLock::new();
        let re = RE_PIC.get_or_init(|| {
            Regex::new(r#"<app-picture[^>]*\bcode="[^"]+"[^>]*>\s*</app-picture>"#).unwrap()
        });
        let re_code = RE_CODE.get_or_init(|| Regex::new(r#"\bcode="([^"]+)""#).unwrap());
        let re_sizes = RE_SIZES.get_or_init(|| Regex::new(r#"\bthumb-sizes="([^"]+)""#).unwrap());
        let re_fmts = RE_FMTS.get_or_init(|| Regex::new(r#"\bthumb-formats="([^"]+)""#).unwrap());

        re.replace_all(html, |caps: &regex::Captures| {
            let tag = caps.get(0).map_or("", |m| m.as_str());

            let code = match re_code.captures(tag).and_then(|c| c.get(1)) {
                Some(m) => m.as_str(),
                None => return String::new(),
            };

            // Parse available sizes (comma-separated, descending) — pick largest for
            // full-size and the closest to 640 for the inline display version.
            let sizes_str = re_sizes.captures(tag)
                .and_then(|c| c.get(1))
                .map_or("640", |m| m.as_str());
            let mut sizes: Vec<u32> = sizes_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            sizes.sort_unstable();

            let display_size = sizes.iter().find(|&&s| s >= 640).copied()
                .or_else(|| sizes.last().copied())
                .unwrap_or(640);

            // Prefer webp for inline display (smaller); use original format
            // (first listed, typically png/jpeg) without size suffix for the
            // full-resolution lightbox version.
            let fmts_str = re_fmts.captures(tag)
                .and_then(|c| c.get(1))
                .map_or("webp", |m| m.as_str());
            let display_ext = if fmts_str.split(',').any(|f| f.trim() == "webp") {
                "webp"
            } else {
                fmts_str.split(',').next().unwrap_or("webp").trim()
            };
            // Original format is the first listed (e.g. "png" in "png,webp").
            let original_ext = fmts_str.split(',').next().unwrap_or("png").trim();

            format!(
                r#"<img src="https://dayz.com/app-static/uploads/{}_{}.{}" data-full="https://dayz.com/app-static/uploads/{}.{}" alt="" loading="lazy" />"#,
                code, display_size, display_ext, code, original_ext
            )
        })
        .into_owned()
    }

    /// Strip HTML tags and decode common entities, returning plain text
    /// suitable for display in the TUI detail panel.
    pub fn html_to_text(&self) -> String {
        let html = match self.content.as_deref() {
            Some(h) if !h.is_empty() => h,
            _ => return String::new(),
        };

        // Replace block-level tags with newlines so paragraphs are preserved.
        let block_re = RE_BLOCK.get_or_init(|| {
            Regex::new(r"(?i)</?(p|br|div|h[1-6]|li|tr|blockquote)[^>]*>").unwrap()
        });
        let text = block_re.replace_all(html, "\n");

        // Drop all remaining tags (inline, custom, etc.)
        let tag_re = RE_TAG.get_or_init(|| Regex::new(r"<[^>]+>").unwrap());
        let text = tag_re.replace_all(&text, "");

        // Decode common HTML entities.
        let entity_re = RE_ENTITY
            .get_or_init(|| Regex::new(r"&(amp|lt|gt|quot|apos|nbsp|ndash|mdash|#\d+);").unwrap());
        let text = entity_re.replace_all(&text, |caps: &regex::Captures| {
            match &caps[1] {
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                "apos" => "'",
                "nbsp" => " ",
                "ndash" => "–",
                "mdash" => "—",
                s if s.starts_with('#') => {
                    // numeric entity — just drop it for now
                    ""
                }
                _ => "",
            }
            .to_string()
        });

        // Collapse runs of blank lines to a single blank line.
        let blank_re = RE_BLANK.get_or_init(|| Regex::new(r"\n{3,}").unwrap());
        let text = blank_re.replace_all(&text, "\n\n");

        text.trim().to_string()
    }

    /// Returns the date portion of `published_at` as "YYYY-MM-DD", or empty string.
    pub fn date(&self) -> &str {
        self.published_at
            .as_deref()
            .and_then(|s| s.get(..10))
            .unwrap_or("")
    }

    /// Build the full URL to the article on dayz.com.
    /// Spaces in slugs (malformed API data) are percent-encoded.
    pub fn url(&self) -> String {
        let cat = self
            .category
            .as_ref()
            .map(|c| c.slug.as_str())
            .unwrap_or("news");
        let slug = self.slug.replace(' ', "%20");
        format!("https://dayz.com/article/{}/{}", cat, slug)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArticleCategory {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArticleAuthor {
    pub name: String,
    pub role: Option<String>,
}

/// Response envelope from `https://dayz.com/api/article`.
#[derive(Debug, Deserialize)]
pub struct NewsResponse {
    pub rows: Vec<Article>,
}

/// The dayz.com news API endpoint (without query string).
pub const NEWS_API_URL: &str = "https://dayz.com/api/article";

/// Parse the news API response body into a list of articles.
/// Shared by the direct (`reqwest`) fetch and the WebView fallback, which both
/// receive the same JSON payload from `…/api/article`.
pub fn parse_news_json(body: &str) -> Result<Vec<Article>> {
    let resp: NewsResponse = serde_json::from_str(body)?;
    Ok(resp.rows)
}

/// Heuristic: does this response body look like a Cloudflare bot-protection
/// challenge page rather than the JSON API payload? dayz.com now serves the
/// "Just a moment…" managed challenge to non-browser clients.
pub fn looks_like_cloudflare_challenge(body: &str) -> bool {
    // `get` returns None on a non-char-boundary index, so this never panics on
    // a multi-byte body — unlike slicing with `&body[..n]`.
    let head = body.get(..2048).unwrap_or(body);
    head.contains("Just a moment")
        || head.contains("challenges.cloudflare.com")
        || head.contains("cf-browser-verification")
        || head.contains("cf_chl_")
}

const NEWS_HOST: &str = "dayz.com";
/// Browser User-Agent presented to dayz.com. Must look like a real browser for
/// Cloudflare to serve the API instead of a challenge.
const NEWS_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0";

/// Issue the news request over a raw, browser-shaped HTTPS/1.1 connection and
/// return `(status, body)`.
///
/// Why not `reqwest`? dayz.com's Cloudflare bot filter rejects this endpoint
/// unless the request looks like a real browser at THREE layers, none of which
/// reqwest produces:
///
/// 1. **TLS fingerprint** - Cloudflare reads the ClientHello (JA3). reqwest's
///    `native-tls` (OpenSSL) is flagged; so is rustls' `ring` provider (verified
///    403). rustls + the `aws-lc-rs` provider (BoringSSL, like Chrome) clears it.
///    We also advertise an `http/1.1` ALPN, without which the request is
///    challenged even with the right cipher suites.
/// 2. **Header casing** — Cloudflare flags lowercase HTTP/1.1 header names, which
///    hyper/reqwest emit by default (`accept`, `user-agent`, …). The `http` crate
///    forces `HeaderName` lowercase, so we use hyper's low-level client with
///    `title_case_headers(true)` to write `Accept`, `User-Agent`, ….
/// 3. **Navigation headers** — `Accept: text/html` + `Sec-Fetch-Mode: navigate`
///    make the request look like a top-level page load rather than an XHR/API
///    call (which is what gets challenged).
async fn fetch_news_raw() -> Result<(u16, String)> {
    use bytes::Bytes;
    use http_body_util::{BodyExt, Empty};
    use hyper_util::rt::TokioIo;
    use std::sync::Arc;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore, pki_types::ServerName};

    let err = |ctx: &str, e: String| crate::errors::Error::Other(format!("news fetch {ctx}: {e}"));

    // rustls with the aws-lc-rs (BoringSSL) provider: its ClientHello matches a
    // Chrome-like fingerprint that Cloudflare accepts (the `ring` provider is
    // reliably challenged). Pin the provider explicitly so we never depend on a
    // process-wide default that other crates (tauri, reqwest) might leave unset.
    let roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let mut config = ClientConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| err("tls config", e.to_string()))?
    .with_root_certificates(roots)
    .with_no_client_auth();
    // ALPN is part of the fingerprint Cloudflare checks: must advertise http/1.1.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));

    let tcp = tokio::net::TcpStream::connect((NEWS_HOST, 443))
        .await
        .map_err(|e| err("connect", e.to_string()))?;
    let dnsname = ServerName::try_from(NEWS_HOST)
        .map_err(|e| err("server name", e.to_string()))?
        .to_owned();
    let stream = connector
        .connect(dnsname, tcp)
        .await
        .map_err(|e| err("tls handshake", e.to_string()))?;

    let (mut sender, conn) = hyper::client::conn::http1::Builder::new()
        .title_case_headers(true)
        .handshake(TokioIo::new(stream))
        .await
        .map_err(|e| err("http handshake", e.to_string()))?;
    // Drive the connection; it completes once the (Connection: close) response
    // is fully read.
    tokio::spawn(async move {
        let _ = conn.await;
    });

    // Header order/casing here mirror a Firefox top-level navigation. The two
    // that actually defeat the challenge are `Accept: text/html` and
    // `Sec-Fetch-Mode: navigate`; the rest round out the browser fingerprint.
    let req = hyper::Request::builder()
        .uri("/api/article?rowsPerPage=50")
        .header("Host", NEWS_HOST)
        .header("User-Agent", NEWS_USER_AGENT)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Upgrade-Insecure-Requests", "1")
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Site", "none")
        .header("Sec-Fetch-User", "?1")
        .header("Connection", "close")
        .body(Empty::<Bytes>::new())
        .map_err(|e| err("build request", e.to_string()))?;

    let resp = sender
        .send_request(req)
        .await
        .map_err(|e| err("send", e.to_string()))?;
    let status = resp.status().as_u16();
    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| err("read body", e.to_string()))?
        .to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

/// Fetch the latest DayZ news articles.
///
/// Performs a single browser-shaped HTTPS request (see [`fetch_news_raw`]) and
/// parses the JSON payload. If Cloudflare ever changes the rules and serves a
/// challenge page instead, we surface [`Error::CloudflareChallenge`] so the
/// caller can fall back to a real WebView.
pub async fn fetch_news() -> Result<Vec<Article>> {
    let (status, body) = fetch_news_raw().await?;
    match parse_news_json(&body) {
        Ok(rows) => Ok(rows),
        Err(e) => {
            if status == 403 || status == 503 || looks_like_cloudflare_challenge(&body) {
                Err(crate::errors::Error::CloudflareChallenge)
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live check that the navigation-header trick still bypasses Cloudflare and
    /// returns parseable JSON. Network-dependent, so ignored by default. Run with:
    /// cargo test -p dayz-community-hub-core -- news::tests::test_live_fetch --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn test_live_fetch() {
        match fetch_news().await {
            Ok(rows) => {
                println!("fetched {} articles", rows.len());
                assert!(!rows.is_empty(), "expected at least one article");
            }
            Err(e) => panic!("fetch_news failed: {e:?}"),
        }
    }
}
