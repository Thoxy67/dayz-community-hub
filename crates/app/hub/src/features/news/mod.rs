//! The dayz.com news feed, and the article images cached on disk.

pub mod images;

use std::path::{Path, PathBuf};

use crate::Hub;
use dz_common::Error as CoreError;
use dz_news::Article;
use serde::{Deserialize, Serialize};

/// How a `fetch_news` error starts when Cloudflare turned the request away.
pub const BLOCKED_PREFIX: &str = "blocked:";

/// One article of the list (its body is [`Hub::fetch_article`]'s).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ArticleDto {
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub date: String,
    pub url: String,
    pub image_url: Option<String>,
    /// As shown (`Game Update`).
    pub category: Option<String>,
    /// In the address (`game-update`), what [`Hub::fetch_article`] asks for.
    pub category_code: Option<String>,
    pub author: Option<String>,
}

/// An article's body, as HTML, and an update's patch notes per platform.
#[derive(Serialize, Clone, Debug, Default)]
pub struct ArticleBodyDto {
    pub html: String,
    pub notes: Vec<PatchNotesDto>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PatchNotesDto {
    pub platform: String,
    pub html: String,
}

/// A dz-news error as the window reads it: Cloudflare's starts with [`BLOCKED_PREFIX`].
fn news_err(e: CoreError) -> String {
    match e {
        CoreError::CloudflareChallenge => {
            format!("{BLOCKED_PREFIX} dayz.com asked for a browser check (Cloudflare)")
        }
        e => e.to_string(),
    }
}

/// `<data dir>/news_cache.json`: the last list fetched, shown at once on the
/// next start while a fresh one is fetched.
fn cache_path() -> PathBuf {
    crate::paths::data_dir().join("news_cache.json")
}

/// The list kept at `path`; none when there is no file or it does not read.
fn read_cache(path: &Path) -> Option<Vec<ArticleDto>> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<Vec<ArticleDto>>(&bytes)
        .ok()
        .filter(|a| !a.is_empty())
}

/// Keep `articles` at `path` for the next start.
fn write_cache(path: &Path, articles: &[ArticleDto]) -> Result<(), String> {
    let data = serde_json::to_vec(articles).map_err(|e| e.to_string())?;
    dz_common::fs::write_atomic(path, &data).map_err(|e| e.to_string())
}

impl Hub {
    /// The articles fetched last time (kept on disk), to show while
    /// [`Hub::fetch_news`] fetches them again; `None` before the first fetch.
    pub async fn cached_news(&self) -> Option<Vec<ArticleDto>> {
        mocked!(self => None);
        tokio::task::spawn_blocking(|| read_cache(&cache_path()))
            .await
            .ok()
            .flatten()
    }

    /// Fetch the latest news articles, without their bodies, and keep them
    /// on disk for [`Hub::cached_news`].
    ///
    /// Browser-shaped requests, to clear Cloudflare. When dayz.com serves a
    /// challenge anyway, the error starts with [`BLOCKED_PREFIX`] (`"blocked:"`),
    /// so the window can offer to open dayz.com instead.
    pub async fn fetch_news(&self) -> Result<Vec<ArticleDto>, String> {
        mocked!(self, w => w.news());
        let articles: Vec<Article> = dz_news::fetch_news().await.map_err(news_err)?;
        let list: Vec<ArticleDto> = articles
            .iter()
            .map(|a| ArticleDto {
                title: a.title.clone(),
                slug: a.slug.clone(),
                excerpt: a.excerpt.clone(),
                date: a.published_at.clone().unwrap_or_default(),
                url: a.url(),
                image_url: a.image_url.clone(),
                category: a.category.as_ref().map(|c| c.title.clone()),
                category_code: a.category.as_ref().map(|c| c.code.clone()),
                author: None,
            })
            .collect();
        if !list.is_empty() {
            let kept = list.clone();
            tokio::task::spawn_blocking(move || {
                if let Err(e) = write_cache(&cache_path(), &kept) {
                    eprintln!("[news] the list was not kept: {e}");
                }
            });
        }
        Ok(list)
    }

    /// One article's body and patch notes (`category` is its `category_code`).
    pub async fn fetch_article(
        &self,
        category: String,
        slug: String,
    ) -> Result<ArticleBodyDto, String> {
        mocked!(self, w => w.article(&slug));
        let b = dz_news::fetch_article(&category, &slug)
            .await
            .map_err(news_err)?;
        Ok(ArticleBodyDto {
            html: b.html,
            notes: b
                .notes
                .into_iter()
                .map(|n| PatchNotesDto {
                    platform: n.platform,
                    html: n.html,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;

    fn article(slug: &str) -> ArticleDto {
        ArticleDto {
            title: format!("About {slug}"),
            slug: slug.into(),
            excerpt: Some("Short".into()),
            date: "2026-10-01T10:00:00Z".into(),
            url: format!("https://dayz.com/article/news/{slug}"),
            image_url: None,
            category: Some("News".into()),
            category_code: Some("news".into()),
            author: None,
        }
    }

    #[test]
    fn the_list_reads_back_as_it_was_kept() {
        let dir = std::env::temp_dir().join(format!("dzch-news-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("news_cache.json");
        assert_eq!(read_cache(&path), None);
        let list = vec![article("one"), article("two")];
        write_cache(&path, &list).unwrap();
        assert_eq!(read_cache(&path), Some(list));
        // A file that does not read, or holds nothing, gives nothing.
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(read_cache(&path), None);
        write_cache(&path, &[]).unwrap();
        assert_eq!(read_cache(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
