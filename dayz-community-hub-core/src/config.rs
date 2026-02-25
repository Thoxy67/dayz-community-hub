use crate::Result;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const APP_NAME: &str = "dayz-community-hub";
const APP_VERSION: &str = "0.1.0";

/// Returns the platform-appropriate data directory.
/// - Linux/macOS: `~/.local/share/dayz-community-hub/`
/// - Windows:     `%APPDATA%\dayz-community-hub\`
pub fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| {
            std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".to_string())
        });
        PathBuf::from(appdata).join(APP_NAME)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(APP_NAME)
    }
}

/// Returns the default profile path: `~/.local/share/dayz-community-hub/profile.json`
pub fn default_profile_path() -> PathBuf {
    default_data_dir().join("profile.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub steam_login: Option<String>,
    /// Steam account password — stored in plaintext so steamcmd can log in
    /// non-interactively without relying on cached credentials.
    #[serde(default)]
    pub steam_password: Option<String>,
    #[serde(default)]
    pub steam_root: Option<String>,
    #[serde(default = "default_steamcmd_enabled")]
    pub steamcmd_enabled: bool,
    pub player: Option<String>,
    /// Steam Web API key — used to fetch player avatar via GetPlayerSummaries.
    #[serde(default)]
    pub steam_api_key: Option<String>,
    /// Steam 64-bit account ID — used with the API key to resolve the avatar.
    #[serde(default)]
    pub steam_id: Option<String>,
    /// BattleMetrics personal access token — used to fetch server rank, uptime and player history.
    #[serde(default)]
    pub battlemetrics_api_key: Option<String>,
    /// Optional explicit path to steamcmd binary (overrides auto-detection).
    #[serde(default)]
    pub steamcmd_path: Option<String>,
    pub favorites: Vec<Favorite>,
    pub history: Vec<History>,
    /// IPs excluded from the server browser (persisted across restarts).
    #[serde(default)]
    pub excluded_ips: Vec<String>,
    #[serde(
        deserialize_with = "deserialize_launch_options",
        default = "LaunchOptions::defaults"
    )]
    pub options: LaunchOptions,
    pub version: String,
    #[serde(skip)]
    pub path: PathBuf,
}

fn default_steamcmd_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Favorite {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub ts: i64,
    // Old profiles may have extra fields like "mods" -- ignore them
    #[serde(flatten)]
    pub extra: Option<serde_json::Value>,
}

impl History {
    /// Returns a human-readable relative time string (e.g., "2 hours ago")
    pub fn relative_time(&self) -> String {
        crate::utils::format_relative_time(self.ts)
    }

    /// Returns a formatted absolute time string
    pub fn absolute_time(&self) -> String {
        crate::utils::format_absolute_time(self.ts)
    }
}

/// A single launch option with enabled flag, optional value, and description.
#[derive(Debug, Clone, Serialize)]
pub struct LaunchOption {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub description: String,
}

/// Custom deserializer for LaunchOption that handles the old profile format
/// where `value` could be a bool, number, string, or null.
impl<'de> Deserialize<'de> for LaunchOption {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawOption {
            enabled: bool,
            #[serde(default)]
            value: Option<serde_json::Value>,
            #[serde(default)]
            description: Option<String>,
        }

        let raw = RawOption::deserialize(deserializer)?;
        let value = raw.value.and_then(|v| match v {
            serde_json::Value::String(s) => Some(s),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            serde_json::Value::Null => None,
            other => Some(other.to_string()),
        });

        Ok(LaunchOption {
            enabled: raw.enabled,
            value,
            description: raw.description.unwrap_or_default(),
        })
    }
}

/// Launch options. Deserialized from the old bash-script flat format
/// or from our own struct format.
#[derive(Debug, Clone, Serialize)]
pub struct LaunchOptions {
    pub window: LaunchOption,
    pub noborder: LaunchOption,
    pub nosplash: LaunchOption,
    pub skipintro: LaunchOption,
    pub nolauncher: LaunchOption,
    pub file_patching: LaunchOption,
    pub do_logs: LaunchOption,
    pub buldozer: LaunchOption,
    pub winxp: LaunchOption,
    pub high: LaunchOption,
    pub world: LaunchOption,
    pub no_pause: LaunchOption,
    pub max_mem: LaunchOption,
    pub max_vram: LaunchOption,
    pub cpu_count: LaunchOption,
    pub ex_threads: LaunchOption,
    pub no_benchmark: LaunchOption,
    pub script_debug: LaunchOption,
    pub profiles: LaunchOption,
}

