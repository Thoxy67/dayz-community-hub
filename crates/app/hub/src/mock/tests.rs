#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

use std::sync::{Arc, Mutex};

use crate::features::browser::ServerQuery;
use crate::features::stats::StatsRange;
use crate::{Hub, HubConfig, HubEvent, HubMode};

fn query() -> ServerQuery {
    serde_json::from_value(serde_json::json!({
        "search": "", "map": null, "firstPerson": "all", "password": "all",
        "battleye": "all", "modded": "all", "official": "all",
        "hideEmpty": false, "hideFull": false, "maxPing": 0,
        "showExcluded": false, "sort": "none", "asc": true,
        "offset": 0, "limit": 50
    }))
    .unwrap()
}

#[test]
fn the_generator_makes_mock_ts_world() {
    let g = super::generate::generate(1_790_000_000);
    assert_eq!(g.servers.len(), 9500);
    assert_eq!(g.mods.len(), 24);
    assert_eq!(g.articles.len(), 8);
    // The first servers are the officials and their lookalikes.
    assert_eq!(g.servers[0].name, "4100 | EUROPE - DE");
    assert!(g.servers[0].official);
    assert_eq!(g.servers[3].name, "4203 | UNITED STATES - NY | x10 Loot");
    assert_eq!(g.servers[0].ip, "45.0.0.0");
    assert_eq!(g.servers.iter().filter(|s| s.official).count(), 20);
    // As the reference screenshot shows them (var/reference/servers.png).
    let players: i64 = g.servers.iter().map(|s| s.players).sum();
    assert_eq!(players, 258_643);
    assert_eq!((g.servers[0].players, g.servers[0].max_players), (41, 60));
    assert_eq!(g.servers[0].time, "17:40");
    assert_eq!(g.servers[3].mods_count, 24);
    assert_eq!(g.mods.iter().filter(|m| m.update_available).count(), 4);
    assert!(g.play_log.last().unwrap().open);
    assert_eq!(super::ping_of("45.0.0.0:27016"), 92);
}

