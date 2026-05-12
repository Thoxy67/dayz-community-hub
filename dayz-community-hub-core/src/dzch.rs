//! `.dzch` server connection file format.
//!
//! A `.dzch` file is a small JSON document that describes everything needed to
//! connect to a DayZ server: address, port, optional password, and the list of
//! required Workshop mods.
//!
//! The same structure can be encoded into a `dzch://` URL for one-click connect
//! links (e.g. shared in Discord).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current file-format version.  Bump when adding breaking fields.
pub const DZCH_VERSION: u8 = 1;

/// A mod entry inside a `.dzch` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DzchMod {
    pub id: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

/// The `.dzch` server connection config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DzchConfig {
    /// Format version (currently 1).
    pub version: u8,
    /// Server IP or hostname.
    pub ip: String,
    /// Game port (default 2302).  Used to connect.
    pub port: u16,
    /// Query port (A2S).  Used to fetch live server info.
    /// When absent the receiver can fall back to heuristics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_port: Option<u16>,
    /// Server display name (informational).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Connection password (empty / absent = no password).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Required Workshop mods.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<DzchMod>,
}

impl DzchConfig {
    /// Read a `.dzch` file from disk.
    pub fn read_file(path: &Path) -> crate::Result<Self> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Other(format!("failed to read .dzch file: {e}")))?;
        let config: Self = serde_json::from_str(&data)
            .map_err(|e| crate::Error::Other(format!("invalid .dzch file: {e}")))?;
        Ok(config)
    }

    /// Write this config to a `.dzch` file on disk.
    pub fn write_file(&self, path: &Path) -> crate::Result<()> {
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| crate::Error::Other(format!("failed to serialise .dzch: {e}")))?;
        std::fs::write(path, data)
            .map_err(|e| crate::Error::Other(format!("failed to write .dzch file: {e}")))?;
        Ok(())
    }

    /// Parse a `dzch://` URL into a config.
    ///
    /// Format: `dzch://<ip>:<port>[?password=<pw>&mods=<id1>,<id2>&name=<n>]`
    pub fn from_url(url: &str) -> crate::Result<Self> {
        let stripped = url
            .strip_prefix("dzch://")
            .ok_or_else(|| crate::Error::Other("not a dzch:// URL".into()))?;

        // Split host[:port] from query string
        let (authority, query) = match stripped.find('?') {
            Some(i) => (&stripped[..i], &stripped[i + 1..]),
            None => (stripped, ""),
        };

        // Strip trailing slashes from authority
        let authority = authority.trim_end_matches('/');

        // Parse ip:port
        let (ip, port) = match authority.rfind(':') {
            Some(i) => {
                let port_str = &authority[i + 1..];
                let port: u16 = port_str.parse().map_err(|_| {
                    crate::Error::Other(format!("invalid port in dzch URL: {port_str}"))
                })?;
                (authority[..i].to_string(), port)
            }
            None => (authority.to_string(), 2302),
        };

        // Parse query params
        let mut password = None;
        let mut mods = Vec::new();
        let mut name = String::new();
        let mut query_port: Option<u16> = None;

        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            if let Some((key, val)) = pair.split_once('=') {
                let val = urlencoding::decode(val).unwrap_or(val.into());
                match key {
                    "password" | "pw" => password = Some(val.into_owned()),
                    "name" => name = val.into_owned(),
                    "qport" | "query_port" => {
                        query_port = val.parse::<u16>().ok();
                    }
                    "mods" => {
                        for id_str in val.split(',') {
                            if let Ok(id) = id_str.trim().parse::<i64>() {
                                mods.push(DzchMod {
                                    id,
                                    name: String::new(),
                                });
                            }
                        }
                    }
                    _ => {} // ignore unknown params
                }
            }
        }

        Ok(DzchConfig {
            version: DZCH_VERSION,
            ip,
            port,
            query_port,
            name,
            password,
            mods,
        })
    }

    /// Encode this config as a `dzch://` URL.
    pub fn to_url(&self) -> String {
        let mut url = format!("dzch://{}:{}", self.ip, self.port);
        let mut params = Vec::new();

        if let Some(qp) = self.query_port {
            params.push(format!("qport={qp}"));
        }
        if !self.name.is_empty() {
            params.push(format!("name={}", urlencoding::encode(&self.name)));
        }
        if let Some(pw) = &self.password
            && !pw.is_empty() {
                params.push(format!("password={}", urlencoding::encode(pw)));
            }
        if !self.mods.is_empty() {
            let ids: Vec<String> = self.mods.iter().map(|m| m.id.to_string()).collect();
            params.push(format!("mods={}", ids.join(",")));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        url
    }
}