/// Custom deserializer that reads the old bash-script flat JSON format
/// where keys are camelCase (filePathing, doLogs, noPause, etc.)
/// and maps them to our struct fields.
fn deserialize_launch_options<'de, D>(
    deserializer: D,
) -> std::result::Result<LaunchOptions, D::Error>
where
    D: Deserializer<'de>,
{
    use std::collections::HashMap;

    // Deserialize as a map of option_name -> LaunchOption
    let map: HashMap<String, LaunchOption> = HashMap::deserialize(deserializer)?;

    fn get_opt(
        map: &HashMap<String, LaunchOption>,
        keys: &[&str],
        default: LaunchOption,
    ) -> LaunchOption {
        for key in keys {
            if let Some(opt) = map.get(*key) {
                return opt.clone();
            }
        }
        default
    }

    fn default_off(desc: &str) -> LaunchOption {
        LaunchOption {
            enabled: false,
            value: None,
            description: desc.to_string(),
        }
    }
    fn default_on(desc: &str) -> LaunchOption {
        LaunchOption {
            enabled: true,
            value: None,
            description: desc.to_string(),
        }
    }

    Ok(LaunchOptions {
        window: get_opt(&map, &["window"], default_off("Run in windowed mode")),
        noborder: get_opt(&map, &["noborder"], default_off("Borderless window")),
        nosplash: get_opt(&map, &["nosplash"], default_on("Skip splash screen")),
        skipintro: get_opt(
            &map,
            &["skipintro", "skipIntro"],
            default_on("Skip intro video"),
        ),
        nolauncher: get_opt(
            &map,
            &["nolauncher", "noLauncher"],
            default_on("Skip launcher"),
        ),
        file_patching: get_opt(
            &map,
            &["file_patching", "filePathing", "filePatching"],
            default_off("Enable file patching"),
        ),
        do_logs: get_opt(&map, &["do_logs", "doLogs"], default_off("Enable logging")),
        buldozer: get_opt(&map, &["buldozer"], default_off("Buldozer mode")),
        winxp: get_opt(&map, &["winxp"], default_off("DirectX 9 compatibility")),
        high: get_opt(&map, &["high"], default_on("High process priority")),
        world: get_opt(
            &map,
            &["world"],
            LaunchOption {
                enabled: true,
                value: Some("empty".to_string()),
                description: "World to load".to_string(),
            },
        ),
        no_pause: get_opt(
            &map,
            &["no_pause", "noPause"],
            default_off("Don't pause when unfocused"),
        ),
        max_mem: get_opt(
            &map,
            &["max_mem", "maxMem"],
            default_off("Max memory in MB"),
        ),
        max_vram: get_opt(
            &map,
            &["max_vram", "maxVRAM"],
            default_off("Max VRAM in MB"),
        ),
        cpu_count: get_opt(
            &map,
            &["cpu_count", "cpuCount"],
            default_off("Number of CPU cores"),
        ),
        ex_threads: get_opt(
            &map,
            &["ex_threads", "exThreads"],
            default_off("Number of threads"),
        ),
        no_benchmark: get_opt(
            &map,
            &["no_benchmark", "noBenchmark"],
            default_off("Disable benchmark"),
        ),
        script_debug: get_opt(
            &map,
            &["script_debug", "scriptDebug"],
            default_off("Script debug mode"),
        ),
        profiles: get_opt(
            &map,
            &["profiles"],
            default_off("Custom profiles directory"),
        ),
    })
}

