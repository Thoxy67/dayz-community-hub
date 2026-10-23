//! The dayz.com news: the articles, and each one's body and patch notes.
//!
//! dayz.com is a React Router site. Its pages' data is served at the same address with
//! `.data` added, as a turbo-stream ([`turbo`]): `/news.data?page=N` lists ten articles
//! a page (no bodies), `/article/<category>/<slug>.data` holds one article: its body as
//! HTML and, for an update, the patch notes of each platform as tiptap documents
//! ([`tiptap`]). Every request goes through one browser-shaped HTTPS exchange that
//! Cloudflare lets through ([`get`]).

pub mod tiptap;
pub mod turbo;

use dz_common::{Error, Result};
use serde_json::Value;

/// How many pages of ten articles the list reads.
pub const PAGES: u32 = 5;

/// One article of the list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Article {
    pub id: String,
    pub slug: String,
    pub title: String,
    /// The lead (`intro_text`, else the description dayz.com gives search engines).
    pub excerpt: Option<String>,
    pub image_url: Option<String>,
    /// ISO 8601, e.g. "2026-10-15T10:00:00.000Z".
    pub published_at: Option<String>,
    pub category: Option<Category>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Category {
    /// In the address (`game-update`, `Dev-blogs`).
    pub code: String,
    /// As shown (`Game Update`).
    pub title: String,
}

/// An article's body.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArticleBody {
    pub html: String,
    /// An update's patch notes, one per platform, as HTML.
    pub notes: Vec<PatchNotes>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PatchNotes {
    /// `Steam`, `Xbox/Microsoft Store`, `PlayStation`.
    pub platform: String,
    pub html: String,
}

impl Article {
    /// Returns the date portion of `published_at` as "YYYY-MM-DD", or empty string.
    pub fn date(&self) -> &str {
        self.published_at
            .as_deref()
            .and_then(|s| s.get(..10))
            .unwrap_or("")
    }

    /// The article on dayz.com.
    pub fn url(&self) -> String {
        let cat = self.category.as_ref().map_or("news", |c| c.code.as_str());
        format!("https://dayz.com{}", article_path(cat, &self.slug))
    }
}

/// `/article/<category>/<slug>`, each part percent-encoded.
fn article_path(category: &str, slug: &str) -> String {
    format!("/article/{}/{}", encode(category), encode(slug))
}

fn encode(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for b in part.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A route's loader data in a decoded `.data` answer.
fn route<'a>(data: &'a Value, id: &str) -> Option<&'a Value> {
    data.get(id)?.get("data")
}

fn article(v: &Value) -> Option<Article> {
    Some(Article {
        id: string(v, "id")?,
        slug: string(v, "slug")?,
        title: string(v, "title")?,
        excerpt: string(v, "intro_text").or_else(|| string(v, "seoDescription")),
        image_url: string(v, "preview_image_url").or_else(|| string(v, "image")),
        published_at: string(v, "published_at"),
        category: v.get("category").and_then(|c| {
            Some(Category {
                code: string(c, "code")?,
                title: string(c, "title").or_else(|| string(c, "code"))?,
            })
        }),
    })
}

/// The articles of one `/news.data` page.
pub fn parse_news_page(stream: &str) -> Result<Vec<Article>> {
    let data = turbo::decode(stream).ok_or_else(|| Error::Other("not a dayz.com page".into()))?;
    let list = route(&data, "routes/news/route")
        .and_then(|r| r.get("articles"))
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Other("dayz.com's news page has no articles".into()))?;
    Ok(list.iter().filter_map(article).collect())
}

/// One article's body, from its `.data` answer.
pub fn parse_article(stream: &str) -> Result<ArticleBody> {
    let data = turbo::decode(stream).ok_or_else(|| Error::Other("not a dayz.com page".into()))?;
    let r = route(&data, "routes/article/$category/$slug/route")
        .ok_or_else(|| Error::Other("dayz.com has no such article".into()))?;
    let html = string(r, "bodyHtml").unwrap_or_default();
    let notes = r
        .get("article")
        .and_then(|a| a.get("changelogs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let platform = c
                .get("platforms")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|p| string(p, "title"))
                .collect::<Vec<_>>()
                .join(" · ");
            let doc: Value = serde_json::from_str(c.get("content")?.as_str()?).ok()?;
            let html = tiptap::to_html(&doc);
            (!html.is_empty()).then_some(PatchNotes { platform, html })
        })
        .collect();
    Ok(ArticleBody { html, notes })
}

