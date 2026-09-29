//! The Steam account's avatar and the number of players in DayZ.

use tauri::State;

use crate::error::{HttpResultExt, ResultExt, send_ok};
use crate::state::SharedState;

/// Fetch the Steam avatar for the configured account and cache it as a data: URI.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_steam_avatar(
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let (api_key, steam_id) = {
        let s = state.read().await;
        (
            s.ctl.profile().steam_api_key.clone(),
            s.ctl.profile().steam_id.clone(),
        )
    };

    let (api_key, steam_id) = match (api_key, steam_id) {
        (Some(k), Some(id)) if !k.is_empty() && !id.is_empty() => (k, id),
        _ => {
            state.write().await.cached_avatar = None;
            return Ok(None);
        }
    };

    let client = crate::net::api();

    // The key goes as a query parameter, never formatted into a string that
    // could end up in an error message.
    let resp: serde_json::Value = send_ok(
        client
            .get("https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/")
            .query(&[("key", api_key.as_str()), ("steamids", steam_id.as_str())]),
    )
    .await
    .http_err("Steam profile")?
    .json::<serde_json::Value>()
    .await
    .http_err("Steam profile")?;

    let avatar_img_url = resp["response"]["players"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p["avatarmedium"].as_str())
        .map(|s: &str| s.to_string());

    let data_uri = match avatar_img_url {
        None => None,
        Some(img_url) => {
            let img_resp = send_ok(client.get(&img_url))
                .await
                .http_err("Steam avatar")?;
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
            let bytes = img_resp.bytes().await.http_err("Steam avatar")?;
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Some(format!("data:{content_type};base64,{b64}"))
        }
    };

    state.write().await.cached_avatar = data_uri.clone();
    Ok(data_uri)
}

/// Fetch Steam player count for DayZ.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_steam_player_count(state: State<'_, SharedState>) -> Result<u32, String> {
    let client = {
        let state = state.read().await;
        state.ctl.http_client().clone()
    };
    dz_api::fetch_steam_player_count(&client).await.cmd_err()
}