impl LaunchOptions {
    /// Create default launch options matching the bash script defaults.
    pub fn defaults() -> Self {
        Self {
            window: LaunchOption {
                enabled: false,
                value: None,
                description: "Run in windowed mode".to_string(),
            },
            noborder: LaunchOption {
                enabled: false,
                value: None,
                description: "Borderless window".to_string(),
            },
            nosplash: LaunchOption {
                enabled: true,
                value: None,
                description: "Skip splash screen".to_string(),
            },
            skipintro: LaunchOption {
                enabled: true,
                value: None,
                description: "Skip intro video".to_string(),
            },
            nolauncher: LaunchOption {
                enabled: true,
                value: None,
                description: "Skip launcher".to_string(),
            },
            file_patching: LaunchOption {
                enabled: false,
                value: None,
                description: "Enable file patching".to_string(),
            },
            do_logs: LaunchOption {
                enabled: false,
                value: None,
                description: "Enable logging".to_string(),
            },
            buldozer: LaunchOption {
                enabled: false,
                value: None,
                description: "Buldozer mode".to_string(),
            },
            winxp: LaunchOption {
                enabled: false,
                value: None,
                description: "DirectX 9 compatibility".to_string(),
            },
            high: LaunchOption {
                enabled: true,
                value: None,
                description: "High process priority".to_string(),
            },
            world: LaunchOption {
                enabled: true,
                value: Some("empty".to_string()),
                description: "World to load (empty = no world at start)".to_string(),
            },
            no_pause: LaunchOption {
                enabled: false,
                value: None,
                description: "Don't pause when unfocused".to_string(),
            },
            max_mem: LaunchOption {
                enabled: false,
                value: None,
                description: "Max memory in MB".to_string(),
            },
            max_vram: LaunchOption {
                enabled: false,
                value: None,
                description: "Max VRAM in MB".to_string(),
            },
            cpu_count: LaunchOption {
                enabled: false,
                value: None,
                description: "Number of CPU cores to use".to_string(),
            },
            ex_threads: LaunchOption {
                enabled: false,
                value: None,
                description: "Number of threads".to_string(),
            },
            no_benchmark: LaunchOption {
                enabled: false,
                value: None,
                description: "Disable benchmark on startup".to_string(),
            },
            script_debug: LaunchOption {
                enabled: false,
                value: None,
                description: "Script debug mode".to_string(),
            },
            profiles: LaunchOption {
                enabled: false,
                value: None,
                description: "Custom profiles directory path".to_string(),
            },
        }
    }

    /// Convert enabled options to command-line arguments.
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        let all_options: Vec<(&str, &LaunchOption)> = vec![
            ("-window", &self.window),
            ("-noborder", &self.noborder),
            ("-nosplash", &self.nosplash),
            ("-skipIntro", &self.skipintro),
            ("-nolauncher", &self.nolauncher),
            ("-filePatching", &self.file_patching),
            ("-doLogs", &self.do_logs),
            ("-buldozer", &self.buldozer),
            ("-winxp", &self.winxp),
            ("-high", &self.high),
            ("-world", &self.world),
            ("-noPause", &self.no_pause),
            ("-maxMem", &self.max_mem),
            ("-maxVRAM", &self.max_vram),
            ("-cpuCount", &self.cpu_count),
            ("-exThreads", &self.ex_threads),
            ("-noBenchmark", &self.no_benchmark),
            ("-scriptDebug", &self.script_debug),
            ("-profiles", &self.profiles),
        ];

        for (flag, opt) in all_options {
            if opt.enabled {
                if let Some(ref value) = opt.value {
                    args.push(format!("{}={}", flag, value));
                } else {
                    args.push(flag.to_string());
                }
            }
        }

        args
    }

    /// Get all options as mutable references for the options editor.
    pub fn all_options_mut(&mut self) -> Vec<(&str, &mut LaunchOption)> {
        vec![
            ("window", &mut self.window),
            ("noborder", &mut self.noborder),
            ("nosplash", &mut self.nosplash),
            ("skipintro", &mut self.skipintro),
            ("nolauncher", &mut self.nolauncher),
            ("file_patching", &mut self.file_patching),
            ("do_logs", &mut self.do_logs),
            ("buldozer", &mut self.buldozer),
            ("winxp", &mut self.winxp),
            ("high", &mut self.high),
            ("world", &mut self.world),
            ("no_pause", &mut self.no_pause),
            ("max_mem", &mut self.max_mem),
            ("max_vram", &mut self.max_vram),
            ("cpu_count", &mut self.cpu_count),
            ("ex_threads", &mut self.ex_threads),
            ("no_benchmark", &mut self.no_benchmark),
            ("script_debug", &mut self.script_debug),
            ("profiles", &mut self.profiles),
        ]
    }

    /// Get all options as immutable references for display.
    pub fn all_options(&self) -> Vec<(&str, &LaunchOption)> {
        vec![
            ("window", &self.window),
            ("noborder", &self.noborder),
            ("nosplash", &self.nosplash),
            ("skipintro", &self.skipintro),
            ("nolauncher", &self.nolauncher),
            ("file_patching", &self.file_patching),
            ("do_logs", &self.do_logs),
            ("buldozer", &self.buldozer),
            ("winxp", &self.winxp),
            ("high", &self.high),
            ("world", &self.world),
            ("no_pause", &self.no_pause),
            ("max_mem", &self.max_mem),
            ("max_vram", &self.max_vram),
            ("cpu_count", &self.cpu_count),
            ("ex_threads", &self.ex_threads),
            ("no_benchmark", &self.no_benchmark),
            ("script_debug", &self.script_debug),
            ("profiles", &self.profiles),
        ]
    }
}

