//! The dayz.com news feed, and the article images cached on disk.

pub(crate) mod images;
pub(crate) mod webview;

use dz_common::Error as CoreError;
use dz_news::Article;
use serde::Serialize;
use tauri::AppHandle;

use webview::fetch_news_via_webview;

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

/// Fetch the latest news articles.
///
/// Tries a direct HTTP request first (browser-shaped, to clear Cloudflare). If
/// dayz.com ever serves a challenge instead, we fall back to fetching it inside
/// a WebView (see [`fetch_news_via_webview`]).
#[tauri::command]
pub(crate) async fn fetch_news(app: AppHandle) -> Result<Vec<ArticleDto>, String> {
    let articles: Vec<Article> = match dz_news::fetch_news().await {
        Ok(articles) => articles,
        Err(CoreError::CloudflareChallenge) => fetch_news_via_webview(&app).await?,
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
