//! What the mods view shows, worked out from the store (= the derived state of
//! `features/mods/View.svelte`, `where.ts`, `review.svelte.ts`, `steamcmd-log.ts`): the
//! rows (installed mods and Steam's incoming subscriptions), filtered, sorted and grouped
//! by status; whose copy a mod is; Workshop ids out of pasted text; SteamCMD's progress.

use dz_hub::features::mods::{InstalledModDto, ModSourceDto};
use dz_hub::features::steamworks::SteamSubscriptionDto;
use rustc_hash::FxHashSet;

use crate::store::mods::Mods;

/// Which rows show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    #[default]
    All,
    Updates,
    Unmanaged,
}

impl Filter {
    pub fn from_id(s: &str) -> Option<Filter> {
        match s {
            "all" => Some(Filter::All),
            "updates" => Some(Filter::Updates),
            "unmanaged" => Some(Filter::Unmanaged),
            _ => None,
        }
    }
}

/// The column the list is sorted by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Col {
    Name,
    #[default]
    Status,
    Managed,
    Size,
    Local,
}

impl Col {
    pub fn from_id(s: &str) -> Option<Col> {
        match s {
            "name" => Some(Col::Name),
            "status" => Some(Col::Status),
            "managed" => Some(Col::Managed),
            "size" => Some(Col::Size),
            "local" => Some(Col::Local),
            _ => None,
        }
    }

    /// The first click on a column sorts it this way.
    pub fn first_asc(self) -> bool {
        matches!(self, Col::Name | Col::Status | Col::Managed)
    }
}

/// Status groups, in the order the list sorts them.
pub const INCOMING: i8 = -1;
pub const BEHIND: i8 = 0;
pub const UNCHECKED: i8 = 1;
pub const CURRENT: i8 = 2;

/// A row: an installed mod (`installed` is its index in the store's list), or a
/// subscription Steam is still fetching (`None`), not on disk yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: u64,
    pub installed: Option<usize>,
    pub name: String,
    pub size: u64,
    pub status: i8,
}

/// What the list draws: a group's heading or a row (an index into [`Listing::rows`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    Group { status: i8, count: usize, size: u64 },
    Row(usize),
}

/// The list as it stands.
#[derive(Default)]
pub struct Listing {
    /// Every row, installed or incoming, before the filters (`all`).
    pub total: usize,
    /// Rows that pass the filters, sorted.
    pub rows: Vec<Row>,
    /// What is drawn: rows under group headings when sorted by status, folded groups
    /// without their rows.
    pub items: Vec<Item>,
    /// Incoming rows (Steam fetching, not on disk).
    pub incoming: usize,
    /// What the listing was made from.
    key: u64,
}

/// What the list is made from: the store and the view's choices.
pub struct Query<'a> {
    pub query: &'a str,
    pub filter: Filter,
    pub col: Col,
    pub asc: bool,
    pub folded: &'a FxHashSet<i8>,
}

/// A mod's status group.
pub fn status_of(m: Option<&InstalledModDto>) -> i8 {
    match m {
        None => INCOMING,
        Some(m) if m.update_available => BEHIND,
        Some(m) if m.remote_updated.is_some() => CURRENT,
        Some(_) => UNCHECKED,
    }
}

/// A Steam item that is subscribed, not on disk, and on its way.
fn is_incoming(i: &SteamSubscriptionDto, mods: &Mods) -> bool {
    i.subscribed
        && mods.by_id(i.id).is_none()
        && (i.downloading || i.pending || i.needs_update || !i.installed)
}

fn fingerprint(mods: &Mods, q: &Query<'_>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    for m in &mods.installed {
        (
            m.id,
            m.managed,
            m.update_available,
            m.remote_updated,
            m.local_updated,
            m.size,
            &m.name,
        )
            .hash(&mut h);
    }
    if let Some(s) = &mods.steam {
        for i in &s.items {
            (
                i.id,
                i.subscribed,
                i.installed,
                i.needs_update,
                i.downloading,
                i.pending,
                i.bytes_total,
            )
                .hash(&mut h);
        }
        s.details.len().hash(&mut h);
    }
    mods.op.names.len().hash(&mut h);
    (q.query, q.filter, q.col, q.asc).hash(&mut h);
    let mut folded: Vec<i8> = q.folded.iter().copied().collect();
    folded.sort_unstable();
    folded.hash(&mut h);
    h.finish()
}

fn by_name(a: &Row, b: &Row) -> std::cmp::Ordering {
    let (la, lb) = (a.name.to_lowercase(), b.name.to_lowercase());
    la.cmp(&lb).then_with(|| a.name.cmp(&b.name))
}

