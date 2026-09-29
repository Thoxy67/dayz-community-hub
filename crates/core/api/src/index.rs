//! O(1) lookup of a server by address, without allocating per lookup.

use rustc_hash::FxHashMap;

use crate::Server;

/// Where each server of a list sits, by IP.
///
/// Keyed by the IP string alone, so a lookup borrows the caller's `&str`
/// (`Box<str>: Borrow<str>`) instead of building a `(String, port)` key for
/// every call; the handful of servers sharing an IP are then scanned in list
/// order, which keeps "first occurrence wins" for duplicate endpoints.
#[derive(Debug, Default, Clone)]
pub struct ServerIndex {
    by_ip: FxHashMap<Box<str>, Vec<Slot>>,
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    query_port: i64,
    game_port: i64,
    idx: usize,
}

impl ServerIndex {
    /// Index `servers` by position.
    pub fn build(servers: &[Server]) -> Self {
        let mut by_ip: FxHashMap<Box<str>, Vec<Slot>> =
            FxHashMap::with_capacity_and_hasher(servers.len(), Default::default());
        for (idx, s) in servers.iter().enumerate() {
            let slot = Slot {
                query_port: s.endpoint.port,
                game_port: s.game_port,
                idx,
            };
            match by_ip.get_mut(s.endpoint.ip.as_str()) {
                Some(slots) => slots.push(slot),
                None => {
                    by_ip.insert(s.endpoint.ip.as_str().into(), vec![slot]);
                }
            }
        }
        Self { by_ip }
    }

    /// Position of the server whose query port is `port`.
    pub fn by_query_port(&self, ip: &str, port: i64) -> Option<usize> {
        self.by_ip
            .get(ip)?
            .iter()
            .find(|s| s.query_port == port)
            .map(|s| s.idx)
    }

    /// Position of the server whose game port is `port`.
    pub fn by_game_port(&self, ip: &str, port: i64) -> Option<usize> {
        self.by_ip
            .get(ip)?
            .iter()
            .find(|s| s.game_port == port)
            .map(|s| s.idx)
    }

    /// Query port first, then game port.
    pub fn flexible(&self, ip: &str, port: i64) -> Option<usize> {
        self.by_query_port(ip, port)
            .or_else(|| self.by_game_port(ip, port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Endpoint;

    fn server(ip: &str, qport: i64, gport: i64) -> Server {
        Server {
            endpoint: Endpoint {
                ip: ip.into(),
                port: qport,
            },
            game_port: gport,
            ..Default::default()
        }
    }

    #[test]
    fn lookups() {
        let list = [
            server("1.1.1.1", 27016, 2302),
            server("1.1.1.1", 27017, 2402),
            server("2.2.2.2", 27016, 2302),
            server("1.1.1.1", 27016, 9999),
        ];
        let ix = ServerIndex::build(&list);
        assert_eq!(ix.by_query_port("1.1.1.1", 27017), Some(1));
        // First occurrence wins.
        assert_eq!(ix.by_query_port("1.1.1.1", 27016), Some(0));
        assert_eq!(ix.by_game_port("1.1.1.1", 2402), Some(1));
        assert_eq!(ix.flexible("2.2.2.2", 2302), Some(2));
        assert_eq!(ix.flexible("3.3.3.3", 2302), None);
    }
}
