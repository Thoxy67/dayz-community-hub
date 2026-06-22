use dayz_community_hub_core::errors::Error as CoreError;
use dayz_community_hub_core::news::{self, Article};
use tauri::{AppHandle, State};

use crate::commands::news_webview::fetch_news_via_webview;
use crate::dto::*;
use crate::state::SharedState;

/// Fetch the latest news articles.
///
/// Tries a direct HTTP request first (browser-shaped, to clear Cloudflare). If
/// dayz.com ever serves a challenge instead, we fall back to fetching it inside
/// a WebView (see [`fetch_news_via_webview`]).
#[tauri::command]
pub(crate) async fn fetch_news(app: AppHandle) -> Result<Vec<ArticleDto>, String> {
    let articles: Vec<Article> = match news::fetch_news().await {
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

/// Get app stats.
#[tauri::command]
pub(crate) async fn get_app_stats(state: State<'_, SharedState>) -> Result<AppStatsDto, String> {
    let state = state.read().await;
    let total_players: i64 = state.servers.iter().map(|s| s.players).sum();
    Ok(AppStatsDto {
        server_count: state.servers.len(),
        total_players,
        player_name: state.ctl.profile().player.clone(),
        steam_login: state.ctl.steamcmd_login().map(|s| s.to_string()),
        has_steamcmd: state.ctl.has_steamcmd(),
    })
}
