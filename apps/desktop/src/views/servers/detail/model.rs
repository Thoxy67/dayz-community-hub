//! What the panel knows about one server (= `detail/model.svelte.ts`), gathered once a
//! frame from the stores so every part reads the same values: the listed row (borrowed
//! from the hub's snapshot, with its mods: no fetch), the live A2S answer, the mods it
//! runs against what is installed, DayZ Metrics' long view. Nothing here computes over
//! more than this one server.

use std::sync::Arc;

use dz_hub::features::a2s::{A2sDetailsDto, A2sPlayerDto};
use dz_hub::features::metrics::ServerMetricsDto;

use crate::app::Ctx;
use crate::parts::{ModRef, ModState};
use crate::store::server_data::Entry;

/// DayZ Metrics' verdict on the player count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Population {
    Fake,
    Suspect,
    Ok,
}

/// The listed row's facts, copied (one server).
#[derive(Clone, Debug, Default)]
pub struct Listed {
    pub name: String,
    pub map: String,
    pub version: String,
    pub environment: String,
    pub time: String,
    pub password: bool,
    pub first_person: bool,
    pub battleye: Option<bool>,
    pub vac: bool,
    pub official: bool,
    pub mimics_official: bool,
    pub mods_count: usize,
    pub mods_missing: Option<u32>,
    pub mods_stale: u32,
    pub excluded: bool,
    pub players: i64,
    pub max: i64,
    pub bots: u32,
}

/// The head-count: players, slots, bots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub players: i64,
    pub max: i64,
    pub bots: u32,
}

pub struct Model {
    pub ip: String,
    pub query_port: i64,
    pub game_port: i64,
    /// The A2S cache key ("ip:query_port").
    pub key: String,
    pub title: String,
    pub map: String,
    /// The server's own name, if known (not the address the title falls back to).
    pub search_name: String,
    pub listed: Option<Listed>,
    pub a2s: Option<Arc<A2sDetailsDto>>,
    pub live_loading: bool,
    pub live_error: Option<String>,
    pub count: Option<Count>,
    /// Missing first, then behind, then the rest (`ModRef::sort`).
    pub mod_rows: Vec<ModRef>,
    /// The mods' ids in the server's load order.
    pub mod_ids: Vec<u64>,
    pub missing: usize,
    pub stale: usize,
    pub installed: usize,
    pub mods_count: usize,
    pub metrics: Entry<ServerMetricsDto>,
    pub population: Option<Population>,
    /// Longest on the server first.
    pub players: Vec<A2sPlayerDto>,
    pub sessions: usize,
    pub last_played: Option<i64>,
    pub favorite: bool,
    pub excluded: bool,
    pub ping: Option<u32>,
    pub ping_pending: bool,
    /// "ip:game port" and the `dzch://` link, written once per model.
    address: String,
    share: String,
}

/// What [`Model::of`] reads, as a cheap key (a few lookups, one formatted address, no
/// copy): the list (its generation) and this server's own words in it (live counts, ping,
/// flags), its A2S answer and DayZ Metrics entry, its ping asked on request, the
/// profile's favourites and history, the mods on disk, and this server's state that
/// actions change at once (a query or a ping asked, an address excluded). Nothing global
/// that moves with every hub event or every ping elsewhere: during a scan the panel is
/// made again only when its own server changes.
pub fn key(cx: &Ctx, ip: &str, port: i64, name: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let s = &cx.stores;
    let mut h = rustc_hash::FxHasher::default();
    let row = s.servers.row_of(ip, port);
    (
        ip,
        port,
        name,
        s.servers.snapshot().generation(),
        row.map(|r| r.state_bits()),
        s.profile.stamp(),
        s.profile.is_excluded(ip),
    )
        .hash(&mut h);
    let qp = row.map_or(port, |r| i64::from(r.query_port()));
    let key = format!("{ip}:{qp}");
    let entry = s.server_data.a2s_entry(&key);
    if let Some(e) = entry {
        (
            e.data.as_ref().map(|d| Arc::as_ptr(d) as usize),
            e.loading,
            &e.error,
        )
            .hash(&mut h);
    }
    // The ports `of` works out, the same way: the metrics and the ping are read with them.
    let a2s = entry.and_then(|e| e.data.as_deref());
    let query_port = row
        .map(|r| i64::from(r.query_port()))
        .or(a2s.map(|a| a.query_port))
        .unwrap_or(port);
    let game_port = row
        .map(|r| i64::from(r.game_port()))
        .or(a2s.and_then(|a| a.game_port))
        .unwrap_or(port);
    let metrics = s.server_data.metrics(
        ip,
        u32::try_from(game_port).unwrap_or(0),
        u32::try_from(query_port).unwrap_or(0),
    );
    (
        metrics.data.as_ref().map(|d| Arc::as_ptr(d) as usize),
        metrics.loading,
        &metrics.error,
        s.servers.ping_ms(ip, query_port),
    )
        .hash(&mut h);
    (
        s.server_data.a2s_loading(&key),
        s.servers.pending.contains(&key),
    )
        .hash(&mut h);
    s.mods.revision.hash(&mut h);
    h.finish()
}

