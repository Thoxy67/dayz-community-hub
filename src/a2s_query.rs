use crate::api::Server;
use crate::{Result, errors::Error};
use a2s::{A2SClient, rules};
use std::time::Duration;

/// Query server information using A2S protocol
pub async fn query_server_info(server: &Server) -> Result<a2s::info::Info> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {}", e)))?;

    let timeout = Duration::from_secs(5);
    let _ = client.set_timeout(timeout);

    // Use endpoint port (query port)
    let query_port = server.endpoint.port as u16;
    let addr = format!("{}:{}", server.endpoint.ip, query_port);

    client
        .info(&addr)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S query failed: {}", e)))
}

/// Query player information using A2S protocol
pub async fn query_player_info(server: &Server) -> Result<Vec<a2s::players::Player>> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {}", e)))?;

    let timeout = Duration::from_secs(5);
    let _ = client.set_timeout(timeout);

    let query_port = server.endpoint.port as u16;
    let addr = format!("{}:{}", server.endpoint.ip, query_port);

    client
        .players(&addr)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S players query failed: {}", e)))
}

/// Query rules (cvars) using A2S protocol
pub async fn query_rules(server: &Server) -> Result<Vec<rules::Rule>> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {}", e)))?;

    let timeout = Duration::from_secs(5);
    let _ = client.set_timeout(timeout);

    let query_port = server.endpoint.port as u16;
    let addr = format!("{}:{}", server.endpoint.ip, query_port);

    client
        .rules(&addr)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S rules query failed: {}", e)))
}

/// Get comprehensive server information including players and rules
pub async fn get_server_details(server: &Server) -> Result<ServerDetails> {
    let info = query_server_info(server).await?;
    let players = query_player_info(server).await.ok();
    let rules = query_rules(server).await.ok();

    Ok(ServerDetails {
        info,
        players,
        rules,
    })
}

/// Comprehensive server details from A2S queries
#[derive(Debug, Clone)]
pub struct ServerDetails {
    pub info: a2s::info::Info,
    pub players: Option<Vec<a2s::players::Player>>,
    pub rules: Option<Vec<rules::Rule>>,
}

impl ServerDetails {
    /// Format as a string for display
    pub fn format(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Server: {}", self.info.name));
        lines.push(format!("Map: {}", self.info.map));
        lines.push(format!("Game: {}", self.info.game));
        lines.push(format!(
            "Players: {}/{}",
            self.info.players, self.info.max_players
        ));
        lines.push(format!("Version: {}", self.info.version));

        if let Some(ref players) = self.players {
            if !players.is_empty() {
                lines.push("Online players:".to_string());
                for player in players {
                    lines.push(format!("  {} ({} score)", player.name, player.score));
                }
            }
        }

        if let Some(ref rules) = self.rules {
            lines.push("Rules:".to_string());
            for rule in rules {
                lines.push(format!("  {} = {}", rule.name, rule.value));
            }
        }

        lines.join("\n")
    }
}
