//! `cargo xtask parity [--check]`: the feature catalog of the Svelte interface
//! (`apps/desktop/src/catalog.rs`) against what the egui window does.
//!
//! - `parity` runs the ignored test `catalog::tests::write_checklist`, which writes
//!   `docs/parity-checklist.md`.
//! - `parity --check` runs `catalog::tests::parity_floor`, which fails when the share of features
//!   done falls below the floor pinned in the catalog (a floor that only rises).
//!
//! Until the catalog exists, both say so and succeed. A filter that runs no test is a failure: a
//! renamed test must not pass silently.

use std::path::Path;

const CATALOG: &str = "apps/desktop/src/catalog.rs";

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    if !root.join(CATALOG).is_file() {
        println!("parity: {CATALOG} does not exist yet, nothing to measure");
        return Ok(());
    }
    let test = if check {
        "catalog::tests::parity_floor"
    } else {
        "catalog::tests::write_checklist"
    };
    let args = [
        "test",
        "-p",
        crate::UI_PACKAGE,
        "--lib",
        test,
        "--",
        // write_checklist is ignored, parity_floor is not: run it whichever it is.
        "--include-ignored",
        "--exact",
    ];
    let what = format!("cargo {}", args.join(" "));
    eprintln!("$ {what}");
    let out = crate::cargo(root)
        .args(args)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|e| format!("{what}: failed to start: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    print!("{stdout}");
    if !out.status.success() {
        return Err(format!("{what}: exited with {}", out.status));
    }
    if !ran_one(&stdout) {
        return Err(format!(
            "{what}: no test named `{test}` ran (is it in {CATALOG}?)"
        ));
    }
    if !check {
        println!("parity: wrote docs/parity-checklist.md");
    }
    Ok(())
}

/// Whether the libtest output reports at least one passed test.
fn ran_one(stdout: &str) -> bool {
    stdout.lines().any(|l| {
        l.trim_start()
            .strip_prefix("test result: ok. ")
            .and_then(|r| r.split_whitespace().next())
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|n| n > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_the_tests_that_ran() {
        assert!(ran_one(
            "running 1 test\ntest result: ok. 1 passed; 0 failed;"
        ));
        assert!(!ran_one(
            "running 0 tests\ntest result: ok. 0 passed; 0 failed;"
        ));
    }
}
