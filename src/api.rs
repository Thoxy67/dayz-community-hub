use crate::Result;
use serde::{Deserialize, Serialize};

pub type ServerList = Root;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub status: i64,
    pub result: Vec<Results>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    pub game_port: i64,
    pub sponsor: bool,
    pub profile: bool,
    pub endpoint: Endpoint,
    pub game: String,
    pub name: String,
    pub name_override: bool,
    pub map: String,
    pub folder: Option<String>,
    pub players: i64,
    pub max_players: i64,
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub mission: String,
    pub vac: bool,
    pub battl_eye: Option<bool>,
    pub first_person_only: bool,
    pub shard: String,
    pub time_acceleration: Option<i64>,
    pub time: String,
    pub mods: Vec<Mod>,
    pub original_name: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub ip: String,
    pub port: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub name: String,
    pub steam_workshop_id: i64,
}

pub type Server = Results;

/// Fetch the full server list from the DayZSA Launcher API.
pub async fn fetch_servers(client: &reqwest::Client) -> Result<ServerList> {
    let resp = client
        .get("https://dayzsalauncher.com/api/v1/launcher/servers/dayz")
        .send()
        .await?
        .json::<ServerList>()
        .await?;
    Ok(resp)
}

impl ServerList {
    pub fn find_server(&self, ip: &str, port: u16) -> Option<&Server> {
        self.result
            .iter()
            .find(|server| server.endpoint.ip == ip && server.endpoint.port as u16 == port)
    }

    pub fn find_server_by_game_port(&self, ip: &str, game_port: u16) -> Option<&Server> {
        self.result
            .iter()
            .find(|server| server.endpoint.ip == ip && server.game_port as u16 == game_port)
    }

    /// Find server by IP, trying both endpoint port and game port.
    pub fn find_server_flexible(&self, ip: &str, port: u16) -> Option<&Server> {
        self.find_server(ip, port)
            .or_else(|| self.find_server_by_game_port(ip, port))
    }

    /// Find all servers matching an IP address.
    pub fn find_servers_by_ip(&self, ip: &str) -> Vec<&Server> {
        self.result
            .iter()
            .filter(|s| s.endpoint.ip.trim() == ip.trim())
            .collect()
    }

    /// Total number of servers.
    pub fn count(&self) -> usize {
        self.result.len()
    }
}

/// Battlemetrics API response for server lookup.
#[derive(Debug, Deserialize)]
struct BattlemetricsResponse {
    data: Vec<BattlemetricsServer>,
}

#[derive(Debug, Deserialize)]
struct BattlemetricsServer {
    id: String,
}

/// Look up a server on Battlemetrics and return the URL.
/// Matches the bash script's `getBattlemetricsURL()`.
pub async fn get_battlemetrics_url(
    client: &reqwest::Client,
    ip: &str,
    server_name: Option<&str>,
) -> Result<Option<String>> {
    let search = match server_name {
        Some(name) => format!("{} {}", ip, name),
        None => ip.to_string(),
    };

    let resp = client
        .get("https://api.battlemetrics.com/servers")
        .query(&[
            ("page[size]", "10"),
            ("filter[game]", "dayz"),
            ("filter[search]", &search),
        ])
        .send()
        .await?;

    let data: BattlemetricsResponse = resp.json().await?;

    if let Some(server) = data.data.first() {
        Ok(Some(format!(
            "https://www.battlemetrics.com/servers/dayz/{}",
            server.id
        )))
    } else {
        Ok(None)
    }
}
