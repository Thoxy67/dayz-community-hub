//! mock.ts' invented data, made in the same order from the same seed, so the
//! same servers, mods and sessions come out.

use dz_profile::{
    Favorite, FavoriteGroup, FavoriteLabel, History, LabelColor, ModDownloader, Profile, Session,
    SessionKind,
};
use std::net::Ipv4Addr;

use crate::features::news::ArticleDto;

use super::World;

const MAPS: [&str; 9] = [
    "chernarusplus",
    "enoch",
    "sakhal",
    "deerisle",
    "namalsk",
    "banov",
    "pripyat",
    "esseker",
    "takistanplus",
];
const TAGS: [&str; 11] = [
    "PVP",
    "PVE",
    "1PP",
    "3PP",
    "Loot x2",
    "Trader",
    "Raid Weekends",
    "Vanilla+",
    "Hardcore",
    "KOTH",
    "Airdrops",
];
const REGIONS: [&str; 9] = ["EU", "US", "RU", "UK", "DE", "FR", "AU", "PL", "NA-East"];
const WORDS: [&str; 8] = [
    "Frontline",
    "Wasteland",
    "Survivor",
    "Nomad",
    "Blackout",
    "Last Light",
    "Iron Wolves",
    "Dead Frequency",
];
const MAX_PLAYERS: [i64; 8] = [40, 50, 60, 60, 60, 80, 100, 127];
const OFFICIAL_REGIONS: [&str; 5] = [
    "EUROPE - DE",
    "EUROPE - FR",
    "EUROPE - GB",
    "UNITED STATES - NY",
    "AUSTRALIA - SYD",
];
const MOD_NAMES: [&str; 24] = [
    "CF",
    "Community-Online-Tools",
    "Dabs Framework",
    "VPPAdminTools",
    "Expansion-Core",
    "BuilderItems",
    "MuchCarKey",
    "Code Lock",
    "BaseBuildingPlus",
    "DayZ-Expansion-Map",
    "SchanaModParty",
    "Trader",
    "MMG Storage",
    "RedFalcon Heliz",
    "Airdrop-Upgraded",
    "Breachingcharge",
    "Server_Information_Panel",
    "GoreZ",
    "Survivor Animations",
    "WindstridesClothing",
    "Mass'sManyItemOverhaul",
    "CannabisPlus",
    "SNAFU Weapons",
    "DeerIsle",
];
/// mock.ts' launch options, in its order: (key, description).
const OPTIONS: [(&str, &str); 20] = [
    ("window", "Run in windowed mode"),
    ("noborder", "Borderless window"),
    ("nosplash", "Skip splash screen"),
    ("skipintro", "Skip intro videos"),
    ("nolauncher", "Skip the Bohemia launcher"),
    ("nocharmenu", "Skip the character menu"),
    ("file_patching", "Load unpacked files"),
    ("do_logs", "Write RPT logs"),
    ("high", "High process priority"),
    ("world", "World loaded at start"),
    ("no_pause", "Keep running when unfocused"),
    ("max_mem", "Maximum memory (MB)"),
    ("max_vram", "Maximum video memory (MB)"),
    ("cpu_count", "CPU cores to use"),
    ("ex_threads", "Extra threads mask"),
    ("no_benchmark", "Skip the benchmark"),
    ("script_debug", "Script debugging"),
    ("buldozer", "Buldozer mode"),
    ("winxp", "DirectX 9"),
    ("profiles", "Profile folder"),
];
const ARTICLE_TITLES: [&str; 8] = [
    "Update 1.28 is live",
    "Status Report – September",
    "Frostline: what's next",
    "Community spotlight",
    "Server hosting changes",
    "Stable Update 1.27",
    "Winter event",
    "Console patch notes",
];

pub(crate) const STEAM_ROOT: &str = "/home/player/.local/share/Steam";
pub(crate) const LAUNCHER_DIR: &str = "/home/player/.local/share/dayz-community-hub";

const DAY: i64 = 86_400;

/// mock.ts' `rnd`: `seed = (seed * 1103515245 + 12345) % 2147483648`, in
/// doubles as JavaScript computes it (the product overflows 2^53 and is
/// rounded the same way), over 2^31.
struct Rng(f64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = (self.0 * 1_103_515_245.0 + 12_345.0) % 2_147_483_648.0;
        self.0 / 2_147_483_648.0
    }

    /// `Math.floor(rnd() * n)`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() * n as f64).floor() as usize
    }

    /// `pick(a)`.
    fn pick<T: Copy + Default>(&mut self, a: &[T]) -> T {
        let i = self.below(a.len());
        a.get(i).copied().unwrap_or_default()
    }
}