impl Model {
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn metrics_data(&self) -> Option<&ServerMetricsDto> {
        self.metrics.data.as_deref()
    }

    pub fn missing_ids(&self) -> Vec<u64> {
        self.ids(ModState::Missing)
    }

    pub fn stale_ids(&self) -> Vec<u64> {
        self.ids(ModState::Stale)
    }

    fn ids(&self, s: ModState) -> Vec<u64> {
        self.mod_rows
            .iter()
            .filter(|m| m.state == s)
            .map(|m| m.id)
            .collect()
    }

    /// The server as a `dzch://` link, which opens this app straight on it, with the
    /// mods it runs.
    pub fn share_link(&self) -> &str {
        &self.share
    }

    fn make_share_link(&self) -> String {
        let gp = u16::try_from(self.game_port).unwrap_or(0);
        let qp = u16::try_from(self.query_port).ok();
        let mut link = crate::store::connect::dzch_link(&self.ip, gp, qp, Some(&self.title));
        if !self.mod_ids.is_empty() {
            let ids: Vec<String> = self.mod_ids.iter().map(u64::to_string).collect();
            link.push(if link.contains('?') { '&' } else { '?' });
            link.push_str("mods=");
            link.push_str(&ids.join(","));
        }
        link
    }

    /// Gather it for `(ip, port)`, `port` the query port or (unlisted) the one asked.
    pub fn of(cx: &Ctx, ip: &str, port: i64, name: &str) -> Self {
        let s = &cx.stores;
        let row = s.servers.row_of(ip, port);
        let listed = row.map(|r| {
            let v = r.server;
            Listed {
                name: v.name.clone(),
                map: r.map.to_string(),
                version: v.version.clone(),
                environment: v.environment.clone(),
                time: v.time.clone(),
                password: v.password,
                first_person: v.first_person_only,
                battleye: v.battl_eye,
                vac: v.vac,
                official: v.official,
                mimics_official: r.mimics_official,
                mods_count: v.mods.len(),
                mods_missing: r.mods_missing(),
                mods_stale: r.mods_stale(),
                excluded: r.excluded(),
                players: r.players(),
                max: r.max_players(),
                bots: r.bots(),
            }
        });
        let qp_listed = row.map(|r| i64::from(r.query_port()));
        let key = format!("{ip}:{}", qp_listed.unwrap_or(port));
        let entry = s.server_data.a2s_entry(&key);
        let a2s = entry.and_then(|e| e.data.clone());
        let live_loading = entry.is_some_and(|e| e.loading) || s.server_data.a2s_loading(&key);
        let live_error = entry.and_then(|e| e.error.clone());
        let query_port = qp_listed
            .or(a2s.as_ref().map(|a| a.query_port))
            .unwrap_or(port);
        let game_port = row
            .map(|r| i64::from(r.game_port()))
            .or(a2s.as_ref().and_then(|a| a.game_port))
            .unwrap_or(port);
        let title = [
            listed.as_ref().map(|l| l.name.as_str()),
            a2s.as_ref().map(|a| a.server_name.as_str()),
            Some(name),
        ]
        .into_iter()
        .flatten()
        .find(|n| !n.is_empty())
        .map_or_else(|| format!("{ip}:{port}"), str::to_string);
        let search_name = [
            listed.as_ref().map(|l| l.name.as_str()),
            a2s.as_ref().map(|a| a.server_name.as_str()),
            Some(name),
        ]
        .into_iter()
        .flatten()
        .find(|n| !n.is_empty())
        .unwrap_or("")
        .to_string();
        let map = listed
            .as_ref()
            .map(|l| l.map.clone())
            .filter(|m| !m.is_empty())
            .or_else(|| a2s.as_ref().map(|a| a.map.clone()))
            .unwrap_or_default();
        let count = match (&listed, &a2s) {
            (Some(l), _) => Some(Count {
                players: l.players,
                max: l.max,
                bots: l.bots,
            }),
            (None, Some(a)) => Some(Count {
                players: i64::from(a.players),
                max: i64::from(a.max_players),
                bots: u32::from(a.bots),
            }),
            _ => None,
        };

        // A listed server's mods come from the list; an unlisted one's from its own A2S
        // answer, which carries Workshop ids too.
        let mods = &s.mods;
        let mut mod_rows: Vec<ModRef> = Vec::new();
        let mut push = |id: i64, name: &str| {
            let Ok(id) = u64::try_from(id) else {
                return;
            };
            let have = mods.by_id(id);
            let state = match have {
                None => ModState::Missing,
                Some(h) if h.update_available => ModState::Stale,
                Some(_) => ModState::Installed,
            };
            let name = if name.is_empty() {
                have.map_or("", |h| h.name.as_str())
            } else {
                name
            };
            mod_rows.push(ModRef::new(id, name, state, have.map_or(0, |h| h.size)));
        };
        match (row, &a2s) {
            (Some(r), _) => {
                for m in &r.server.mods {
                    push(m.steam_workshop_id, &m.name);
                }
            }
            (None, Some(a)) => {
                for m in &a.mods_a2s {
                    push(m.steam_workshop_id, &m.name);
                }
            }
            _ => {}
        }
        let mod_ids: Vec<u64> = mod_rows.iter().map(|m| m.id).collect();
        ModRef::sort(&mut mod_rows);
        let missing = mod_rows
            .iter()
            .filter(|m| m.state == ModState::Missing)
            .count();
        let stale = mod_rows
            .iter()
            .filter(|m| m.state == ModState::Stale)
            .count();
        let installed = mod_rows.len() - missing;
        let mods_count = if !mod_rows.is_empty() {
            mod_rows.len()
        } else if let Some(l) = &listed {
            l.mods_count
        } else {
            a2s.as_ref().map_or(0, |a| a.mods_a2s.len())
        };

        let gp32 = u32::try_from(game_port).unwrap_or(0);
        let qp32 = u32::try_from(query_port).unwrap_or(0);
        let metrics = s.server_data.metrics(ip, gp32, qp32);
        let population = metrics.data.as_deref().map(population_of);

        let mut players = a2s
            .as_ref()
            .map(|a| a.players_list.clone())
            .unwrap_or_default();
        players.sort_by(|a, b| b.duration.total_cmp(&a.duration));

        let (gp16, qp16) = (
            u16::try_from(game_port).unwrap_or(0),
            u16::try_from(query_port).unwrap_or(0),
        );
        let history: Vec<_> = s
            .profile
            .history
            .iter()
            .filter(|h| h.ip == ip && (h.port == qp16 || h.port == gp16))
            .collect();
        let favorite = s.profile.is_favorite(ip, qp16) || s.profile.is_favorite(ip, gp16);
        let excluded = s.profile.is_excluded(ip);
        let ping = s.servers.ping_ms(ip, query_port);
        let ping_pending = s.servers.pending.contains(&key);
        let mut m = Self {
            address: format!("{ip}:{game_port}"),
            share: String::new(),
            ip: ip.to_string(),
            query_port,
            game_port,
            key,
            title,
            map,
            search_name,
            listed,
            a2s,
            live_loading,
            live_error,
            count,
            mod_rows,
            mod_ids,
            missing,
            stale,
            installed,
            mods_count,
            metrics,
            population,
            players,
            sessions: history.len(),
            last_played: history.first().map(|h| h.ts),
            favorite,
            excluded,
            ping,
            ping_pending,
        };
        m.share = m.make_share_link();
        m
    }
}

