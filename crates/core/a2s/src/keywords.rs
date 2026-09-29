//! What DayZ packs into A2S_INFO's keywords (the "gametags").
//!
//! A DayZ server answers A2S_INFO with its extended data flag set and a
//! keywords string such as
//! `battleye,no3rd,external,privHive,shard000,lqs0,etm4.000000,entm2.000000,mod,14:09`.
//! The Steam master server hands the same tags back with `#` between them and
//! `-` for the decimal point (`etm4-000000`), so both are accepted.
//! See `docs/a2s-dayz.md` for the sources.

/// The server's settings, as DayZ states them in its keywords.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DayzInfo {
    /// `battleye`: BattlEye anti-cheat is on.
    pub battleye: bool,
    /// `no3rd`: third-person view is off (first person only).
    pub first_person_only: bool,
    /// `external`: a community server; official servers do not carry it.
    pub external: bool,
    /// `privHive`: the server keeps its own characters (private hive).
    pub private_hive: bool,
    /// `mod`: the server runs mods.
    pub modded: bool,
    /// `whitelisting`: only whitelisted players may join.
    pub whitelisted: bool,
    /// `allowedFilePatching`: clients may load unpacked files.
    pub file_patching: bool,
    /// `isDLC`: the server needs a DLC (Livonia, Frostline).
    pub dlc: bool,
    /// `shardNNN`: the hive shard (`000`/`001` for official ones).
    pub shard: Option<String>,
    /// `lqsN`: players waiting in the login queue.
    pub login_queue: Option<u32>,
    /// `etmX`: how much faster than real time the day passes.
    pub time_accel: Option<f32>,
    /// `entmX`: the same, at night.
    pub night_time_accel: Option<f32>,
    /// `HH:MM`: the in-game clock.
    pub game_time: Option<String>,
    /// `portN`: the game port, when the server states it here.
    pub game_port: Option<u16>,
    /// Tags this parser does not know, kept as they came.
    pub unknown: Vec<String>,
}

/// A number as DayZ writes it: `4.000000`, or `4-000000` through the master server.
fn number(s: &str) -> Option<f32> {
    s.replace('-', ".")
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

fn is_clock(tag: &str) -> bool {
    let b = tag.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b[..2].iter().all(u8::is_ascii_digit)
        && b[3..].iter().all(u8::is_ascii_digit)
}

impl DayzInfo {
    /// Read a keywords string. Never fails: what is not understood lands in `unknown`.
    pub fn parse(keywords: &str) -> Self {
        let mut info = DayzInfo::default();
        for tag in keywords
            .split([',', '#'])
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            match tag {
                // "battleeye" is how some tools (and older builds) spell it.
                "battleye" | "battleeye" => info.battleye = true,
                "no3rd" => info.first_person_only = true,
                "external" => info.external = true,
                "privHive" | "privhive" => info.private_hive = true,
                "mod" => info.modded = true,
                "whitelisting" => info.whitelisted = true,
                "allowedFilePatching" => info.file_patching = true,
                "isDLC" => info.dlc = true,
                t if is_clock(t) => info.game_time = Some(t.to_string()),
                t if t.len() > 4 && t.starts_with("entm") => {
                    info.night_time_accel = number(&t[4..]);
                }
                t if t.len() > 3 && t.starts_with("etm") => info.time_accel = number(&t[3..]),
                t if t.len() > 3 && t.starts_with("lqs") => info.login_queue = t[3..].parse().ok(),
                t if t.len() > 5 && t.starts_with("shard") => info.shard = Some(t[5..].to_string()),
                t if t.len() > 4 && t.starts_with("port") => info.game_port = t[4..].parse().ok(),
                t => info.unknown.push(t.to_string()),
            }
        }
        info
    }

    /// An official server: a shard of Bohemia's public hive, not a community one.
    pub fn official(&self) -> bool {
        !self.external && !self.private_hive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_community_server() {
        let i = DayzInfo::parse(
            "battleye,no3rd,external,privHive,shard000,lqs3,etm8.000000,entm2.000000,mod,20:25",
        );
        assert!(i.battleye && i.first_person_only && i.external && i.private_hive && i.modded);
        assert_eq!(i.shard.as_deref(), Some("000"));
        assert_eq!(i.login_queue, Some(3));
        assert_eq!(i.time_accel, Some(8.0));
        assert_eq!(i.night_time_accel, Some(2.0));
        assert_eq!(i.game_time.as_deref(), Some("20:25"));
        assert!(i.unknown.is_empty());
        assert!(!i.official());
    }

    #[test]
    fn the_master_server_spelling() {
        let i = DayzInfo::parse(
            "lqs0#1722#battleye#no3rd#external#privhive#shard123abc#etm6-000000#entm8-000000#mod",
        );
        assert_eq!(i.login_queue, Some(0));
        assert_eq!(i.time_accel, Some(6.0));
        assert_eq!(i.night_time_accel, Some(8.0));
        assert_eq!(i.shard.as_deref(), Some("123abc"));
        assert!(i.private_hive);
        assert_eq!(i.unknown, vec!["1722".to_string()]);
    }

    #[test]
    fn an_official_server_and_the_rarer_flags() {
        let i = DayzInfo::parse(
            "battleeye,shard001,lqs0,etm4.200000,entm4.000000,14:09,isDLC,whitelisting,allowedFilePatching,port2302",
        );
        assert!(i.battleye && i.dlc && i.whitelisted && i.file_patching);
        assert!(!i.first_person_only && i.official());
        assert_eq!(i.game_port, Some(2302));
        assert_eq!(i.time_accel, Some(4.2));
    }

    #[test]
    fn nonsense_is_kept_not_fatal() {
        let i = DayzInfo::parse(",,etm,lqsX,shard,entmfoo, 99:99x ,");
        assert_eq!(i.time_accel, None);
        assert_eq!(i.login_queue, None);
        assert!(i.unknown.contains(&"etm".to_string()));
        assert!(i.unknown.contains(&"shard".to_string()));
    }
}
