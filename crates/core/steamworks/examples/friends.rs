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
}
