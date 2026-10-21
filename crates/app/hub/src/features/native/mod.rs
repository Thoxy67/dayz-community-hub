//! What the desktop did for the window. Opening links, picking and saving
//! files and the clipboard are the egui window's own now; what stays here is
//! placing the player on the map from their IP address, and the rule for
//! which links may be opened ([`is_openable`]), which the window applies.

use crate::Hub;
use serde::{Deserialize, Serialize};

use crate::error::{HttpResultExt, send_ok};

/// Where the player is, approximately, from their IP address.
#[derive(Serialize, Clone, Debug)]
pub struct GeoLocation {
    pub lat: f64,
    pub lon: f64,
    pub city: String,
    pub country: String,
    pub country_code: String,
}

/// Schemes the window may ask the system to open (`open_url` refused the rest). A link in a news article
/// is remote content: `file:` or a custom scheme would hand the OS whatever
/// it pointed at.
pub fn is_openable(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    ["https://", "http://", "steam://", "mailto:"]
        .iter()
        .any(|p| lower.starts_with(p))
}

impl Hub {
    /// The player's approximate location, from ip-api.com.
    pub async fn geolocate_ip(&self) -> Result<GeoLocation, String> {
        mocked!(self => Ok(crate::mock::fixtures::geolocation()));
        // ip-api's free tier is plain HTTP only.
        let r: IpApi =
            send_ok(crate::net::api().get(
                "http://ip-api.com/json/?fields=status,message,lat,lon,city,country,countryCode",
            ))
            .await
            .http_err("IP geolocation")?
            .json()
            .await
            .http_err("IP geolocation")?;
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
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    #[test]
    fn only_web_and_steam_links_open() {
        assert!(super::is_openable("https://dayz.com"));
        assert!(super::is_openable("steam://run/221100"));
        assert!(!super::is_openable("file:///etc/passwd"));
        assert!(!super::is_openable("javascript:alert(1)"));
        assert!(!super::is_openable("C:\\Windows\\calc.exe"));
    }
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
