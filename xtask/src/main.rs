//! Workspace tooling for the egui window: `cargo xtask <command>`.
//!
//! Pure Rust (std, serde_json, image). External tools (`cargo`, `cargo xwin`) are run through
//! `std::process::Command`. Modelled on `~/wordcraft/xtask`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

mod assets;
mod layers;
mod parity;
mod shots;
mod visual_diff;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The crates of the egui rewrite: the ones `ci` formats, lints, tests and cross-checks.
pub const NEW_CRATES: &[&str] = &["dz-hub", "dayz-community-hub-egui", "xtask"];
/// The egui window's package name: the only workspace crate allowed to use egui.
pub const UI_PACKAGE: &str = "dayz-community-hub-egui";

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci              fmt --check, clippy -D warnings, tests (dz-hub, dayz-community-hub-egui,
                  xtask), assets, layers, parity --check, cargo xwin check when installed;
                  stops at the first failure
  assets          every file under apps/desktop/assets has a row in ATTRIBUTION.md
  layers          no workspace crate but dayz-community-hub-egui reaches egui, eframe,
                  egui_extras, winit, rfd, arboard or egui_kittest
  parity [--check]
                  write docs/parity-checklist.md from apps/desktop/src/catalog.rs;
                  --check runs the parity floor test instead
  shots [views...]
                  render each view headless (ui_shot example) into var/shots/<view>.png
  visual-diff [views...]
                  compare var/shots/<view>.png with var/reference/<view>.png (SSIM),
                  write var/shots/<view>-diff.png and the table in docs/ui-parity.md
  visual-diff --pair <shot.png> <reference.png> [diff.png]
                  compare two PNG files and print the numbers (docs untouched)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let result = root().and_then(|root| match args.first().map(String::as_str) {
        Some("ci") => cmd_ci(&root),
        Some("assets") => assets::run(&root),
        Some("layers") => layers::run(&root),
        Some("parity") => parity::run(&root, rest.contains(&"--check")),
        Some("shots") => shots::run(&root, &rest),
        Some("visual-diff") => visual_diff::run(&root, &rest),
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Workspace root (parent of the xtask crate).
pub fn root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask has no parent directory".to_string())
}

/// `cargo` (the one running us when there is one), in the workspace root.
pub fn cargo(root: &Path) -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root);
    c
}

/// Runs a command with inherited output; an error names it and its exit status.
pub fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    eprintln!("$ {what}");
    let status = cmd
        .status()
        .map_err(|e| format!("{what}: failed to start: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what}: exited with {status}"))
    }
}

/// `cargo <args> -p <crate>...` for the given crates.
fn cargo_for(root: &Path, args: &[&str], crates: &[&str], tail: &[&str]) -> (Command, String) {
    let mut c = cargo(root);
    let mut words: Vec<&str> = args.to_vec();
    for k in crates {
        words.push("-p");
        words.push(k);
    }
    words.extend_from_slice(tail);
    c.args(&words);
    (c, format!("cargo {}", words.join(" ")))
}

/// Whether an executable named `name` is on PATH.
pub fn on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let exts: &[&str] = if cfg!(windows) {
        &["exe", "cmd", "bat"]
    } else {
        &[""]
    };
    std::env::split_paths(&path).any(|dir| {
        exts.iter().any(|ext| {
            let file = if ext.is_empty() {
                dir.join(name)
            } else {
                dir.join(format!("{name}.{ext}"))
            };
            file.is_file()
        })
    })
}

fn cmd_ci(root: &Path) -> Result<(), String> {
    type Step<'a> = (&'static str, Box<dyn Fn() -> Result<(), String> + 'a>);
    let tested = &NEW_CRATES[..2];
    let steps: Vec<Step> = vec![
        (
            "fmt",
            Box::new(|| {
                let (c, what) = cargo_for(root, &["fmt", "--check"], NEW_CRATES, &[]);
                run(c, &what)
            }),
        ),
        (
            "clippy",
            Box::new(|| {
                let (c, what) = cargo_for(
                    root,
                    &["clippy"],
                    NEW_CRATES,
                    &["--all-targets", "--", "-D", "warnings"],
                );
                run(c, &what)
            }),
        ),
        (
            "test",
            Box::new(|| {
                let (c, what) = cargo_for(root, &["test"], tested, &[]);
                run(c, &what)
            }),
        ),
        ("assets", Box::new(|| assets::run(root))),
        ("layers", Box::new(|| layers::run(root))),
        ("parity", Box::new(|| parity::run(root, true))),
        (
            "xwin",
            Box::new(|| {
                if !on_path("cargo-xwin") {
                    println!("skipped: cargo-xwin not installed");
                    return Ok(());
                }
                let (c, what) = cargo_for(
                    root,
                    &["xwin", "check"],
                    tested,
                    &["--target", "x86_64-pc-windows-msvc"],
                );
                run(c, &what)
            }),
        ),
    ];
    let mut done = Vec::new();
    for (name, f) in &steps {
        eprintln!("\n=== ci: {name} ===");
        if let Err(e) = f() {
            println!("\nCI summary:");
            for d in &done {
                println!("  ok    {d}");
            }
            println!("  FAIL  {name}: {e}");
            for (n, _) in steps.iter().skip(done.len() + 1) {
                println!("  skip  {n}");
            }
            return Err(format!("ci failed at `{name}`"));
        }
        done.push(*name);
    }
    println!(
        "\nCI summary: all {} steps passed ({})",
        done.len(),
        done.join(", ")
    );
    Ok(())
}
