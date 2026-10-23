//! Downloading workshop mods through steamcmd, with per-mod progress.

use dz_common::{Error, Result};

use crate::output::{
    ascii_contains_ci, clean_line, extract_mod_id_from_line, is_login_ok, is_password_prompt,
    is_steam_guard_prompt, progress_bytes,
};
use crate::{ModProgress, ProgressTx, PtyInputRx, SteamCmd};

/// Most `password:` prompts answered in one session: a saved password once,
/// then what the player types.
const MAX_PASSWORD_PROMPTS: u8 = 3;

/// Most items one session is given. Each adds about fifty characters to
/// SteamCMD's command line, and Windows refuses a command line past 32 767
/// (CreateProcessW): a larger batch runs as several sessions, one after the
/// other, on the login the first one cached.
const SESSION_ITEMS: usize = 300;

/// How many more sessions retry the items SteamCMD gave up on with a
/// timeout, which large mods commonly hit. SteamCMD resumes from the chunks
/// it already has, on its cached login.
const TIMEOUT_RETRIES: u8 = 2;

/// What one session saw, besides the progress it sent.
#[derive(Default)]
struct Seen {
    succeeded: rustc_hash::FxHashSet<u64>,
    /// Items SteamCMD said failed, with its reason (`Timeout`, …).
    failed: rustc_hash::FxHashMap<u64, String>,
    logged_in: bool,
    steam_guard_timed_out: bool,
    invalid_password: bool,
    /// The player cancelled at the password prompt.
    cancelled: bool,
}

/// What carries from one session of an operation to the next.
struct Carry<'a> {
    /// Each item's place in the operation and its name.
    lookup: rustc_hash::FxHashMap<u64, (usize, &'a str)>,
    total: usize,
    /// A password saved by an earlier version: typed once, in the first
    /// session that asks.
    saved_password: Option<String>,
    saved_password_tried: bool,
    /// `LoggedIn` was sent: once per operation.
    announced: bool,
}

impl SteamCmd {
    /// Download/update multiple mods with per-mod progress, into SteamCMD's
    /// own directory. The Steam client keeps running: SteamCMD does not
    /// touch its libraries.
    ///
    /// Batches the mods into steamcmd sessions of at most [`SESSION_ITEMS`]
    /// and streams their output line by line via a PTY (Linux) / ConPTY
    /// (Windows), so progress is reported as each mod finishes. Items that
    /// timed out are tried again, [`TIMEOUT_RETRIES`] times at most.
    ///
    /// `mods_info` is a list of (mod_id, display_name) pairs. `validate`
    /// re-checks every file of every mod: only for a repair.
    pub async fn download_mods_with_progress(
        &self,
        mods_info: &[(u64, String)],
        validate: bool,
        tx: &ProgressTx,
        mut pty_input: PtyInputRx,
    ) -> Vec<(u64, Result<()>)> {
        let total = mods_info.len();

        if total == 0 {
            let _ = tx.send(ModProgress::Finished {
                ok: 0,
                failed: 0,
                total: 0,
                hint: None,
            });
            return Vec::new();
        }

        if !self.has_real_login() {
            let err = "Workshop downloads require a non-anonymous Steam login".to_string();
            let results: Vec<(u64, Result<()>)> = mods_info
                .iter()
                .map(|(id, _)| (*id, Err(Error::SteamCmd(err.clone()))))
                .collect();
            let _ = tx.send(ModProgress::Finished {
                ok: 0,
                failed: total,
                total,
                hint: Some("Set your Steam login in Settings → Steam first.".to_string()),
            });
            return results;
        }

        let _ = tx.send(ModProgress::Starting {
            current: 1,
            total,
            mod_id: mods_info[0].0,
            name: mods_info[0].1.clone(),
        });

        let mut carry = self.carry(mods_info);
        let ids: Vec<u64> = mods_info.iter().map(|(id, _)| *id).collect();
        let mut seen = Seen::default();
        let mut go_on = true;
        for chunk in ids.chunks(SESSION_ITEMS) {
            go_on = self
                .session_into(chunk, validate, &mut carry, &mut seen, tx, &mut pty_input)
                .await;
            if !go_on {
                break;
            }
        }
        for round in 1..=TIMEOUT_RETRIES {
            let again: Vec<u64> = ids
                .iter()
                .copied()
                .filter(|id| seen.failed.get(id).is_some_and(|why| is_timeout(why)))
                .collect();
            if !go_on || again.is_empty() {
                break;
            }
            let _ = tx.send(ModProgress::LogLine(format!(
                "Trying again the {} item(s) that timed out ({round}/{TIMEOUT_RETRIES})",
                again.len()
            )));
            for chunk in again.chunks(SESSION_ITEMS) {
                go_on = self
                    .session_into(chunk, validate, &mut carry, &mut seen, tx, &mut pty_input)
                    .await;
                if !go_on {
                    break;
                }
            }
        }

        // Build final results — anything not seen as succeeded is failed
        let results: Vec<(u64, Result<()>)> = mods_info
            .iter()
            .enumerate()
            .map(|(idx, (id, name))| {
                if seen.succeeded.contains(id) {
                    return (*id, Ok(()));
                }
                let error = match seen.failed.get(id) {
                    Some(why) => format!("Download failed ({why})"),
                    None => "Download failed".to_string(),
                };
                let _ = tx.send(ModProgress::Failed {
                    current: idx + 1,
                    total,
                    mod_id: *id,
                    name: name.clone(),
                    error: error.clone(),
                });
                (*id, Err(Error::SteamCmd(error)))
            })
            .collect();

        let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
        let failed = total - ok;
        let timed_out = results
            .iter()
            .any(|(id, r)| r.is_err() && seen.failed.get(id).is_some_and(|w| is_timeout(w)));
        let hint = finished_hint(&seen, failed > 0, timed_out);
        let _ = tx.send(ModProgress::Finished {
            ok,
            failed,
            total,
            hint,
        });
        results
    }

