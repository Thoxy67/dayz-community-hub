//! What the desktop does for the window: open links, pick and save files,
//! copy text, and place the player on the map from their IP address.
//!
//! These go through Rust so the window needs no plugin permissions of its own.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};
use tauri_plugin_opener::OpenerExt;

use crate::error::ResultExt;
use crate::state::insecure_client;

/// A file-type filter of a file dialog: a label and its extensions (no dot).
#[derive(Deserialize, Clone, Debug, specta::Type)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Where the player is, approximately, from their IP address.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct GeoLocation {
    pub lat: f64,
    pub lon: f64,
    pub city: String,
    pub country: String,
    pub country_code: String,
}

/// Open a web page in the system browser.
#[tauri::command]
#[specta::specta]
pub(crate) fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).cmd_err()
}

/// Copy text to the clipboard.
#[tauri::command]
#[specta::specta]
pub(crate) fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).cmd_err()
}

fn dialog(app: &AppHandle, title: String, filters: &[FileFilter]) -> FileDialogBuilder<tauri::Wry> {
    let mut d = app.dialog().file().set_title(title);
    for f in filters {
        let exts: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
        d = d.add_filter(&f.name, &exts);
    }
    d
}

fn to_string(path: Option<FilePath>) -> Option<String> {
    path.map(|p| p.to_string())
}

/// A file (or a folder, with `directory`) the player chose; null if they
/// cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pick_file(
    app: AppHandle,
    title: String,
    directory: bool,
    filters: Vec<FileFilter>,
) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let d = dialog(&app, title, &filters);
    if directory {
        d.pick_folder(move |p| {
            let _ = tx.send(to_string(p));
        });
    } else {
        d.pick_file(move |p| {
            let _ = tx.send(to_string(p));
        });
    }
    rx.await.cmd_err()
}

/// Where the player wants a file written; null if they cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn save_file(
    app: AppHandle,
    title: String,
    default_name: String,
    filters: Vec<FileFilter>,
) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    dialog(&app, title, &filters)
        .set_file_name(default_name)
        .save_file(move |p| {
            let _ = tx.send(to_string(p));
        });
    rx.await.cmd_err()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpApi {
    status: String,
    message: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    city: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
}

/// The player's approximate location, from ip-api.com.
#[tauri::command]
#[specta::specta]
pub(crate) async fn geolocate_ip() -> Result<GeoLocation, String> {
    let r: IpApi = insecure_client()
        .get("http://ip-api.com/json/?fields=status,message,lat,lon,city,country,countryCode")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?
        .json()
        .await
        .cmd_err()?;
    match (r.status.as_str(), r.lat, r.lon) {
        ("success", Some(lat), Some(lon)) => Ok(GeoLocation {
            lat,
            lon,
            city: r.city.unwrap_or_default(),
            country: r.country.unwrap_or_default(),
            country_code: r.country_code.unwrap_or_default(),
        }),
        _ => Err(r.message.unwrap_or_else(|| "IP geolocation failed".into())),
    }
}
