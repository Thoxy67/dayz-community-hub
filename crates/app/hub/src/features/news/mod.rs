//! The dayz.com news feed, and the article images cached on disk.

pub mod images;

use crate::Hub;
use dz_common::Error as CoreError;
use dz_news::Article;
use serde::Serialize;

/// How a `fetch_news` error starts when Cloudflare turned the request away.
pub const BLOCKED_PREFIX: &str = "blocked:";

/// One article of the list (its body is [`Hub::fetch_article`]'s).
#[derive(Serialize, Clone, Debug)]
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

impl Hub {
    /// Fetch the latest news articles, without their bodies.
    ///
    /// Browser-shaped requests, to clear Cloudflare. When dayz.com serves a
    /// challenge anyway, the error starts with [`BLOCKED_PREFIX`] (`"blocked:"`),
    /// so the window can offer to open dayz.com instead.
    pub async fn fetch_news(&self) -> Result<Vec<ArticleDto>, String> {
        mocked!(self, w => w.news());
        let articles: Vec<Article> = dz_news::fetch_news().await.map_err(news_err)?;
        Ok(articles
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
            .collect())
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
