//! Live check of the friends list: `cargo run --example friends`.
fn main() {
    dz_steamworks::serve_if_worker();
    match dz_steamworks::friends() {
        Ok(f) => {
            println!("{} friends", f.len());
            for x in f.iter().filter(|x| x.state != 0 || x.game.is_some()) {
                println!("  {} state={} game={:?}", x.name, x.state, x.game);
            }
        }
        Err(e) => println!("{e}"),
    }
    match dz_steamworks::me() {
        Ok(m) => println!(
            "me: {} {} avatar {:?}",
            m.steam_id,
            m.name,
            m.avatar.as_ref().map(|a| (a.0, a.1, a.2.len()))
        ),
        Err(e) => println!("me: {e}"),
    }
    match dz_steamworks::workshop_details(&[1559212036, 1564026768]) {
        Ok(d) => d
            .iter()
            .for_each(|d| println!("ugc {} {:?} updated {}", d.id, d.title, d.updated)),
        Err(e) => println!("ugc: {e}"),
    }
}
