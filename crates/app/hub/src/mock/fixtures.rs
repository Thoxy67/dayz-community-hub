//! What the mock answers where the real command would ask the network,
//! Steam, the game or the disk: mock.ts' answers, in the real DTOs.

use dz_dayzmetrics::{RestartSchedule, WipeEvent, WipeSchedule};
use dz_game::battleye::{BattlEyeState, BattlEyeStatus};

use crate::features::a2s::{A2sDayzDto, A2sDetailsDto, A2sPlayerDto, A2sRuleDto};
use crate::features::friends::{
    FriendDto, FriendServerDto, FriendStatus, FriendsDto, FriendsSource,
};
use crate::features::gamepad::GamepadStatus;
use crate::features::launch::SteamLaunchInfoDto;
use crate::features::metrics::{HeatCell, ServerMetricsDto};
use crate::features::mods::{InstalledModDto, ModSourceDto, ModUsageDto};
use crate::features::native::GeoLocation;
use crate::features::offline::MissionSaveDto;
use crate::features::servers::ModDto;
use crate::features::servers::sources::{
    DzsaStatus, KeptStatus, Phase, SourcesChanged, SteamStatus,
};
use crate::features::steam::SteamMeDto;
use crate::features::steamcmd::SteamcmdDirsDto;
use crate::features::steamcmd::detect::{DayzDetectDto, SteamcmdStatusDto};
use crate::features::steamworks::{
    SteamSubscriptionDto, SteamSubscriptionsDto, SteamworksStatusDto, WorkshopItemDto,
};
use crate::features::system::{GpuDto, SystemSpecsDto};
use crate::features::updater::{UpdateInfo, UpdateKind, UpdateSupport};

use super::generate::LAUNCHER_DIR;
use super::{World, iso, iso_date, utc_hour, utc_weekday};

const DAY: i64 = 86_400;
const DAYZ_STEAMAPPS: &str = "/mnt/ssd2/SteamLibrary/steamapps";

fn now_ms() -> i64 {
    crate::features::ping::now_ms()
}

fn now_secs() -> i64 {
    now_ms().div_euclid(1000)
}

/// A listing half way through, through the Web API (mock.ts' `mockSources`).
pub(super) fn sources(now: i64) -> SourcesChanged {
    SourcesChanged {
        dzsa: DzsaStatus {
            phase: Phase::Done,
            count: 10_005,
            error: None,
            at: Some(now - 40),
        },
        steam: SteamStatus {
            phase: Phase::Running,
            needs_key: false,
            note: None,
            requests_done: 37,
            requests_total: 112,
            current: Some("empty · map chernarusplus · d*".to_string()),
            waiting_until: None,
            listed: 41_230,
            farms: 31_884,
            known: 6210,
            retried: 265,
            queried: 3136,
            answered: 2871,
            no_mods: 94,
            fakes: 2210,
            added: 2690,
            started: Some(now - 34),
            finished: None,
        },
        kept: KeptStatus {
            phase: Phase::Done,
            total: 412,
            checked: 412,
            alive: 361,
            removed: 18,
            fakes: 33,
        },
        duplicates: 2,
        total: 19_107,
        official: 1180,
    }
}

impl World {
    pub(crate) fn installed_mods(&self) -> Vec<InstalledModDto> {
        self.mods
            .iter()
            .map(|m| InstalledModDto {
                name: m.name.clone(),
                id: m.id,
                local_updated: m.local_updated,
                size: m.size,
                size_human: m.size_human.clone(),
                managed: m.managed,
                source: if m.steam {
                    ModSourceDto::Steam
                } else {
                    ModSourceDto::Launcher
                },
                path: m.path.clone(),
                other_copy: m.other_copy,
                remote_updated: m.remote_updated,
                update_available: m.update_available,
            })
            .collect()
    }