    /// Log in and quit (`+login <user> +quit`), in the same PTY session as a
    /// download: the password and Steam Guard are asked for the same way,
    /// and SteamCMD caches the login for the downloads after. Ends with a
    /// `Finished` whose `ok` is 1 when the login worked.
    pub async fn login_with_progress(&self, tx: &ProgressTx, mut pty_input: PtyInputRx) {
        if !self.has_real_login() {
            let _ = tx.send(ModProgress::Finished {
                ok: 0,
                failed: 1,
                total: 1,
                hint: Some("Set your Steam login in Settings → Steam first.".to_string()),
            });
            return;
        }
        let mut carry = self.carry(&[]);
        let mut seen = Seen::default();
        self.session_into(&[], false, &mut carry, &mut seen, tx, &mut pty_input)
            .await;
        let _ = tx.send(ModProgress::Finished {
            ok: usize::from(seen.logged_in),
            failed: usize::from(!seen.logged_in),
            total: 1,
            hint: finished_hint(&seen, false, false),
        });
    }

    fn carry<'a>(&self, mods_info: &'a [(u64, String)]) -> Carry<'a> {
        Carry {
            // O(1) mod_id → (idx, name) lookup so we don't linear-scan
            // mods_info on every "Downloading"/"Success" line.
            lookup: mods_info
                .iter()
                .enumerate()
                .map(|(idx, (id, name))| (*id, (idx, name.as_str())))
                .collect(),
            total: mods_info.len(),
            saved_password: self.saved_password.clone(),
            saved_password_tried: false,
            announced: false,
        }
    }

    /// Run one session for `items` and fold what it saw into `seen`: an item
    /// that came through leaves the failed ones. False when the operation
    /// must stop there (no login, the player cancelled).
    async fn session_into(
        &self,
        items: &[u64],
        validate: bool,
        carry: &mut Carry<'_>,
        seen: &mut Seen,
        tx: &ProgressTx,
        pty_input: &mut PtyInputRx,
    ) -> bool {
        let now = self
            .run_session(items, validate, carry, tx, pty_input)
            .await;
        for id in &now.succeeded {
            seen.failed.remove(id);
        }
        seen.succeeded.extend(now.succeeded);
        for (id, why) in now.failed {
            if !seen.succeeded.contains(&id) {
                seen.failed.insert(id, why);
            }
        }
        seen.logged_in |= now.logged_in;
        seen.steam_guard_timed_out |= now.steam_guard_timed_out;
        seen.invalid_password |= now.invalid_password;
        seen.cancelled |= now.cancelled;
        now.logged_in && !now.cancelled
    }

    /// One steamcmd session under a PTY: log in, download `items` (none for
    /// a login alone), quit. Reports progress and the login on `tx`; the
    /// caller sends each failure and `Finished`.
    async fn run_session(
        &self,
        items: &[u64],
        validate: bool,
        carry: &mut Carry<'_>,
        tx: &ProgressTx,
        pty_input: &mut PtyInputRx,
    ) -> Seen {
        let total = carry.total;
        let args = self.session_args(items, validate);
        let mut seen = Seen::default();

        // SteamCMD's own places: created here, the Steam client's never.
        let _ = std::fs::create_dir_all(&self.content_dir);
        if let Some(home) = &self.home_dir {
            let _ = std::fs::create_dir_all(home);
        }
        self.detach_steam_library_link();

        let (mut child, mut chunk_rx, mut pty_writer) = match self.spawn_pty_streamed(&args) {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.send(ModProgress::LogLine(e.clone()));
                seen.failed = items.iter().map(|id| (*id, e.clone())).collect();
                return seen;
            }
        };

        // Accumulate all output so we can split on newlines ourselves.
        // Steam Guard mobile is detected by text: steamcmd flushes the
        // "Please confirm..." / "Waiting for confirmation..." lines once the
        // user approves on the phone.
        let mod_lookup = &carry.lookup;
        let mut current_idx: usize = 0;
        let mut steam_guard_sent = false;
        let mut password_prompts: u8 = 0;
        let mut logged_in = false;
        let mut sg_start: Option<std::time::Instant> = None;
        let mut login_phase = true; // true until first download activity is seen
        let mut buf = String::new();
        // ConPTY (portable-pty opens it with PSUEDOCONSOLE_INHERIT_CURSOR) sends
        // a DSR cursor-position query before producing any output and BLOCKS the
        // child until it gets a reply. We answer it exactly once. See below.
        let mut dsr_replied = false;
        // Last transient (carriage-return) progress line we forwarded, so we can
        // skip re-sending identical "progress: x%" lines.
        let mut last_progress = String::new();
        // The item SteamCMD is downloading: its progress lines do not say.
        let mut downloading: Option<u64> = None;

        loop {
            let maybe_chunk = chunk_rx.recv().await;
            let raw_chunk = match maybe_chunk {
                Some(c) => c,
                None => break, // channel closed — steamcmd exited
            };

            // ── Unblock ConPTY's startup cursor-position query ──────────────
            // The Windows ConPTY emits `ESC[6n` (Device Status Report: report
            // cursor position) at startup and will not run the child until a
            // terminal replies with `ESC[<row>;<col>R` on stdin. Nothing replied
            // before, so steamcmd hung with an empty log panel. Reply once with a
            // synthetic position to release it. (Harmless no-op on a Linux PTY,
            // which doesn't send this query.)
            if !dsr_replied && raw_chunk.contains("\x1b[6n") {
                use std::io::Write;
                let _ = pty_writer.write_all(b"\x1b[1;1R");
                let _ = pty_writer.flush();
                dsr_replied = true;
            }

            // Prevent unbounded buffer growth (1 MB limit)
            const MAX_BUF_SIZE: usize = 1024 * 1024;
            if buf.len() + raw_chunk.len() > MAX_BUF_SIZE {
                buf.clear();
                eprintln!("[SteamCmd] output buffer exceeded limit, clearing");
            }
            buf.push_str(&raw_chunk);

            // Process any complete newline-terminated lines that have accumulated.
            // Hot path: a single mod download produces hundreds of progress lines.
            // Previously each line allocated 4–6 Strings (raw_line, buf realloc,
            // strip_ansi.into_owned, line.clone for log, two to_lowercase copies).
            // We now slice into `buf` in place, drain after we're done reading,
            // and use ASCII case-insensitive contains so the only allocation
            // per line is the one the channel send genuinely requires.
            // The lines are read in place and drained once at the end: a
            // drain per line moved the rest of a 4 KiB chunk once per line.
            let mut start = 0;
            while let Some(nl) = buf[start..].find('\n') {
                let nl_pos = start + nl;
                {
                    let stripped = clean_line(&buf[start..nl_pos]);
                    let line: &str = stripped.as_ref();

                    // Forward every non-empty line to the UI log panel.
                    if !line.trim().is_empty() {
                        let _ = tx.send(ModProgress::LogLine(line.to_owned()));
                    }
                    if let (Some(mod_id), Some((done, total))) = (downloading, progress_bytes(line))
                    {
                        let _ = tx.send(ModProgress::Bytes {
                            mod_id,
                            done,
                            total,
                        });
                    }

                    // Text-based Steam Guard detection on complete lines.
                    // NOTE: no `has_password` guard — Steam Guard can appear
                    // after the interactive password prompt flow too.
                    if login_phase && !steam_guard_sent && is_steam_guard_prompt(line) {
                        let _ = tx.send(ModProgress::SteamGuardMobileRequired);
                        steam_guard_sent = true;
                        sg_start = Some(std::time::Instant::now());
                        eprintln!("[SteamGuard] prompt detected (t=0.0s)");
                    }

                    // Detect login errors from complete lines. Only while
                    // logging in: "ERROR! Download item … failed (Timeout)."
                    // later is a download's timeout, not Steam Guard's.
                    let elapsed = sg_start.map(|s| s.elapsed().as_secs_f32()).unwrap_or(0.0);
                    if login_phase && is_steam_guard_timeout(line) {
                        seen.steam_guard_timed_out = true;
                        eprintln!("[SteamGuard] TIMED OUT (t={elapsed:.1}s)");
                    }
                    if ascii_contains_ci(line, "invalid password") {
                        seen.invalid_password = true;
                        if carry.saved_password_tried {
                            let _ = tx.send(ModProgress::SavedPasswordRefused);
                        }
                    }
                    if !logged_in && is_login_ok(line) {
                        logged_in = true;
                        login_phase = false;
                        announce(&mut carry.announced, tx);
                    }
                    if let Some((id, why)) = download_failure(line) {
                        login_phase = false;
                        seen.failed.insert(id, why);
                    }
                    // Log when Steam Guard is resolved.
                    if steam_guard_sent
                        && !seen.steam_guard_timed_out
                        && ascii_contains_ci(line, "waiting for")
                        && ascii_contains_ci(line, "ok")
                    {
                        eprintln!("[SteamGuard] confirmed OK (t={elapsed:.1}s)");
                    }
                    if ascii_contains_ci(line, "retrying") {
                        eprintln!("[SteamGuard] retrying (t={elapsed:.1}s)");
                    }

                    if line.contains("Success. Downloaded item")
                        || line.contains("already up to date")
                    {
                        login_phase = false;
                        if !logged_in {
                            logged_in = true;
                            announce(&mut carry.announced, tx);
                        }
                        if let Some(id) = extract_mod_id_from_line(line)
                            && let Some(&(idx, name)) = mod_lookup.get(&id)
                        {
                            seen.failed.remove(&id);
                            seen.succeeded.insert(id);
                            let _ = tx.send(ModProgress::Done {
                                current: idx + 1,
                                total,
                                mod_id: id,
                                name: name.to_owned(),
                            });
                            current_idx = idx + 1;
                        }
                    }
                    if line.contains("Downloading item") {
                        login_phase = false;
                        let id = extract_mod_id_from_line(line);
                        downloading = id.filter(|id| mod_lookup.contains_key(id));
                        if let Some(id) = id
                            && let Some(&(idx, name)) = mod_lookup.get(&id)
                            && idx + 1 > current_idx
                        {
                            let _ = tx.send(ModProgress::Starting {
                                current: idx + 1,
                                total,
                                mod_id: id,
                                name: name.to_owned(),
                            });
                            current_idx = idx + 1;
                        }
                    }
                }
                start = nl_pos + 1;
            }
            buf.drain(..start);

            // ── Real-time download progress (carriage-return overwrites) ────
            // steamcmd reports download progress by rewriting one line with '\r'
            // (e.g. "Update state (0x61) downloading, progress: 45.50 …") and
            // only emits a final '\n' when the item completes. The newline loop
            // above never sees those, so without this the log looks frozen for
            // the whole download. The visible "current line" is whatever follows
            // the last '\r' in the partial buffer; forward it as a transient line
            // that the UI overwrites in place. Skip when a real prompt is pending
            // (no '\r' present) so we don't clobber "password:" detection.
            if let Some(cr) = buf.rfind('\r') {
                {
                    let tail = clean_line(buf[cr + 1..].trim_end());
                    let tail = tail.as_ref().trim();
                    if !tail.is_empty() && tail != last_progress {
                        last_progress = tail.to_owned();
                        let _ = tx.send(ModProgress::LogProgress(last_progress.clone()));
                        if let (Some(mod_id), Some((done, total))) =
                            (downloading, progress_bytes(tail))
                        {
                            let _ = tx.send(ModProgress::Bytes {
                                mod_id,
                                done,
                                total,
                            });
                        }
                    }
                }
                // Drop everything up to and including the last '\r' so the buffer
                // doesn't accumulate every progress rewrite (only the live tail is
                // kept — which is also what the prompt checks below inspect).
                buf.drain(..=cr);
            }

            // Check the partial-line remainder (content not yet terminated
            // by '\n'). steamcmd writes prompts like "password:" without a
            // trailing newline, so they would never be processed by the loop
            // above.
            // Partial-buffer Steam Guard check (no `has_password` guard — works
            // for both saved-password and interactive-password flows).
            if login_phase && !steam_guard_sent && is_steam_guard_prompt(&buf) {
                let _ = tx.send(ModProgress::SteamGuardMobileRequired);
                steam_guard_sent = true;
            }

            // Detect the `password:` prompt (no trailing newline). SteamCMD
            // asks when it has no cached login: a password saved by an
            // earlier version is typed once, otherwise the player is asked.
            if login_phase && password_prompts < MAX_PASSWORD_PROMPTS && is_password_prompt(&buf) {
                password_prompts += 1;
                let answer = match carry.saved_password.take() {
                    Some(pw) => {
                        carry.saved_password_tried = true;
                        Some(pw)
                    }
                    None => {
                        let _ = tx.send(ModProgress::PasswordRequired);
                        // Closed when the player cancels.
                        pty_input.recv().await
                    }
                };
                match answer {
                    Some(password) => {
                        // Write the password followed by Enter to the PTY.
                        use std::io::Write;
                        let _ = pty_writer.write_all(password.as_bytes());
                        // Enter is CR to a console: ConPTY turns LF into
                        // Ctrl+J, which never ends the password read. A Unix
                        // PTY maps CR to LF, so CR works on both.
                        let _ = pty_writer.write_all(b"\r");
                        let _ = pty_writer.flush();
                        buf.clear();
                    }
                    None => {
                        let _ = child.kill();
                        seen.cancelled = true;
                        break;
                    }
                }
            }
        }

        let _ = child.wait();
        seen.logged_in = logged_in;
        seen
    }
}

