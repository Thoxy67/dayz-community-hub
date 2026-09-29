//! Downloading workshop mods through steamcmd, with per-mod progress.

use dz_common::{Error, Result};

use crate::output::{
    ascii_contains_ci, extract_mod_id_from_line, is_password_prompt, is_steam_guard_prompt,
    strip_ansi,
};
use crate::{ModProgress, ProgressTx, PtyInputRx, SteamClient, SteamCmd};

impl SteamCmd {
    /// Download/update multiple mods with per-mod progress.
    ///
    /// Batches all mods into a single steamcmd invocation and streams stdout
    /// line-by-line via a PTY (Linux) / ConPTY (Windows). Both platforms
    /// receive real-time output so progress is reported as each mod finishes.
    ///
    /// `mods_info` is a list of (mod_id, display_name) pairs.
    pub async fn download_mods_with_progress(
        &self,
        mods_info: &[(u64, String)],
        tx: &ProgressTx,
        pty_input: PtyInputRx,
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
                hint: Some(
                    "Set your Steam login in the profile, then run:\n  steamcmd +login YOUR_USERNAME +quit"
                        .to_string(),
                ),
            });
            return results;
        }

        // Steam and steamcmd share the same auth session — running both at the same
        // time kicks the user offline. Shut Steam down cleanly before starting steamcmd.
        let _ = tx.send(ModProgress::ShuttingDownSteam);
        SteamClient::shutdown_for_steamcmd().await;

        // Use a single batched PTY invocation on all platforms.
        // ConPTY on Windows provides the same real-time line-buffered output
        // as a Linux PTY, so there is no need for the one-per-mod fallback.
        self.download_mods_batched(mods_info, tx, total, pty_input)
            .await
    }

    /// Batch all mods into a single steamcmd invocation, stream stdout via PTY.
    /// Works on both Linux (PTY) and Windows (ConPTY).
    async fn download_mods_batched(
        &self,
        mods_info: &[(u64, String)],
        tx: &ProgressTx,
        total: usize,
        mut pty_input: PtyInputRx,
    ) -> Vec<(u64, Result<()>)> {
        let _ = tx.send(ModProgress::Starting {
            current: 1,
            total,
            mod_id: mods_info[0].0,
            name: mods_info[0].1.clone(),
        });

        let mut args: Vec<std::ffi::OsString> = Vec::new();
        args.push("+@ShutdownOnFailedCommand".into());
        args.push("0".into());
        if let Some(steam_parent) = self.steam_root.parent() {
            args.push("+force_install_dir".into());
            args.push(steam_parent.as_os_str().into());
        }
        args.push("+login".into());
        args.push((&self.login).into());
        if let Some(ref pw) = self.password {
            args.push(pw.into());
        }
        for (mod_id, _) in mods_info {
            args.push("+workshop_download_item".into());
            args.push(self.game_id.to_string().into());
            args.push(mod_id.to_string().into());
            args.push("validate".into());
        }
        args.push("+quit".into());

        let (mut child, mut chunk_rx, mut pty_writer) = match self.spawn_pty_streamed(&args) {
            Ok(v) => v,
            Err(e) => {
                let results = mods_info
                    .iter()
                    .map(|(id, _)| (*id, Err(Error::SteamCmd(e.clone()))))
                    .collect();
                let _ = tx.send(ModProgress::Finished {
                    ok: 0,
                    failed: total,
                    total,
                    hint: None,
                });
                return results;
            }
        };

        // Accumulate all output so we can split on newlines ourselves.
        // Steam Guard mobile is detected by text: steamcmd flushes the
        // "Please confirm..." / "Waiting for confirmation..." lines once the
        // user approves on the phone.
        let mut succeeded = rustc_hash::FxHashSet::<u64>::with_capacity_and_hasher(
            mods_info.len(),
            Default::default(),
        );
        // O(1) mod_id → (idx, name) lookup so we don't linear-scan mods_info
        // on every "Downloading"/"Success" line.  For N mods downloading with
        // ~3 progress matches each, this turns O(N²) into O(N).
        let mod_lookup: rustc_hash::FxHashMap<u64, (usize, &str)> = mods_info
            .iter()
            .enumerate()
            .map(|(idx, (id, name))| (*id, (idx, name.as_str())))
            .collect();
        let mut current_idx: usize = 0;
        let mut steam_guard_sent = false;
        let mut password_prompt_sent = false;
        let mut steam_guard_timed_out = false;
        let mut invalid_password = false;
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
            while let Some(nl_pos) = buf.find('\n') {
                // Scope the borrow on `buf` so we can `.drain` after.
                let advance_to = nl_pos + 1;
                {
                    let stripped = strip_ansi(&buf[..nl_pos]);
                    let line: &str = stripped.as_ref();

                    // Forward every non-empty line to the UI log panel.
                    if !line.trim().is_empty() {
                        let _ = tx.send(ModProgress::LogLine(line.to_owned()));
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

                    // Detect login errors from complete lines.
                    let elapsed = sg_start.map(|s| s.elapsed().as_secs_f32()).unwrap_or(0.0);
                    if ascii_contains_ci(line, "timed out waiting for confirmation")
                        || ascii_contains_ci(line, "wait for confirmation timed out")
                        || (ascii_contains_ci(line, "error") && ascii_contains_ci(line, "timeout"))
                    {
                        steam_guard_timed_out = true;
                        eprintln!("[SteamGuard] TIMED OUT (t={elapsed:.1}s)");
                    }
                    if ascii_contains_ci(line, "invalid password") {
                        invalid_password = true;
                    }
                    // Log when Steam Guard is resolved.
                    if steam_guard_sent
                        && !steam_guard_timed_out
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
                        if let Some(id) = extract_mod_id_from_line(line)
                            && let Some(&(idx, name)) = mod_lookup.get(&id)
                        {
                            succeeded.insert(id);
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
                        if let Some(id) = extract_mod_id_from_line(line)
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
                // Borrow on buf released; advance in place (no realloc).
                buf.drain(..advance_to);
            }

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
                    let tail = strip_ansi(buf[cr + 1..].trim_end());
                    let tail = tail.as_ref().trim();
                    if !tail.is_empty() && tail != last_progress {
                        last_progress = tail.to_owned();
                        let _ = tx.send(ModProgress::LogProgress(last_progress.clone()));
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

            // Detect the `password:` prompt (no trailing newline).
            // steamcmd emits this when cached credentials are missing.
            if login_phase && !password_prompt_sent && is_password_prompt(&buf) {
                password_prompt_sent = true;
                let _ = tx.send(ModProgress::PasswordRequired);

                // Wait for the user to provide a password via the input channel.
                // If the channel is closed (user cancelled / dropped), kill the
                // process and treat it as a credential failure.
                match pty_input.recv().await {
                    Some(password) => {
                        // Write the password followed by Enter to the PTY.
                        use std::io::Write;
                        let _ = pty_writer.write_all(password.as_bytes());
                        let _ = pty_writer.write_all(b"\n");
                        let _ = pty_writer.flush();
                        buf.clear();
                    }
                    None => {
                        // User cancelled — kill the process
                        let _ = child.kill();
                        break;
                    }
                }
            }
        }

        let _ = child.wait();

        // Build final results — anything not seen as succeeded is failed
        let results: Vec<(u64, Result<()>)> = mods_info
            .iter()
            .enumerate()
            .map(|(idx, (id, name))| {
                if succeeded.contains(id) {
                    (*id, Ok(()))
                } else {
                    let _ = tx.send(ModProgress::Failed {
                        current: idx + 1,
                        total,
                        mod_id: *id,
                        name: name.clone(),
                        error: "Download failed".to_string(),
                    });
                    (*id, Err(Error::SteamCmd("Download failed".to_string())))
                }
            })
            .collect();

        let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
        let failed = results.iter().filter(|(_, r)| r.is_err()).count();
        let _ = tx.send(ModProgress::Finished {
            ok,
            failed,
            total,
            hint: if invalid_password {
                Some(
                    "Invalid password. Check your Steam password in Settings and try again."
                        .to_string(),
                )
            } else if steam_guard_timed_out {
                Some(
                    "Steam Guard confirmation timed out. Open the Steam Mobile app faster next time, or try again."
                        .to_string(),
                )
            } else if failed > 0 {
                Some(format!(
                    "Some downloads failed. Try:\n  steamcmd +login {} +quit",
                    self.login
                ))
            } else {
                None
            },
        });
        results
    }
}