    /// The first mods run by two favourites and a played server; a few by
    /// history only.
    pub(crate) fn mods_usage(
        &self,
        favorites: &[String],
        first_played: Option<&str>,
    ) -> Vec<ModUsageDto> {
        self.mods
            .iter()
            .take(14)
            .enumerate()
            .map(|(i, m)| ModUsageDto {
                id: m.id,
                favorites: if i < 6 {
                    favorites.iter().take(1 + i % 3).cloned().collect()
                } else {
                    Vec::new()
                },
                history: if i % 2 == 0 {
                    vec![first_played.unwrap_or("4170").to_string()]
                } else {
                    Vec::new()
                },
            })
            .collect()
    }

    /// Most Steam-library mods subscribed (one not), one downloading, one
    /// waiting, a launcher mod also subscribed, and two subscriptions not on
    /// disk yet (one downloading, one waiting).
    pub(crate) fn steam_subscriptions(&self) -> SteamSubscriptionsDto {
        let cycle = (now_ms().rem_euclid(60_000)) as f64 / 60_000.0;
        let at = |i: usize| self.mods.get(i).map_or(0, |m| m.id);
        let title = |i: usize| self.mods.get(i).map(|m| m.name.clone()).unwrap_or_default();
        let item = |id: u64| SteamSubscriptionDto {
            id,
            subscribed: true,
            installed: true,
            needs_update: false,
            downloading: false,
            pending: false,
            bytes_done: 0,
            bytes_total: 0,
        };
        let mut items: Vec<SteamSubscriptionDto> =
            [0, 3, 6, 9, 21, 1].iter().map(|&i| item(at(i))).collect();
        items.push(SteamSubscriptionDto {
            needs_update: true,
            downloading: true,
            bytes_total: 1_400_000_000,
            bytes_done: (1_400_000_000.0 * cycle).floor() as u64,
            ..item(at(12))
        });
        items.push(SteamSubscriptionDto {
            needs_update: true,
            pending: true,
            ..item(at(18))
        });
        items.push(SteamSubscriptionDto {
            installed: false,
            needs_update: true,
            downloading: true,
            bytes_total: 640_000_000,
            bytes_done: (640_000_000.0 * cycle).floor() as u64,
            ..item(2_900_000_001)
        });
        items.push(SteamSubscriptionDto {
            installed: false,
            needs_update: true,
            ..item(2_900_000_002)
        });
        items.retain(|x| x.id != 0);
        // The first mod (CF-like) required by a few others, one of which
        // also needs a mod that is not installed.
        let detail = |id: u64, title: String, requires: Vec<u64>| WorkshopItemDto {
            id,
            title,
            requires,
        };
        let mut details = vec![
            detail(at(0), title(0), Vec::new()),
            detail(at(3), title(3), vec![at(0)]),
            detail(at(6), title(6), vec![at(0), 2_900_000_003]),
            detail(2_900_000_001, "Incoming Weapons Pack".into(), vec![at(0)]),
            detail(2_900_000_002, "Base Building Plus".into(), Vec::new()),
            detail(2_900_000_003, "Missing Library".into(), Vec::new()),
        ];
        details.retain(|d| d.id != 0);
        SteamSubscriptionsDto {
            available: true,
            reason: None,
            items,
            details,
        }
    }

