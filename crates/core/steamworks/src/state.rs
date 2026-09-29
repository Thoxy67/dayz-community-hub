//! What Steam says about an item, decoded: `EItemState` flags, `EResult`
//! codes, and the callback structures read from raw bytes.

use std::fmt;

/// `EItemState` flags (isteamugc.h).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemState(pub u32);

impl ItemState {
    pub const SUBSCRIBED: u32 = 1;
    pub const LEGACY: u32 = 2;
    pub const INSTALLED: u32 = 4;
    pub const NEEDS_UPDATE: u32 = 8;
    pub const DOWNLOADING: u32 = 16;
    pub const DOWNLOAD_PENDING: u32 = 32;
    pub const DISABLED_LOCALLY: u32 = 64;

    fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
    pub fn subscribed(self) -> bool {
        self.has(Self::SUBSCRIBED)
    }
    pub fn installed(self) -> bool {
        self.has(Self::INSTALLED)
    }
    pub fn needs_update(self) -> bool {
        self.has(Self::NEEDS_UPDATE)
    }
    pub fn downloading(self) -> bool {
        self.has(Self::DOWNLOADING)
    }
    pub fn pending(self) -> bool {
        self.has(Self::DOWNLOAD_PENDING)
    }
    /// Installed, current, and nothing more on its way: the download is over.
    pub fn ready(self) -> bool {
        self.installed() && !self.needs_update() && !self.downloading() && !self.pending()
    }
}

impl fmt::Debug for ItemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The flags in words, for the log: "subscribed, installed, downloading".
impl fmt::Display for ItemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NAMES: [(u32, &str); 7] = [
            (ItemState::SUBSCRIBED, "subscribed"),
            (ItemState::LEGACY, "legacy"),
            (ItemState::INSTALLED, "installed"),
            (ItemState::NEEDS_UPDATE, "needs update"),
            (ItemState::DOWNLOADING, "downloading"),
            (ItemState::DOWNLOAD_PENDING, "download pending"),
            (ItemState::DISABLED_LOCALLY, "disabled locally"),
        ];
        let words: Vec<&str> = NAMES
            .iter()
            .filter(|(flag, _)| self.has(*flag))
            .map(|(_, name)| *name)
            .collect();
        if words.is_empty() {
            f.write_str("unknown to Steam")
        } else {
            f.write_str(&words.join(", "))
        }
    }
}

/// `k_EResultOK`.
pub(crate) const RESULT_OK: i32 = 1;

/// An `EResult` a Workshop call failed with, as a sentence's end.
pub fn result_text(code: i32) -> String {
    match code {
        1 => "it worked".into(),
        2 => "Steam reported a generic failure".into(),
        3 => "Steam has no connection to its servers".into(),
        8 => "Steam rejected the request".into(),
        9 => "the item does not exist, or was removed from the Workshop".into(),
        15 => "access denied: the item is private, friends-only or hidden".into(),
        16 => "Steam timed out".into(),
        17 => "the account is banned from the Workshop".into(),
        20 => "the Workshop is unavailable right now".into(),
        25 => "Steam's limit was exceeded; try again later".into(),
        84 => "Steam is rate-limiting requests; try again later".into(),
        n => format!("Steam error code {n}"),
    }
}

/// Callback ids (steam_api_internal.h bases plus each struct's offset).
pub(crate) const CALL_COMPLETED: i32 = 703; // SteamAPICallCompleted_t
pub(crate) const SUBSCRIBE_RESULT: i32 = 1313; // RemoteStorageSubscribePublishedFileResult_t
pub(crate) const UNSUBSCRIBE_RESULT: i32 = 1315; // RemoteStorageUnsubscribePublishedFileResult_t
pub(crate) const DOWNLOAD_RESULT: i32 = 3406; // DownloadItemResult_t
pub(crate) const UGC_QUERY_COMPLETED: i32 = 3401; // SteamUGCQueryCompleted_t

/// Where a `uint64` that follows a 4-byte field starts: Valve packs
/// callbacks to 4 bytes on Linux and macOS, to 8 on Windows.
const U64_AFTER_U32: usize = if cfg!(windows) { 8 } else { 4 };

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// `SteamAPICallCompleted_t`: which call finished, its result's callback id
/// and size.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CallCompleted {
    pub call: u64,
    pub callback: i32,
    pub size: u32,
}

pub(crate) fn call_completed(b: &[u8]) -> Option<CallCompleted> {
    Some(CallCompleted {
        call: u64_at(b, 0)?,
        callback: i32_at(b, 8)?,
        size: u32_at(b, 12)?,
    })
}

/// `RemoteStorageSubscribePublishedFileResult_t`: (result, item). The
/// unsubscribe result (`…UnsubscribePublishedFileResult_t`) has the same layout.
pub(crate) fn subscribe_result(b: &[u8]) -> Option<(i32, u64)> {
    Some((i32_at(b, 0)?, u64_at(b, U64_AFTER_U32)?))
}

