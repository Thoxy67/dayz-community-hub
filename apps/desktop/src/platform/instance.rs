//! One launcher window per user (Tauri's single-instance plugin, without D-Bus or a
//! named mutex): the same code on Linux and Windows.
//!
//! - `<dir>/dayz-community-hub-<key>.lock` is locked exclusively by the first process
//!   for as long as it runs (the system drops the lock when it exits or crashes, so
//!   nothing goes stale).
//! - That process listens on `127.0.0.1:<random port>` and writes `<port> <token>` to
//!   `<dir>/dayz-community-hub-<key>.port` (mode 0600 on Unix; `dir` is the per-user
//!   runtime or local data directory), the token 32 random bytes in hex.
//! - A later launch finds the lock taken, reads the port file and sends one JSON line
//!   `{"token": …, "args": CliArgs}`; the first answers `ok` and hands the arguments
//!   on, the later one exits 0. If the first cannot be reached in a few seconds, the
//!   later one runs on its own rather than not at all.
//!
//! The key separates worlds: `live` for the real launcher, a hash of `--data-dir` when
//! one is given, `--instance <name>` when asked; `--mock` takes part only with
//! `--instance`, so an invented world never joins (or captures) the real one.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dz_hub::CliArgs;
use serde::{Deserialize, Serialize};

/// A hand-off line longer than this is refused.
const MAX_LINE: u64 = 64 * 1024;
/// How long a later launch keeps trying to reach the first.
const REACH_FOR: Duration = Duration::from_secs(4);
/// One read or write on a hand-off connection.
const IO_TIMEOUT: Duration = Duration::from_secs(3);

/// How this process starts.
pub enum Start {
    /// The first: listen, then run the window.
    Primary(Primary),
    /// Another instance took the arguments: exit 0.
    HandedOff,
    /// No single instance (no directory, an unreachable first): run anyway.
    Alone,
}

/// The first instance: holds the lock until dropped (the end of `main`).
pub struct Primary {
    _lock: File,
    port_file: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Hello {
    token: String,
    args: CliArgs,
}

/// The per-user directory the lock and port files live in.
pub fn default_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(d) = dirs::runtime_dir() {
        return Some(d);
    }
    dirs::data_local_dir().map(|d| d.join("dayz-community-hub"))
}

/// The key for this launch (see the module docs); `None` takes no part.
pub fn key(mock: bool, instance: Option<&str>, data_dir: Option<&Path>) -> Option<String> {
    if let Some(name) = instance {
        let name: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(48)
            .collect();
        return (!name.is_empty()).then(|| format!("x-{name}"));
    }
    if mock {
        return None;
    }
    Some(match data_dir {
        Some(d) => format!("dir-{:016x}", fnv1a(d.to_string_lossy().as_bytes())),
        None => "live".into(),
    })
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn files(dir: &Path, key: &str) -> (PathBuf, PathBuf) {
    let stem = format!("dayz-community-hub-{key}");
    (
        dir.join(format!("{stem}.lock")),
        dir.join(format!("{stem}.port")),
    )
}

/// Become the first instance, or hand `args` to it.
pub fn start(dir: &Path, key: &str, args: &CliArgs) -> Start {
    if std::fs::create_dir_all(dir).is_err() {
        return Start::Alone;
    }
    let (lock_path, port_file) = files(dir, key);
    let Ok(lock) = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
    else {
        return Start::Alone;
    };
    match lock.try_lock() {
        Ok(()) => Start::Primary(Primary {
            _lock: lock,
            port_file,
        }),
        Err(TryLockError::WouldBlock) => {
            if hand_off(&port_file, args, REACH_FOR) {
                Start::HandedOff
            } else {
                Start::Alone
            }
        }
        Err(TryLockError::Error(_)) => Start::Alone,
    }
}

/// Send `args` to the first instance; true once it answered `ok`.
fn hand_off(port_file: &Path, args: &CliArgs, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    loop {
        if try_hand_off(port_file, args).is_ok() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        // The first may still be starting: its port file comes a moment after the lock.
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn try_hand_off(port_file: &Path, args: &CliArgs) -> Result<(), String> {
    let text = std::fs::read_to_string(port_file).map_err(|e| e.to_string())?;
    let (port, token) = parse_port_file(&text).ok_or("bad port file")?;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, IO_TIMEOUT).map_err(|e| e.to_string())?;
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let hello = Hello {
        token: token.to_string(),
        args: args.clone(),
    };
    let mut line = serde_json::to_string(&hello).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut answer = String::new();
    BufReader::new(stream.take(64))
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    if answer.trim() == "ok" {
        Ok(())
    } else {
        Err(format!("refused: {}", answer.trim()))
    }
}

fn parse_port_file(text: &str) -> Option<(u16, &str)> {
    let mut parts = text.split_whitespace();
    let port = parts.next()?.parse().ok().filter(|p| *p > 0)?;
    let token = parts.next().filter(|t| !t.is_empty())?;
    Some((port, token))
}

impl Primary {
    /// Listen for later launches; `on_args` runs on the listener's thread for each.
    pub fn listen(&self, on_args: impl Fn(CliArgs) + Send + 'static) -> Result<(), String> {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| format!("listen: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let token = random_token()?;
        write_port_file(&self.port_file, port, &token)?;
        std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    if let Some(args) = serve(stream, &token) {
                        on_args(args);
                    }
                }
            })
            .map_err(|e| format!("listener thread: {e}"))?;
        Ok(())
    }
}

