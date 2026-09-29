//! The HTTP clients the shell uses, each built once so its connection pool
//! stays warm, and each with timeouts: a request to a host that stops
//! answering fails with a message instead of hanging a command forever.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;

const USER_AGENT: &str = concat!("DayZCommunityHub/", env!("CARGO_PKG_VERSION"));
/// Browser-like, for hosts that turn away unknown clients.
const BROWSER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:147.0) Gecko/20100101 Firefox/147.0";

fn build(b: reqwest::ClientBuilder) -> Client {
    // Building only fails when the TLS backend cannot initialise; a default
    // client still lets the rest of the app work.
    b.build().unwrap_or_else(|_| Client::new())
}

/// JSON APIs (Steam, the Workshop, BattleMetrics, ip-api): short answers.
pub(crate) fn api() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        build(
            Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(20)),
        )
    })
}

/// Large downloads (updates, SteamCMD): no total limit, but a stall of 30 s
/// between two reads fails the download. Only Windows downloads those.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn download() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        build(
            Client::builder()
                .user_agent(BROWSER_AGENT)
                .connect_timeout(Duration::from_secs(15))
                .read_timeout(Duration::from_secs(30)),
        )
    })
}

/// DayZ's CDN only: its certificate fails validation, so certificate checks
/// are off, which is why nothing else may use this client.
pub(crate) fn dayz_cdn() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        build(
            Client::builder()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .user_agent(BROWSER_AGENT)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30)),
        )
    })
}
