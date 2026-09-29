//! DayZ's command-line options, as the profile stores them.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

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

/// Custom deserializer that reads the old bash-script flat JSON format
/// where keys are camelCase (filePathing, doLogs, noPause, etc.)
/// and maps them to our struct fields.
///
/// Uses `LaunchOptions::defaults()` as the single source of truth for
/// descriptions and default enabled/value states — no duplication.
pub(crate) fn deserialize_launch_options<'de, D>(
    deserializer: D,
) -> std::result::Result<LaunchOptions, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, LaunchOption> = HashMap::deserialize(deserializer)?;

    /// Look up a launch option from the deserialized map by trying each alias
    /// key in order, falling back to the provided default from `defaults()`.
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

    // Legacy alias keys: the old bash-script profile used camelCase names,
    // so we accept both snake_case and camelCase variants.
    let d = LaunchOptions::defaults();
    Ok(LaunchOptions {
        window: get_opt(&map, &["window"], d.window),
        noborder: get_opt(&map, &["noborder"], d.noborder),
        nosplash: get_opt(&map, &["nosplash"], d.nosplash),
        skipintro: get_opt(&map, &["skipintro", "skipIntro"], d.skipintro),
        nolauncher: get_opt(&map, &["nolauncher", "noLauncher"], d.nolauncher),
        file_patching: get_opt(
            &map,
            &["file_patching", "filePathing", "filePatching"],
            d.file_patching,
        ),
        do_logs: get_opt(&map, &["do_logs", "doLogs"], d.do_logs),
        buldozer: get_opt(&map, &["buldozer"], d.buldozer),
        winxp: get_opt(&map, &["winxp"], d.winxp),
        high: get_opt(&map, &["high"], d.high),
        world: get_opt(&map, &["world"], d.world),
        no_pause: get_opt(&map, &["no_pause", "noPause"], d.no_pause),
        max_mem: get_opt(&map, &["max_mem", "maxMem"], d.max_mem),
        max_vram: get_opt(&map, &["max_vram", "maxVRAM"], d.max_vram),
        cpu_count: get_opt(&map, &["cpu_count", "cpuCount"], d.cpu_count),
        ex_threads: get_opt(&map, &["ex_threads", "exThreads"], d.ex_threads),
        no_benchmark: get_opt(&map, &["no_benchmark", "noBenchmark"], d.no_benchmark),
        script_debug: get_opt(&map, &["script_debug", "scriptDebug"], d.script_debug),
        profiles: get_opt(&map, &["profiles"], d.profiles),
    })
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

impl LaunchOptions {
    /// Create default launch options matching the bash script defaults.
    /// This is the **single source of truth** for option names, defaults, and descriptions.
    pub fn defaults() -> Self {
        Self {
            window: LaunchOption {
                enabled: false,
                value: None,
                description: "Run in windowed mode".into(),
            },
            noborder: LaunchOption {
                enabled: false,
                value: None,
                description: "Borderless window".into(),
            },
            nosplash: LaunchOption {
                enabled: true,
                value: None,
                description: "Skip splash screen".into(),
            },
            skipintro: LaunchOption {
                enabled: true,
                value: None,
                description: "Skip intro video".into(),
            },
            nolauncher: LaunchOption {
                enabled: true,
                value: None,
                description: "Skip launcher".into(),
            },
            file_patching: LaunchOption {
                enabled: false,
                value: None,
                description: "Enable file patching".into(),
            },
            do_logs: LaunchOption {
                enabled: false,
                value: None,
                description: "Enable logging".into(),
            },
            buldozer: LaunchOption {
                enabled: false,
                value: None,
                description: "Buldozer mode".into(),
            },
            winxp: LaunchOption {
                enabled: false,
                value: None,
                description: "DirectX 9 compatibility".into(),
            },
            high: LaunchOption {
                enabled: true,
                value: None,
                description: "High process priority".into(),
            },
            world: LaunchOption {
                enabled: true,
                value: Some("empty".into()),
                description: "World to load".into(),
            },
            no_pause: LaunchOption {
                enabled: false,
                value: None,
                description: "Don't pause when unfocused".into(),
            },
            max_mem: LaunchOption {
                enabled: false,
                value: None,
                description: "Max memory in MB".into(),
            },
            max_vram: LaunchOption {
                enabled: false,
                value: None,
                description: "Max VRAM in MB".into(),
            },
            cpu_count: LaunchOption {
                enabled: false,
                value: None,
                description: "Number of CPU cores".into(),
            },
            ex_threads: LaunchOption {
                enabled: false,
                value: None,
                description: "Number of threads".into(),
            },
            no_benchmark: LaunchOption {
                enabled: false,
                value: None,
                description: "Disable benchmark".into(),
            },
            script_debug: LaunchOption {
                enabled: false,
                value: None,
                description: "Script debug mode".into(),
            },
            profiles: LaunchOption {
                enabled: false,
                value: None,
                description: "Custom profiles directory".into(),
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

impl LaunchOptions {
    /// The option stored under `key` (the snake_case name the window uses).
    pub fn get_mut(&mut self, key: &str) -> Option<&mut LaunchOption> {
        self.all_options_mut()
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, opt)| opt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_camel_case_keys_are_read() {
        let json = r#"{"skipIntro":{"enabled":false},"maxMem":{"enabled":true,"value":4096}}"#;
        let mut de = serde_json::Deserializer::from_str(json);
        let opts = deserialize_launch_options(&mut de).unwrap();
        assert!(!opts.skipintro.enabled);
        assert_eq!(opts.max_mem.value.as_deref(), Some("4096"));
        // Missing keys fall back to the defaults.
        assert!(opts.nosplash.enabled);
    }

    #[test]
    fn enabled_options_become_args() {
        let mut opts = LaunchOptions::defaults();
        opts.get_mut("max_mem").unwrap().enabled = true;
        opts.get_mut("max_mem").unwrap().value = Some("8192".into());
        let args = opts.to_args();
        assert!(args.contains(&"-maxMem=8192".to_string()));
        assert!(args.contains(&"-world=empty".to_string()));
        assert!(opts.get_mut("nope").is_none());
    }
}