impl Profile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            // Create default profile
            let mut profile = Self::default_with_version(APP_VERSION);
            profile.path = path.to_path_buf();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            profile.save()?;
            return Ok(profile);
        }
        let data = std::fs::read_to_string(path)?;
        let mut profile: Profile = serde_json::from_str(&data)?;
        profile.path = path.to_path_buf();
        Ok(profile)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.path, data)?;
        Ok(())
    }

    pub fn default_with_version(version: &str) -> Self {
        Self {
            steam_login: None,
            steam_password: None,
            steam_root: None,
            steamcmd_enabled: true,
            steamcmd_path: None,
            player: None,
            steam_api_key: None,
            steam_id: None,
            battlemetrics_api_key: None,
            favorites: Vec::new(),
            history: Vec::new(),
            excluded_ips: Vec::new(),
            options: LaunchOptions::defaults(),
            version: version.to_string(),
            path: PathBuf::new(),
        }
    }

    pub fn add_favorite(&mut self, name: String, ip: String, port: u16) {
        if !self.favorites.iter().any(|f| f.ip == ip && f.port == port) {
            self.favorites.push(Favorite { name, ip, port });
        }
    }

    pub fn remove_favorite(&mut self, ip: &str, port: u16) {
        self.favorites.retain(|f| f.ip != ip || f.port != port);
    }

    pub fn remove_favorite_by_name(&mut self, name: &str) {
        self.favorites.retain(|f| f.name != name);
    }

    pub fn is_favorite(&self, ip: &str, port: u16) -> bool {
        self.favorites.iter().any(|f| f.ip == ip && f.port == port)
    }

    pub fn add_history(&mut self, name: String, ip: String, port: u16) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        self.history.retain(|h| h.ip != ip || h.port != port);
        self.history.insert(
            0,
            History {
                name,
                ip,
                port,
                ts,
                extra: None,
            },
        );

        // No cap — history is unlimited.
    }

    /// Add an IP to the excluded list (no-op if already present).
    pub fn add_excluded_ip(&mut self, ip: String) {
        if !self.excluded_ips.contains(&ip) {
            self.excluded_ips.push(ip);
        }
    }

    /// Remove an IP from the excluded list.
    pub fn remove_excluded_ip(&mut self, ip: &str) {
        self.excluded_ips.retain(|e| e != ip);
    }

    /// Returns true if this IP is excluded.
    pub fn is_excluded(&self, ip: &str) -> bool {
        self.excluded_ips.iter().any(|e| e == ip)
    }

    pub fn get_favorites_with_status<'a>(
        &'a self,
        servers: &'a [crate::api::Server],
    ) -> Vec<FavoriteStatus<'a>> {
        // Build O(1) lookup maps keyed by both query port and game port so each
        // favorite look-up is O(1) instead of O(n) (was O(n×m) overall).
        let mut by_query: HashMap<(&str, u16), &crate::api::Server> =
            HashMap::with_capacity(servers.len());
        let mut by_game: HashMap<(&str, u16), &crate::api::Server> =
            HashMap::with_capacity(servers.len());
        for s in servers {
            by_query.insert((s.endpoint.ip.as_str(), s.endpoint.port as u16), s);
            by_game.insert((s.endpoint.ip.as_str(), s.game_port as u16), s);
        }

        self.favorites
            .iter()
            .map(|favorite| {
                let server = by_query
                    .get(&(favorite.ip.as_str(), favorite.port))
                    .or_else(|| by_game.get(&(favorite.ip.as_str(), favorite.port)))
                    .copied();
                FavoriteStatus { favorite, server }
            })
            .collect()
    }
}

pub struct FavoriteStatus<'a> {
    pub favorite: &'a Favorite,
    pub server: Option<&'a crate::api::Server>,
}
