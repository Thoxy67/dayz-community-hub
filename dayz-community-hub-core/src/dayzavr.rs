//! DayZavr community integration (read-only server browsing + mod manifest).
//!
//! DayZavr is a third-party DayZ community that runs custom-modded servers.
//! Their launcher publishes a public server list (`servers.xml`) and a mod
//! manifest (`MODS/FILE.log`). This module fetches and parses that public data.
//!
//! IMPORTANT: this module only ever touches the **mods** distribution
//! (`/LAUNCHER/UPLOADGAME/MODS/...`), never the separate game torrent in the
//! parent directory (which bundles a cracked DayZ client). Mod installation is
//! intended for users who already own DayZ on Steam, mirroring DayZavr's own
//! "Steam mode" where only the mod PBOs are downloaded.

use crate::{Result, errors::Error};
use serde::Serialize;

/// Base launcher path on the DayZavr stats host. `region1` is a mirror of
/// `region2`/`region3`; we use region1 and could fail over later.
pub const DAYZAVR_BASE: &str = "http://region1.stats.dayzavr.ru:22480/LAUNCHER";
/// Public server list.
pub const SERVERS_URL: &str = "http://region1.stats.dayzavr.ru:22480/LAUNCHER/servers.xml";
/// Mods directory (manifests + the mods-only torrent).
pub const MODS_BASE: &str = "http://region1.stats.dayzavr.ru:22480/LAUNCHER/UPLOADGAME/MODS";

/// A DayZavr community server as advertised in `servers.xml`.
#[derive(Debug, Clone, Serialize)]
pub struct DayzavrServer {
    /// `load id` attribute.
    pub id: u32,
    pub name: String,
    /// Primary connect host (first region).
    pub host: String,
    /// All regional hosts (region1/2/3) for the same server.
    pub all_hosts: Vec<String>,
    /// Game (connect) port.
    pub game_port: u16,
    /// Steam query port (A2S) — DayZavr's `portUDP`.
    pub query_port: u16,
    pub password: bool,
    pub players: u32,
    pub max_players: u32,
    /// Mod folder names this server requires (e.g. `@DayZavrCore`).
    pub mods: Vec<String>,
    /// In-game time string.
    pub time: String,
    /// Restart schedule string.
    pub restart: String,
    /// Discord invite, if advertised.
    pub discord: Option<String>,
    /// Banner image URL (first region), if advertised.
    pub image: Option<String>,
}

fn child_text(node: roxmltree::Node, tag: &str) -> Option<String> {
    node.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
        .map(|s| s.trim().to_string())
}

/// Parse DayZavr's `-mod=@A;@B;...` field into a list of mod folder names.
/// The field is wrapped in quotes and prefixed with `-mod=`.
pub fn parse_mod_list(field: &str) -> Vec<String> {
    let cleaned = field.trim().trim_matches('"');
    let cleaned = cleaned.strip_prefix("-mod=").unwrap_or(cleaned);
    cleaned
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.starts_with('@'))
        .map(|s| s.to_string())
        .collect()
}

/// Parse the `servers.xml` body into a list of servers.
pub fn parse_servers(xml: &str) -> Result<Vec<DayzavrServer>> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| Error::Other(format!("DayZavr servers.xml parse error: {e}")))?;
    let mut out = Vec::new();
    for load in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("load"))
    {
        let name = child_text(load, "name").unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let all_hosts: Vec<String> = child_text(load, "ip")
            .unwrap_or_default()
            .split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let host = all_hosts.first().cloned().unwrap_or_default();
        let image = child_text(load, "Image")
            .and_then(|s| s.split('|').next().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty());
        out.push(DayzavrServer {
            id: load.attribute("id").and_then(|s| s.parse().ok()).unwrap_or(0),
            name,
            host,
            all_hosts,
            game_port: child_text(load, "port").and_then(|s| s.parse().ok()).unwrap_or(0),
            query_port: child_text(load, "portUDP")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            password: child_text(load, "pass")
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            players: child_text(load, "play_host")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            max_players: child_text(load, "MaxPlayers")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            mods: parse_mod_list(&child_text(load, "mods").unwrap_or_default()),
            time: child_text(load, "time").unwrap_or_default(),
            restart: child_text(load, "TimeRestart").unwrap_or_default(),
            discord: child_text(load, "Diskord").filter(|s| !s.is_empty()),
            image,
        });
    }
    Ok(out)
}

