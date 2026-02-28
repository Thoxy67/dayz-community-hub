use dayz_community_hub_core::{
    api::Server,
    config::Profile,
    mods::{self, InstalledMod},
    steamcmd::ModProgress,
};

use crate::dto::*;

/// Expands to the 14 fields shared by `ServerDto` and `ServerSlimDto`.
macro_rules! server_base_fields {
    ($s:expr) => {{
        let s = $s;
        (
            s.game_port,
            s.endpoint.ip.clone(),
            s.endpoint.port,
            s.name.clone(),
            s.map.clone(),
            s.players,
            s.max_players,
            s.environment.clone(),
            s.password,
            s.version.clone(),
            s.first_person_only,
            s.time.clone(),
            s.mods.len(),
            s.vac,
            s.battl_eye,
        )
    }};
}

pub(crate) fn server_to_dto(s: &Server) -> ServerDto {
    let (
        game_port,
        ip,
        query_port,
        name,
        map,
        players,
        max_players,
        environment,
        password,
        version,
        first_person_only,
        time,
        mods_count,
        vac,
        battl_eye,
    ) = server_base_fields!(s);
    ServerDto {
        game_port,
        ip,
        query_port,
        name,
        map,
        players,
        max_players,
        environment,
        password,
        version,
        first_person_only,
        time,
        mods_count,
        vac,
        battl_eye,
        mods: s
            .mods
            .iter()
            .map(|m| ModDto {
                name: m.name.clone(),
                steam_workshop_id: m.steam_workshop_id,
            })
            .collect(),
    }
}

pub(crate) fn server_to_slim_dto(s: &Server) -> ServerSlimDto {
    let (
        game_port,
        ip,
        query_port,
        name,
        map,
        players,
        max_players,
        environment,
        password,
        version,
        first_person_only,
        time,
        mods_count,
        vac,
        battl_eye,
    ) = server_base_fields!(s);
    ServerSlimDto {
        game_port,
        ip,
        query_port,
        name,
        map,
        players,
        max_players,
        environment,
        password,
        version,
        first_person_only,
        time,
        mods_count,
        vac,
        battl_eye,
    }
}

pub(crate) fn profile_to_dto(profile: &Profile) -> ProfileDto {
    ProfileDto {
        steam_login: profile.steam_login.clone(),
        steam_password: profile.steam_password.clone(),
        steam_root: profile.steam_root.clone(),
        steamcmd_enabled: profile.steamcmd_enabled,
        steamcmd_path: profile.steamcmd_path.clone(),
        player: profile.player.clone(),
        steam_api_key: profile.steam_api_key.clone(),
        steam_id: profile.steam_id.clone(),
        battlemetrics_api_key: profile.battlemetrics_api_key.clone(),
        user_location: profile.user_location,
        favorites: profile
            .favorites
            .iter()
            .map(|f| FavoriteDto {
                name: f.name.clone(),
                ip: f.ip.clone(),
                port: f.port,
                password: f.password.clone(),
            })
            .collect(),
        history: profile
            .history
            .iter()
            .map(|h| HistoryDto {
                name: h.name.clone(),
                ip: h.ip.clone(),
                port: h.port,
                ts: h.ts,
                relative_time: h.relative_time(),
            })
            .collect(),
        options: profile
            .options
            .all_options()
            .iter()
            .map(|(key, opt)| LaunchOptionDto {
                key: key.to_string(),
                enabled: opt.enabled,
                value: opt.value.clone(),
                description: opt.description.clone(),
            })
            .collect(),
        excluded_ips: profile.excluded_ips.clone(),
    }
}

pub(crate) fn installed_mod_to_dto(
    m: &InstalledMod,
    update_cache: &std::collections::HashMap<u64, i64>,
) -> InstalledModDto {
    let remote_updated = update_cache.get(&m.id).copied();
    let update_available = remote_updated.map(|r| r > m.local_updated).unwrap_or(false);
    InstalledModDto {
        name: m.name.clone(),
        id: m.id,
        local_updated: m.local_updated,
        size: m.size,
        size_human: mods::format_size(m.size),
        managed: m.managed,
        remote_updated,
        update_available,
    }
}

pub(crate) fn mod_progress_to_event(msg: &ModProgress) -> ModProgressEvent {
    match msg {
        ModProgress::ShuttingDownSteam => ModProgressEvent {
            kind: "shutting_down_steam".into(),
            current: 0,
            total: 0,
            mod_id: 0,
            name: "Closing Steam...".into(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::SteamGuardMobileRequired => ModProgressEvent {
            kind: "steam_guard_mobile_required".into(),
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::PasswordRequired => ModProgressEvent {
            kind: "password_required".into(),
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::LogLine(line) => ModProgressEvent {
            kind: "log_line".into(),
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: Some(line.clone()),
        },
        ModProgress::Starting {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent {
            kind: "starting".into(),
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: name.clone(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::Done {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent {
            kind: "done".into(),
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: name.clone(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::Failed {
            current,
            total,
            mod_id,
            name,
            error,
        } => ModProgressEvent {
            kind: "failed".into(),
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: format!("{} ({})", name, error),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::Finished {
            ok,
            failed,
            total: _,
            hint,
        } => ModProgressEvent {
            kind: "finished".into(),
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: *ok,
            failed: *failed,
            hint: hint.clone(),
            log_line: None,
        },
    }
}