    /// Ten friends, the ones in DayZ on the busiest servers.
    pub(crate) fn friends(&self) -> FriendsDto {
        let mut top: Vec<&super::MockServer> = self.servers.iter().collect();
        top.sort_by_key(|x| std::cmp::Reverse(x.players));
        let on = |i: usize| {
            top.get(i).map(|s| FriendServerDto {
                ip: s.ip.clone(),
                port: u16::try_from(s.query_port).unwrap_or(0),
                name: Some(s.name.clone()),
                map: Some(s.map.clone()),
                players: Some(s.players),
                max_players: Some(s.max_players),
            })
        };
        let unlisted = || {
            Some(FriendServerDto {
                ip: "203.0.113.7".to_string(),
                port: 2302,
                name: None,
                map: None,
                players: None,
                max_players: None,
            })
        };
        let now = self.now;
        let friend = |i: i64,
                      name: &str,
                      status: FriendStatus,
                      game: Option<&str>,
                      server: Option<FriendServerDto>| FriendDto {
            steam_id: format!("765611980000001{i}"),
            name: name.to_string(),
            status,
            avatar: None,
            game: game.map(str::to_string),
            in_dayz: game == Some("DayZ"),
            server,
            last_online: (status == FriendStatus::Offline).then(|| now - (i + 1) * 5400),
        };
        use FriendStatus as S;
        FriendsDto {
            source: Some(FriendsSource::WebApi),
            error: None,
            friends: vec![
                friend(0, "Kozak", S::Online, Some("DayZ"), on(2)),
                friend(1, "nightwolf", S::Online, Some("DayZ"), on(0)),
                friend(8, "Pavel", S::Online, Some("DayZ"), on(0)),
                friend(2, "Bambi Hunter", S::Away, Some("DayZ"), None),
                friend(9, "Yuri", S::Snooze, Some("DayZ"), unlisted()),
                friend(3, "Mira", S::Online, Some("Counter-Strike 2"), None),
                friend(4, "Tobi", S::Busy, None, None),
                friend(5, "ZedSlayer", S::Online, None, None),
                friend(6, "Old Gus", S::Offline, None, None),
                friend(7, "Lena", S::Offline, None, None),
            ],
        }
    }

    /// A server's live details: the listed server's, else an unlisted one
    /// announcing three mods.
    pub(crate) fn a2s(&self, ip: &str) -> A2sDetailsDto {
        let s = self.server_by_ip(ip).map(|(_, s)| s);
        let players = s.map_or(0, |s| s.players);
        let max = s.map_or(60, |s| s.max_players);
        let byte = |n: i64| u8::try_from(n.clamp(0, 255)).unwrap_or(0);
        let a2s_mods = [
            ("Community Framework", 1_559_212_036),
            ("VPPAdminTools", 1_828_439_124),
            ("Dabs Framework", 2_545_327_648),
        ];
        let map = s.map_or("", |s| s.map.as_str());
        A2sDetailsDto {
            server_name: s.map_or_else(|| "?".to_string(), |s| s.name.clone()),
            game: "DayZ".to_string(),
            players: byte(players),
            max_players: byte(max),
            bots: 0,
            map: map.to_string(),
            version: s.map_or("", |s| s.version).to_string(),
            players_list: (0..players.clamp(0, 12))
                .map(|i| A2sPlayerDto {
                    name: format!("Survivor {}", i + 1),
                    score: 0,
                    duration: (300 + i * 611) as f32,
                })
                .collect(),
            mods: Vec::new(),
            mods_from_a2s: if s.is_some() {
                Vec::new()
            } else {
                a2s_mods.iter().map(|(n, _)| (*n).to_string()).collect()
            },
            mods_a2s: if s.is_some() {
                Vec::new()
            } else {
                a2s_mods
                    .iter()
                    .map(|(n, id)| ModDto {
                        name: (*n).to_string(),
                        steam_workshop_id: *id,
                    })
                    .collect()
            },
            dayz: Some(A2sDayzDto {
                battleye: true,
                first_person_only: s.is_some_and(|s| s.first_person_only),
                official: false,
                private_hive: true,
                whitelisted: false,
                dlc: map == "enoch" || map == "sakhal",
                shard: Some("000".to_string()),
                login_queue: Some(if players >= max { 7 } else { 0 }),
                time_accel: Some(8.0),
                night_time_accel: Some(2.0),
                game_time: Some("14:09".to_string()),
            }),
            rules: [
                ("allowedBuild", "0"),
                ("clientPort", "0"),
                ("dedicated", "1"),
                ("island", if map.is_empty() { "chernarusplus" } else { map }),
                ("language", "65545"),
                ("platform", "win"),
                ("requiredBuild", "0"),
                ("requiredVersion", "129"),
                ("timeLeft", "15"),
            ]
            .iter()
            .map(|(name, value)| A2sRuleDto {
                name: (*name).to_string(),
                value: (*value).to_string(),
            })
            .collect(),
            query_port: s.map_or(27_016, |s| s.query_port),
            game_port: Some(s.map_or(2302, |s| s.game_port)),
        }
    }