impl Listing {
    /// Brings the listing up to date; work only when something it shows changed.
    pub fn update(&mut self, mods: &Mods, q: &Query<'_>) {
        let key = fingerprint(mods, q);
        if key == self.key && self.key != 0 {
            return;
        }
        self.key = key;
        let mut all: Vec<Row> = Vec::with_capacity(mods.installed.len() + 4);
        if let Some(s) = &mods.steam {
            for i in s.items.iter().filter(|i| is_incoming(i, mods)) {
                all.push(Row {
                    id: i.id,
                    installed: None,
                    name: mods.name_of(i.id),
                    size: i.bytes_total,
                    status: INCOMING,
                });
            }
        }
        self.incoming = all.len();
        for (i, m) in mods.installed.iter().enumerate() {
            all.push(Row {
                id: m.id,
                installed: Some(i),
                name: m.name.clone(),
                size: m.size,
                status: status_of(Some(m)),
            });
        }
        self.total = all.len();
        let needle = q.query.trim().to_lowercase();
        let installed = |r: &Row| r.installed.and_then(|i| mods.installed.get(i));
        all.retain(|r| {
            let m = installed(r);
            let pass = match q.filter {
                Filter::All => true,
                Filter::Updates => m.is_none_or(|m| m.update_available),
                Filter::Unmanaged => m.is_some_and(|m| !m.managed),
            };
            pass && (needle.is_empty()
                || r.name.to_lowercase().contains(&needle)
                || r.id.to_string().contains(&needle))
        });
        let asc = q.asc;
        let dir = |o: std::cmp::Ordering| if asc { o } else { o.reverse() };
        all.sort_by(|a, b| match q.col {
            Col::Name => dir(by_name(a, b)),
            Col::Size => dir(a.size.cmp(&b.size)),
            Col::Local => {
                let la = installed(a).map_or(0, |m| m.local_updated);
                let lb = installed(b).map_or(0, |m| m.local_updated);
                dir(la.cmp(&lb))
            }
            Col::Managed => {
                let ma = installed(a).is_some_and(|m| m.managed);
                let mb = installed(b).is_some_and(|m| m.managed);
                dir(mb.cmp(&ma)).then_with(|| by_name(a, b))
            }
            Col::Status => dir(a.status.cmp(&b.status)).then_with(|| by_name(a, b)),
        });
        self.rows = all;
        self.items.clear();
        if q.col != Col::Status {
            self.items.extend((0..self.rows.len()).map(Item::Row));
            return;
        }
        let mut cur: Option<i8> = None;
        for (i, r) in self.rows.iter().enumerate() {
            if cur != Some(r.status) {
                cur = Some(r.status);
                let members = self.rows.iter().filter(|x| x.status == r.status);
                let (count, size) = members.fold((0, 0u64), |(c, s), x| (c + 1, s + x.size));
                self.items.push(Item::Group {
                    status: r.status,
                    count,
                    size,
                });
            }
            if !q.folded.contains(&r.status) {
                self.items.push(Item::Row(i));
            }
        }
    }

    /// Forget what the listing was made from (the next update rebuilds it).
    pub fn invalidate(&mut self) {
        self.key = 0;
    }

    /// The rows the keyboard walks (not folded away), as indices into `rows`.
    pub fn visible(&self) -> impl Iterator<Item = usize> + '_ {
        self.items.iter().filter_map(|i| match i {
            Item::Row(r) => Some(*r),
            Item::Group { .. } => None,
        })
    }

    /// Where row `r` is among the items.
    pub fn item_of(&self, r: usize) -> Option<usize> {
        self.items.iter().position(|i| *i == Item::Row(r))
    }

    pub fn row_by_id(&self, id: u64) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }
}

/// Whose copy of a mod the launcher uses (`where.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    /// The launcher's own download.
    Launcher,
    /// A Steam subscription: Steam keeps it current.
    Subscribed,
    /// A Steam library copy the account is not subscribed to: nobody updates it.
    Unsubscribed,
    /// A Steam library copy while Steam cannot be asked.
    Steam,
}

pub fn where_of(m: &InstalledModDto, item: Option<&SteamSubscriptionDto>, answered: bool) -> Where {
    if m.source == ModSourceDto::Launcher {
        return Where::Launcher;
    }
    if !answered {
        return Where::Steam;
    }
    if item.is_some_and(|i| i.subscribed) {
        Where::Subscribed
    } else {
        Where::Unsubscribed
    }
}

