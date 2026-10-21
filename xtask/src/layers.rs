//! `cargo xtask layers`: the window toolkit stays in the window.
//!
//! No workspace crate other than `dayz-community-hub-egui` may depend on egui, eframe,
//! egui_extras, winit, rfd, arboard or egui_kittest: directly (any kind of dependency, on any
//! target), or through a workspace crate it depends on (normal or build dependencies; a
//! dev-dependency does not travel). Depending on the window crate itself counts too. External
//! crates are judged by their direct dependencies only: the Tauri shell may use a plugin that
//! uses rfd, but may not name rfd itself (it goes when the Tauri shell goes, M4.2).
//!
//! The rules run on a small model (`Crate`) so they are testable without `cargo metadata`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

/// External crates that make a window.
pub const UI_CRATES: &[&str] = &[
    "egui",
    "eframe",
    "egui_extras",
    "winit",
    "rfd",
    "arboard",
    "egui_kittest",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    /// A dev-dependency (does not travel to dependents).
    pub dev: bool,
}

#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub deps: Vec<Dep>,
}

/// The workspace packages and their declared dependencies, from
/// `cargo metadata --no-deps --format-version 1`.
pub fn from_metadata(meta: &Value) -> Result<Vec<Crate>, String> {
    let packages = meta
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata: no `packages`")?;
    let mut out = Vec::new();
    for p in packages {
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .ok_or("cargo metadata: a package without a name")?;
        let deps = p
            .get("dependencies")
            .and_then(Value::as_array)
            .map(|deps| {
                deps.iter()
                    .filter_map(|d| {
                        Some(Dep {
                            name: d.get("name")?.as_str()?.to_string(),
                            dev: d.get("kind").and_then(Value::as_str) == Some("dev"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Crate {
            name: name.to_string(),
            deps,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// One crate that reaches a window crate, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub krate: String,
    /// Workspace crates walked through, then the external crate.
    pub path: Vec<String>,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {}", self.krate, self.path.join(" -> "))
    }
}

/// Every crate other than `allowed` that reaches a UI crate (the first path found for each).
pub fn check(crates: &[Crate], allowed: &str) -> Vec<Violation> {
    let by_name: BTreeMap<&str, &Crate> = crates.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut out = Vec::new();
    for c in crates {
        if c.name == allowed {
            continue;
        }
        if let Some(path) = reach(c, &by_name, allowed, true) {
            out.push(Violation {
                krate: c.name.clone(),
                path,
            });
        }
    }
    out
}

/// Breadth-first walk from `start`: the path to the first UI crate or the window crate.
fn reach(
    start: &Crate,
    by_name: &BTreeMap<&str, &Crate>,
    allowed: &str,
    with_dev: bool,
) -> Option<Vec<String>> {
    let mut seen = BTreeSet::from([start.name.as_str()]);
    let mut queue: std::collections::VecDeque<(&Crate, Vec<String>, bool)> =
        std::collections::VecDeque::from([(start, Vec::new(), with_dev)]);
    while let Some((c, path, dev_ok)) = queue.pop_front() {
        for d in &c.deps {
            if d.dev && !dev_ok {
                continue;
            }
            let mut next = path.clone();
            next.push(d.name.clone());
            if UI_CRATES.contains(&d.name.as_str()) || d.name == allowed {
                return Some(next);
            }
            if let Some(dep) = by_name.get(d.name.as_str())
                && seen.insert(dep.name.as_str())
            {
                queue.push_back((dep, next, false));
            }
        }
    }
    None
}

fn metadata(root: &Path) -> Result<Value, String> {
    let out = crate::cargo(root)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: bad JSON: {e}"))
}

pub fn run(root: &Path) -> Result<(), String> {
    let crates = from_metadata(&metadata(root)?)?;
    let violations = check(&crates, crate::UI_PACKAGE);
    if violations.is_empty() {
        println!(
            "layers: {} workspace crates, only {} uses the window toolkit ({})",
            crates.len(),
            crate::UI_PACKAGE,
            UI_CRATES.join(", ")
        );
        Ok(())
    } else {
        Err(format!(
            "{} crate(s) reach the window toolkit (only {} may):\n  {}",
            violations.len(),
            crate::UI_PACKAGE,
            violations
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n  ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, deps: &[(&str, bool)]) -> Crate {
        Crate {
            name: name.into(),
            deps: deps
                .iter()
                .map(|(n, dev)| Dep {
                    name: (*n).into(),
                    dev: *dev,
                })
                .collect(),
        }
    }

    #[test]
    fn only_the_window_may_use_egui() {
        let crates = vec![
            krate(
                "ui",
                &[("egui", false), ("hub", false), ("egui_kittest", true)],
            ),
            krate("hub", &[("core", false), ("serde", false)]),
            krate("core", &[]),
            krate("xtask", &[("serde_json", false)]),
        ];
        assert!(check(&crates, "ui").is_empty());
    }

    #[test]
    fn direct_transitive_and_dev_uses_are_caught() {
        let crates = vec![
            krate("ui", &[("egui", false)]),
            krate("hub", &[("core", false)]),
            krate("core", &[("rfd", false)]),
            krate("tests", &[("winit", true)]),
            krate("tool", &[("ui", false)]),
            krate("dev-only", &[("tests", true)]),
        ];
        let v: Vec<String> = check(&crates, "ui")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            v,
            vec![
                "hub -> core -> rfd".to_string(),
                "core -> rfd".into(),
                "tests -> winit".into(),
                "tool -> ui".into(),
            ]
        );
    }

    #[test]
    fn reads_cargo_metadata() {
        let meta = serde_json::json!({"packages": [
            {"name": "b", "dependencies": [{"name": "egui", "kind": "dev"}]},
            {"name": "a", "dependencies": [{"name": "b", "kind": null}]},
        ]});
        let crates = from_metadata(&meta).unwrap();
        assert_eq!(crates[0].name, "a");
        assert_eq!(
            crates[1].deps,
            vec![Dep {
                name: "egui".into(),
                dev: true
            }]
        );
    }
}
