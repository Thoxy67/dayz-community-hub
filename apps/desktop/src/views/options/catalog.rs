//! What each launch option is (= `features/options/catalog.ts` and `steam-options.ts`):
//! its flag, its group, its icon, the words that name it and whether it takes a value;
//! what the hardware suggests; Steam's launch options taken apart. The profile only
//! carries keys, on/off and a value; everything a person reads comes from here.

use dz_hub::features::system::SystemSpecsDto;
use egui::Color32;

use crate::i18n::t;
use crate::icons::{Icon, lucide};
use crate::theme::Tokens;

/// How a value is typed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    None,
    /// Megabytes: a number, "MB" inside the field.
    Mb,
    Count,
    /// `-exThreads`' bitmask.
    Threads,
    Text,
    Folder,
}

impl ValueKind {
    /// `type="number"` in the Svelte field.
    pub fn numeric(self) -> bool {
        matches!(self, ValueKind::Mb | ValueKind::Count | ValueKind::Threads)
    }

    /// `w-40` (text, folder) or `w-24`.
    pub fn width(self) -> f32 {
        if matches!(self, ValueKind::Text | ValueKind::Folder) {
            160.0
        } else {
            96.0
        }
    }
}

pub struct Meta {
    pub key: &'static str,
    pub flag: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    pub icon: Icon,
    pub value: ValueKind,
}

const fn m(
    key: &'static str,
    flag: &'static str,
    label: &'static str,
    desc: &'static str,
    icon: Icon,
    value: ValueKind,
) -> Meta {
    Meta {
        key,
        flag,
        label,
        desc,
        icon,
        value,
    }
}

use ValueKind as V;

pub const META: &[Meta] = &[
    m(
        "window",
        "-window",
        "labelWindowed",
        "descWindow",
        lucide::APP_WINDOW,
        V::None,
    ),
    m(
        "noborder",
        "-noborder",
        "labelBorderless",
        "descNoborder",
        lucide::FRAME,
        V::None,
    ),
    m(
        "nosplash",
        "-nosplash",
        "labelNosplash",
        "descNosplash",
        lucide::IMAGE_OFF,
        V::None,
    ),
    m(
        "skipintro",
        "-skipIntro",
        "labelSkipintro",
        "descSkipintro",
        lucide::SKIP_FORWARD,
        V::None,
    ),
    m(
        "nolauncher",
        "-nolauncher",
        "labelNolauncher",
        "descNolauncher",
        lucide::SQUARE_ARROW_OUT_UP_RIGHT,
        V::None,
    ),
    m(
        "nocharmenu",
        "-nocharmenu",
        "labelNocharmenu",
        "descNocharmenu",
        lucide::USER_ROUND_X,
        V::None,
    ),
    m(
        "high",
        "-high",
        "labelHigh",
        "descHigh",
        lucide::ARROW_UP_NARROW_WIDE,
        V::None,
    ),
    m(
        "max_mem",
        "-maxMem",
        "labelMaxMem",
        "descMaxMem",
        lucide::MEMORY_STICK,
        V::Mb,
    ),
    m(
        "max_vram",
        "-maxVRAM",
        "labelMaxVram",
        "descMaxVram",
        lucide::MONITOR_COG,
        V::Mb,
    ),
    m(
        "cpu_count",
        "-cpuCount",
        "labelCpuCount",
        "descCpuCount",
        lucide::CPU,
        V::Count,
    ),
    m(
        "ex_threads",
        "-exThreads",
        "labelExThreads",
        "descExThreads",
        lucide::LAYERS,
        V::Threads,
    ),
    m(
        "no_benchmark",
        "-noBenchmark",
        "labelNoBenchmark",
        "descNoBenchmark",
        lucide::CHART_NO_AXES_COLUMN,
        V::None,
    ),
    m(
        "world",
        "-world",
        "labelWorld",
        "descWorld",
        lucide::MAP,
        V::Text,
    ),
    m(
        "no_pause",
        "-noPause",
        "labelNoPause",
        "descNoPause",
        lucide::CIRCLE_PAUSE,
        V::None,
    ),
    m(
        "file_patching",
        "-filePatching",
        "labelFilePatching",
        "descFilePatching",
        lucide::FILE_CODE,
        V::None,
    ),
    m(
        "do_logs",
        "-doLogs",
        "labelDoLogs",
        "descDoLogs",
        lucide::SCROLL_TEXT,
        V::None,
    ),
    m(
        "script_debug",
        "-scriptDebug",
        "labelScriptDebug",
        "descScriptDebug",
        lucide::BUG,
        V::Text,
    ),
    m(
        "buldozer",
        "-buldozer",
        "labelBuldozer",
        "descBuldozer",
        lucide::CONSTRUCTION,
        V::None,
    ),
    m(
        "winxp",
        "-winxp",
        "labelWinxp",
        "descWinxp",
        lucide::TV,
        V::None,
    ),
    m(
        "profiles",
        "-profiles",
        "labelProfiles",
        "descProfiles",
        lucide::FOLDER_OPEN,
        V::Folder,
    ),
];

