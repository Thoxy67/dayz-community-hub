use dayzsa_ml::{Result, config, ctl::DayzCtl};

#[tokio::main]
async fn main() -> Result<()> {
    let profile_path = config::default_profile_path();

    // Initialize DayzCtl (creates default profile if missing)
    let mut ctl = DayzCtl::new(&profile_path).await?;

    // Show profile info
    let profile = ctl.profile();
    println!("Profile: {}", profile.path.display());
    println!(
        "  Login: {}",
        profile.steam_login.as_deref().unwrap_or("not set")
    );
    println!(
        "  Player: {}",
        profile.player.as_deref().unwrap_or("not set")
    );
    println!(
        "  Steam root: {}",
        profile.steam_root.as_deref().unwrap_or("not set")
    );
    println!(
        "  SteamCMD: {}",
        if ctl.has_steamcmd() {
            "found"
        } else {
            "not found"
        }
    );
    println!("  Launch options: {}", profile.options.to_args().join(" "));

    // System checks
    match dayzsa_ml::system::check_max_map_count() {
        Ok(check) => println!("  vm.max_map_count: {}", check.recommendation()),
        Err(e) => println!("  vm.max_map_count: could not check ({})", e),
    }

    // Fetch servers
    println!("\nFetching server list...");
    let servers = ctl.fetch_servers().await?;
    println!("Found {} servers", servers.count());

    // Show first 5
    for (i, server) in servers.result.iter().take(5).enumerate() {
        println!(
            "  {}. {} | {}:{} | {}/{} players | {} mods | {}",
            i + 1,
            server.name,
            server.endpoint.ip,
            server.game_port,
            server.players,
            server.max_players,
            server.mods.len(),
            server.map,
        );
    }

    // Show installed mods
    match ctl.get_installed_mods() {
        Ok(mods) => {
            println!("\nInstalled mods: {}", mods.len());
            for m in mods.iter().take(5) {
                let managed = if m.managed { " [managed]" } else { "" };
                println!(
                    "  - {} (ID: {}) {}{}",
                    m.name,
                    m.id,
                    dayzsa_ml::mods::format_size(m.size),
                    managed,
                );
            }
            if mods.len() > 5 {
                println!("  ... and {} more", mods.len() - 5);
            }
        }
        Err(e) => println!("\nCould not scan mods: {}", e),
    }

    // Show favorites
    println!("\nFavorites: {}", ctl.profile().favorites.len());
    println!("History: {}", ctl.profile().history.len());

    println!("\nRun `cargo run --bin cli` for the TUI interface.");

    Ok(())
}