/// Send `LoggedIn` once per operation, however many sessions it runs.
fn announce(announced: &mut bool, tx: &ProgressTx) {
    if !std::mem::replace(announced, true) {
        let _ = tx.send(ModProgress::LoggedIn);
    }
}

/// SteamCMD gave up waiting for the Steam Guard confirmation.
fn is_steam_guard_timeout(line: &str) -> bool {
    ascii_contains_ci(line, "timed out waiting for confirmation")
        || ascii_contains_ci(line, "wait for confirmation timed out")
        || (ascii_contains_ci(line, "error") && ascii_contains_ci(line, "timeout"))
}

/// `ERROR! Download item 1559212036 failed (Timeout).`: the item and the
/// reason SteamCMD gives.
fn download_failure(line: &str) -> Option<(u64, String)> {
    if !(ascii_contains_ci(line, "download item") && ascii_contains_ci(line, "failed")) {
        return None;
    }
    let id = extract_mod_id_from_line(line)?;
    let after = &line[line.find("failed")? + "failed".len()..];
    let why = after
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(why, _)| why.trim())
        .filter(|why| !why.is_empty())
        .unwrap_or("failed");
    Some((id, why.to_string()))
}

/// A failure reason that a later try may get past.
fn is_timeout(why: &str) -> bool {
    ascii_contains_ci(why, "timeout") || ascii_contains_ci(why, "timed out")
}