pub fn meta(key: &str) -> Option<&'static Meta> {
    META.iter().find(|m| m.key == key)
}

/// A group's colour, named as the Svelte `tone` classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupTone {
    Accent,
    Info,
    Ok,
    Map,
    Mods,
    Muted,
}

impl GroupTone {
    pub fn color(self, t: &Tokens) -> Color32 {
        match self {
            GroupTone::Accent => t.accent,
            GroupTone::Info => t.info,
            GroupTone::Ok => t.ok,
            GroupTone::Map => t.map,
            GroupTone::Mods => t.mods,
            GroupTone::Muted => t.fg_muted,
        }
    }
}

pub struct Group {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: Icon,
    pub tone: GroupTone,
    pub keys: &'static [&'static str],
}

pub const GROUPS: &[Group] = &[
    Group {
        id: "performance",
        label: "groupPerformance",
        icon: lucide::GAUGE,
        tone: GroupTone::Accent,
        keys: &[
            "high",
            "max_mem",
            "max_vram",
            "cpu_count",
            "ex_threads",
            "no_benchmark",
        ],
    },
    Group {
        id: "window",
        label: "groupWindow",
        icon: lucide::MONITOR,
        tone: GroupTone::Info,
        keys: &["window", "noborder"],
    },
    Group {
        id: "startup",
        label: "groupStartup",
        icon: lucide::ROCKET,
        tone: GroupTone::Ok,
        keys: &["nosplash", "skipintro", "nolauncher", "nocharmenu"],
    },
    Group {
        id: "world",
        label: "groupWorld",
        icon: lucide::GLOBE,
        tone: GroupTone::Map,
        keys: &["world", "no_pause"],
    },
    Group {
        id: "developer",
        label: "groupDeveloper",
        icon: lucide::CODE_XML,
        tone: GroupTone::Mods,
        keys: &[
            "file_patching",
            "do_logs",
            "script_debug",
            "buldozer",
            "winxp",
            "profiles",
        ],
    },
];

/// Options the catalog does not list (a newer backend).
pub const OTHER: Group = Group {
    id: "other",
    label: "groupOther",
    icon: lucide::SLIDERS_HORIZONTAL,
    tone: GroupTone::Muted,
    keys: &[],
};

/// Whether a group of the catalog lists `key`.
pub fn listed(key: &str) -> bool {
    GROUPS.iter().any(|g| g.keys.contains(&key))
}

/// The flag as DayZ receives it: `-maxMem=8192`, or `-window`.
pub fn flag_text(key: &str, value: Option<&str>) -> String {
    let flag = match meta(key) {
        Some(m) => m.flag.to_string(),
        None => format!("-{key}"),
    };
    match value {
        Some(v) if !v.is_empty() => format!("{flag}={v}"),
        _ => flag,
    }
}

/// The option's name in the current language.
pub fn label(key: &str) -> &str {
    match meta(key) {
        Some(m) => t("options", m.label),
        None => key,
    }
}

/// What the `-exThreads` bitmask means.
pub fn threads_meaning(draft: &str) -> String {
    let Some(n) = parse_int(draft) else {
        return String::new();
    };
    match n {
        0 => t("options", "threadsNone").to_string(),
        7 => t("options", "threadsAll").to_string(),
        1 => t("options", "threadsFile").to_string(),
        n if n < 0 => format!("-0b{:03b}", n.unsigned_abs()),
        n => format!("0b{n:03b}"),
    }
}

/// `parseInt(s, 10)`: leading blanks, a sign, then digits (the rest ignored).
fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim_start();
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let digits: String = rest
        .chars()
        .take_while(char::is_ascii_digit)
        .take(18)
        .collect();
    let n: i64 = digits.parse().ok()?;
    Some(if neg { -n } else { n })
}

/// A tuned flag: the option and, when it takes one, its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rec {
    pub key: &'static str,
    pub value: Option<String>,
}

/// Tuned flags for the detected hardware: physical cores for -cpuCount (the engine favours
/// physical over logical), the three worker threads (7 = file + geometry + texture) on four
/// cores or more, about 60 % of memory for -maxMem clamped to 2–16 GB, and the two broadly
/// safe wins.
pub fn recommend(s: &SystemSpecsDto) -> Vec<Rec> {
    let cpu = s.physical_cores.min(s.logical_cores).max(1);
    let ex = if cpu >= 4 {
        "7"
    } else if cpu >= 2 {
        "3"
    } else {
        "1"
    };
    let mem = ((s.total_memory_mb as f64 * 0.6) / 256.0).round() * 256.0;
    let mem = mem.clamp(2048.0, 16384.0) as u64;
    vec![
        Rec {
            key: "cpu_count",
            value: Some(cpu.to_string()),
        },
        Rec {
            key: "ex_threads",
            value: Some(ex.to_string()),
        },
        Rec {
            key: "max_mem",
            value: Some(mem.to_string()),
        },
        Rec {
            key: "high",
            value: None,
        },
        Rec {
            key: "no_benchmark",
            value: None,
        },
    ]
}