#[test]
fn dates_are_written_as_javascript_does() {
    assert_eq!(super::iso(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(super::iso(1_790_000_000_123), "2026-09-21T14:13:20.123Z");
    assert_eq!(super::iso_date(951_782_400), "2000-02-29");
    assert_eq!(super::utc_weekday(0), 4);
}

/// One test for the whole hub: the mock world lives in process-wide state
/// (the live store, the refresh, the play log) that tests running in
/// parallel would share.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mock_hub_answers_as_mock_ts_did() {
    let events: Arc<Mutex<Vec<HubEvent>>> = Arc::default();
    let sink = {
        let events = Arc::clone(&events);
        Arc::new(move |e| events.lock().unwrap().push(e))
    };
    let hub = Hub::new(
        HubConfig {
            mode: HubMode::Mock,
            ..Default::default()
        },
        sink,
    )
    .unwrap();
    let dir = hub.mock().unwrap().data_dir().to_path_buf();
    assert!(dir.join("profile.json").exists());
    assert!(!dir.starts_with(dz_common::paths::default_data_dir()));
    hub.start();
    assert!(!hub.check_first_launch().await);

    let init = hub.initialize().await.unwrap();
    assert_eq!(init.server_count, 9500);
    assert!(init.list_error.is_none());

    // The real browser over the mock's list.
    let page = hub.servers_query(query()).await.unwrap();
    assert_eq!(page.total, 9499, "one server's IP is excluded");
    assert_eq!(page.rows.len(), 50);
    assert_eq!(
        page.stats.players,
        258_643 - page_excluded_players(&hub).await
    );
    assert_eq!(page.stats.best_ping, Some(54));
    let first = &page.rows[0];
    assert_eq!(first.name, "4100 | EUROPE - DE");
    assert_eq!(first.ping_ms, Some(92));
    assert!(first.official);
    let lookalike = &page.rows[3];
    assert!(lookalike.mimics_official && lookalike.favorite);
    assert_eq!(lookalike.mods_count, 24);
    assert_eq!(lookalike.mods_missing, Some(0));

    // As var/reference/servers.png shows them: "1 ↑1", "22 −22", a timeout.
    let r = &page.rows[7];
    assert_eq!(
        (r.mods_count, r.mods_missing, r.mods_stale),
        (1, Some(0), 1)
    );
    let r = &page.rows[11];
    assert_eq!(
        (r.mods_count, r.mods_missing, r.mods_stale),
        (22, Some(22), 0)
    );
    assert!(page.rows[8].ping_failed);
    let stats = hub.get_app_stats().await.unwrap();
    assert_eq!(stats.server_count, 9500);
    assert_eq!(stats.total_players, 258_643);
    assert_eq!(stats.steam_login.as_deref(), Some("survivor_42"));
    assert!(stats.has_steamcmd);
    let sources = hub.sources_status().await;
    assert_eq!((sources.total, sources.official), (19_107, 1180));

    let profile = hub.get_profile().await.unwrap();
    assert_eq!(profile.player.as_deref(), Some("Survivor"));
    assert_eq!(profile.favorites.len(), 6);
    assert_eq!(profile.history.len(), 12);
    assert_eq!(profile.excluded_ips.len(), 1);

    let mods = hub.get_installed_mods().await.unwrap();
    assert_eq!(mods.len(), 24);
    assert_eq!(mods.iter().filter(|m| m.update_available).count(), 4);
    assert!(mods.iter().any(|m| m.remote_updated.is_none()));
    assert_eq!(hub.mods_usage().await.unwrap().len(), 14);

    let all = hub.play_stats(StatsRange::All, 0).await.unwrap();
    assert!(all.sessions > 100, "{} sessions", all.sessions);
    assert_eq!(all.places, 7);
    assert!(all.current.is_some());
    let page = hub
        .play_sessions(String::new(), None, None, 0, 100)
        .await
        .unwrap();
    assert_eq!(page.total as usize, all.sessions as usize);

    let news = hub.fetch_news().await.unwrap();
    assert_eq!(news.len(), 8);
    assert_eq!(news[0].title, "Update 1.28 is live");

    // Fixtures for what would leave the machine.
    assert_eq!(hub.steam_friends(false).await.unwrap().friends.len(), 10);
    assert_eq!(hub.fetch_steam_player_count().await.unwrap(), 61_204);
    let ip = first.ip.clone();
    assert_eq!(
        hub.query_a2s(ip.clone(), 27016, None)
            .await
            .unwrap()
            .players,
        41
    );
    let m = hub
        .fetch_server_metrics(ip.clone(), 2302, 27016, String::new())
        .await
        .unwrap();
    assert_eq!(m.player_history.len(), 288);
    assert_eq!(
        hub.fetch_metrics_history(1, "7d".into())
            .await
            .unwrap()
            .len(),
        336
    );
    assert_eq!(hub.fetch_metrics_heatmap(1).await.unwrap().len(), 168);
    assert!(hub.check_for_update().await.unwrap().is_some());
    assert_eq!(hub.ping_single(ip.clone(), 27016, None).await.unwrap(), 92);

    // A mod operation plays its steps through the real slot.
    hub.start_mod_operation(
        crate::features::steamcmd::ModOpType::UpdateStale,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let op = hub.mod_op_attach().await.unwrap();
    assert_eq!(op.kind, crate::features::steamcmd::ModOpType::UpdateStale);
    hub.cancel_mod_operation().await.unwrap();
    assert!(hub.mod_op_state().await.is_none());

    // The scan pings the whole list at mock.ts' pace.
    let first_progress = hub.start_scan().await.unwrap();
    assert!(first_progress.running);
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    let scan = hub.ping_scan_state().await.unwrap();
    assert!(scan.progress.done > 0, "{:?}", scan.progress);
    hub.cancel_ping().await.unwrap();

    // A join is recorded and reported, nothing started.
    hub.launch_server(ip, 27016, None).await.unwrap();
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, HubEvent::LaunchDone(n) if n == "4100 | EUROPE - DE"))
    );

    drop(hub);
}

/// Players on the servers hidden by the excluded IP.
async fn page_excluded_players(hub: &Hub) -> u64 {
    let w = hub.mock().unwrap();
    let ip = &w.servers[40].ip;
    w.servers
        .iter()
        .filter(|s| &s.ip == ip)
        .map(|s| s.players as u64)
        .sum()
}
