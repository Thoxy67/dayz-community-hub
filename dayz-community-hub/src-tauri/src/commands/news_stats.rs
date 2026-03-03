use dayz_community_hub_core::news;
use tauri::State;

use crate::dto::*;
use crate::state::{SharedState, insecure_client};
use crate::utils::error::ResultExt;

/// Fetch the latest news articles.
#[tauri::command]
pub(crate) async fn fetch_news() -> Result<Vec<ArticleDto>, String> {
    let articles = news::fetch_news(insecure_client()).await.cmd_err()?;
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
    let state = state.lock().await;
    let total_players: i64 = state.servers.iter().map(|s| s.players).sum();
    Ok(AppStatsDto {
        server_count: state.servers.len(),
        total_players,
        player_name: state.ctl.profile().player.clone(),
        steam_login: state.ctl.steamcmd_login().map(|s| s.to_string()),
        has_steamcmd: state.ctl.has_steamcmd(),
    })
}
