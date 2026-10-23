//! The profile: account settings, favorites, history and excluded IPs.
//! Every change is written to disk once the state lock is released.

mod dto;
pub(crate) mod io;

pub(crate) use dto::profile_to_dto;
pub use dto::{FavoriteDto, HistoryDto, LaunchOptionDto, ProfileDto};

use dz_game::ctl::{SteamInputs, SteamSetup};

use crate::Hub;
use crate::state::{SharedState, mutate_profile};

/// The account and ping settings, as the settings dialog saves them.
#[derive(serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettingsInput {
    pub player: Option<String>,
    pub steam_login: Option<String>,
    pub steam_root: Option<String>,
    pub steamcmd_enabled: bool,
    pub steamcmd_path: Option<String>,
    pub steam_api_key: Option<String>,
    pub steam_id: Option<String>,
    /// (longitude, latitude).
    pub user_location: Option<(f64, f64)>,
    /// Clamped to 5-100.
    pub ping_concurrency: u32,
    /// Clamped to 1000-5000 ms.
    pub ping_timeout_auto: u32,
    /// Clamped to 1000-30000 ms.
    pub ping_timeout_manual: u32,
    /// Clamped to 0-5.
    pub ping_max_retries: u32,
    pub ping_scan_favorites: bool,
    pub ping_scan_history: bool,
    pub ping_scan_servers: bool,
}

/// A path as people paste it, made usable: surrounding spaces and the quotes
/// Windows' "Copy as path" adds go, and on Linux a leading `~` is the home
/// folder. Empty is no path.
fn clean_path(p: Option<String>) -> Option<String> {
    let p = p?;
    let mut s = p.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            s = s[1..s.len() - 1].trim();
        }
    }
    if s.is_empty() {
        return None;
    }
    #[cfg(unix)]
    if (s == "~" || s.starts_with("~/"))
        && let Some(home) = std::env::var_os("HOME")
    {
        return Some(format!("{}{}", home.to_string_lossy(), &s[1..]));
    }
    Some(s.to_string())
}

/// Find Steam and SteamCMD again for the profile as it stands, on a blocking
/// thread with no lock held (it probes the disks: every drive on Windows),
/// then swap the result in, unless the profile's Steam settings moved
/// meanwhile: the change that moved them rebuilds after this one.
pub(crate) async fn rebuild_steamcmd(state: &SharedState) {
    let inputs = SteamInputs::of(state.read().await.ctl.profile());
    let asked = inputs.clone();
    let Ok(setup) = tokio::task::spawn_blocking(move || SteamSetup::resolve(&asked)).await else {
        return;
    };
    let mut s = state.write().await;
    if SteamInputs::of(s.ctl.profile()) == inputs {
        s.ctl.set_steam_setup(setup);
    }
}

impl Hub {
    /// The current profile.
    pub async fn get_profile(&self) -> Result<ProfileDto, String> {
        let state = self.state()?;
        Ok(profile_to_dto(state.read().await.ctl.profile()))
    }