/// "fake" when the site says so outright or its behaviour check does; "suspect" for the
/// softer signs (a suspicious curve, player reports, a name that passes for official).
pub fn population_of(m: &ServerMetricsDto) -> Population {
    let v = m.behavior_verdict.as_deref().unwrap_or("").to_lowercase();
    if m.is_fake || v == "fake" {
        Population::Fake
    } else if v.starts_with("susp") || m.suspect || m.flagged || m.mimics_official {
        Population::Suspect
    } else {
        Population::Ok
    }
}

/// "2 h 15 min" until an ISO time, or `None` once it is past or unreadable (`until.ts`).
pub fn until_text(iso: &str, now_ms: i64) -> Option<String> {
    let t = chrono::DateTime::parse_from_rfc3339(iso)
        .ok()?
        .timestamp_millis();
    let mins = ((t - now_ms) as f64 / 60_000.0).round() as i64;
    if mins < 0 {
        return None;
    }
    if mins < 60 {
        return Some(format!("{mins} min"));
    }
    let (h, m) = (mins / 60, mins % 60);
    Some(if m > 0 {
        format!("{h} h {m} min")
    } else {
        format!("{h} h")
    })
}

/// An ISO time as Unix seconds.
pub fn iso_secs(iso: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|d| d.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn until_reads_like_the_svelte_one() {
        let now = iso_secs("2026-01-01T10:00:00Z").unwrap_or(0) * 1000;
        assert_eq!(
            until_text("2026-01-01T10:45:00Z", now).as_deref(),
            Some("45 min")
        );
        assert_eq!(
            until_text("2026-01-01T12:00:00Z", now).as_deref(),
            Some("2 h")
        );
        assert_eq!(
            until_text("2026-01-01T12:15:00Z", now).as_deref(),
            Some("2 h 15 min")
        );
        assert_eq!(until_text("2026-01-01T09:00:00Z", now), None);
        assert_eq!(until_text("nonsense", now), None);
    }
}
