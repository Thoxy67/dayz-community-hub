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

/// Fetch the latest DayZ news articles.
/// Matches the bash script's `updateDayzNews()` / `getDayzNews()`.
///
/// dayz.com uses a self-signed / expired cert on its API path, so the caller
/// must supply an insecure `reqwest::Client` (TLS verification disabled).
/// Using a shared, long-lived client keeps the connection pool alive between
/// calls instead of establishing a new TLS handshake each time.
///
/// dayz.com now fronts this endpoint with a Cloudflare managed challenge that a
/// plain HTTP client can't pass. When that happens the body is challenge HTML
/// instead of JSON; we surface it as [`Error::CloudflareChallenge`] so the
/// caller can retry inside a real WebView.
pub async fn fetch_news(client: &reqwest::Client) -> Result<Vec<Article>> {
    let resp = client
        .get(NEWS_API_URL)
        .query(&[("rowsPerPage", "50")])
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    match parse_news_json(&body) {
        Ok(rows) => Ok(rows),
        Err(e) => {
            if status == reqwest::StatusCode::FORBIDDEN
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
                || looks_like_cloudflare_challenge(&body)
            {
                Err(crate::errors::Error::CloudflareChallenge)
            } else {
                Err(e)
            }
        }
    }
}
