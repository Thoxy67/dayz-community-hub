//! What the player keeps with a favourite besides its address: a note, coloured
//! labels, a group (a named section of the favourites view), and a character
//! name and launch arguments of its own for joining it.
//!
//! Every field is optional on disk (`#[serde(default)]`, left out when empty),
//! so a profile written before these existed loads, and a build that does not
//! know them reads past them.
//!
//! Edits go through [`FavoriteEdit`] applied to a [`FavoriteBook`]: the hub
//! applies it to the profile it keeps, the window to its own copy for an
//! optimistic update, with the same rules (names trimmed and bounded, labels
//! and groups that do not exist dropped).

use serde::{Deserialize, Serialize};

use crate::Favorite;

/// Longest label or group name kept, in characters.
pub const MAX_NAME: usize = 32;
/// Longest note kept, in characters.
pub const MAX_NOTE: usize = 500;
/// Longest character name kept, in characters.
pub const MAX_CHARACTER: usize = 64;
/// Longest launch argument line kept, in characters.
pub const MAX_ARGS: usize = 500;
/// Most labels a profile defines.
pub const MAX_LABELS: usize = 24;
/// Most groups a profile defines.
pub const MAX_GROUPS: usize = 32;

/// A label's colour: one of the theme's tones, so it follows the theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelColor {
    #[default]
    Accent,
    Ok,
    Warn,
    Err,
    Info,
    Map,
    Mods,
    Neutral,
}

impl LabelColor {
    /// Every colour, in the order the picker shows them.
    pub const ALL: [LabelColor; 8] = [
        LabelColor::Accent,
        LabelColor::Ok,
        LabelColor::Warn,
        LabelColor::Err,
        LabelColor::Info,
        LabelColor::Map,
        LabelColor::Mods,
        LabelColor::Neutral,
    ];

    /// A colour this version does not know (a newer one wrote it, or a hand
    /// edit) reads as the default instead of failing the whole profile.
    fn lenient<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(serde_json::from_value(v).unwrap_or_default())
    }
}

/// A label the player made: a name and a colour. Favourites carry labels by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FavoriteLabel {
    pub name: String,
    #[serde(default, deserialize_with = "LabelColor::lenient")]
    pub color: LabelColor,
}

/// A named section of the favourites view, in the order the player put them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FavoriteGroup {
    pub name: String,
    /// Folded in the view: its favourites hidden under its heading.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub collapsed: bool,
}

/// What the edit dialog saves for one favourite.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteMeta {
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub character: Option<String>,
    #[serde(default)]
    pub launch_args: String,
}

impl FavoriteMeta {
    /// What a favourite holds now.
    pub fn of(f: &Favorite) -> Self {
        Self {
            note: f.note.clone(),
            labels: f.labels.clone(),
            group: f.group.clone(),
            character: f.character.clone(),
            launch_args: f.launch_args.clone(),
        }
    }
}

/// One change to the favourites' notes, labels or groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FavoriteEdit {
    /// The favourite at `ip:port` takes `meta` (labels and a group that do not
    /// exist are dropped).
    Meta {
        ip: String,
        port: u16,
        meta: FavoriteMeta,
    },
    /// Make a label, or (with `old`) rename and recolour one; the favourites
    /// carrying it follow the new name.
    SetLabel {
        old: Option<String>,
        name: String,
        color: LabelColor,
    },
    /// Delete a label, and take it off every favourite.
    RemoveLabel { name: String },
    /// Make a group, at the end.
    AddGroup { name: String },
    /// Rename a group; its favourites follow.
    RenameGroup { old: String, name: String },
    /// Delete a group; its favourites go back to no group.
    RemoveGroup { name: String },
    /// Move a group `delta` places (clamped to the ends).
    MoveGroup { name: String, delta: i32 },
    /// Fold or unfold a group.
    CollapseGroup { name: String, collapsed: bool },
    /// Put the favourite at `ip:port` in a group (or in none).
    MoveToGroup {
        ip: String,
        port: u16,
        group: Option<String>,
    },
}

/// What an edit names: a label or a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Label,
    Group,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Label => "label",
            Kind::Group => "group",
        }
    }
}

