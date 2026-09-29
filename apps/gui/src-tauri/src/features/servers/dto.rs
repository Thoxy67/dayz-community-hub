use dz_api::Server;
use serde::Serialize;

/// A server with its mod list, for the detail panel and the connect flow.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct ServerDto {
    pub game_port: i64,
    pub ip: String,
    pub query_port: i64,
    pub name: String,
    pub map: String,
    pub players: i64,
    pub max_players: i64,
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub first_person_only: bool,
    pub time: String,
    pub mods_count: usize,
    pub mods: Vec<ModDto>,
    pub vac: bool,
    pub battl_eye: Option<bool>,
}

#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct ModDto {
    pub name: String,
    pub steam_workshop_id: i64,
}

/// Response from the `initialize` command.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct InitResult {
    pub server_count: usize,
    pub from_cache: bool,
    /// True when no profile existed yet (the setup wizard should run).
    pub is_first_launch: bool,
}

/// The title bar's counters.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct AppStatsDto {
    pub server_count: usize,
    pub total_players: i64,
    pub player_name: Option<String>,
    pub steam_login: Option<String>,
    pub has_steamcmd: bool,
}

pub(crate) fn server_to_dto(s: &Server) -> ServerDto {
    ServerDto {
        game_port: s.game_port,
        ip: s.endpoint.ip.clone(),
        query_port: s.endpoint.port,
        name: s.name.clone(),
        map: s.map.clone(),
        players: s.players,
        max_players: s.max_players,
        environment: s.environment.clone(),
        password: s.password,
        version: s.version.clone(),
        first_person_only: s.first_person_only,
        time: s.time.clone(),
        mods_count: s.mods.len(),
        mods: mods_to_dto(s),
        vac: s.vac,
        battl_eye: s.battl_eye,
    }
}

pub(crate) fn mods_to_dto(s: &Server) -> Vec<ModDto> {
    s.mods
        .iter()
        .map(|m| ModDto {
            name: m.name.clone(),
            steam_workshop_id: m.steam_workshop_id,
        })
        .collect()
}