    /// Save the account and ping settings.
    pub async fn save_profile_settings(
        &self,
        settings: ProfileSettingsInput,
    ) -> Result<(), String> {
        let state = self.state()?;
        let ProfileSettingsInput {
            player,
            steam_login,
            steam_root,
            steamcmd_enabled,
            steamcmd_path,
            steam_api_key,
            steam_id,
            user_location,
            ping_concurrency,
            ping_timeout_auto,
            ping_timeout_manual,
            ping_max_retries,
            ping_scan_favorites,
            ping_scan_history,
            ping_scan_servers,
        } = settings;
        let steam_moved = mutate_profile(&state, |s| {
            let credentials_changed = {
                let p = s.ctl.profile();
                p.steam_api_key != steam_api_key || p.steam_id != steam_id
            };
            let steamcmd_before = SteamInputs::of(s.ctl.profile());
            let profile = s.ctl.profile_mut();
            profile.player = player;
            // Another account: SteamCMD's cached login and a saved password
            // were the old one's.
            if profile.steam_login != steam_login {
                profile.steamcmd_logged_in = None;
                profile.steam_password = None;
            }
            profile.steam_login = steam_login;
            profile.steam_root = clean_path(steam_root);
            profile.steamcmd_enabled = steamcmd_enabled;
            profile.steamcmd_path = clean_path(steamcmd_path);
            profile.steam_api_key = steam_api_key;
            profile.steam_id = steam_id;
            profile.user_location = user_location;
            profile.ping_concurrency = ping_concurrency.clamp(5, 100);
            profile.ping_timeout_auto = ping_timeout_auto.clamp(1000, 5000);
            profile.ping_timeout_manual = ping_timeout_manual.clamp(1000, 30000);
            profile.ping_max_retries = ping_max_retries.min(5);
            profile.ping_scan_favorites = ping_scan_favorites;
            profile.ping_scan_history = ping_scan_history;
            profile.ping_scan_servers = ping_scan_servers;
            // New avatar credentials: fetch the avatar again.
            if credentials_changed {
                s.cached_avatar = None;
            }
            // Finding Steam and SteamCMD probes the disks (every drive on
            // Windows): only when what it reads changed, not on every ping
            // setting saved, and after the lock is let go.
            Ok(SteamInputs::of(s.ctl.profile()) != steamcmd_before)
        })
        .await?;
        if steam_moved {
            rebuild_steamcmd(&state).await;
        }
        Ok(())
    }

    /// Add a favorite (or update its name and password).
    pub async fn add_favorite(
        &self,
        name: String,
        ip: String,
        port: u16,
        password: Option<String>,
    ) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| {
            s.ctl.profile_mut().add_favorite(name, ip, port, password);
            Ok(())
        })
        .await
    }

    pub async fn remove_favorite(&self, ip: String, port: u16) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| {
            s.ctl.profile_mut().remove_favorite(&ip, port);
            Ok(())
        })
        .await
    }

    /// Change a favourite's note, labels, group, character name or launch
    /// arguments, or the labels and groups themselves. The edits apply in
    /// order and all or none, so a label made and then given to a favourite
    /// arrive together.
    pub async fn edit_favorites(&self, edits: Vec<dz_profile::FavoriteEdit>) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| s.ctl.profile_mut().edit_favorites_all(edits)).await
    }

    pub async fn remove_history_entry(&self, ip: String, port: u16) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| {
            s.ctl.profile_mut().remove_history(&ip, port);
            Ok(())
        })
        .await
    }

    pub async fn clear_history(&self) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| {
            s.ctl.profile_mut().history.clear();
            Ok(())
        })
        .await
    }

    /// Hide an IP's servers from the browser.
    pub async fn add_excluded_ip(&self, ip: String) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| {
            s.ctl.profile_mut().add_excluded_ip(ip);
            Ok(())
        })
        .await
    }

    pub async fn remove_excluded_ip(&self, ip: String) -> Result<(), String> {
        let state = self.state()?;
        mutate_profile(&state, |s| {
            s.ctl.profile_mut().remove_excluded_ip(&ip);
            Ok(())
        })
        .await
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod clean_path_tests {
    use super::clean_path;

    #[test]
    fn pasted_paths_are_tidied() {
        assert_eq!(clean_path(None), None);
        assert_eq!(clean_path(Some("  ".into())), None);
        assert_eq!(
            clean_path(Some("\"D:\\SteamLibrary\" ".into())).as_deref(),
            Some("D:\\SteamLibrary")
        );
        assert_eq!(
            clean_path(Some("'/mnt/games'".into())).as_deref(),
            Some("/mnt/games")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_leading_tilde_is_home() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            clean_path(Some("~/SteamLibrary".into())),
            Some(format!("{home}/SteamLibrary"))
        );
        assert_eq!(
            clean_path(Some("~user/x".into())).as_deref(),
            Some("~user/x")
        );
    }
}