    /// What DayZ Metrics knows of the server at `ip` (the first server when
    /// it is not listed); one in seven is a fake.
    pub(crate) fn metrics(&self, ip: &str) -> ServerMetricsDto {
        let found = self.server_by_ip(ip);
        let idx = found.map_or(0, |(i, _)| i);
        let s = found.map(|(_, s)| s).or_else(|| self.servers.first());
        let fake = found.is_some_and(|(i, _)| i % 7 == 3);
        let max = s.map_or(60, |s| s.max_players) as f64;
        let now_ms = now_ms();
        let now = now_ms.div_euclid(1000);
        let history: Vec<(i64, f64)> = (0..288i64)
            .map(|i| {
                let t = now - (287 - i) * 300;
                let wave =
                    0.55 + 0.4 * (((utc_hour(t) - 6) as f64 / 24.0) * std::f64::consts::TAU).sin();
                let v = (max * wave * if fake { 1.0 } else { 0.8 }).round().max(0.0);
                (t, v)
            })
            .collect();
        let id = 160_155 + idx as u64;
        let hours = |h: i64| iso(now_ms + h * 3_600_000);
        ServerMetricsDto {
            id,
            url: format!("https://dayzmetrics.com/server/{id}"),
            name: s.map(|s| s.name.clone()).unwrap_or_default(),
            map: s.map(|s| s.map.clone()),
            version: s.map(|s| s.version.to_string()),
            status: Some("online".to_string()),
            country: ["DE", "FR", "US", "RU", "GB"]
                .get(idx % 5)
                .map(|c| (*c).to_string()),
            players: s.map(|s| s.players as f64),
            max_players: Some(max),
            queue: Some(0.0),
            rank_pos: Some(8.0 + idx as f64),
            rank_score: Some(4.0021),
            rank_alive: Some(0.888),
            rank_demand: Some(0.17),
            avg_players_7d: Some(max * 0.45),
            peak_7d: Some(max),
            uptime_7d: Some(99.3),
            wow_pct: Some(if fake { -12.0 } else { 8.0 }),
            first_seen: Some("2026-05-24T07:17:31+00:00".to_string()),
            last_seen: Some(iso(now_ms)),
            ping_lo: Some(6.0),
            ping_hi: Some(83.0),
            ping_jitter: Some(7.0),
            ping_stability: Some(0.9),
            time_accel: Some(8.0),
            night_time_accel: Some(2.0),
            restart: Some(RestartSchedule {
                period_hours: Some(3.0),
                last_restart: Some(hours(-2)),
                next_restart: Some(iso(now_ms + 57 * 60_000)),
                confidence: Some("high".to_string()),
                slots_utc: [
                    "00:00", "03:00", "06:00", "09:00", "12:00", "15:00", "18:00", "21:00",
                ]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
                coverage: Some(0.96),
                unscheduled_7d: Some(2.0),
                restart_loops_7d: Some(0.0),
                last_unscheduled: Some(hours(-72)),
            }),
            wipe: Some(WipeSchedule {
                last: Some("2026-09-19".to_string()),
                last_source: Some("announced".to_string()),
                days_since: Some(10.0),
                next: Some("2026-10-17".to_string()),
                next_source: Some("predicted".to_string()),
                days_until: Some(18.0),
                period_days: Some(28.0),
                phase: Some("mid".to_string()),
                confidence: Some(0.8),
                events: vec![
                    WipeEvent {
                        on: Some("2026-09-19".to_string()),
                        source: Some("announced".to_string()),
                        confidence: Some(1.0),
                        corroborated: true,
                    },
                    WipeEvent {
                        on: Some("2026-08-22".to_string()),
                        source: Some("surge".to_string()),
                        confidence: Some(0.4),
                        corroborated: false,
                    },
                ],
            }),
            is_fake: fake,
            fake_reasons: if fake {
                vec![
                    "Player count never drops below 90 %".to_string(),
                    "Same names reappear on every restart".to_string(),
                ]
            } else {
                Vec::new()
            },
            behavior_verdict: Some(if fake { "fake" } else { "real" }.to_string()),
            behavior_score: Some(if fake { 0.1 } else { 0.76 }),
            flagged: false,
            flag_reason: None,
            suspect: false,
            mimics_official: false,
            description: None,
            game_time: Some("14:20".to_string()),
            game_time_at: Some(iso(now_ms)),
            discord: Some("https://discord.gg/example".to_string()),
            website: None,
            links: Vec::new(),
            notices: Vec::new(),
            playstyle: None,
            vanilla_band: Some("Lightly Modded".to_string()),
            vanilla_score: Some(37.9),
            mod_count: s.map(|s| s.mods_count as f64),
            mod_total_bytes: Some(5_228_339_261.0),
            ping_history: history.iter().map(|&(t, _)| (t, 8.0)).collect(),
            player_history: history,
        }
    }
}

