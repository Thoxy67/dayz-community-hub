//! `cargo xtask shots [views...]`: every view rendered headless on the mock hub, into
//! `var/shots/<view>.png`, to be compared with `var/reference/<view>.png` by `visual-diff`.
//!
//! For each view this writes `var/shots/<view>.jsonl` and runs
//! `cargo run -p dayz-community-hub --example ui_shot -- <script>`.
//!
//! # The ui_shot script format
//!
//! One JSON object per line, run in order; blank lines and lines starting with `//` are ignored.
//!
//! | line | effect |
//! |---|---|
//! | `{"size": [w, h]}` | the window's inner size in logical points (before anything is drawn) |
//! | `{"ppp": 1.0}` | pixels per point (optional; the default is 1, so a shot is `w × h` pixels) |
//! | `{"locale": "fr"}` | the interface language (en fr de es ru) |
//! | `{"theme": "chernarus"}` | a theme preset by id |
//! | `{"view": "servers"}` | open a view by its id (the rail's ids) |
//! | `{"command": "id", "args": {…}}` | run a command of the registry (`commands.rs`) |
//! | `{"steps": n}` | run `n` frames (lets the mock hub answer and animations settle) |
//! | `{"shot": "/abs/path.png"}` | save the current frame as a PNG |
//!
//! xtask writes:
//!
//! ```text
//! {"size":[1400,860]}
//! {"locale":"fr"}
//! {"theme":"chernarus"}
//! {"view":"servers"}
//! {"steps":30}
//! {"shot":"/…/var/shots/servers.png"}
//! ```
//!
//! The reference images (`var/reference/`) are the record of the Svelte interface (removed in
//! M4.2) on its mock backend, chernarus theme, 1400×860, French: files, never regenerated.

use std::path::Path;

/// Every view of the rail, in its order.
pub const VIEWS: &[&str] = &[
    "servers",
    "favorites",
    "history",
    "friends",
    "connect",
    "offline",
    "mods",
    "options",
    "news",
    "stats",
    "settings",
    "about",
];

pub const SIZE: [u32; 2] = [1400, 860];
pub const LOCALE: &str = "fr";
pub const THEME: &str = "chernarus";
pub const STEPS: u32 = 30;

const EXAMPLE: &str = "apps/desktop/examples/ui_shot.rs";

/// The script that renders `view` into `shot`.
pub fn script(view: &str, shot: &Path) -> String {
    // The setup is not a place of the rail: it opens over the servers (`setup.open`).
    let open = if view == "setup" {
        serde_json::json!({ "command": "setup.open" })
    } else {
        serde_json::json!({ "view": view })
    };
    let lines = [
        serde_json::json!({ "size": SIZE }),
        serde_json::json!({ "locale": LOCALE }),
        serde_json::json!({ "theme": THEME }),
        open,
        serde_json::json!({ "steps": STEPS }),
        serde_json::json!({ "shot": shot.to_string_lossy() }),
    ];
    let mut out = String::new();
    for l in lines {
        out.push_str(&l.to_string());
        out.push('\n');
    }
    out
}

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    if !root.join(EXAMPLE).is_file() {
        return Err(format!(
            "{EXAMPLE} does not exist yet: nothing can render the views"
        ));
    }
    let views: Vec<&str> = if args.is_empty() {
        VIEWS.to_vec()
    } else {
        args.to_vec()
    };
    let dir = root.join("var/shots");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut failed = Vec::new();
    for view in &views {
        if view.is_empty() || view.contains(['/', '\\', '.']) {
            return Err(format!("`{view}` is not a view id"));
        }
        let shot = dir.join(format!("{view}.png"));
        let script_path = dir.join(format!("{view}.jsonl"));
        std::fs::write(&script_path, script(view, &shot))
            .map_err(|e| format!("{}: {e}", script_path.display()))?;
        let mut c = crate::cargo(root);
        c.args([
            "run",
            "-q",
            "-p",
            crate::UI_PACKAGE,
            "--example",
            "ui_shot",
            "--",
        ])
        .arg(&script_path);
        match crate::run(c, &format!("ui_shot {view}")) {
            Ok(()) if shot.is_file() => println!("shots: {}", shot.display()),
            Ok(()) => failed.push(format!("{view}: ui_shot wrote no {}", shot.display())),
            Err(e) => failed.push(e),
        }
    }
    if failed.is_empty() {
        println!("shots: {} view(s) in {}", views.len(), dir.display());
        Ok(())
    } else {
        Err(format!(
            "{} of {} view(s) failed:\n  {}",
            failed.len(),
            views.len(),
            failed.join("\n  ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_is_one_json_object_per_line() {
        let s = script("mods", Path::new("/x/var/shots/mods.png"));
        let lines: Vec<serde_json::Value> = s
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0]["size"], serde_json::json!([1400, 860]));
        assert_eq!(lines[3]["view"], "mods");
        assert_eq!(lines[5]["shot"], "/x/var/shots/mods.png");
    }
}