/// One of mock.ts' servers.
#[derive(Clone, Debug)]
pub(crate) struct MockServer {
    pub game_port: u16,
    pub ip: Ipv4Addr,
    pub query_port: u16,
    pub name: String,
    pub map: String,
    pub players: i64,
    pub max_players: i64,
    pub environment: &'static str,
    pub password: bool,
    pub version: &'static str,
    pub first_person_only: bool,
    pub time: String,
    pub mods_count: usize,
    pub battl_eye: bool,
    pub official: bool,
}

impl MockServer {
    /// mock.ts' pretend disk: of this server's mods, how many are missing.
    pub(crate) fn mods_missing(&self) -> usize {
        if self.mods_count == 0 {
            return 0;
        }
        // mock.ts read the address as text: its last character's code.
        let last = self.ip.to_string().bytes().last().map_or(0, usize::from);
        [0, 0, 3, 1, self.mods_count]
            .get(last % 5)
            .copied()
            .unwrap_or(0)
    }

    /// And how many are behind.
    pub(crate) fn mods_stale(&self) -> usize {
        if self.mods_count == 0 {
            return 0;
        }
        // mock.ts added the address text's length.
        let len = self.ip.to_string().len() as i64;
        let k = (len + i64::from(self.query_port)).rem_euclid(5) as usize;
        [0, 2, 0, 1, 0].get(k).copied().unwrap_or(0)
    }
}

/// One of mock.ts' installed mods.
#[derive(Clone, Debug)]
pub(crate) struct MockMod {
    pub name: String,
    pub id: u64,
    pub local_updated: i64,
    pub size: u64,
    pub size_human: String,
    pub managed: bool,
    pub steam: bool,
    pub path: String,
    pub other_copy: bool,
    pub remote_updated: Option<i64>,
    pub update_available: bool,
}

pub(super) struct Generated {
    pub servers: Vec<MockServer>,
    pub mods: Vec<MockMod>,
    pub articles: Vec<ArticleDto>,
    pub play_log: Vec<Session>,
}

/// Everything mock.ts makes at load, in its order (the generator is shared).
pub(super) fn generate(now: i64) -> Generated {
    let mut rng = Rng(42.0);
    let servers = servers(&mut rng);
    let mods = mods(&mut rng);
    let articles = articles(now);
    let play_log = play_log(&mut rng, &servers, now);
    Generated {
        servers,
        mods,
        articles,
        play_log,
    }
}

fn servers(rng: &mut Rng) -> Vec<MockServer> {
    let mut out: Vec<MockServer> = (0..9500usize)
        .map(|i| {
            let max = rng.pick(&MAX_PLAYERS);
            let players = if rng.next() < 0.3 {
                0
            } else if rng.next() < 0.1 {
                max
            } else {
                (rng.next() * max as f64).floor() as i64
            };
            // Each part below 256, as mock.ts writes them.
            let part = |n: usize| (n % 256) as u8;
            let ip = Ipv4Addr::new(
                part(45 + (i % 180)),
                part((i * 7) % 255),
                part((i * 13) % 255),
                part((i * 31) % 250),
            );
            let map = rng.pick(&MAPS);
            let fp = rng.next() < 0.35;
            let region = rng.pick(&REGIONS);
            let word = rng.pick(&WORDS);
            let shown = if map == "chernarusplus" {
                "Chernarus"
            } else {
                map
            };
            let tag1 = rng.pick(&TAGS);
            let tag2 = rng.pick(&TAGS);
            let name = format!(
                "[{region}] {word} {shown} | {tag1} | {tag2}{}",
                if fp { " | 1PP" } else { "" }
            );
            let environment = if rng.next() < 0.8 { "w" } else { "l" };
            let password = rng.next() < 0.06;
            let hours = rng.below(24);
            let minutes = rng.below(60);
            let mods_count = if rng.next() < 0.35 { 0 } else { rng.below(40) };
            let battl_eye = rng.next() < 0.9;
            MockServer {
                game_port: 2302 + (i as u16 % 5) * 100,
                ip,
                query_port: 27016 + (i as u16 % 5) * 100,
                name,
                map: map.to_string(),
                players,
                max_players: max,
                environment,
                password,
                // A few servers still on the version before (one of them the mock game's).
                version: if i % 17 == 5 {
                    "1.27.159674"
                } else {
                    "1.28.162391"
                },
                first_person_only: fp,
                time: format!("{hours:02}:{minutes:02}"),
                mods_count,
                battl_eye,
                official: false,
            }
        })
        .collect();

    // Twenty official servers, and between them a few community ones
    // wearing their names.
    for (i, x) in out.iter_mut().enumerate().take(26) {
        let region = OFFICIAL_REGIONS.get(i % 5).copied().unwrap_or_default();
        if i % 4 != 3 {
            x.name = format!(
                "{} | {region}{}",
                4100 + i * 7,
                if x.first_person_only {
                    " | 1st Person Only"
                } else {
                    ""
                }
            );
            x.map = ["chernarusplus", "enoch", "sakhal"]
                .get(i % 3)
                .copied()
                .unwrap_or_default()
                .to_string();
            x.password = false;
            x.mods_count = 0;
            x.battl_eye = true;
            x.official = true;
        } else {
            x.name = format!("{} | {region} | x10 Loot", 4200 + i);
        }
    }
    out
}

