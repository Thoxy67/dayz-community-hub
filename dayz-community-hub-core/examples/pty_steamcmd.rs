//! Standalone reproduction of the app's PTY-based steamcmd streaming.
//!
//! Reads steamcmd path + login from the real profile.json, spawns steamcmd
//! under a ConPTY (Windows) / PTY (unix) exactly like `spawn_pty_streamed`,
//! and prints every raw chunk with control characters made visible so we can
//! see (a) whether any output arrives at all and (b) how lines are framed
//! (`\r` carriage-return progress vs `\n` newlines).
//!
//! Run:  cargo run -p dayz-community-hub-core --example pty_steamcmd -- 2819373632

use std::io::Read;
use std::sync::{Arc, Mutex};

fn main() {
    let mod_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2819373632".to_string());

    // --- Load profile.json -------------------------------------------------
    let profile_path = std::env::var("APPDATA")
        .map(|a| format!("{a}\\dayz-community-hub\\profile.json"))
        .unwrap_or_else(|_| {
            format!(
                "{}/.config/dayz-community-hub/profile.json",
                std::env::var("HOME").unwrap_or_default()
            )
        });
    let raw = std::fs::read_to_string(&profile_path)
        .unwrap_or_else(|e| panic!("read {profile_path}: {e}"));
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse profile.json");

    let steamcmd_path = v["steamcmd_path"].as_str().expect("steamcmd_path").to_string();
    let login = v["steam_login"].as_str().expect("steam_login").to_string();
    let password = v["steam_password"].as_str().map(|s| s.to_string());

    eprintln!("steamcmd : {steamcmd_path}");
    eprintln!("login    : {login}");
    eprintln!("mod_id   : {mod_id}");
    eprintln!("--- streaming (control chars shown as \\r \\n \\e \\t) ---\n");

    // --- Build args (mirrors download_mods_batched) ------------------------
    let mut args: Vec<std::ffi::OsString> = Vec::new();
    args.push("+@ShutdownOnFailedCommand".into());
    args.push("0".into());
    args.push("+login".into());
    args.push((&login).into());
    if let Some(pw) = &password {
        args.push(pw.into());
    }
    args.push("+workshop_download_item".into());
    args.push("221100".into());
    args.push((&mod_id).into());
    args.push("validate".into());
    args.push("+quit".into());

    // --- Spawn under PTY (mirrors spawn_pty_streamed) ----------------------
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 220,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");

    let mut cmd = CommandBuilder::new(&steamcmd_path);
    for a in &args {
        cmd.arg(a);
    }

    let mut child = pair.slave.spawn_command(cmd).expect("spawn steamcmd");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let mut writer = pair.master.take_writer().expect("take writer");

    let master_holder: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> =
        Arc::new(Mutex::new(Some(pair.master)));
    let master_for_watcher = Arc::clone(&master_holder);
    std::thread::spawn(move || {
        let status = child.wait();
        eprintln!("\n--- child exited: {status:?} ---");
        drop(master_for_watcher.lock().unwrap().take());
    });

    let mut buf = [0u8; 4096];
    let mut total_bytes = 0usize;
    let mut total_reads = 0usize;
    let mut saw_nl = 0usize;
    let mut saw_cr = 0usize;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                eprintln!("\n--- reader EOF (Ok(0)) ---");
                break;
            }
            Err(e) => {
                eprintln!("\n--- reader err: {e} ---");
                break;
            }
            Ok(n) => {
                total_reads += 1;
                total_bytes += n;
                // ConPTY (opened with PSUEDOCONSOLE_INHERIT_CURSOR by portable-pty)
                // emits a DSR cursor-position query `ESC[6n` at startup and BLOCKS
                // until it receives a reply on stdin. Reply with a fake position so
                // the console unblocks and steamcmd starts producing output.
                if buf[..n].windows(4).any(|w| w == b"\x1b[6n") {
                    use std::io::Write;
                    eprintln!("\n>>> got ESC[6n, replying ESC[1;1R <<<");
                    let _ = writer.write_all(b"\x1b[1;1R");
                    let _ = writer.flush();
                }
                for &b in &buf[..n] {
                    match b {
                        b'\n' => {
                            saw_nl += 1;
                            print!("\\n\n");
                        }
                        b'\r' => {
                            saw_cr += 1;
                            print!("\\r");
                        }
                        0x1b => print!("\\e"),
                        b'\t' => print!("\\t"),
                        0x20..=0x7e => print!("{}", b as char),
                        _ => print!("\\x{b:02x}"),
                    }
                }
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }
    }
    let _ = master_holder.lock().unwrap().take();

    eprintln!(
        "\n=== summary: {total_reads} reads, {total_bytes} bytes, {saw_nl} '\\n', {saw_cr} '\\r' ==="
    );
}
