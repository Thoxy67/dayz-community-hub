//! The account fields being edited (= `account-form.svelte.ts`): identity, Steam, API keys
//! and location, held apart from the profile until saved, so the page can say what changed
//! and put it back. The form follows the profile while nothing is pending.
//!
//! Saving is optimistic: the profile store takes the values at once (the unsaved bar goes
//! away on the click), writes them, and reloads the profile if the write fails; the form
//! then puts the player's edits back so nothing typed is lost.

use dz_hub::features::profile::ProfileDto;

use crate::store::profile::{AccountSettings, Profile};

/// The fields, as typed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields {
    pub player: String,
    pub steam_login: String,
    pub steam_root: String,
    pub steamcmd_path: String,
    pub steam_api_key: String,
    pub steam_id: String,
    /// (longitude, latitude), as the profile keeps it.
    pub location: Option<(f64, f64)>,
}

fn s(v: Option<&String>) -> &str {
    v.map_or("", String::as_str)
}

/// A blank (or spaces) field is saved as nothing.
fn or_none(v: &str) -> Option<String> {
    let t = v.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl Fields {
    pub fn of(p: Option<&ProfileDto>) -> Self {
        let Some(p) = p else {
            return Self::default();
        };
        Self {
            player: s(p.player.as_ref()).to_string(),
            steam_login: s(p.steam_login.as_ref()).to_string(),
            steam_root: s(p.steam_root.as_ref()).to_string(),
            steamcmd_path: s(p.steamcmd_path.as_ref()).to_string(),
            steam_api_key: s(p.steam_api_key.as_ref()).to_string(),
            steam_id: s(p.steam_id.as_ref()).to_string(),
            location: p.user_location,
        }
    }

    /// Whether these are the profile's values (`Fields::of(p) == self`, without copying).
    pub fn matches(&self, p: Option<&ProfileDto>) -> bool {
        let Some(p) = p else {
            return *self == Self::default();
        };
        self.player == s(p.player.as_ref())
            && self.steam_login == s(p.steam_login.as_ref())
            && self.steam_root == s(p.steam_root.as_ref())
            && self.steamcmd_path == s(p.steamcmd_path.as_ref())
            && self.steam_api_key == s(p.steam_api_key.as_ref())
            && self.steam_id == s(p.steam_id.as_ref())
            && self.location == p.user_location
    }

    pub fn settings(&self) -> AccountSettings {
        AccountSettings {
            player: or_none(&self.player),
            steam_login: or_none(&self.steam_login),
            steam_root: or_none(&self.steam_root),
            steamcmd_path: or_none(&self.steamcmd_path),
            steam_api_key: or_none(&self.steam_api_key),
            steam_id: or_none(&self.steam_id),
            user_location: self.location,
        }
    }
}

#[derive(Default)]
pub struct AccountForm {
    pub f: Fields,
    base: Fields,
    /// What the last save sent, put back if the write fails.
    sent: Option<Fields>,
    /// The store's failed account writes when the last save was sent.
    failures: u32,
}

impl AccountForm {
    pub fn dirty(&self) -> bool {
        self.f != self.base
    }

    /// Follow the profile as long as the player has not started editing.
    pub fn sync(&mut self, profile: &Profile) {
        if profile.account_failures != self.failures {
            self.failures = profile.account_failures;
            if let Some(sent) = self.sent.take() {
                self.f = sent;
            }
        }
        let p = profile.data.as_ref();
        if !self.base.matches(p) {
            let next = Fields::of(p);
            if !self.dirty() {
                self.f = next.clone();
            }
            self.base = next;
        }
    }

    /// Every field back to the profile's value.
    pub fn discard(&mut self, profile: &Profile) {
        self.f = Fields::of(profile.data.as_ref());
        self.base = self.f.clone();
    }

    /// Write every field at once.
    pub fn save(&mut self, profile: &mut Profile) {
        if profile.data.is_none() {
            return;
        }
        self.failures = profile.account_failures;
        self.sent = Some(self.f.clone());
        profile.save_account(self.f.settings());
        self.discard(profile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_fields_are_saved_as_nothing() {
        let f = Fields {
            player: "  Bob ".into(),
            steam_login: "   ".into(),
            ..Fields::default()
        };
        let s = f.settings();
        assert_eq!(s.player.as_deref(), Some("Bob"));
        assert_eq!(s.steam_login, None);
        assert!(Fields::default().matches(None));
    }
}