/// `SteamUGCQueryCompleted_t`: (query handle, result, results returned).
pub(crate) fn query_completed(b: &[u8]) -> Option<(u64, i32, u32)> {
    Some((u64_at(b, 0)?, i32_at(b, 8)?, u32_at(b, 12)?))
}

/// From the start of a `SteamUGCDetails_t`: the item's id, the result, and
/// its title (`char[129]` at offset 24, after four 4-byte fields; the same
/// under pack(4) and pack(8)).
pub(crate) fn details_head(b: &[u8]) -> Option<(u64, i32, String)> {
    let title = b.get(24..24 + 129)?;
    let end = title.iter().position(|&c| c == 0).unwrap_or(title.len());
    Some((
        u64_at(b, 0)?,
        i32_at(b, 8)?,
        String::from_utf8_lossy(&title[..end]).into_owned(),
    ))
}

/// Where `m_unNumChildren` sits in a `SteamUGCDetails_t`: after the 8000-byte
/// description, the 64-bit fields fall on 4-byte boundaries under Linux's
/// pack(4) and on 8-byte ones under Windows' pack(8).
const NUM_CHILDREN_AT: usize = if cfg!(windows) { 9768 } else { 9760 };

/// How many items a `SteamUGCDetails_t` says the item requires.
pub(crate) fn details_children(b: &[u8]) -> Option<u32> {
    u32_at(b, NUM_CHILDREN_AT)
}

/// `DownloadItemResult_t`: (app, item, result).
pub(crate) fn download_result(b: &[u8]) -> Option<(u32, u64, i32)> {
    Some((
        u32_at(b, 0)?,
        u64_at(b, U64_AFTER_U32)?,
        i32_at(b, U64_AFTER_U32 + 8)?,
    ))
}

/// A download's share done, 0-100; 0 while its size is unknown.
pub fn percent(done: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (done.min(total) as f64 / total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_is_ready_only_when_installed_and_idle() {
        let installed = ItemState(ItemState::SUBSCRIBED | ItemState::INSTALLED);
        assert!(installed.ready());
        for extra in [
            ItemState::NEEDS_UPDATE,
            ItemState::DOWNLOADING,
            ItemState::DOWNLOAD_PENDING,
        ] {
            assert!(!ItemState(installed.0 | extra).ready(), "{extra}");
        }
        assert!(!ItemState(ItemState::SUBSCRIBED).ready());
        assert!(!ItemState(0).ready());
    }

    #[test]
    fn states_read_as_words() {
        assert_eq!(ItemState(0).to_string(), "unknown to Steam");
        assert_eq!(
            ItemState(ItemState::SUBSCRIBED | ItemState::NEEDS_UPDATE | ItemState::DOWNLOADING)
                .to_string(),
            "subscribed, needs update, downloading"
        );
        // Steam's own value for a subscribed item being fetched: 1+8+16+32.
        let st = ItemState(57);
        assert!(st.subscribed() && st.needs_update() && st.downloading() && st.pending());
        assert!(!st.installed());
    }

    #[test]
    fn results_read_as_sentences() {
        assert!(result_text(15).contains("private"));
        assert!(result_text(9).contains("does not exist"));
        assert_eq!(result_text(1234), "Steam error code 1234");
    }

    #[test]
    fn percent_is_bounded_and_safe_on_an_unknown_size() {
        assert_eq!(percent(0, 0), 0.0);
        assert_eq!(percent(50, 200), 25.0);
        assert_eq!(percent(300, 200), 100.0);
    }

    #[test]
    fn call_completed_is_read_from_its_bytes() {
        let mut b = Vec::new();
        b.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        b.extend_from_slice(&SUBSCRIBE_RESULT.to_le_bytes());
        b.extend_from_slice(&16u32.to_le_bytes());
        assert_eq!(
            call_completed(&b),
            Some(CallCompleted {
                call: 0x1122_3344_5566_7788,
                callback: SUBSCRIBE_RESULT,
                size: 16
            })
        );
        assert_eq!(call_completed(&b[..12]), None);
    }

    /// The bytes of a struct `{ 4-byte field; uint64; … }` as Valve packs it
    /// on this platform.
    fn packed(first: [u8; 4], id: u64, rest: &[u8]) -> Vec<u8> {
        let mut b = first.to_vec();
        b.resize(U64_AFTER_U32, 0);
        b.extend_from_slice(&id.to_le_bytes());
        b.extend_from_slice(rest);
        b
    }

    #[test]
    fn subscribe_and_download_results_follow_valves_packing() {
        let sub = packed(15i32.to_le_bytes(), 1_559_212_036, &[]);
        assert_eq!(subscribe_result(&sub), Some((15, 1_559_212_036)));

        let dl = packed(221_100u32.to_le_bytes(), 1_559_212_036, &9i32.to_le_bytes());
        assert_eq!(download_result(&dl), Some((221_100, 1_559_212_036, 9)));
        assert_eq!(download_result(&dl[..dl.len() - 1]), None);
    }
}
