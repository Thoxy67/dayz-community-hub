//! Mods downloaded by the running Steam client (dz-steamworks), reported
//! as the same progress a SteamCMD session sends, so the window shows both
//! alike.
//!
//! The Steamworks session runs on a thread of its own (the API is not
//! thread-safe) and ends with the download: Steam then stops showing DayZ
//! as running. Dropping the future (the operation cancelled) stops it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use dz_common::{Error, Result};
use dz_steamcmd::{ModProgress, ProgressTx};
use dz_steamworks::Event;
use tokio::sync::oneshot;

/// Said after a failure to connect, where SteamCMD would work instead.
const TRY_STEAMCMD: &str = "Or switch mod downloads back to SteamCMD in Settings → Steam.";

/// Sets the flag when dropped: the session thread sees it and closes.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Have Steam download `mods` ((id, name) pairs). Sends progress on `tx`,
/// ending with `Finished`; returns each mod's folder or error.
pub(crate) async fn download(
    mods: &[(u64, String)],
    validate: bool,
    tx: &ProgressTx,
) -> Vec<(u64, Result<PathBuf>)> {
    let total = mods.len();
    if total == 0 {
        let _ = tx.send(ModProgress::Finished {
            ok: 0,
            failed: 0,
            total: 0,
            hint: None,
        });
        return Vec::new();
    }
    if validate {
        let _ = tx.send(ModProgress::LogLine(
            "Steam checks a mod's files itself; to force it, unsubscribe from the mod in Steam and download it again."
                .into(),
        ));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let _cancel_on_drop = CancelOnDrop(cancel.clone());
    let (done_tx, done_rx) = oneshot::channel();
    let owned = mods.to_vec();
    let events = tx.clone();
    let spawned = std::thread::Builder::new()
        .name("steamworks".into())
        .spawn(move || {
            let ids: Vec<u64> = owned.iter().map(|(id, _)| *id).collect();
            let outcome = dz_steamworks::download(&ids, &cancel, &mut |e| {
                for msg in progress_of(e, &owned) {
                    let _ = events.send(msg);
                }
            });
            let _ = done_tx.send(outcome);
        });
    let outcome = match spawned {
        Ok(_) => done_rx
            .await
            .unwrap_or_else(|_| Err("The Steam download stopped unexpectedly.".into())),
        Err(e) => Err(format!("The Steam download could not start ({e}).")),
    };

    match outcome {
        Ok(results) => {
            let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
            let _ = tx.send(ModProgress::Finished {
                ok,
                failed: total - ok,
                total,
                hint: None,
            });
            results
                .into_iter()
                .map(|(id, r)| (id, r.map_err(Error::Mod)))
                .collect()
        }
        Err(why) => {
            let _ = tx.send(ModProgress::LogLine(why.clone()));
            for (i, (id, name)) in mods.iter().enumerate() {
                let _ = tx.send(ModProgress::Failed {
                    current: i + 1,
                    total,
                    mod_id: *id,
                    name: name.clone(),
                    error: "not downloaded".into(),
                });
            }
            let _ = tx.send(ModProgress::Finished {
                ok: 0,
                failed: total,
                total,
                hint: Some(format!("{why} {TRY_STEAMCMD}")),
            });
            mods.iter()
                .map(|(id, _)| (*id, Err(Error::Mod(why.clone()))))
                .collect()
        }
    }
}

/// The line the window reads a download's bytes from: the shape of
/// SteamCMD's own ("progress: 42.17 (123 / 456)"), which it already parses.
pub(crate) fn progress_line(id: u64, done: u64, total: u64) -> String {
    format!(
        "Steam: item {id} downloading, progress: {:.2} ({done} / {total})",
        dz_steamworks::percent(done, total)
    )
}

/// What the window is sent for a session's event. `mods` names the items.
fn progress_of(e: Event, mods: &[(u64, String)]) -> Vec<ModProgress> {
    let total = mods.len();
    let name = |index: usize| mods.get(index).map(|(_, n)| n.clone()).unwrap_or_default();
    match e {
        Event::Log(line) => vec![ModProgress::LogLine(line)],
        Event::Starting { index, id } => vec![
            ModProgress::Starting {
                current: index + 1,
                total,
                mod_id: id,
                name: name(index),
            },
            // "Downloading item" also tells the window a new item's bytes begin.
            ModProgress::LogLine(format!(
                "Downloading item {id} ({}) through Steam",
                name(index)
            )),
        ],
        Event::Progress {
            id, done, total, ..
        } => vec![ModProgress::LogProgress(progress_line(id, done, total))],
        Event::Installed { index, id, path } => vec![
            ModProgress::LogLine(format!("Installed {id} at {}", path.display())),
            ModProgress::Done {
                current: index + 1,
                total,
                mod_id: id,
                name: name(index),
            },
        ],
        Event::Failed { index, id, error } => vec![
            ModProgress::LogLine(format!("{id} failed: {error}")),
            ModProgress::Failed {
                current: index + 1,
                total,
                mod_id: id,
                name: name(index),
                error,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_read_as_steamcmds() {
        // The window's pattern (apps/gui/src/features/mods/steamcmd-log.ts).
        let re = regex::Regex::new(r"(?i)progress:\s*([\d.]+)\s*\((\d+)\s*/\s*(\d+)\)").unwrap();
        let line = progress_line(1_559_212_036, 50, 200);
        let c = re.captures(&line).expect("matches");
        assert_eq!(&c[1], "25.00");
        assert_eq!(&c[2], "50");
        assert_eq!(&c[3], "200");
        assert!(re.is_match(&progress_line(1, 0, 0)));
    }

    #[test]
    fn events_become_the_progress_steamcmd_sends() {
        let mods = vec![(10, "CF".to_string()), (20, "Trader".to_string())];
        let msgs = progress_of(Event::Starting { index: 1, id: 20 }, &mods);
        assert!(matches!(
            &msgs[0],
            ModProgress::Starting { current: 2, total: 2, mod_id: 20, name } if name == "Trader"
        ));
        assert!(
            matches!(&msgs[1], ModProgress::LogLine(l) if l.to_lowercase().contains("downloading item 20"))
        );

        let msgs = progress_of(
            Event::Installed {
                index: 0,
                id: 10,
                path: PathBuf::from("/lib/workshop/content/221100/10"),
            },
            &mods,
        );
        assert!(matches!(
            &msgs[1],
            ModProgress::Done {
                current: 1,
                mod_id: 10,
                ..
            }
        ));

        let msgs = progress_of(
            Event::Failed {
                index: 0,
                id: 10,
                error: "private".into(),
            },
            &mods,
        );
        assert!(
            matches!(&msgs[1], ModProgress::Failed { mod_id: 10, error, .. } if error == "private")
        );

        let msgs = progress_of(
            Event::Progress {
                index: 0,
                id: 10,
                done: 1,
                total: 4,
            },
            &mods,
        );
        assert!(matches!(&msgs[0], ModProgress::LogProgress(l) if l.contains("25.00 (1 / 4)")));
    }
}