/// `toFixed`: rounded half away from zero, as near as it matters here.
fn fixed(v: f64, digits: usize) -> String {
    format!("{v:.digits$}")
}

fn mods(rng: &mut Rng) -> Vec<MockMod> {
    MOD_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let size = (rng.next() * 2_000_000_000.0).floor() as u64 + 2_000_000;
            let local = 1_750_000_000 + (rng.next() * 5_000_000.0).floor() as i64;
            let stale = i % 6 == 0;
            let id = 1_559_212_036 + i as u64 * 97_331;
            let steam = i % 3 == 0;
            MockMod {
                name: (*name).to_string(),
                id,
                local_updated: local,
                size,
                size_human: if size > 1_000_000_000 {
                    format!("{} GB", fixed(size as f64 / 1e9, 1))
                } else {
                    format!("{} MB", fixed(size as f64 / 1e6, 0))
                },
                managed: i % 5 != 0,
                steam,
                path: if steam {
                    format!("{STEAM_ROOT}/steamapps/workshop/content/221100/{id}")
                } else {
                    format!(
                        "{LAUNCHER_DIR}/steamcmd-content/steamapps/workshop/content/221100/{id}"
                    )
                },
                other_copy: i == 3,
                // One the Workshop check has not reached (a hidden or removed item).
                remote_updated: if i == 7 {
                    None
                } else if stale {
                    Some(local + DAY * (4 + i as i64))
                } else {
                    Some(local)
                },
                update_available: stale,
            }
        })
        .collect()
}

fn articles(now: i64) -> Vec<ArticleDto> {
    let now_ms = now * 1000;
    ARTICLE_TITLES
        .iter()
        .enumerate()
        .map(|(i, title)| ArticleDto {
            title: (*title).to_string(),
            slug: format!("article-{i}"),
            excerpt: Some(
                "Survivors, a new update brings changes to vehicles, base building and the economy across every map."
                    .to_string(),
            ),
            date: super::iso(now_ms - i as i64 * DAY * 1000 * 9),
            url: "https://dayz.com/article/updates/stable-update".to_string(),
            image_url: None,
            category: Some(if i % 2 == 1 { "Status Report" } else { "Updates" }.to_string()),
            category_code: Some(if i % 2 == 1 { "status-report" } else { "updates" }.to_string()),
            author: Some("Bohemia Interactive".to_string()),
        })
        .collect()
}

