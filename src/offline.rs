use crate::Result;
use flate2::read::GzDecoder;
use reqwest;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;

const COMMUNITY_OFFLINE_REPO: &str = "Arkensor/DayZCommunityOfflineMode";
const MISSIONS_DIR: &str = "Missions";

pub struct OfflineMode {
    dayz_path: PathBuf,
}

impl OfflineMode {
    pub fn new(dayz_path: impl AsRef<Path>) -> Self {
        Self {
            dayz_path: dayz_path.as_ref().to_path_buf(),
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
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!(
                "https://github.com/{}/releases/latest",
                COMMUNITY_OFFLINE_REPO
            ))
            .send()
            .await?;

        // Get the final URL after redirects
        let url = resp.url().to_string();
        let tag = url.split('/').last().unwrap_or("").to_string();
        Ok(tag)
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

        let client = reqwest::Client::new();
        let response = client.get(&tarball_url).send().await?;
        let bytes = response.bytes().await?;

        let missions_path = self.missions_path();
        fs::create_dir_all(&missions_path)?;

        let decoder = GzDecoder::new(&bytes[..]);
        let mut archive = Archive::new(decoder);

        // Extract and strip the first directory component
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let components: Vec<_> = path.components().collect();

            if components.len() > 1 {
                let new_path = components[1..].iter().collect::<PathBuf>();
                let full_path = missions_path.join(new_path);

                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                entry.unpack(&full_path)?;
            }
        }

        Ok(())
    }

    fn write_version_file(&self, version: &str) -> Result<()> {
        let version_file = self
            .missions_path()
            .join("DayZCommunityOfflineMode.ChernarusPlus")
            .join(".version");

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
