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

/// JSON APIs (Steam, the Workshop, ip-api, the launcher's server list): short
/// answers; a longer one (the server list, several MB) sets its own limit
/// on the request.
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

/// DayZ Metrics: its own client, so the site sees which app asks and where
/// to find it, as a courtesy for using the endpoints its pages use.
pub(crate) fn dayzmetrics() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        build(
            Client::builder()
                .user_agent(concat!(
                    "DayZ-Community-Hub/",
                    env!("CARGO_PKG_VERSION"),
                    " (+https://git.thoxy.xyz/thoxy/dayz-community-hub)"
                ))
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(20)),
        )
    })
}

/// Large downloads (updates, SteamCMD, the offline mode's archive, through
/// `DayzCtl`): no total limit, but a stall of 30 s between two reads fails
/// the download.
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

/// Pictures from the rest of the web (an article's pictures and videos, the
/// window's avatars and Workshop previews): checked certificates, a browser's
/// name, short limits.
pub fn pictures() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        build(
            Client::builder()
                .user_agent(BROWSER_AGENT)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30)),
        )
    })
}

/// The A2S engine every scan, listing and check shares: one socket per
/// address family and one pace for the whole app, so a listing running
/// beside the ping scan does not add a burst of its own. Made again if the
/// runtime it ran on has gone (a test's).
pub(crate) fn a2s() -> dz_a2s::Engine {
    static E: std::sync::Mutex<Option<dz_a2s::Engine>> = std::sync::Mutex::new(None);
    let mut engine = E.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match engine.as_ref() {
        Some(e) if !e.is_closed() => e.clone(),
        _ => {
            let e = dz_a2s::Engine::new(dz_a2s::Pace::default());
            *engine = Some(e.clone());
            e
        }
    }
}
