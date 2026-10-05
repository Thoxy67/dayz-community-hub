//! Running steamcmd inside a pseudo-terminal.
//!
//! steamcmd only line-buffers (and only prompts for a password or a Steam
//! Guard code) when its stdio is a terminal, so it always runs under a PTY
//! (Linux) or ConPTY (Windows) here, never with piped stdio.

use tokio::sync::mpsc;

use crate::SteamCmd;

/// Bundle of handles handed back from `spawn_pty_streamed`:
/// (child handle that can kill steamcmd, stdout chunk receiver, PTY writer for stdin).
pub(crate) type PtyStreamHandles = (
    Box<dyn portable_pty::Child + Send + Sync>,
    mpsc::UnboundedReceiver<String>,
    Box<dyn std::io::Write + Send>,
);

impl SteamCmd {
    /// Spawn steamcmd under a PTY and stream its output as raw byte chunks.
    ///
    /// Returns `(child, chunk_rx)`. The caller drains `chunk_rx` until it closes
    /// (signals EOF / process exit), then calls `child.wait()`.
    ///
    /// Using a PTY forces steamcmd's stdio into line-buffered mode so every
    /// line arrives immediately (Linux PTY / Windows ConPTY).
    ///
    /// **Windows ConPTY caveat**: the master reader does *not* return EOF when
    /// the child exits — ConPTY keeps the handle open. We work around this by
    /// running a second watcher thread that calls `child.wait()` and then drops
    /// the master, which finally unblocks the reader and lets it return an error,
    /// breaking the read loop and closing `chunk_rx`.
    /// PTY writer handle — wrapped so it can be shared across threads.
    /// The download task uses this to send password / Steam Guard codes to
    /// the running steamcmd process.
    ///
    /// Bundles the three handles spawn_pty_streamed hands back: the (no-op)
    /// child, the stdout chunk receiver, and the PTY writer.
    pub(crate) fn spawn_pty_streamed(
        &self,
        args: &[std::ffi::OsString],
    ) -> std::result::Result<PtyStreamHandles, String> {
        use portable_pty::{CommandBuilder, PtySize, native_pty_system};
        use std::io::Read;
        use std::sync::{Arc, Mutex};

        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 220,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {e}"))?;

        let mut cmd = CommandBuilder::new(&self.steamcmd_path);
        for arg in args {
            cmd.arg(arg);
        }
        #[cfg(unix)]
        if let Some(home) = &self.home_dir {
            cmd.env("HOME", home);
        }

        let mut child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to start steamcmd: {e}"))?;
        drop(pty_pair.slave);

        let mut reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {e}"))?;

        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take PTY writer: {e}"))?;

        // Wrap the master in an Arc<Mutex<Option<...>>> so the watcher thread
        // can drop it (closing the ConPTY handle) once the child exits, which
        // unblocks the reader thread that is blocked on `reader.read()`.
        let master_holder: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> =
            Arc::new(Mutex::new(Some(pty_pair.master)));
        let master_for_watcher = Arc::clone(&master_holder);

        // Watcher thread: wait for the child to exit, then drop the master.
        // `child` is moved here; the caller receives a handle that can still
        // kill it (a cancelled operation must stop steamcmd, not leave it
        // downloading or waiting at a prompt).
        let killer = child.clone_killer();
        let exited = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let exited_for_watcher = Arc::clone(&exited);
        let (exit_tx, exit_rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _ = child.wait();
            exited_for_watcher.store(true, std::sync::atomic::Ordering::SeqCst);
            // Dropping the master closes the ConPTY output pipe, which makes
            // the reader return an error and exit its loop.
            drop(
                master_for_watcher
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take(),
            );
            let _ = exit_tx.send(());
        });

        let (chunk_tx, chunk_rx) = mpsc::unbounded_channel::<String>();
        tokio::task::spawn_blocking(move || {
            // Keep _master_holder alive until the watcher drops it.
            let _master_holder = master_holder;
            // 4 KiB read buffer — 16x fewer syscalls and channel sends than
            // the prior 256-byte buffer, which dominated CPU during big mod
            // downloads.  steamcmd's PTY output is line-oriented and already
            // small per line, so this only batches what would otherwise be
            // multiple syscalls back-to-back.
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                        let _ = chunk_tx.send(s);
                    }
                }
            }
            // Drain the exit channel so the watcher thread is not leaked.
            let _ = exit_rx.recv();
        });

        Ok((Box::new(PtyChild { killer, exited }), chunk_rx, writer))
    }
}

/// The handle `spawn_pty_streamed` returns. The real child is owned by the
/// watcher thread, which waits on it and then drops the PTY master to unblock
/// the reader; this keeps a killer for it. `kill()` stops steamcmd, and so
/// does dropping the handle while steamcmd still runs: an aborted operation
/// (Cancel) drops it. `wait()` returns at once: the watcher does the waiting.
pub(crate) struct PtyChild {
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    exited: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl PtyChild {
    fn running(&self) -> bool {
        !self.exited.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl std::fmt::Debug for PtyChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyChild").field("running", &self.running()).finish()
    }
}

impl Drop for PtyChild {
    fn drop(&mut self) {
        if self.running() {
            let _ = self.killer.kill();
        }
    }
}

impl portable_pty::ChildKiller for PtyChild {
    fn kill(&mut self) -> std::io::Result<()> {
        // Never signal a pid that has exited: it may belong to someone else now.
        if self.running() {
            self.killer.kill()
        } else {
            Ok(())
        }
    }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        self.killer.clone_killer()
    }
}

impl portable_pty::Child for PtyChild {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        Ok(Some(portable_pty::ExitStatus::with_exit_code(0)))
    }
    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        Ok(portable_pty::ExitStatus::with_exit_code(0))
    }
    fn process_id(&self) -> Option<u32> {
        None
    }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        None
    }
}