/// Steam's launch options for a game, taken apart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SteamOptions {
    /// `NAME=value` set before the game: Proton, DXVK/VKD3D, Mesa switches.
    pub env: Vec<(String, String)>,
    /// Programs the game runs inside (`mangohud`, `gamemoderun`…).
    pub wrappers: Vec<String>,
    /// Arguments after the game, given to DayZ itself.
    pub args: Vec<String>,
    /// `%command%` was written: without it Steam adds everything after the game.
    pub has_command: bool,
}

/// Split like a shell would: spaces separate, quotes group.
fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in s.chars() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                cur.push(c);
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
            any = true;
        } else if c.is_whitespace() {
            if !cur.is_empty() || any {
                out.push(std::mem::take(&mut cur));
            }
            any = false;
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() || any {
        out.push(cur);
    }
    out
}

/// `NAME=value` with a shell variable name.
fn env_pair(t: &str) -> Option<(String, String)> {
    let (k, v) = t.split_once('=')?;
    let mut chars = k.chars();
    let first = chars.next()?;
    let ok = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| (k.to_string(), v.to_string()))
}

pub fn parse_steam_options(raw: &str) -> SteamOptions {
    let w = words(raw.trim());
    let Some(at) = w.iter().position(|x| x == "%command%") else {
        return SteamOptions {
            args: w,
            ..Default::default()
        };
    };
    let mut env = Vec::new();
    let mut wrappers = Vec::new();
    for t in w.iter().take(at) {
        match env_pair(t).filter(|_| wrappers.is_empty()) {
            Some(p) => env.push(p),
            None => wrappers.push(t.clone()),
        }
    }
    SteamOptions {
        env,
        wrappers,
        args: w.get(at + 1..).map(<[String]>::to_vec).unwrap_or_default(),
        has_command: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs(physical: u32, logical: u32, mb: u64) -> SystemSpecsDto {
        SystemSpecsDto {
            logical_cores: logical,
            physical_cores: physical,
            total_memory_mb: mb,
            cpu_name: None,
            os: None,
            gpus: Vec::new(),
        }
    }

    #[test]
    fn recommendations_follow_the_hardware() {
        let r = recommend(&specs(8, 16, 32_768));
        assert_eq!(r[0].value.as_deref(), Some("8"));
        assert_eq!(r[1].value.as_deref(), Some("7"));
        assert_eq!(r[2].value.as_deref(), Some("16384"));
        assert_eq!(r[3].value, None);
        let r = recommend(&specs(2, 4, 8192));
        assert_eq!(r[1].value.as_deref(), Some("3"));
        assert_eq!(r[2].value.as_deref(), Some("4864"));
        let r = recommend(&specs(0, 0, 1024));
        assert_eq!(r[0].value.as_deref(), Some("1"));
        assert_eq!(r[1].value.as_deref(), Some("1"));
        assert_eq!(r[2].value.as_deref(), Some("2048"));
    }

    #[test]
    fn flags_are_written_as_dayz_reads_them() {
        assert_eq!(flag_text("max_mem", Some("8192")), "-maxMem=8192");
        assert_eq!(flag_text("window", None), "-window");
        assert_eq!(flag_text("window", Some("")), "-window");
        assert_eq!(flag_text("newthing", Some("1")), "-newthing=1");
        assert_eq!(META.len(), 20);
        assert!(
            GROUPS
                .iter()
                .flat_map(|g| g.keys)
                .all(|k| meta(k).is_some())
        );
    }

    #[test]
    fn threads_meaning_reads_the_bitmask() {
        assert_eq!(threads_meaning("5"), "0b101");
        assert_eq!(threads_meaning("2x"), "0b010");
        assert_eq!(threads_meaning(""), "");
        assert_eq!(threads_meaning("abc"), "");
    }

    #[test]
    fn steam_options_are_taken_apart() {
        let s = parse_steam_options(
            "PROTON_USE_NTSYNC=1 RADV_PERFTEST='gpl,nggc' mangohud FOO=bar %command% -nolauncher",
        );
        assert_eq!(
            s.env,
            vec![
                ("PROTON_USE_NTSYNC".into(), "1".into()),
                ("RADV_PERFTEST".into(), "gpl,nggc".into())
            ]
        );
        assert_eq!(
            s.wrappers,
            vec!["mangohud".to_string(), "FOO=bar".to_string()]
        );
        assert_eq!(s.args, vec!["-nolauncher".to_string()]);
        assert!(s.has_command);
        let s = parse_steam_options("-nosplash \"\" -window");
        assert!(!s.has_command);
        assert_eq!(s.args, vec!["-nosplash", "", "-window"]);
        assert_eq!(parse_steam_options("   "), SteamOptions::default());
    }
}
