//! How the app reaches Steam: Steam's Web API (a key in the profile) or the
//! running Steam client (Steamworks), each the other's fallback.
//!
//! The Web API is preferred where it does the same job, since it opens no
//! Steam session (Steam shows DayZ running while one is open) and is not
//! rationed like the client's server list. The client fills in what the
//! profile lacks: the Steam id is read from it when none is set, and it
//! answers whatever the Web API cannot without a key.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::state::SharedState;

/// What the profile gives for the Web API.
#[derive(Clone, Debug, Default)]
pub(crate) struct WebAccess {
    pub key: Option<String>,
    /// From the profile, else from the Steam client.
    pub steam_id: Option<String>,
}

fn clean(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

/// The Web API key, if any.
pub(crate) async fn web_key(state: &SharedState) -> Option<String> {
    clean(state.read().await.ctl.profile().steam_api_key.as_deref())
}

/// The key, and the Steam id: the profile's, else (with a key) the Steam
/// client's, which is then saved in the profile so the settings show it.
pub(crate) async fn web(state: &SharedState) -> WebAccess {
    let (key, id) = {
        let s = state.read().await;
        let p = s.ctl.profile();
        (
            clean(p.steam_api_key.as_deref()),
            clean(p.steam_id.as_deref()),
        )
    };
    let steam_id = match id {
        Some(id) => Some(id),
        // Only worth a Steam session when the key can use it.
        None if key.is_none() => None,
        None => {
            let found = me().await.ok().map(|m| m.steam_id.to_string());
            if let Some(id) = &found {
                let id = id.clone();
                let _ = crate::state::mutate_profile(state, |s| {
                    let p = s.ctl.profile_mut();
                    if clean(p.steam_id.as_deref()).is_none() {
                        p.steam_id = Some(id);
                    }
                    Ok(())
                })
                .await;
            }
            found
        }
    };
    WebAccess { key, steam_id }
}

/// How long the client's answer about the account is reused.
const ME_FRESH: Duration = Duration::from_secs(10 * 60);
static ME: Mutex<Option<(Instant, dz_steamworks::Me)>> = Mutex::new(None);

/// Who is logged in to the Steam client, with their avatar: asked at most
/// every few minutes (each ask shows DayZ running for a moment).
pub(crate) async fn me() -> Result<dz_steamworks::Me, String> {
    if let Some((at, me)) = ME.lock().ok().and_then(|m| m.clone())
        && at.elapsed() < ME_FRESH
    {
        return Ok(me);
    }
    let me = tauri::async_runtime::spawn_blocking(dz_steamworks::me)
        .await
        .map_err(|e| e.to_string())??;
    if let Ok(mut m) = ME.lock() {
        *m = Some((Instant::now(), me.clone()));
    }
    Ok(me)
}

/// An RGBA picture as a `data:` URI the window can show: a 32-bit BMP, which
/// needs no encoder.
pub(crate) fn rgba_data_uri(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    let pixels = (width as usize).checked_mul(height as usize)?;
    if rgba.len() < pixels * 4 || pixels == 0 {
        return None;
    }
    const HEAD: u32 = 14 + 40;
    let size = HEAD + (pixels as u32) * 4;
    let mut b = Vec::with_capacity(size as usize);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&HEAD.to_le_bytes());
    // BITMAPINFOHEADER; a negative height is a top-down picture.
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(width as i32).to_le_bytes());
    b.extend_from_slice(&(-(height as i32)).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(&[0u8; 24]);
    for [r, g, bl, a] in rgba[..pixels * 4].as_chunks::<4>().0 {
        b.extend_from_slice(&[*bl, *g, *r, *a]);
    }
    use base64::Engine;
    Some(format!(
        "data:image/bmp;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&b)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_becomes_a_bmp_of_the_right_size() {
        let uri = rgba_data_uri(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        let b64 = uri.strip_prefix("data:image/bmp;base64,").unwrap();
        use base64::Engine;
        let b = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        assert_eq!(&b[..2], b"BM");
        assert_eq!(b.len(), 54 + 8);
        // Red, as BGRA.
        assert_eq!(&b[54..58], &[0, 0, 255, 255]);
        assert!(rgba_data_uri(2, 2, &[0; 4]).is_none());
    }
}