/// The sentence the summary ends on, if anything went wrong.
fn finished_hint(seen: &Seen, failed: bool, timed_out: bool) -> Option<String> {
    Some(if seen.invalid_password {
        "Steam refused the password. Try again and type it at the prompt.".to_string()
    } else if seen.steam_guard_timed_out {
        "Steam Guard confirmation timed out. Open the Steam Mobile app faster next time, or try again."
            .to_string()
    } else if !seen.logged_in {
        "SteamCMD did not log in. Use \"Log in to SteamCMD\" in Settings → Steam.".to_string()
    } else if timed_out {
        "Steam's servers timed out on some downloads, even after trying again. Try again later: SteamCMD carries on where it stopped."
            .to_string()
    } else if failed {
        "Some downloads failed. Try again, or repair the mod to re-check its files.".to_string()
    } else {
        return None;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_timeout_is_not_steam_guards() {
        let line = "ERROR! Download item 1559212036 failed (Timeout).";
        assert_eq!(
            download_failure(line),
            Some((1559212036, "Timeout".to_string()))
        );
        assert!(is_timeout("Timeout"));
        assert!(!is_timeout("Access Denied"));
        assert_eq!(
            download_failure("ERROR! Download item 42 failed (Access Denied)."),
            Some((42, "Access Denied".to_string()))
        );
        assert_eq!(download_failure("Downloading item 42 ..."), None);
        assert_eq!(
            download_failure("Download item 7 failed."),
            Some((7, "failed".to_string()))
        );
        // Still a timeout to the login's checks: which is why they only run
        // while logging in.
        assert!(is_steam_guard_timeout(line));
        assert!(is_steam_guard_timeout("Timed out waiting for confirmation"));
    }

    #[test]
    fn the_hint_names_what_went_wrong() {
        let mut seen = Seen {
            logged_in: true,
            ..Seen::default()
        };
        assert_eq!(finished_hint(&seen, false, false), None);
        assert!(finished_hint(&seen, true, true).is_some_and(|h| h.contains("timed out")));
        assert!(finished_hint(&seen, true, false).is_some_and(|h| h.contains("failed")));
        seen.steam_guard_timed_out = true;
        assert!(finished_hint(&seen, true, true).is_some_and(|h| h.contains("Steam Guard")));
        let not_in = Seen::default();
        assert!(finished_hint(&not_in, true, false).is_some_and(|h| h.contains("did not log in")));
    }

    #[test]
    fn large_batches_split_under_the_command_line_limit() {
        let c = SteamCmd::new(
            "C:\\steamcmd\\steamcmd.exe",
            "C:\\Program Files (x86)\\Steam\\steamapps",
            "C:\\Users\\someone with a long name\\AppData\\Roaming\\dayz-community-hub\\steamcmd-content",
            Some("player".into()),
        );
        let ids: Vec<u64> = (0..1_000).map(|i| 3_000_000_000 + i).collect();
        let chunks: Vec<&[u64]> = ids.chunks(SESSION_ITEMS).collect();
        assert_eq!(chunks.len(), 4);
        for chunk in chunks {
            let line: usize = c
                .session_args(chunk, true)
                .iter()
                // Each argument, quoted, and a space.
                .map(|a| a.len() + 3)
                .sum::<usize>()
                + c.steamcmd_path.as_os_str().len();
            assert!(line + 4_096 < 32_767, "{line} characters");
        }
    }
}