/// Why an edit was refused (it changed nothing). The window says it in the player's
/// language; [`std::fmt::Display`] is the English the hub's errors carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// No favourite is saved as `ip:port`.
    NotFavorite { ip: String, port: u16 },
    /// The name is empty once trimmed.
    NoName(Kind),
    /// Another label or group has that name.
    Taken(Kind, String),
    /// There are as many as a profile keeps already.
    TooMany(Kind, usize),
    /// No label or group has that name.
    Missing(Kind, String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::NotFavorite { ip, port } => write!(f, "{ip}:{port} is not a favourite"),
            EditError::NoName(k) => write!(f, "A {} needs a name", k.word()),
            EditError::Taken(k, n) => write!(f, "A {} named {n} exists already", k.word()),
            EditError::TooMany(k, max) => write!(f, "At most {max} {}s", k.word()),
            EditError::Missing(k, n) => write!(f, "No {} named {n}", k.word()),
        }
    }
}

impl std::error::Error for EditError {}

/// The favourites, their labels and their groups, to apply an edit to.
pub struct FavoriteBook<'a> {
    pub favorites: &'a mut Vec<Favorite>,
    pub labels: &'a mut Vec<FavoriteLabel>,
    pub groups: &'a mut Vec<FavoriteGroup>,
}