/// Heuristic: does this response body look like a Cloudflare bot-protection
/// challenge page rather than the page's data? dayz.com serves the
/// "Just a moment…" managed challenge to clients it does not trust.
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
/// Cloudflare to serve the data instead of a challenge.
const NEWS_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0";

/// The TLS set-up every news request shares, built once: the roots copied and
/// the provider made for the first request only (5 list pages and the bodies
/// each used to build their own).
///
/// rustls with the aws-lc-rs (BoringSSL) provider: its ClientHello matches a
/// Chrome-like fingerprint that Cloudflare accepts (the `ring` provider is
/// reliably challenged). The provider is pinned explicitly so nothing depends on
/// a process-wide default that other crates (reqwest) might leave unset.
///
/// Sessions are not resumed ([`NoResumption`]): a resumed ClientHello carries a
/// pre-shared key and a remembered key share, which changes the fingerprint
/// Cloudflare reads; every connection says what a first one says.
fn tls_config() -> Result<std::sync::Arc<tokio_rustls::rustls::ClientConfig>> {
    use std::sync::{Arc, OnceLock};
    use tokio_rustls::rustls::client::Resumption;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};
    static CONFIG: OnceLock<std::result::Result<Arc<ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            };
            let mut config = ClientConfig::builder_with_provider(Arc::new(
                tokio_rustls::rustls::crypto::aws_lc_rs::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_root_certificates(roots)
            .with_no_client_auth();
            // ALPN is part of the fingerprint Cloudflare checks: must advertise http/1.1.
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            config.resumption = Resumption::store(Arc::new(NoResumption));
            Ok(Arc::new(config))
        })
        .clone()
        .map_err(|e| Error::Other(format!("news fetch tls config: {e}")))
}

/// A session store that keeps nothing: each connection's ClientHello is the one
/// a fresh configuration sends (the extensions advertised, no ticket, no hint).
#[derive(Debug)]
struct NoResumption;