/// Fetch and parse the DayZavr server list.
pub async fn fetch_servers(client: &reqwest::Client) -> Result<Vec<DayzavrServer>> {
    let body = client.get(SERVERS_URL).send().await?.text().await?;
    parse_servers(&body)
}

/// One file in the mod manifest (`MODS/FILE.log`): `\@Mod\Addons\x.pbo|crc32|size`.
#[derive(Debug, Clone)]
pub struct ModFile {
    /// Forward-slash relative path, e.g. `@DayZavrCore/Addons/x.pbo`.
    pub path: String,
    /// CRC32 checksum (hex in the manifest).
    pub crc32: u32,
    /// Size in bytes.
    pub size: u64,
}

impl ModFile {
    /// The mod folder this file belongs to (first path segment, e.g. `@DayZavrCore`).
    pub fn mod_folder(&self) -> &str {
        self.path.split('/').next().unwrap_or("")
    }
}

/// Parse `MODS/FILE.log` into a manifest of files.
pub fn parse_manifest(text: &str) -> Vec<ModFile> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split('|');
        let (Some(path), Some(crc), Some(size)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let Ok(crc32) = u32::from_str_radix(crc.trim(), 16) else {
            continue;
        };
        let Ok(size) = size.trim().parse::<u64>() else {
            continue;
        };
        let path = path.trim_start_matches('\\').replace('\\', "/");
        if path.is_empty() {
            continue;
        }
        out.push(ModFile { path, crc32, size });
    }
    out
}

/// Fetch and parse the mod manifest.
pub async fn fetch_manifest(client: &reqwest::Client) -> Result<Vec<ModFile>> {
    let body = client
        .get(format!("{MODS_BASE}/FILE.log"))
        .send()
        .await?
        .text()
        .await?;
    Ok(parse_manifest(&body))
}

/// Total download size (bytes) for the given set of mod folders.
pub fn mods_total_size(manifest: &[ModFile], mods: &[String]) -> u64 {
    manifest
        .iter()
        .filter(|f| mods.iter().any(|m| m == f.mod_folder()))
        .map(|f| f.size)
        .sum()
}

/// Build a `^(@ModA|@ModB)/` regex (for librqbit `only_files_regex`) that
/// matches only the requested mod folders in the torrent. Mod names are regex
/// escaped (they contain spaces, apostrophes, etc.). Returns `None` if empty.
pub fn mods_only_regex(mods: &[String]) -> Option<String> {
    if mods.is_empty() {
        return None;
    }
    let alts: Vec<String> = mods.iter().map(|m| regex::escape(m)).collect();
    Some(format!("^({})/", alts.join("|")))
}

/// Manifest entries belonging to the requested mods (the files to download).
pub fn mod_files<'a>(manifest: &'a [ModFile], mods: &[String]) -> Vec<&'a ModFile> {
    manifest
        .iter()
        .filter(|f| mods.iter().any(|m| m == f.mod_folder()))
        .collect()
}

/// True when every manifest file for `mod_folder` exists locally (under
/// `!Workshop`) with a matching size. Size-only (fast); librqbit re-verifies
/// piece hashes during download, so same-size-but-modified files self-heal.
pub fn mod_is_complete(dayz_path: &std::path::Path, manifest: &[ModFile], mod_folder: &str) -> bool {
    let workshop = dayz_path.join("!Workshop");
    let mut any = false;
    for f in manifest.iter().filter(|f| f.mod_folder() == mod_folder) {
        any = true;
        let local = workshop.join(&f.path);
        match std::fs::metadata(&local) {
            Ok(m) if m.len() == f.size => {}
            _ => return false,
        }
    }
    any // false for unknown mods (no manifest entries)
}