/// Player counts over `range`: a daily wave, busier at weekends.
pub(crate) fn metrics_history(range: &str) -> Vec<(i64, f64)> {
    let span: i64 = match range {
        "7d" => 7,
        "2w" => 14,
        "1m" => 30,
        _ => 1,
    };
    let step = if span <= 1 {
        300
    } else if span <= 7 {
        1800
    } else {
        3600
    };
    let n = span * DAY / step;
    let now = now_secs();
    (0..n)
        .map(|i| {
            let t = now - (n - 1 - i) * step;
            let wave =
                0.5 + 0.4 * (((utc_hour(t) - 6) as f64 / 24.0) * std::f64::consts::TAU).sin();
            let weekend = if utc_weekday(t) % 6 == 0 { 1.15 } else { 1.0 };
            (t, (60.0 * wave * weekend).round())
        })
        .collect()
}

/// A month of daily ranks, climbing.
pub(crate) fn metrics_rank_history() -> Vec<(String, f64)> {
    let now = now_secs();
    (0..30i64)
        .map(|i| {
            (
                iso_date(now - (29 - i) * DAY),
                40.0 - (i as f64 * 1.1).round(),
            )
        })
        .collect()
}

/// Average players by weekday and hour: busiest in the evening.
pub(crate) fn metrics_heatmap() -> Vec<HeatCell> {
    (0..168u32)
        .map(|i| {
            let dow = i / 24;
            let hour = i % 24;
            let wave = 0.5 + 0.45 * (((hour as f64 - 13.0) / 24.0) * std::f64::consts::TAU).sin();
            let busy = if dow % 6 == 0 { 1.2 } else { 1.0 };
            HeatCell {
                dow: dow as u8,
                hour: hour as u8,
                avg: (70.0 * wave * busy).round(),
            }
        })
        .collect()
}

