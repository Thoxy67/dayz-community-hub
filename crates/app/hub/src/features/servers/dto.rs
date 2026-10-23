use dz_api::Server;
use serde::Serialize;

/// A server with its mod list, for the detail panel and the connect flow.
#[derive(Serialize, Clone, Debug)]
pub struct ServerDto {
    /// 0 when unknown.
    pub game_port: u16,
    pub ip: dz_api::Ipv4Addr,
    pub query_port: u16,
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

#[derive(Serialize, Clone, Debug)]
pub struct ModDto {
    pub name: String,
    pub steam_workshop_id: i64,
}

/// Response from the `initialize` command.
#[derive(Serialize, Clone, Debug)]
pub struct InitResult {
    pub server_count: usize,
    pub from_cache: bool,
    /// True when no profile existed yet (the setup wizard should run).
    pub is_first_launch: bool,
    /// Why there is no list: no cache and the API could not be reached.
    pub list_error: Option<String>,
    /// This process had already started: the window reloaded (F5, a crash
    /// recovery, "Retry") and finds the backend's work as it left it. A
    /// resumed window starts nothing merely because it loaded.
    pub resumed: bool,
    /// This process, as a token: what a window saved in `sessionStorage`
    /// belongs to it only if it was saved under the same one.
    pub session: String,
}

/// The title bar's counters.
#[derive(Serialize, Clone, Debug)]
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
        ip: s.ip(),
        query_port: s.query_port(),
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
            name: m.name.to_string(),
            steam_workshop_id: m.steam_workshop_id,
        })
        .collect()
}
