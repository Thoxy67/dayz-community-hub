use dayz_community_hub_core::{api, config, system};
use dayz_community_hub_ui::{App, draw_ui, handle_key, run_setup_if_needed};
use ratatui::{Terminal, backend::TermionBackend};
use std::io;
use termion::{input::TermRead, raw::IntoRawMode};

#[tokio::main]
async fn main() -> dayz_community_hub_core::Result<()> {
    let profile_path = config::default_profile_path();

    // First-run setup wizard
    run_setup_if_needed(&profile_path)?;

    // Initialise controller
    let mut ctl = dayz_community_hub_core::ctl::DayzCtl::new(&profile_path).await?;

    // Use cached server list (5 min TTL) or fetch fresh
    let cache_path = config::default_data_dir().join("server_list_cache.json");
    const CACHE_TTL: u64 = 300;
    let servers_list = if let Some(cache) = api::load_server_list_cache(&cache_path) {
        if cache.is_fresh(CACHE_TTL) {
            println!(
                "Using cached server list ({} servers, cached {}s ago)...",
                cache.list.result.len(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .saturating_sub(cache.fetched_at)
            );
            cache.list
        } else {
            println!("Cache expired, fetching server list...");
            let list = ctl.fetch_servers().await?.clone();
            api::save_server_list_cache(&cache_path, &list);
            list
        }
    } else {
        println!("Fetching server list...");
        let list = ctl.fetch_servers().await?.clone();
        api::save_server_list_cache(&cache_path, &list);
        list
    };

    let server_count = servers_list.result.len();
    let servers = servers_list.result;
    println!("Loaded {} servers. Starting TUI...", server_count);

    // Set up terminal
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(ctl, servers);

    // Background initialisations
    app.refresh_installed_mods();
    app.start_background_ping();
    app.fetch_steam_players();
    app.fetch_news_if_needed();

    // System check
    if let Ok(check) = system::check_max_map_count() {
        if !check.ok {
            app.set_warn(format!("vm.max_map_count too low ({})", check.current));
        }
    }

    // Main event loop
    let mut running = true;
    let async_stdin = termion::async_stdin();
    let mut key_iter = async_stdin.keys();

    while running {
        if let Ok((_, h)) = termion::terminal_size() {
            app.term_height = h;
        }

        app.poll_progress();
        app.poll_pings();
        app.poll_background();
        app.poll_misc();

        terminal.draw(|f| draw_ui(f, &app))?;

        while let Some(key_result) = key_iter.next() {
            let key = match key_result {
                Ok(k) => k,
                Err(_) => break,
            };
            if !handle_key(&mut app, key) {
                running = false;
            }
            break; // one key per frame
        }

        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    terminal.clear()?;
    terminal.show_cursor()?;

    Ok(())
}