impl Drop for Primary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.port_file);
    }
}

/// One later launch: its arguments when it knew the token.
fn serve(stream: TcpStream, token: &str) -> Option<CliArgs> {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let mut writer = stream.try_clone().ok()?;
    let mut line = String::new();
    BufReader::new(stream.take(MAX_LINE))
        .read_line(&mut line)
        .ok()?;
    let hello = serde_json::from_str::<Hello>(line.trim()).ok();
    match hello {
        Some(h) if same(&h.token, token) => {
            let _ = writer.write_all(b"ok\n");
            Some(h.args)
        }
        _ => {
            let _ = writer.write_all(b"denied\n");
            None
        }
    }
}

/// Token comparison that does not stop at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn random_token() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| e.to_string())?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Written beside, then renamed over: a later launch never reads half a file.
fn write_port_file(path: &Path, port: u16, token: &str) -> Result<(), String> {
    let tmp = path.with_extension(format!("port.{}", std::process::id()));
    let mut open = OpenOptions::new();
    open.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    let mut f = open
        .open(&tmp)
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    f.write_all(format!("{port} {token}\n").as_bytes())
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dzch-instance-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn keys_keep_worlds_apart() {
        assert_eq!(key(false, None, None).as_deref(), Some("live"));
        assert_eq!(key(true, None, None), None);
        assert_eq!(key(true, Some("t/../1"), None).as_deref(), Some("x-t1"));
        assert_eq!(key(false, Some("//"), None), None);
        let a = key(false, None, Some(Path::new("/a"))).unwrap();
        let b = key(false, None, Some(Path::new("/b"))).unwrap();
        assert!(a.starts_with("dir-") && a != b);
    }

    #[test]
    fn port_files_are_read_with_care() {
        assert_eq!(parse_port_file("4000 abc\n"), Some((4000, "abc")));
        assert_eq!(parse_port_file("0 abc"), None);
        assert_eq!(parse_port_file("4000"), None);
        assert_eq!(parse_port_file("x abc"), None);
        assert_eq!(parse_port_file(""), None);
    }

    #[test]
    fn a_second_launch_hands_its_arguments_to_the_first() {
        let dir = temp_dir("handoff");
        let Start::Primary(first) = start(&dir, "t", &CliArgs::none()) else {
            panic!("the first launch is the primary");
        };
        let (tx, rx) = mpsc::channel();
        first
            .listen(move |a| {
                let _ = tx.send(a);
            })
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let (_, port_file) = files(&dir, "t");
            let mode = std::fs::metadata(&port_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let args = CliArgs {
            open: Some("dzch://1.2.3.4:2302".into()),
            ..CliArgs::none()
        };
        assert!(matches!(start(&dir, "t", &args), Start::HandedOff));
        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got.open.as_deref(), Some("dzch://1.2.3.4:2302"));

        // Another key is another world.
        let Start::Primary(other) = start(&dir, "u", &CliArgs::none()) else {
            panic!("another key is its own primary");
        };
        drop(other);

        // A wrong token is refused.
        let (_, port_file) = files(&dir, "t");
        let text = std::fs::read_to_string(&port_file).unwrap();
        let (port, _) = parse_port_file(&text).unwrap();
        std::fs::write(&port_file, format!("{port} nope\n")).unwrap();
        assert!(!hand_off(&port_file, &args, Duration::ZERO));
        assert!(rx.try_recv().is_err());

        // Once the first is gone, the next launch is the first.
        drop(first);
        assert!(!port_file.exists());
        assert!(matches!(
            start(&dir, "t", &CliArgs::none()),
            Start::Primary(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hostile_lines_are_refused() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::thread::spawn(move || {
            let mut s = TcpStream::connect(addr).unwrap();
            s.write_all(b"{\"token\": 5}\n").unwrap();
            let mut a = String::new();
            BufReader::new(s).read_line(&mut a).unwrap();
            a
        });
        let (stream, _) = listener.accept().unwrap();
        assert!(serve(stream, "tok").is_none());
        assert_eq!(client.join().unwrap().trim(), "denied");
        assert!(same("abc", "abc") && !same("abc", "abd") && !same("ab", "abc"));
    }
}
