//! `cargo xtask assets`: every file under `apps/desktop/assets/` is covered by a row of
//! `ATTRIBUTION.md`.
//!
//! A row is a Markdown table line (`| … |`); its first cell names one or more paths in backticks,
//! relative to the repository root: a file (`` `apps/desktop/assets/fonts/A.ttf` ``), a directory
//! (`` `apps/desktop/assets/fonts/` ``, with or without the slash) or a glob
//! (`` `apps/desktop/assets/fonts/IBMPlex*.ttf` ``, `**` crosses directories, `?` is one
//! character). Paths use `/` on every platform.

use std::path::Path;

const ASSET_ROOT: &str = "apps/desktop/assets";

/// The backticked paths in the first cell of each table row.
pub fn patterns(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in md.lines() {
        let Some(row) = line.trim().strip_prefix('|') else {
            continue;
        };
        let first = row.split('|').next().unwrap_or("");
        let mut parts = first.split('`');
        parts.next();
        // Every odd piece is inside backticks.
        while let Some(inside) = parts.next() {
            let p = inside.trim().trim_start_matches("./");
            if !p.is_empty() {
                out.push(p.replace('\\', "/"));
            }
            parts.next();
        }
    }
    out
}

/// Whether `path` (repo-relative, `/`-separated) is covered by `pattern`.
pub fn covers(pattern: &str, path: &str) -> bool {
    if pattern.contains(['*', '?']) {
        return glob(pattern.as_bytes(), path.as_bytes());
    }
    let dir = pattern.trim_end_matches('/');
    path == dir
        || path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `*` matches within one path segment, `**` across segments, `?` one non-`/` byte.
fn glob(p: &[u8], s: &[u8]) -> bool {
    match p.split_first() {
        None => s.is_empty(),
        Some((b'*', rest)) => {
            if let Some(rest2) = rest.strip_prefix(b"*") {
                // `**` is any run of bytes; `**/` may also match no directory at all.
                rest2.strip_prefix(b"/").is_some_and(|r| glob(r, s))
                    || (0..=s.len()).any(|i| glob(rest2, s.get(i..).unwrap_or_default()))
            } else {
                (0..=s.len())
                    .take_while(|&i| i == 0 || s.get(i - 1) != Some(&b'/'))
                    .any(|i| glob(rest, s.get(i..).unwrap_or_default()))
            }
        }
        Some((b'?', rest)) => match s.split_first() {
            Some((c, srest)) if *c != b'/' => glob(rest, srest),
            _ => false,
        },
        Some((c, rest)) => match s.split_first() {
            Some((d, srest)) if c == d => glob(rest, srest),
            _ => false,
        },
    }
}

/// The files under `dir`, repo-relative with `/`, sorted.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if kind.is_dir() {
            walk(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(parts.join("/"));
        }
    }
    Ok(())
}

/// A licence text shipped beside the assets it covers: the attribution itself.
fn is_licence(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.starts_with("LICENSE") || name == "OFL.txt"
}

/// The files no pattern covers (licence texts excepted).
pub fn missing(files: &[String], patterns: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|f| !is_licence(f) && !patterns.iter().any(|p| covers(p, f)))
        .cloned()
        .collect()
}

pub fn run(root: &Path) -> Result<(), String> {
    let dir = root.join(ASSET_ROOT);
    let mut files = Vec::new();
    if dir.is_dir() {
        walk(root, &dir, &mut files)?;
    }
    files.sort();
    let md_path = root.join("ATTRIBUTION.md");
    let md = std::fs::read_to_string(&md_path).map_err(|e| {
        format!(
            "{}: {e} ({} asset files to attribute)",
            md_path.display(),
            files.len()
        )
    })?;
    let miss = missing(&files, &patterns(&md));
    if miss.is_empty() {
        println!(
            "assets: all {} files under {ASSET_ROOT}/ are attributed in ATTRIBUTION.md",
            files.len()
        );
        Ok(())
    } else {
        Err(format!(
            "{} file(s) under {ASSET_ROOT}/ lack an ATTRIBUTION.md row (author, source, licence):\n  {}",
            miss.len(),
            miss.join("\n  ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_paths_from_the_first_cell_only() {
        let md =
            "| Asset | Author |\n|---|---|\n| `a/b.ttf`, `c/` | me `not/this` |\nnot | `a row` |\n";
        assert_eq!(patterns(md), vec!["a/b.ttf".to_string(), "c/".into()]);
    }

    #[test]
    fn files_directories_and_globs_cover() {
        assert!(covers("a/b.ttf", "a/b.ttf"));
        assert!(!covers("a/b.ttf", "a/b.ttf2"));
        assert!(covers("a/fonts/", "a/fonts/x.ttf"));
        assert!(covers("a/fonts", "a/fonts/sub/x.ttf"));
        assert!(!covers("a/fonts", "a/fontsx/x.ttf"));
        assert!(covers("a/fonts/*.ttf", "a/fonts/x.ttf"));
        assert!(!covers("a/fonts/*.ttf", "a/fonts/sub/x.ttf"));
        assert!(covers("a/**/*.ttf", "a/fonts/sub/x.ttf"));
        assert!(covers("a/**/*.ttf", "a/x.ttf"));
        assert!(covers("a/**", "a/b/c"));
        assert!(covers("a/IBM?lex-*", "a/IBMPlex-400.ttf"));
        assert!(!covers("a/?", "a/"));
    }

    #[test]
    fn reports_the_uncovered() {
        let files = vec!["a/x.svg".to_string(), "a/fonts/y.ttf".into()];
        assert_eq!(
            missing(&files, &["a/fonts/".into()]),
            vec!["a/x.svg".to_string()]
        );
    }
}