impl tokio_rustls::rustls::client::ClientSessionStore for NoResumption {
    fn set_kx_hint(
        &self,
        _: tokio_rustls::rustls::pki_types::ServerName<'static>,
        _: tokio_rustls::rustls::NamedGroup,
    ) {
    }
    fn kx_hint(
        &self,
        _: &tokio_rustls::rustls::pki_types::ServerName<'_>,
    ) -> Option<tokio_rustls::rustls::NamedGroup> {
        None
    }
    fn set_tls12_session(
        &self,
        _: tokio_rustls::rustls::pki_types::ServerName<'static>,
        _: tokio_rustls::rustls::client::Tls12ClientSessionValue,
    ) {
    }
    fn tls12_session(
        &self,
        _: &tokio_rustls::rustls::pki_types::ServerName<'_>,
    ) -> Option<tokio_rustls::rustls::client::Tls12ClientSessionValue> {
        None
    }
    fn remove_tls12_session(&self, _: &tokio_rustls::rustls::pki_types::ServerName<'static>) {}
    fn insert_tls13_ticket(
        &self,
        _: tokio_rustls::rustls::pki_types::ServerName<'static>,
        _: tokio_rustls::rustls::client::Tls13ClientSessionValue,
    ) {
    }
    fn take_tls13_ticket(
        &self,
        _: &tokio_rustls::rustls::pki_types::ServerName<'static>,
    ) -> Option<tokio_rustls::rustls::client::Tls13ClientSessionValue> {
        None
    }
}

/// GET `path` on dayz.com over a raw, browser-shaped HTTPS/1.1 connection and
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
async fn get(path: &str) -> Result<(u16, String)> {
    use bytes::Bytes;
    use http_body_util::{BodyExt, Empty};
    use hyper_util::rt::TokioIo;
    use tokio_rustls::rustls::pki_types::ServerName;

    let err = |ctx: &str, e: String| Error::Other(format!("news fetch {ctx}: {e}"));
    let connector = tokio_rustls::TlsConnector::from(tls_config()?);

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
        .uri(path)
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

/// `path` on dayz.com, parsed by `parse`; a challenge or refusal is
/// [`Error::CloudflareChallenge`]. The whole exchange is bounded: a site that stops
/// answering must not leave the news loading forever.
async fn fetch<T>(path: &str, parse: impl FnOnce(&str) -> Result<T>) -> Result<T> {
    let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(20), get(path))
        .await
        .map_err(|_| Error::Other("dayz.com did not answer in time".into()))??;
    if status == 403 || status == 503 || looks_like_cloudflare_challenge(&body) {
        return Err(Error::CloudflareChallenge);
    }
    if status == 404 {
        return Err(Error::Other(format!("dayz.com has no {path}")));
    }
    if !(200..300).contains(&status) {
        return Err(Error::Other(format!(
            "dayz.com answered {status} for {path}"
        )));
    }
    parse(&body)
}

/// The latest articles, newest first: [`PAGES`] pages read at once. The first page
/// must come; a later one that fails only shortens the list.
pub async fn fetch_news() -> Result<Vec<Article>> {
    let mut pages = tokio::task::JoinSet::new();
    for page in 1..=PAGES {
        pages.spawn(async move {
            let path = if page == 1 {
                "/news.data".to_owned()
            } else {
                format!("/news.data?page={page}")
            };
            (page, fetch(&path, parse_news_page).await)
        });
    }
    let mut got: Vec<(u32, Vec<Article>)> = Vec::new();
    while let Some(done) = pages.join_next().await {
        let Ok((page, result)) = done else { continue };
        match result {
            Ok(list) => got.push((page, list)),
            Err(e) if page == 1 => return Err(e),
            Err(_) => {}
        }
    }
    got.sort_by_key(|(page, _)| *page);
    let mut seen = std::collections::HashSet::new();
    Ok(got
        .into_iter()
        .flat_map(|(_, list)| list)
        .filter(|a| seen.insert(a.id.clone()))
        .collect())
}

/// One article's body and patch notes.
pub async fn fetch_article(category: &str, slug: &str) -> Result<ArticleBody> {
    let path = format!("{}.data", article_path(category, slug));
    fetch(&path, parse_article).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = include_str!("../tests/news.data");

    #[test]
    fn a_news_page_lists_its_articles() {
        let list = parse_news_page(PAGE).unwrap();
        assert_eq!(list.len(), 10);
        let a = &list[0];
        assert_eq!(a.title, "DayZ Badlands is Out Now!");
        assert_eq!(a.date(), "2026-10-15");
        assert_eq!(
            a.url(),
            "https://dayz.com/article/game-update/dayz-badlands-out-now"
        );
        assert!(
            a.image_url
                .as_deref()
                .is_some_and(|u| u.starts_with("https://files.bistudio.com/"))
        );
        assert!(
            a.excerpt
                .as_deref()
                .is_some_and(|e| e.starts_with("Dear Survivors"))
        );
        assert_eq!(
            a.category.as_ref().map(|c| c.title.as_str()),
            Some("Game Update")
        );
    }

    #[test]
    fn an_article_has_its_body_and_notes() {
        let stream = r#"[{"_1":2},"routes/article/$category/$slug/route",{"_3":4},"data",{"_5":6,"_7":8},"bodyHtml","<p>Hi</p>","article",{"_9":10},"changelogs",[11],{"_12":13,"_14":15},"content","{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Fixed\"}]}]}","platforms",[16],{"_17":18},"title","Steam"]"#;
        let b = parse_article(stream).unwrap();
        assert_eq!(b.html, "<p>Hi</p>");
        assert_eq!(
            b.notes,
            [PatchNotes {
                platform: "Steam".into(),
                html: "<p>Fixed</p>".into()
            }]
        );
    }

    #[test]
    fn pages_that_are_not_news_are_errors() {
        assert!(parse_news_page("<!DOCTYPE html><title>404 | DayZ</title>").is_err());
        assert!(parse_article("[{}]").is_err());
    }

    #[test]
    fn addresses_are_encoded() {
        assert_eq!(article_path("a b", "x/y?"), "/article/a%20b/x%2Fy%3F");
    }

    /// Live check that the browser-shaped request still gets through Cloudflare and that
    /// the pages still read. Network-dependent, so ignored by default. Run with:
    /// cargo test -p dz-news -- tests::live --ignored --nocapture
    #[tokio::test]
    #[ignore = "reaches dayz.com"]
    async fn live() {
        let list = fetch_news().await.unwrap();
        println!("fetched {} articles", list.len());
        assert!(list.len() > 10, "more than one page");
        let a = &list[0];
        let cat = a.category.as_ref().map_or("news", |c| c.code.as_str());
        let body = fetch_article(cat, &a.slug).await.unwrap();
        println!(
            "{}: {} bytes, {} notes",
            a.title,
            body.html.len(),
            body.notes.len()
        );
        assert!(!body.html.is_empty());
    }
}
