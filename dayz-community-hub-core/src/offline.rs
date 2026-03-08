use crate::Result;
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;

const COMMUNITY_OFFLINE_REPO: &str = "Arkensor/DayZCommunityOfflineMode";
const MISSIONS_DIR: &str = "Missions";
/// User-Agent required by GitHub API (any non-empty string works).
const UA: &str = "dayz-community-hub/0.1";

pub struct OfflineMode {
    dayz_path: PathBuf,
    client: Client,
}

impl OfflineMode {
    /// Create a new `OfflineMode` using a shared HTTP client.
    pub fn new(dayz_path: impl AsRef<Path>, client: Client) -> Self {
        Self {
            dayz_path: dayz_path.as_ref().to_path_buf(),
            client,
        }
    }

    pub fn missions_path(&self) -> PathBuf {
        self.dayz_path.join(MISSIONS_DIR)
    }

    pub fn get_available_missions(&self) -> Result<Vec<String>> {
        let missions_path = self.missions_path();
        if !missions_path.exists() {
            return Ok(Vec::new());
        }
        let mut missions = Vec::new();
        for entry in fs::read_dir(&missions_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    missions.push(name.to_string_lossy().to_string());
                }
            }
        }
        Ok(missions)
    }

    pub async fn update(&self) -> Result<()> {
        let latest_tag = self.get_latest_tag().await?;
        let current_version = self.get_current_version();

        if current_version.as_deref() == Some(&latest_tag) {
            return Ok(());
        }

        self.download_and_extract(&latest_tag).await?;
        self.write_version_file(&latest_tag)?;

        Ok(())
    }

    async fn get_latest_tag(&self) -> Result<String> {
        // Use the GitHub REST API — requires a User-Agent header.
        #[derive(Deserialize)]
        struct Release {
            tag_name: String,
        }

        let url = format!(
            "https://api.github.com/repos/{}/releases/latest",
            COMMUNITY_OFFLINE_REPO
        );

        let release: Release = self
            .client
            .get(&url)
            .header("User-Agent", UA)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?
            .json::<Release>()
            .await
            .map_err(|e| {
                crate::errors::Error::Other(format!("Failed to parse GitHub release info: {}", e))
            })?;

        Ok(release.tag_name)
    }

    fn get_current_version(&self) -> Option<String> {
        let version_file = self
            .missions_path()
            .join("DayZCommunityOfflineMode.ChernarusPlus")
            .join(".version");

        if version_file.exists() {
            fs::read_to_string(version_file).ok()
        } else {
            None
        }
    }

    async fn download_and_extract(&self, tag: &str) -> Result<()> {
        // GitHub's tarball endpoint redirects to a CDN URL.
        // We must send the User-Agent on the initial request; reqwest follows
        // the redirect automatically, but we also need to ensure the response
        // is actually a gzip stream before handing it to flate2.
        let tarball_url = if tag.is_empty() {
            format!(
                "https://api.github.com/repos/{}/tarball",
                COMMUNITY_OFFLINE_REPO
            )
        } else {
            format!(
                "https://api.github.com/repos/{}/tarball/{}",
                COMMUNITY_OFFLINE_REPO, tag
            )
        };

        let response = self
            .client
            .get(&tarball_url)
            .header("User-Agent", UA)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(crate::errors::Error::Other(format!(
                "GitHub returned HTTP {} for tarball download",
                status
            )));
        }

        let bytes = response.bytes().await?;

        // Sanity-check: a gzip stream starts with the magic bytes 0x1f 0x8b.
        if bytes.len() < 2 || bytes[0] != 0x1f || bytes[1] != 0x8b {
            return Err(crate::errors::Error::Other(format!(
                "Downloaded data is not a gzip stream (got {} bytes, first bytes: {:?}). \
                 The GitHub API may have returned an error page.",
                bytes.len(),
                &bytes[..bytes.len().min(64)]
            )));
        }

        let missions_path = self.missions_path();
        fs::create_dir_all(&missions_path)?;

        let decoder = GzDecoder::new(&bytes[..]);
        let mut archive = Archive::new(decoder);

        // The GitHub tarball layout is:
        //   Arkensor-DayZCommunityOfflineMode-HASH/
        //   Arkensor-DayZCommunityOfflineMode-HASH/Missions/
        //   Arkensor-DayZCommunityOfflineMode-HASH/Missions/DayZCommunityOfflineMode.ChernarusPlus/...
        //
        // We want to extract only the contents of the inner `Missions/` folder
        // directly into `dayz_path/Missions/`, so we must strip 2 components:
        //   [HASH-dir] [Missions] → written to missions_path/
        //
        // Entries with fewer than 3 components (the hash-dir, "Missions", and
        // at least one more segment) are skipped (they're the top-level or the
        // Missions directory itself).
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let components: Vec<_> = path.components().collect();

            // Skip entries that don't live inside the Missions/ sub-directory.
            // We need at least: [hash-dir, "Missions", <mission-folder>, ...]
            if components.len() <= 2 {
                continue;
            }
            // Only keep entries whose second component is "Missions".
            let second = components[1].as_os_str().to_string_lossy();
            if second != "Missions" {
                continue;
            }

            // Strip the first two components (hash-dir + "Missions").
            let new_path: PathBuf = components[2..].iter().collect();
            let full_path = missions_path.join(&new_path);

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&full_path).map_err(|e| {
                    crate::errors::Error::Other(format!(
                        "Failed to create directory {:?}: {}",
                        full_path, e
                    ))
                })?;
            } else {
                // Ensure parent directory exists before writing the file.
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        crate::errors::Error::Other(format!(
                            "Failed to create parent dir {:?}: {}",
                            parent, e
                        ))
                    })?;
                }
                // Write directly from the archive stream — no intermediate buffer.
                let mut out = std::fs::File::create(&full_path).map_err(|e| {
                    crate::errors::Error::Other(format!("Failed to create {:?}: {}", full_path, e))
                })?;
                std::io::copy(&mut entry, &mut out).map_err(|e| {
                    crate::errors::Error::Other(format!("Failed to write {:?}: {}", full_path, e))
                })?;
            }
        }

        Ok(())
    }

    fn write_version_file(&self, version: &str) -> Result<()> {
        let version_file = self
            .missions_path()
            .join("DayZCommunityOfflineMode.ChernarusPlus")
            .join(".version");

        // The directory may not exist yet on first install.
        if let Some(parent) = version_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(version_file, version)?;
        Ok(())
    }

    /// Remove all DayZCommunityOfflineMode mission folders from `DayZ/Missions/`.
    /// Only removes directories whose name starts with `DayZCommunityOfflineMode.`.
    /// Returns the number of directories removed.
    pub fn remove_offline_mode(&self) -> Result<usize> {
        let missions_path = self.missions_path();
        if !missions_path.exists() {
            return Ok(0);
        }
        let mut removed = 0usize;
        for entry in fs::read_dir(&missions_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.starts_with("DayZCommunityOfflineMode.") {
                    fs::remove_dir_all(&path).map_err(|e| {
                        crate::errors::Error::Other(format!("Failed to remove {:?}: {}", path, e))
                    })?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    /// Remove a single mission folder from `DayZ/Missions/`.
    pub fn remove_mission(&self, mission: &str) -> Result<()> {
        let mission_path = self.missions_path().join(mission);
        if !mission_path.exists() {
            return Err(crate::errors::Error::Other(format!(
                "Mission not found: {}",
                mission
            )));
        }
        if !mission_path.is_dir() {
            return Err(crate::errors::Error::Other(format!(
                "Not a directory: {}",
                mission
            )));
        }
        std::fs::remove_dir_all(&mission_path).map_err(|e| {
            crate::errors::Error::Other(format!("Failed to remove {:?}: {}", mission_path, e))
        })?;
        Ok(())
    }

    /// Delete `storage_-1/` inside every DayZCommunityOfflineMode mission folder.
    /// This wipes all in-game saves (loot, player state, etc.) without removing
    /// the missions themselves.
    /// Returns the number of save directories removed.
    pub fn clear_offline_saves(&self) -> Result<usize> {
        let missions_path = self.missions_path();
        if !missions_path.exists() {
            return Ok(0);
        }
        let mut removed = 0usize;
        for entry in fs::read_dir(&missions_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.starts_with("DayZCommunityOfflineMode.") {
                    let storage = path.join("storage_-1");
                    if storage.exists() {
                        fs::remove_dir_all(&storage).map_err(|e| {
                            crate::errors::Error::Other(format!(
                                "Failed to remove saves at {:?}: {}",
                                storage, e
                            ))
                        })?;
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }

    pub fn toggle_spawn(&self, mission: &str, enable: bool) -> Result<()> {
        let config_file = self
            .missions_path()
            .join(mission)
            .join("core")
            .join("CommunityOfflineClient.c");

        if !config_file.exists() {
            return Err(crate::errors::Error::Other(format!(
                "Mission config not found: {:?}",
                config_file
            )));
        }

        let content = fs::read_to_string(&config_file)?;
        let new_content = if enable {
            content
                .replace("HIVE_ENABLED = false;", "HIVE_ENABLED = true;")
                .replace("HIVE_ENABLED = 0;", "HIVE_ENABLED = 1;")
        } else {
            content
                .replace("HIVE_ENABLED = true;", "HIVE_ENABLED = false;")
                .replace("HIVE_ENABLED = 1;", "HIVE_ENABLED = 0;")
        };

        fs::write(config_file, new_content)?;
        Ok(())
    }

    pub fn build_launch_args(
        &self,
        mission: &str,
        mod_ids: &[u64],
        _enable_spawn: bool,
    ) -> Vec<String> {
        // Match the flags used by the official DayZCommunityOfflineMode.bat:
        //   DayZ_x64.exe -mission=.\Missions\<name> -nosplash -noPause -noBenchmark
        //                -filePatching -doLogs -scriptDebug=true
        // Via Steam -applaunch we also need -nolauncher to skip the DayZ launcher.
        let mut args = vec![
            format!("-mission=.\\Missions\\{}", mission),
            "-nosplash".to_string(),
            "-noPause".to_string(),
            "-noBenchmark".to_string(),
            "-nolauncher".to_string(),
            "-filePatching".to_string(),
            "-doLogs".to_string(),
            "-scriptDebug=true".to_string(),
        ];

        if !mod_ids.is_empty() {
            let mods_str = mod_ids
                .iter()
                .map(|id| format!("@{}", id))
                .collect::<Vec<_>>()
                .join(";");
            args.push(format!("-mod={}", mods_str));
        }

        args
    }
}
