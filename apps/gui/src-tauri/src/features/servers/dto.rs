use std::sync::Arc;

use dz_api::{Server, ServerList};
use serde::{Serialize, Serializer, ser::SerializeSeq};

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

/// A server as the browser's table shows it: no mod list, only its length.
///
/// Never built at run time: [`ServerSlimList`] writes the same JSON straight
/// from the list. It is what that JSON is described as in `bindings.ts`, and
/// what the test below holds the two against.
#[allow(dead_code)]
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct ServerSlimDto {
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

/// The whole server list, serialized straight from the shared list as
/// [`ServerSlimDto`]s: no per-server clone, and no lock held while the
/// ~18 000 entries are written out.
#[derive(Clone, Debug)]
pub struct ServerSlimList(pub Arc<ServerList>);

/// Described to the window as what it writes: `ServerSlimDto[]`.
impl specta::Type for ServerSlimList {
    fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
        <Vec<ServerSlimDto> as specta::Type>::definition(types)
    }
}

impl Serialize for ServerSlimList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let servers = &self.0.result;
        let mut seq = serializer.serialize_seq(Some(servers.len()))?;
        for s in servers {
            seq.serialize_element(&SlimView(s))?;
        }
        seq.end()
    }
}

/// A borrowed [`ServerSlimDto`]: the same JSON, written from the `Server`.
struct SlimView<'a>(&'a Server);

impl Serialize for SlimView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let s = self.0;
        let mut st = serializer.serialize_struct("ServerSlimDto", 15)?;
        st.serialize_field("game_port", &s.game_port)?;
        st.serialize_field("ip", &s.endpoint.ip)?;
        st.serialize_field("query_port", &s.endpoint.port)?;
        st.serialize_field("name", &s.name)?;
        st.serialize_field("map", &s.map)?;
        st.serialize_field("players", &s.players)?;
        st.serialize_field("max_players", &s.max_players)?;
        st.serialize_field("environment", &s.environment)?;
        st.serialize_field("password", &s.password)?;
        st.serialize_field("version", &s.version)?;
        st.serialize_field("first_person_only", &s.first_person_only)?;
        st.serialize_field("time", &s.time)?;
        st.serialize_field("mods_count", &s.mods.len())?;
        st.serialize_field("vac", &s.vac)?;
        st.serialize_field("battl_eye", &s.battl_eye)?;
        st.end()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The borrowed writer and the owned DTO must produce the same JSON, or
    /// the window's type would lie.
    #[test]
    fn slim_view_matches_slim_dto() {
        let server = Server {
            game_port: 2302,
            endpoint: dz_api::Endpoint {
                ip: "1.2.3.4".into(),
                port: 27016,
            },
            name: "n".into(),
            map: "chernarusplus".into(),
            players: 3,
            max_players: 60,
            environment: "w".into(),
            password: true,
            version: "1.28".into(),
            vac: true,
            battl_eye: Some(true),
            first_person_only: false,
            time: "12:00".into(),
            mods: vec![dz_api::Mod {
                name: "CF".into(),
                steam_workshop_id: 1559212036,
            }],
        };
        let owned = ServerSlimDto {
            game_port: 2302,
            ip: "1.2.3.4".into(),
            query_port: 27016,
            name: "n".into(),
            map: "chernarusplus".into(),
            players: 3,
            max_players: 60,
            environment: "w".into(),
            password: true,
            version: "1.28".into(),
            first_person_only: false,
            time: "12:00".into(),
            mods_count: 1,
            vac: true,
            battl_eye: Some(true),
        };
        let list = ServerSlimList(Arc::new(ServerList {
            status: 0,
            result: vec![server],
        }));
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            serde_json::to_value(vec![owned]).unwrap()
        );
    }
}
