//! The Steam account's avatar and the number of players in DayZ, and how
//! the app reaches Steam ([`access`]).

use tauri::State;

use crate::error::{HttpResultExt, ResultExt, send_ok};
use crate::state::SharedState;

pub(crate) mod access;

/// Who is logged in to the running Steam client.
#[derive(serde::Serialize, Clone, Debug, specta::Type)]
pub struct SteamMeDto {
    pub steam_id: String,
    /// Their Steam name, a good default for the in-game name.
    pub name: String,
    pub avatar: Option<String>,
}

/// The account of the running Steam client, for the setup wizard and the
/// settings to fill in the Steam id and the name. Steam shows DayZ running
/// for a moment.
#[tauri::command]
#[specta::specta]
pub(crate) async fn steam_me() -> Result<SteamMeDto, String> {
    let me = access::me().await?;
    Ok(SteamMeDto {
        steam_id: me.steam_id.to_string(),
        name: me.name,
        avatar: me
            .avatar
            .and_then(|(w, h, rgba)| access::rgba_data_uri(w, h, &rgba)),
    })
}

/// The account's Steam avatar as a `data:` URI, cached: from the Web API
/// when a key is set, else (or when that fails) from the Steam client.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_steam_avatar(
    state: State<'_, SharedState>,
) -> Result<Option<String>, String> {
    let web = access::web(&state).await;
    let mut uri = None;
    if let (Some(key), Some(id)) = (&web.key, &web.steam_id) {
        uri = web_avatar(key, id).await.ok().flatten();
    }
    if uri.is_none() {
        uri = access::me().await.ok().and_then(|m| {
            let (w, h, rgba) = m.avatar?;
            access::rgba_data_uri(w, h, &rgba)
        });
    }
    state.write().await.cached_avatar = uri.clone();
    Ok(uri)
}

/// The avatar through GetPlayerSummaries, fetched and inlined.
async fn web_avatar(api_key: &str, steam_id: &str) -> Result<Option<String>, String> {
    let client = crate::net::api();

    // The key goes as a query parameter, never formatted into a string that
    // could end up in an error message.
    let resp: serde_json::Value = send_ok(
        client
            .get("https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/")
            .query(&[("key", api_key), ("steamids", steam_id)]),
    )
    .await
    .http_err("Steam profile")?
    .json::<serde_json::Value>()
    .await
    .http_err("Steam profile")?;

    let Some(img_url) = resp["response"]["players"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p["avatarmedium"].as_str())
        .map(str::to_string)
    else {
        return Ok(None);
    };
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
    Ok(Some(format!("data:{content_type};base64,{b64}")))
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
