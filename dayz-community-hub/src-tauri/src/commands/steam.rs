use dayz_community_hub_core::api;
use tauri::State;

use crate::state::{SharedState, insecure_client};

/// Fetch the Steam avatar for the configured account and cache it as a data: URI.
#[tauri::command]
pub(crate) async fn fetch_steam_avatar(
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let (api_key, steam_id) = {
        let s = state.lock().await;
        (
            s.ctl.profile().steam_api_key.clone(),
            s.ctl.profile().steam_id.clone(),
        )
    };

    let (api_key, steam_id) = match (api_key, steam_id) {
        (Some(k), Some(id)) if !k.is_empty() && !id.is_empty() => (k, id),
        _ => {
            state.lock().await.cached_avatar = None;
            return Ok(None);
        }
    };

    let url = format!(
        "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/?key={}&steamids={}",
        api_key, steam_id
    );

    let client = insecure_client();

    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    let avatar_img_url = resp["response"]["players"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p["avatarmedium"].as_str())
        .map(|s: &str| s.to_string());

    let data_uri = match avatar_img_url {
        None => None,
        Some(img_url) => {
            let img_resp = client
                .get(&img_url)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let content_type = img_resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("image/jpeg")
                .split(';')
                .next()
                .unwrap_or("image/jpeg")
                .trim()
                .to_string();
            let bytes = img_resp.bytes().await.map_err(|e| e.to_string())?;
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Some(format!("data:{};base64,{}", content_type, b64))
        }
    };

    state.lock().await.cached_avatar = data_uri.clone();
    Ok(data_uri)
}

/// Fetch Steam player count for DayZ.
#[tauri::command]
pub(crate) async fn fetch_steam_player_count(state: State<'_, SharedState>) -> Result<u32, String> {
    let client = {
        let state = state.lock().await;
        state.ctl.http_client().clone()
    };
    api::fetch_steam_player_count(&client)
        .await
        .map_err(|e| e.to_string())
}