impl Where {
    /// The quiet line under a name (`line`) or the details' tag.
    pub fn label(self, line: bool) -> &'static str {
        let k = match (self, line) {
            (Where::Launcher, true) => "whereLauncher",
            (Where::Launcher, false) => "sourceLauncher",
            (Where::Subscribed, true) => "whereSubscribed",
            (Where::Subscribed, false) => "sourceSubscribed",
            (Where::Unsubscribed, true) => "whereNotSubscribed",
            (Where::Steam, true) => "whereSteam",
            (Where::Unsubscribed | Where::Steam, false) => "sourceSteam",
        };
        crate::i18n::t("mods", k)
    }

    /// The sentence that says what it means.
    pub fn hint(self) -> &'static str {
        let k = match self {
            Where::Launcher => "sourceLauncherHint",
            Where::Subscribed => "subscribedHint",
            Where::Unsubscribed => "notSubscribedHint",
            Where::Steam => "sourceSteamHint",
        };
        crate::i18n::t("mods", k)
    }
}

/// Whole days the Workshop's copy is ahead of the one on disk.
pub fn days_behind(local: i64, remote: Option<i64>) -> i64 {
    match remote {
        Some(r) if r > local => (r - local) / 86_400,
        _ => 0,
    }
}

// ── pasted Workshop ids (`review.svelte.ts`) ────────────────────────────────

/// One Workshop id out of a word: a bare number or a link with `?id=`.
pub fn parse_workshop_id(raw: &str) -> Option<u64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let digits: &str = match ["?id=", "&id="].iter().find_map(|k| lower.find(k)) {
        Some(at) => {
            let rest = t.get(at + 4..)?;
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            rest.get(..end)?
        }
        None if t.chars().all(|c| c.is_ascii_digit()) => t,
        None => return None,
    };
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u64>().ok().filter(|id| *id > 0)
}

/// Every distinct id in a block of text, in the order first seen.
pub fn parse_workshop_ids(text: &str) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for part in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        if let Some(id) = parse_workshop_id(part)
            && !out.contains(&id)
        {
            out.push(id);
            if out.len() >= 10_000 {
                break;
            }
        }
    }
    out
}

pub fn workshop_url(id: u64) -> String {
    format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}")
}

// ── A download's progress ──────────────────────────────────────────────────

/// How far the mod being downloaded has got: percent, bytes done, bytes in all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub percent: f64,
    pub done: u64,
    pub total: u64,
    /// When it came, ms after the operation started.
    pub at: u64,
}

/// Where `needle` (ASCII, lowercase) first is in `hay`, ignoring ASCII case: no
/// lowercase copy of the line, which the log is searched through line by line.
pub fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() {
        return Some(0);
    }
    h.windows(n.len())
        .position(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

/// The operation's progress on the mod under way, from the hub's numbers (SteamCMD's
/// lines are read once, in the backend): `None` until the first after it started.
pub fn op_progress(op: &crate::store::mods::ModOp) -> Option<Progress> {
    op.bytes.map(|b| Progress {
        percent: b.bytes.percent(),
        done: b.bytes.done,
        total: b.bytes.total,
        at: b.at_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_without_regard_to_ascii_case() {
        assert_eq!(find_ci("Update state PROGRESS: 1", "progress:"), Some(13));
        assert_eq!(find_ci("Downloading Item 5", "downloading item"), Some(0));
        assert_eq!(find_ci("nothing", "progress:"), None);
        assert_eq!(
            find_ci("é progress:", "progress:"),
            Some(3),
            "bytes, not chars"
        );
    }

    #[test]
    fn workshop_ids_from_text() {
        assert_eq!(parse_workshop_id("1559212036"), Some(1_559_212_036));
        assert_eq!(
            parse_workshop_id("https://steamcommunity.com/sharedfiles/filedetails/?id=1564026768"),
            Some(1_564_026_768)
        );
        assert_eq!(parse_workshop_id("x?foo=1&ID=42&b=2"), Some(42));
        assert_eq!(parse_workshop_id("abc"), None);
        assert_eq!(parse_workshop_id("0"), None);
        assert_eq!(parse_workshop_id("99999999999999999999999"), None);
        assert_eq!(
            parse_workshop_ids("1, 2;3\n2 https://x/?id=4 nope"),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn days() {
        assert_eq!(days_behind(0, Some(86_400 * 3 + 5)), 3);
        assert_eq!(days_behind(10, Some(5)), 0);
        assert_eq!(days_behind(10, None), 0);
    }
}