/// `s` trimmed, its line breaks and controls made spaces, at most `max` characters.
pub fn clean_line(s: &str, max: usize) -> String {
    s.trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

/// A note: trimmed, at most [`MAX_NOTE`] characters, line breaks kept, other
/// controls dropped.
pub fn clean_note(s: &str) -> String {
    s.trim()
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .take(MAX_NOTE)
        .collect::<String>()
        .trim()
        .to_string()
}

impl FavoriteBook<'_> {
    fn label_named(&self, name: &str) -> Option<usize> {
        self.labels.iter().position(|l| l.name == name)
    }

    fn group_named(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|g| g.name == name)
    }

    fn favorite_mut(&mut self, ip: &str, port: u16) -> Result<&mut Favorite, EditError> {
        self.favorites
            .iter_mut()
            .find(|f| f.ip == ip && f.port == port)
            .ok_or_else(|| EditError::NotFavorite {
                ip: ip.to_string(),
                port,
            })
    }

    /// Two labels or two groups of one name (a hand-edited or imported profile):
    /// the first of each name is kept.
    pub fn tidy(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        self.labels.retain(|l| {
            let new = !seen.contains(&l.name);
            if new {
                seen.push(l.name.clone());
            }
            new
        });
        seen.clear();
        self.groups.retain(|g| {
            let new = !seen.contains(&g.name);
            if new {
                seen.push(g.name.clone());
            }
            new
        });
    }

    /// Apply `edits` in order, all or none: the first refused one leaves the
    /// book as it was before the first.
    pub fn apply_all(&mut self, edits: Vec<FavoriteEdit>) -> Result<(), EditError> {
        let (favorites, labels, groups) = (
            self.favorites.clone(),
            self.labels.clone(),
            self.groups.clone(),
        );
        for e in edits {
            if let Err(err) = self.apply(e) {
                *self.favorites = favorites;
                *self.labels = labels;
                *self.groups = groups;
                return Err(err);
            }
        }
        Ok(())
    }

    /// Apply `edit`. Fails (changing nothing) on a favourite, label or group
    /// that does not exist, an empty name, or a name already taken.
    pub fn apply(&mut self, edit: FavoriteEdit) -> Result<(), EditError> {
        match edit {
            FavoriteEdit::Meta { ip, port, meta } => {
                let labels: Vec<String> = {
                    let mut out: Vec<String> = Vec::new();
                    for l in meta.labels {
                        let l = clean_line(&l, MAX_NAME);
                        if self.label_named(&l).is_some() && !out.contains(&l) {
                            out.push(l);
                        }
                    }
                    out
                };
                let group = meta
                    .group
                    .map(|g| clean_line(&g, MAX_NAME))
                    .filter(|g| self.group_named(g).is_some());
                let f = self.favorite_mut(&ip, port)?;
                f.note = clean_note(&meta.note);
                f.labels = labels;
                f.group = group;
                f.character = meta
                    .character
                    .map(|c| clean_line(&c, MAX_CHARACTER))
                    .filter(|c| !c.is_empty());
                f.launch_args = clean_line(&meta.launch_args, MAX_ARGS);
                Ok(())
            }
            FavoriteEdit::SetLabel { old, name, color } => {
                let name = clean_line(&name, MAX_NAME);
                if name.is_empty() {
                    return Err(EditError::NoName(Kind::Label));
                }
                match old {
                    Some(old) => {
                        let at = self
                            .label_named(&old)
                            .ok_or_else(|| EditError::Missing(Kind::Label, old.clone()))?;
                        if name != old && self.label_named(&name).is_some() {
                            return Err(EditError::Taken(Kind::Label, name));
                        }
                        if let Some(l) = self.labels.get_mut(at) {
                            l.name.clone_from(&name);
                            l.color = color;
                        }
                        for f in self.favorites.iter_mut() {
                            for l in &mut f.labels {
                                if *l == old {
                                    l.clone_from(&name);
                                }
                            }
                        }
                    }
                    None => {
                        if self.label_named(&name).is_some() {
                            return Err(EditError::Taken(Kind::Label, name));
                        }
                        if self.labels.len() >= MAX_LABELS {
                            return Err(EditError::TooMany(Kind::Label, MAX_LABELS));
                        }
                        self.labels.push(FavoriteLabel { name, color });
                    }
                }
                Ok(())
            }
            FavoriteEdit::RemoveLabel { name } => {
                let at = self
                    .label_named(&name)
                    .ok_or_else(|| EditError::Missing(Kind::Label, name.clone()))?;
                self.labels.remove(at);
                for f in self.favorites.iter_mut() {
                    f.labels.retain(|l| *l != name);
                }
                Ok(())
            }
            FavoriteEdit::AddGroup { name } => {
                let name = clean_line(&name, MAX_NAME);
                if name.is_empty() {
                    return Err(EditError::NoName(Kind::Group));
                }
                if self.group_named(&name).is_some() {
                    return Err(EditError::Taken(Kind::Group, name));
                }
                if self.groups.len() >= MAX_GROUPS {
                    return Err(EditError::TooMany(Kind::Group, MAX_GROUPS));
                }
                self.groups.push(FavoriteGroup {
                    name,
                    collapsed: false,
                });
                Ok(())
            }
            FavoriteEdit::RenameGroup { old, name } => {
                let name = clean_line(&name, MAX_NAME);
                if name.is_empty() {
                    return Err(EditError::NoName(Kind::Group));
                }
                let at = self
                    .group_named(&old)
                    .ok_or_else(|| EditError::Missing(Kind::Group, old.clone()))?;
                if name != old && self.group_named(&name).is_some() {
                    return Err(EditError::Taken(Kind::Group, name));
                }
                if let Some(g) = self.groups.get_mut(at) {
                    g.name.clone_from(&name);
                }
                for f in self.favorites.iter_mut() {
                    if f.group.as_deref() == Some(old.as_str()) {
                        f.group = Some(name.clone());
                    }
                }
                Ok(())
            }
            FavoriteEdit::RemoveGroup { name } => {
                let at = self
                    .group_named(&name)
                    .ok_or_else(|| EditError::Missing(Kind::Group, name.clone()))?;
                self.groups.remove(at);
                for f in self.favorites.iter_mut() {
                    if f.group.as_deref() == Some(name.as_str()) {
                        f.group = None;
                    }
                }
                Ok(())
            }
            FavoriteEdit::MoveGroup { name, delta } => {
                let at = self
                    .group_named(&name)
                    .ok_or_else(|| EditError::Missing(Kind::Group, name.clone()))?;
                let last = self.groups.len().saturating_sub(1);
                let to = if delta < 0 {
                    at.saturating_sub(delta.unsigned_abs() as usize)
                } else {
                    at.saturating_add(delta as usize).min(last)
                };
                if to != at {
                    let g = self.groups.remove(at);
                    self.groups.insert(to.min(self.groups.len()), g);
                }
                Ok(())
            }
            FavoriteEdit::CollapseGroup { name, collapsed } => {
                let at = self
                    .group_named(&name)
                    .ok_or_else(|| EditError::Missing(Kind::Group, name.clone()))?;
                if let Some(g) = self.groups.get_mut(at) {
                    g.collapsed = collapsed;
                }
                Ok(())
            }
            FavoriteEdit::MoveToGroup { ip, port, group } => {
                let group = match group {
                    Some(g) => Some(
                        self.group_named(&g)
                            .and_then(|i| self.groups.get(i))
                            .map(|g| g.name.clone())
                            .ok_or_else(|| EditError::Missing(Kind::Group, g.clone()))?,
                    ),
                    None => None,
                };
                self.favorite_mut(&ip, port)?.group = group;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fav(ip: &str, port: u16) -> Favorite {
        Favorite {
            name: format!("{ip}:{port}"),
            ip: ip.into(),
            port,
            ..Favorite::default()
        }
    }

    struct Owned {
        favorites: Vec<Favorite>,
        labels: Vec<FavoriteLabel>,
        groups: Vec<FavoriteGroup>,
    }

    impl Owned {
        fn new() -> Self {
            Self {
                favorites: vec![fav("1.1.1.1", 1), fav("2.2.2.2", 2)],
                labels: Vec::new(),
                groups: Vec::new(),
            }
        }

        fn book(&mut self) -> FavoriteBook<'_> {
            FavoriteBook {
                favorites: &mut self.favorites,
                labels: &mut self.labels,
                groups: &mut self.groups,
            }
        }

        fn apply(&mut self, e: FavoriteEdit) -> Result<(), EditError> {
            self.book().apply(e)
        }
    }

    fn label(name: &str, color: LabelColor) -> FavoriteEdit {
        FavoriteEdit::SetLabel {
            old: None,
            name: name.into(),
            color,
        }
    }

    fn group(name: &str) -> FavoriteEdit {
        FavoriteEdit::AddGroup { name: name.into() }
    }

    #[test]
    fn meta_keeps_only_labels_and_groups_that_exist() {
        let mut b = Owned::new();
        b.apply(label("PvE", LabelColor::Ok)).unwrap();
        b.apply(group("Weekend")).unwrap();
        b.apply(FavoriteEdit::Meta {
            ip: "1.1.1.1".into(),
            port: 1,
            meta: FavoriteMeta {
                note: "  admin is Bob\nwipe on Fridays \u{7}".into(),
                labels: vec!["PvE".into(), "ghost".into(), " PvE ".into()],
                group: Some("Nowhere".into()),
                character: Some("  ".into()),
                launch_args: " -nosplash  ".into(),
            },
        })
        .unwrap();
        let f = &b.favorites[0];
        assert_eq!(f.note, "admin is Bob\nwipe on Fridays");
        assert_eq!(f.labels, ["PvE"]);
        assert_eq!(f.group, None);
        assert_eq!(f.character, None);
        assert_eq!(f.launch_args, "-nosplash");
        assert!(
            b.apply(FavoriteEdit::Meta {
                ip: "9.9.9.9".into(),
                port: 1,
                meta: FavoriteMeta::default(),
            })
            .is_err()
        );
    }

    #[test]
    fn renaming_and_removing_a_label_follows_on_the_favourites() {
        let mut b = Owned::new();
        b.apply(label("PvE", LabelColor::Ok)).unwrap();
        b.apply(label("RP", LabelColor::Info)).unwrap();
        assert!(b.apply(label("PvE", LabelColor::Err)).is_err());
        assert!(b.apply(label("  ", LabelColor::Err)).is_err());
        b.favorites[0].labels = vec!["PvE".into(), "RP".into()];
        b.apply(FavoriteEdit::SetLabel {
            old: Some("PvE".into()),
            name: "Chill".into(),
            color: LabelColor::Warn,
        })
        .unwrap();
        assert_eq!(b.favorites[0].labels, ["Chill", "RP"]);
        assert_eq!(b.labels[0].color, LabelColor::Warn);
        assert!(
            b.apply(FavoriteEdit::SetLabel {
                old: Some("Chill".into()),
                name: "RP".into(),
                color: LabelColor::Warn,
            })
            .is_err()
        );
        b.apply(FavoriteEdit::RemoveLabel { name: "RP".into() })
            .unwrap();
        assert_eq!(b.favorites[0].labels, ["Chill"]);
        assert_eq!(b.labels.len(), 1);
    }

    #[test]
    fn groups_are_made_renamed_moved_folded_and_removed() {
        let mut b = Owned::new();
        for g in ["A", "B", "C"] {
            b.apply(group(g)).unwrap();
        }
        assert!(b.apply(group("A")).is_err());
        b.apply(FavoriteEdit::MoveToGroup {
            ip: "2.2.2.2".into(),
            port: 2,
            group: Some("B".into()),
        })
        .unwrap();
        assert!(
            b.apply(FavoriteEdit::MoveToGroup {
                ip: "2.2.2.2".into(),
                port: 2,
                group: Some("Z".into()),
            })
            .is_err()
        );
        b.apply(FavoriteEdit::RenameGroup {
            old: "B".into(),
            name: "Bravo".into(),
        })
        .unwrap();
        assert_eq!(b.favorites[1].group.as_deref(), Some("Bravo"));
        b.apply(FavoriteEdit::MoveGroup {
            name: "C".into(),
            delta: -10,
        })
        .unwrap();
        let names: Vec<&str> = b.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["C", "A", "Bravo"]);
        b.apply(FavoriteEdit::MoveGroup {
            name: "C".into(),
            delta: 1,
        })
        .unwrap();
        b.apply(FavoriteEdit::MoveGroup {
            name: "A".into(),
            delta: i32::MAX,
        })
        .unwrap();
        let names: Vec<&str> = b.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["C", "Bravo", "A"]);
        b.apply(FavoriteEdit::CollapseGroup {
            name: "Bravo".into(),
            collapsed: true,
        })
        .unwrap();
        assert!(b.groups[1].collapsed);
        b.apply(FavoriteEdit::RemoveGroup {
            name: "Bravo".into(),
        })
        .unwrap();
        assert_eq!(b.favorites[1].group, None);
        assert_eq!(b.groups.len(), 2);
    }

    #[test]
    fn an_unknown_label_colour_reads_as_the_default() {
        let l: FavoriteLabel =
            serde_json::from_str(r#"{"name":"x","color":"chartreuse"}"#).unwrap();
        assert_eq!(l.color, LabelColor::Accent);
        let l: FavoriteLabel = serde_json::from_str(r#"{"name":"x"}"#).unwrap();
        assert_eq!(l.color, LabelColor::Accent);
        let l: FavoriteLabel = serde_json::from_str(r#"{"name":"x","color":"warn"}"#).unwrap();
        assert_eq!(l.color, LabelColor::Warn);
    }

    #[test]
    fn refusals_say_what_and_which() {
        let mut b = Owned::new();
        b.apply(label("PvE", LabelColor::Ok)).unwrap();
        b.apply(group("A")).unwrap();
        assert_eq!(
            b.apply(label(" PvE ", LabelColor::Err)),
            Err(EditError::Taken(Kind::Label, "PvE".into()))
        );
        assert_eq!(b.apply(group(" ")), Err(EditError::NoName(Kind::Group)));
        assert_eq!(
            b.apply(FavoriteEdit::RemoveGroup { name: "B".into() }),
            Err(EditError::Missing(Kind::Group, "B".into()))
        );
        assert!(matches!(
            b.apply(FavoriteEdit::MoveToGroup {
                ip: "9.9.9.9".into(),
                port: 1,
                group: None
            }),
            Err(EditError::NotFavorite { .. })
        ));
        for i in 1..MAX_LABELS {
            b.apply(label(&format!("L{i}"), LabelColor::Ok)).unwrap();
        }
        assert_eq!(
            b.apply(label("one more", LabelColor::Ok)),
            Err(EditError::TooMany(Kind::Label, MAX_LABELS))
        );
        assert_eq!(
            EditError::Taken(Kind::Group, "A".into()).to_string(),
            "A group named A exists already"
        );
    }

    #[test]
    fn a_batch_goes_in_order_and_all_or_none() {
        let mut b = Owned::new();
        let meta = |g: &str| FavoriteEdit::Meta {
            ip: "1.1.1.1".into(),
            port: 1,
            meta: FavoriteMeta {
                group: Some(g.into()),
                labels: vec!["New".into()],
                ..FavoriteMeta::default()
            },
        };
        b.book()
            .apply_all(vec![group("G"), label("New", LabelColor::Info), meta("G")])
            .unwrap();
        assert_eq!(b.favorites[0].group.as_deref(), Some("G"));
        assert_eq!(b.favorites[0].labels, ["New"]);
        // The second is refused: the first is undone with it.
        assert!(b.book().apply_all(vec![group("H"), group("G")]).is_err());
        assert_eq!(b.groups.len(), 1);
    }

    #[test]
    fn tidying_keeps_the_first_of_each_name() {
        let mut b = Owned::new();
        b.groups = vec![
            FavoriteGroup {
                name: "A".into(),
                collapsed: true,
            },
            FavoriteGroup {
                name: "A".into(),
                collapsed: false,
            },
        ];
        b.labels = vec![
            FavoriteLabel {
                name: "L".into(),
                color: LabelColor::Ok,
            },
            FavoriteLabel {
                name: "L".into(),
                color: LabelColor::Err,
            },
        ];
        b.book().tidy();
        assert_eq!(b.groups.len(), 1);
        assert!(b.groups[0].collapsed);
        assert_eq!(b.labels.len(), 1);
        assert_eq!(b.labels[0].color, LabelColor::Ok);
    }
}
