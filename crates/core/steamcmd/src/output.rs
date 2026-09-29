//! Reading steamcmd's terminal output: escape sequences, prompts, mod ids.

use regex::Regex;
use std::sync::OnceLock;

/// Strip ANSI/VT100 escape sequences (and stray BEL / carriage returns) from a
/// string. steamcmd embeds colour codes even when stdout is a pipe, and under a
/// Windows ConPTY the output additionally carries private-mode CSI sequences
/// (`ESC[?25h`, `ESC[?1004h`, …) and OSC title sequences (`ESC]0;…BEL`). The
/// old `\x1b\[[0-9;]*[a-zA-Z]` pattern matched none of those, leaving garbage
/// like `[?9001h` in the displayed log.
///
/// Fast path: most steamcmd lines contain none of these bytes, so we
/// short-circuit with a single byte scan and return the borrowed slice.
pub(crate) fn strip_ansi(s: &str) -> std::borrow::Cow<'_, str> {
    if !s
        .as_bytes()
        .iter()
        .any(|&b| b == 0x1b || b == 0x07 || b == b'\r')
    {
        return std::borrow::Cow::Borrowed(s);
    }
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let re = ANSI_RE.get_or_init(|| {
        // OSC: ESC ] … (BEL | ST)  |  CSI: ESC [ params intermediates final
        // (params 0x30-0x3f covers digits ';' '?'), |  2-byte escapes, | lone BEL/CR
        Regex::new(
            r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[()][@-~]|\x1b[=>]|[\x07\r]",
        )
        .unwrap()
    });
    re.replace_all(s, "")
}

/// ASCII-only case-insensitive `contains`.  Avoids the per-line
/// `to_lowercase()` String allocation that the previous implementation made
/// for every steamcmd line we matched against multiple patterns.
pub(crate) fn ascii_contains_ci(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() {
        return true;
    }
    if h.len() < n.len() {
        return false;
    }
    'outer: for start in 0..=h.len() - n.len() {
        for (i, &nb) in n.iter().enumerate() {
            if !h[start + i].eq_ignore_ascii_case(&nb) {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

/// Returns true if the line (after stripping ANSI codes) indicates steamcmd is
/// waiting for any kind of Steam Guard confirmation — either mobile app approval
/// or an email/SMS code entered via stdin.
///
/// Matches patterns emitted by steamcmd:
///   1. "Please confirm the login in the Steam Mobile app on your phone."
///   2. "Waiting for confirmation..." (repeated while polling for mobile)
///   3. "Steam Guard code:" (email/SMS — steamcmd reads the code from stdin)
pub(crate) fn is_steam_guard_prompt(line: &str) -> bool {
    let clean = strip_ansi(line);
    let l = clean.as_ref();
    (ascii_contains_ci(l, "please confirm")
        && (ascii_contains_ci(l, "mobile") || ascii_contains_ci(l, "phone")))
        || ascii_contains_ci(l, "waiting for confirmation")
        || ascii_contains_ci(l, "steam guard code:")
}

/// Returns true if the partial buffer ends with a `password:` prompt.
/// steamcmd emits this without a trailing newline when cached credentials
/// are not found and it falls back to interactive login.
pub(crate) fn is_password_prompt(buf: &str) -> bool {
    let clean = strip_ansi(buf);
    let trimmed = clean.trim_end();
    trimmed.ends_with("password:") || trimmed.ends_with("password: ")
}

/// Regex for extracting a workshop mod ID from a steamcmd output line — compiled once.
static MOD_ID_RE: OnceLock<Regex> = OnceLock::new();

/// Extract a workshop mod ID (number) from a steamcmd output line.
/// Matches patterns like "item 1559212036" or "item:1559212036".
pub(crate) fn extract_mod_id_from_line(line: &str) -> Option<u64> {
    let re = MOD_ID_RE.get_or_init(|| Regex::new(r"item[: ]+(\d+)").unwrap());
    re.captures(line)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_conpty_sequences() {
        let raw = "\x1b]0;steamcmd\x07\x1b[?25hLogging in\x1b[0m\r";
        assert_eq!(strip_ansi(raw), "Logging in");
        assert!(matches!(strip_ansi("plain"), std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn detects_prompts() {
        assert!(is_steam_guard_prompt("Please confirm the login in the Steam Mobile app on your phone."));
        assert!(is_steam_guard_prompt("Steam Guard code:"));
        assert!(is_password_prompt("password: "));
        assert!(!is_password_prompt("Logging in user"));
    }

    #[test]
    fn extracts_mod_ids() {
        assert_eq!(
            extract_mod_id_from_line("Success. Downloaded item 1559212036 to ..."),
            Some(1559212036)
        );
        assert_eq!(extract_mod_id_from_line("item:42"), Some(42));
        assert_eq!(extract_mod_id_from_line("nothing"), None);
    }
}
