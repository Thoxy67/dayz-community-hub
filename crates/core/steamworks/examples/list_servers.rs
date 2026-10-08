//! Live check of the server listing: `cargo run --example list_servers -- noplayers,map=banov`.
use std::sync::atomic::AtomicBool;

use dz_steamworks::ListEvent;

fn main() {
    dz_steamworks::serve_if_worker();
    let parts: Vec<dz_steamworks::Filters> = std::env::args()
        .skip(1)
        .map(|a| {
            a.split(',')
                .map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap_or((kv, "1"));
                    (k.into(), v.into())
                })
                .collect()
        })
        .collect();
    let cancel = AtomicBool::new(false);
    let t = std::time::Instant::now();
    let r = dz_steamworks::list_servers(&parts, &cancel, &mut |e| match e {
        ListEvent::Found { empty, servers } => {
            println!(
                "{:>6.1?} found {} (empty {empty:?}), e.g. {:?}",
                t.elapsed(),
                servers.len(),
                servers[0]
            );
        }
        ListEvent::Done(l) => println!("{:>6.1?} done {l:?}", t.elapsed()),
    });
    println!("{r:?} in {:?}", t.elapsed());
}
