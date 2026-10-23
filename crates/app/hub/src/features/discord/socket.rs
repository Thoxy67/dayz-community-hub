//! Where a running Discord listens: a Unix socket `discord-ipc-<0..9>` in the
//! runtime or temporary directory (and the Flatpak's and the Snap's folders in
//! them), or the named pipe `\\.\pipe\discord-ipc-<0..9>` on Windows.

use tokio::io::{AsyncRead, AsyncWrite};

/// A connection to Discord, whatever carries it.
pub(crate) trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

/// How long one place is tried.
#[cfg(unix)]
const TRY_FOR: std::time::Duration = std::time::Duration::from_secs(2);

/// The places Discord may listen at, from the environment `env` reads, in order.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn unix_candidates(env: impl Fn(&str) -> Option<String>) -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut bases: Vec<PathBuf> = Vec::new();
    for var in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
        if let Some(v) = env(var).filter(|v| !v.trim().is_empty()) {
            let p = PathBuf::from(v);
            if !bases.contains(&p) {
                bases.push(p);
            }
        }
    }
    let tmp = PathBuf::from("/tmp");
    if !bases.contains(&tmp) {
        bases.push(tmp);
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    for b in &bases {
        dirs.push(b.clone());
        dirs.push(b.join("app").join("com.discordapp.Discord"));
        dirs.push(b.join("snap.discord"));
    }
    let mut out = Vec::with_capacity(dirs.len() * 10);
    for d in &dirs {
        for i in 0..10 {
            out.push(d.join(format!("discord-ipc-{i}")));
        }
    }
    out
}

/// The named pipes Discord may listen at.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn pipe_candidates() -> Vec<String> {
    (0..10)
        .map(|i| format!(r"\\.\pipe\discord-ipc-{i}"))
        .collect()
}

/// The first place a running Discord answers at, or `None` (Discord is not
/// running, or not reachable from here).
pub(crate) async fn connect() -> Option<Box<dyn Stream>> {
    #[cfg(unix)]
    {
        for path in unix_candidates(|k| std::env::var(k).ok()) {
            if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                continue;
            }
            if let Ok(Ok(s)) =
                tokio::time::timeout(TRY_FOR, tokio::net::UnixStream::connect(&path)).await
            {
                return Some(Box::new(s));
            }
        }
        None
    }
    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        for name in pipe_candidates() {
            // Opening a pipe does not wait: it is there or not (or busy).
            if let Ok(p) = ClientOptions::new().open(&name) {
                return Some(Box::new(p));
            }
        }
        None
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sockets_are_looked_for_in_the_runtime_dir_flatpak_snap_then_tmp() {
        let env = |k: &str| match k {
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".to_string()),
            "TMPDIR" => Some("/run/user/1000".to_string()),
            "TMP" => Some("  ".to_string()),
            _ => None,
        };
        let c = unix_candidates(env);
        // Two bases (duplicate and blank dropped, /tmp added), three folders each.
        assert_eq!(c.len(), 2 * 3 * 10);
        assert_eq!(c[0], PathBuf::from("/run/user/1000/discord-ipc-0"));
        assert_eq!(c[9], PathBuf::from("/run/user/1000/discord-ipc-9"));
        assert_eq!(
            c[10],
            PathBuf::from("/run/user/1000/app/com.discordapp.Discord/discord-ipc-0")
        );
        assert_eq!(
            c[20],
            PathBuf::from("/run/user/1000/snap.discord/discord-ipc-0")
        );
        assert_eq!(c[30], PathBuf::from("/tmp/discord-ipc-0"));
        assert_eq!(unix_candidates(|_| None).len(), 30);
    }

    #[test]
    fn pipes_are_numbered_zero_to_nine() {
        let p = pipe_candidates();
        assert_eq!(p.len(), 10);
        assert_eq!(p[0], r"\\.\pipe\discord-ipc-0");
        assert_eq!(p[9], r"\\.\pipe\discord-ipc-9");
    }
}
