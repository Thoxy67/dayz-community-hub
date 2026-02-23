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
                crate::errors::Error::Other(format!(
                    "Failed to parse GitHub release info: {}",
                    e
                ))
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

        // Extract and strip the first directory component.
        // We do two passes: first create all directories, then extract files.
        // This avoids failures when a file entry appears before its parent
        // directory entry in the archive (which tar does not guarantee).
        let mut entries_data: Vec<(PathBuf, Vec<u8>, u32)> = Vec::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let mode = entry.header().mode().unwrap_or(0o644);
            let components: Vec<_> = path.components().collect();

            if components.len() <= 1 {
                continue; // skip the top-level directory entry itself
            }

            let new_path: PathBuf = components[1..].iter().collect();
            let full_path = missions_path.join(&new_path);

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&full_path).map_err(|e| {
                    crate::errors::Error::Other(format!(
                        "Failed to create directory {:?}: {}",
                        full_path, e
                    ))
                })?;
            } else {
                // Buffer the file contents so we can create its parent dir first.
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut buf)?;
                entries_data.push((full_path, buf, mode));
            }
        }

        // Second pass: write all files (parent dirs are now guaranteed to exist).
        for (full_path, data, _mode) in entries_data {
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    crate::errors::Error::Other(format!(
                        "Failed to create parent dir {:?}: {}",
                        parent, e
                    ))
                })?;
            }
            fs::write(&full_path, &data).map_err(|e| {
                crate::errors::Error::Other(format!(
                    "Failed to write {:?}: {}",
                    full_path, e
                ))
            })?;
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
        enable_spawn: bool,
    ) -> Vec<String> {
        let mut args = vec![
            "-filePatching".to_string(),
            format!("-mission=./Missions/{}", mission),
        ];

        if !mod_ids.is_empty() {
            let mods_str = mod_ids
                .iter()
                .map(|id| format!("@{}", id))
                .collect::<Vec<_>>()
                .join(";");
            args.push(format!("-mod={}", mods_str));
        }

        args.push("-doLogs".to_string());
        args.push("-scriptDebug=true".to_string());

        if !enable_spawn {
            args.push("-noHive".to_string());
        }

        args
    }
}