/// Play sessions over the past year: a few servers played often in the
/// evening, an offline map now and then, one running now.
fn play_log(rng: &mut Rng, servers: &[MockServer], now: i64) -> Vec<Session> {
    let mut places: Vec<Session> = servers
        .iter()
        .skip(10)
        .take(6)
        .map(|x| Session {
            kind: SessionKind::Server,
            name: x.name.clone(),
            ip: Some(x.ip.to_string()),
            port: Some(x.game_port),
            map: Some(x.map.clone()),
            start: 0,
            end: 0,
            open: false,
            measured: true,
        })
        .collect();
    places.push(Session {
        kind: SessionKind::Offline,
        name: "DayZCommunityOfflineMode.Enoch".to_string(),
        ip: None,
        port: None,
        map: Some("enoch".to_string()),
        start: 0,
        end: 0,
        open: false,
        measured: true,
    });
    const WEIGHTS: [usize; 10] = [0, 0, 0, 1, 1, 2, 3, 4, 5, 6];

    let mut out = Vec::new();
    let mut t = now - 330 * DAY;
    let mut i = 0;
    while t < now - 2 * 3600 {
        let which = WEIGHTS.get(rng.below(10)).copied().unwrap_or(0);
        let day = t - t.rem_euclid(DAY);
        let start = day + (17 + rng.below(6) as i64) * 3600 + rng.below(3600) as i64;
        let secs = 1200 + (rng.next() * 3.0 * 3600.0).floor() as i64;
        if start + secs < now {
            let measured = i > 6;
            i += 1;
            if let Some(p) = places.get(which) {
                out.push(Session {
                    start,
                    end: if measured { start + secs } else { start },
                    open: false,
                    measured,
                    ..p.clone()
                });
            }
        }
        let days = if rng.next() < 0.3 {
            0.5
        } else {
            1.0 + rng.next() * 3.0
        };
        t += (days * DAY as f64).floor() as i64;
    }
    if let Some(p) = places.first() {
        out.push(Session {
            start: now - 4400,
            end: now,
            open: true,
            measured: true,
            ..p.clone()
        });
    }
    out
}

/// mock.ts' profile, as the real one is stored.
pub(super) fn profile(world: &World) -> Profile {
    let servers = &world.servers;
    let mut p = Profile {
        steam_login: Some("survivor_42".to_string()),
        steam_password: None,
        steamcmd_logged_in: Some("survivor_42".to_string()),
        steam_root: Some(STEAM_ROOT.to_string()),
        steamcmd_enabled: true,
        mod_downloader: ModDownloader::Steamcmd,
        steamcmd_path: None,
        steam_server_list: true,
        player: Some("Survivor".to_string()),
        steam_api_key: Some("XXXXXXXX".to_string()),
        steam_id: Some("76561198000000000".to_string()),
        user_location: Some((2.35, 48.85)),
        favorites: servers
            .iter()
            .skip(3)
            .take(6)
            .enumerate()
            .map(|(i, s)| {
                let mut f = Favorite {
                    name: s.name.clone(),
                    ip: s.ip.to_string(),
                    port: s.query_port,
                    ..Favorite::default()
                };
                // A few notes, labels and groups, for the favourites view.
                match i {
                    0 => {
                        f.group = Some("Main".into());
                        f.labels = vec!["PvE".into(), "Friends".into()];
                        f.note = "Trader at Green Mountain · wipe every first Friday".into();
                    }
                    1 => {
                        f.group = Some("Main".into());
                        f.labels = vec!["Hardcore".into()];
                        f.character = Some("Alt".into());
                        f.launch_args = "-maxMem=4096 -world=empty".into();
                    }
                    2 => {
                        f.group = Some("Weekend".into());
                        f.note = "Event server, Saturdays".into();
                    }
                    _ => {}
                }
                f
            })
            .collect(),
        favorite_labels: vec![
            FavoriteLabel {
                name: "PvE".into(),
                color: LabelColor::Ok,
            },
            FavoriteLabel {
                name: "Hardcore".into(),
                color: LabelColor::Err,
            },
            FavoriteLabel {
                name: "Friends".into(),
                color: LabelColor::Info,
            },
        ],
        favorite_groups: vec![
            FavoriteGroup {
                name: "Main".into(),
                collapsed: false,
            },
            FavoriteGroup {
                name: "Weekend".into(),
                collapsed: false,
            },
        ],
        history: servers
            .iter()
            .skip(10)
            .take(12)
            .enumerate()
            .map(|(i, s)| History {
                name: s.name.clone(),
                ip: s.ip.to_string(),
                port: s.query_port,
                ts: world.now - i as i64 * 7200 - 600,
                mod_ids: None,
                extra: None,
            })
            .collect(),
        excluded_ips: servers
            .get(40)
            .map(|s| s.ip.to_string())
            .into_iter()
            .collect(),
        ping_concurrency: 64,
        ping_timeout_auto: 2000,
        ping_timeout_manual: 10_000,
        ping_max_retries: 0,
        ping_scan_favorites: true,
        ping_scan_history: true,
        ping_scan_servers: true,
        ..Profile::default()
    };
    for (i, (key, description)) in OPTIONS.iter().enumerate() {
        if let Some(opt) = p.options.get_mut(key) {
            opt.enabled = i % 3 == 0;
            opt.description = (*description).to_string();
            opt.value = match *key {
                "max_mem" => Some("8192".to_string()),
                "world" => Some("empty".to_string()),
                _ => None,
            };
        }
    }
    p
}