pub(crate) fn offline_missions() -> Vec<String> {
    [
        "DayZCommunityOfflineMode.ChernarusPlus",
        "DayZCommunityOfflineMode.Enoch",
        "DayZCommunityOfflineMode.Sakhal",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

pub(crate) fn offline_saves() -> Vec<MissionSaveDto> {
    let now = now_secs();
    vec![
        MissionSaveDto {
            mission: "DayZCommunityOfflineMode.ChernarusPlus".to_string(),
            bytes: 18_400_000,
            written: Some(now - 2 * DAY),
        },
        MissionSaveDto {
            mission: "DayZCommunityOfflineMode.Sakhal".to_string(),
            bytes: 4_100_000,
            written: Some(now - 3600),
        },
    ]
}

pub(crate) fn gamepad_status() -> GamepadStatus {
    GamepadStatus {
        available: true,
        pads: Vec::new(),
        steam_ui: false,
        steam_deck: false,
    }
}

pub(crate) fn system_specs() -> SystemSpecsDto {
    SystemSpecsDto {
        logical_cores: 16,
        physical_cores: 8,
        total_memory_mb: 32_768,
        cpu_name: Some("AMD Ryzen 7 5800X3D 8-Core Processor".to_string()),
        os: Some("Linux 24.04 Ubuntu".to_string()),
        gpus: vec![GpuDto {
            name: "NVIDIA GeForce RTX 4070".to_string(),
            vram_mb: Some(12_282),
        }],
    }
}

pub(crate) fn update_info() -> UpdateInfo {
    UpdateInfo {
        version: "0.5.0".to_string(),
        current_version: "0.4.1".to_string(),
        body: Some("- New interface".to_string()),
        date: Some(iso(now_ms())),
    }
}

/// An update can be installed, whatever this build is.
pub(crate) fn update_support() -> UpdateSupport {
    UpdateSupport {
        supported: true,
        kind: if cfg!(windows) {
            UpdateKind::WindowsZip
        } else {
            UpdateKind::Appimage
        },
        reason: None,
    }
}

pub(crate) fn steamcmd_status() -> SteamcmdStatusDto {
    SteamcmdStatusDto {
        found: true,
        path: Some("/usr/bin/steamcmd".to_string()),
        platform: "linux".to_string(),
    }
}

pub(crate) fn dayz_detect(path: Option<&str>) -> DayzDetectDto {
    let steamapps = path
        .filter(|p| !p.is_empty())
        .unwrap_or(DAYZ_STEAMAPPS)
        .to_string();
    DayzDetectDto {
        dayz_dir: Some(format!("{steamapps}/common/DayZ")),
        steamapps: Some(steamapps),
        workshop_mods: 37,
    }
}

pub(crate) fn steam_launch_info() -> SteamLaunchInfoDto {
    let strings = |a: &[&str]| a.iter().map(|s| (*s).to_string()).collect();
    SteamLaunchInfoDto {
        linux: true,
        launcher: strings(&["/usr/bin/steam"]),
        applaunch: strings(&["-applaunch", "221100", "-malloc=system", "-name=Survivor"]),
        launch_options: Some(
            "PROTON_USE_NTSYNC=1 PROTON_ENABLE_WAYLAND=1 RADV_PERFTEST=gpl,nggc,sam mangohud game-performance %command% -nolauncher"
                .to_string(),
        ),
        compat_tool: Some("proton-cachyos-native".to_string()),
        compat_tool_default: false,
        prefix: Some(format!("{DAYZ_STEAMAPPS}/compatdata/221100")),
    }
}

pub(crate) fn steam_me() -> SteamMeDto {
    SteamMeDto {
        steam_id: "76561198000000000".to_string(),
        name: "Survivor".to_string(),
        avatar: None,
    }
}

pub(crate) fn battleye() -> BattlEyeStatus {
    BattlEyeStatus {
        state: BattlEyeState::Ready,
        service: true,
    }
}

pub(crate) fn game_data_dirs() -> Vec<String> {
    vec![
        "C:\\Users\\Player\\Documents\\DayZ".to_string(),
        "C:\\Users\\Player\\AppData\\Local\\DayZ".to_string(),
    ]
}

pub(crate) fn steamcmd_dirs() -> SteamcmdDirsDto {
    SteamcmdDirsDto {
        content: format!("{LAUNCHER_DIR}/steamcmd-content"),
        home: Some(format!("{LAUNCHER_DIR}/steamcmd-home")),
    }
}

pub(crate) fn steamworks_status() -> SteamworksStatusDto {
    SteamworksStatusDto {
        library: true,
        error: None,
        steam_running: true,
    }
}

/// Paris, where the mock's player is (the profile's location).
pub(crate) fn geolocation() -> GeoLocation {
    GeoLocation {
        lat: 48.85,
        lon: 2.35,
        city: "Paris".to_string(),
        country: "France".to_string(),
        country_code: "FR".to_string(),
    }
}
