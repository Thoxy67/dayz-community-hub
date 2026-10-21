//! The dayz.com news feed, and the article images cached on disk.

pub mod images;

use crate::Hub;
use dz_common::Error as CoreError;
use dz_news::Article;
use serde::Serialize;

/// How a `fetch_news` error starts when Cloudflare turned the request away.
pub const BLOCKED_PREFIX: &str = "blocked:";

/// One article, ready to render.
#[derive(Serialize, Clone, Debug)]
pub struct ArticleDto {
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content_text: String,
    pub content_html: String,
    pub date: String,
    pub url: String,
    pub image_url: Option<String>,
    pub category: Option<String>,
    pub author: Option<String>,
}

impl Hub {
    /// Fetch the latest news articles.
    ///
    /// A direct HTTP request, browser-shaped to clear Cloudflare. When dayz.com
    /// serves a challenge anyway, the error starts with [`BLOCKED_PREFIX`]
    /// (`"blocked:"`), so the window can offer to open dayz.com instead.
    pub async fn fetch_news(&self) -> Result<Vec<ArticleDto>, String> {
        mocked!(self, w => Ok(w.articles.clone()));
        let articles: Vec<Article> = match dz_news::fetch_news().await {
            Ok(articles) => articles,
            Err(CoreError::CloudflareChallenge) => {
                return Err(format!(
                    "{BLOCKED_PREFIX} dayz.com asked for a browser check (Cloudflare)"
                ));
            }
            Err(e) => return Err(e.to_string()),
        };
        Ok(articles
            .iter()
            .map(|a| ArticleDto {
                title: a.title.clone(),
                slug: a.slug.clone(),
                excerpt: a.excerpt.clone(),
                content_text: a.html_to_text(),
                content_html: a.content_html(),
                date: a.date().to_string(),
                url: a.url(),
                image_url: a.image.as_ref().map(|img| {
                    format!(
                        "https://dayz.com/app-static/uploads/article/{}/{}",
                        a.id, img
                    )
                }),
                category: a.category.as_ref().map(|c| c.name.clone()),
                author: a.author.as_ref().map(|au| au.name.clone()),
            })
            .collect())
    }
}