/// Of `requested`, the mods that are missing or incomplete and must be fetched.
pub fn mods_needing_download(
    dayz_path: &std::path::Path,
    manifest: &[ModFile],
    requested: &[String],
) -> Vec<String> {
    requested
        .iter()
        .filter(|m| !mod_is_complete(dayz_path, manifest, m))
        .cloned()
        .collect()
}

/// Mod folders present in `!Workshop` that are complete per the manifest.
/// Only installed folders are checked, so this is cheap.
pub fn installed_complete_mods(dayz_path: &std::path::Path, manifest: &[ModFile]) -> Vec<String> {
    let workshop = dayz_path.join("!Workshop");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&workshop) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir()
                && name.starts_with('@')
                && mod_is_complete(dayz_path, manifest, &name)
            {
                out.push(name);
            }
        }
    }
    out
}

#[cfg(unix)]
fn symlink_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}
#[cfg(windows)]
fn symlink_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(src, dst)
}

/// Symlink each `!Workshop/@Mod` into the DayZ root as `@Mod` (how DayZ/DZSA load
/// local mods) so the game can be launched with a plain `-mod=@Mod` instead of a
/// `!Workshop\@Mod` path, which DayZ (especially under Proton) often fails to
/// resolve — leaving the server to BattlEye-kick the unmodded client.
/// Returns the mod folder names that are available in the root.
pub fn link_mods_to_root(dayz_path: &std::path::Path, mods: &[String]) -> Vec<String> {
    let workshop = dayz_path.join("!Workshop");
    let mut available = Vec::new();
    for m in mods {
        let src = workshop.join(m);
        if !src.is_dir() {
            continue;
        }
        let dst = dayz_path.join(m);
        // Already present (real folder or a prior symlink) — reuse it.
        if dst.exists() || dst.symlink_metadata().is_ok() {
            available.push(m.clone());
            continue;
        }
        if symlink_dir(&src, &dst).is_ok() {
            available.push(m.clone());
        }
    }
    available
}

/// Remove the entire `<dayz_path>/!Workshop` directory (all installed mods).
/// Returns the `@Mod` folder names that were present (for reporting).
pub fn clear_installed_mods(dayz_path: &std::path::Path) -> Result<Vec<String>> {
    let workshop = dayz_path.join("!Workshop");
    if !workshop.is_dir() {
        return Ok(Vec::new());
    }
    // Record the mod folders before deleting, so we can report a count.
    let removed: Vec<String> = std::fs::read_dir(&workshop)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with('@'))
                .collect()
        })
        .unwrap_or_default();
    std::fs::remove_dir_all(&workshop)?;
    // Remove the root `@Mod` symlinks created for launch (now dangling).
    for name in &removed {
        let link = dayz_path.join(name);
        if link.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false) {
            let _ = std::fs::remove_file(&link);
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mod_list() {
        let f = "\"-mod=@Cl0ud's Military Gear;@DayZavrCore;;@CF\"";
        assert_eq!(parse_mod_list(f), ["@Cl0ud's Military Gear", "@DayZavrCore", "@CF"]);
    }

    #[test]
    fn parses_manifest_line() {
        let m = parse_manifest("\\@CF\\Addons\\cf.pbo|4b7c80a0|43215275\n\\@CF\\v.txt|a1064c88|6");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].path, "@CF/Addons/cf.pbo");
        assert_eq!(m[0].crc32, 0x4b7c80a0);
        assert_eq!(m[0].size, 43215275);
        assert_eq!(m[0].mod_folder(), "@CF");
    }

    #[test]
    fn parses_servers() {
        let xml = r#"<server><load id="3"><name>Test</name><ip>a.ru|b.ru</ip><port>2372</port><pass></pass><portUDP>15019</portUDP><play_host>12</play_host><MaxPlayers>45</MaxPlayers><mods>"-mod=@A;@B"</mods><time>14:30</time><TimeRestart>x</TimeRestart><Diskord>https://d</Diskord></load></server>"#;
        let s = parse_servers(xml).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, 3);
        assert_eq!(s[0].host, "a.ru");
        assert_eq!(s[0].query_port, 15019);
        assert_eq!(s[0].players, 12);
        assert_eq!(s[0].mods, ["@A", "@B"]);
    }
}
