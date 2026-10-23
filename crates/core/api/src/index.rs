//! O(1) lookup of a server by address, without allocating per lookup.

use rustc_hash::FxHashMap;

use crate::{Server, SocketAddrV4};

/// Where each server of a list sits, by its query address and by its
/// address to join.
///
/// Keyed by the addresses themselves (six bytes, hashed as integers): a
/// lookup builds no key, and the index is two tables for the whole list
/// rather than an entry allocated per IP. The first server at an address
/// keeps it, so "first occurrence wins" for duplicate endpoints.
#[derive(Debug, Default, Clone)]
pub struct ServerIndex {
    by_query: FxHashMap<SocketAddrV4, u32>,
    by_join: FxHashMap<SocketAddrV4, u32>,
}

impl ServerIndex {
    /// Index `servers` by position.
    pub fn build<S: std::borrow::Borrow<Server>>(servers: &[S]) -> Self {
        let n = servers.len();
        let mut by_query = FxHashMap::with_capacity_and_hasher(n, Default::default());
        let mut by_join = FxHashMap::with_capacity_and_hasher(n, Default::default());
        for (idx, s) in servers.iter().enumerate() {
            let s = s.borrow();
            let idx = idx as u32;
            by_query.entry(s.endpoint).or_insert(idx);
            if let Some(join) = s.join_addr() {
                by_join.entry(join).or_insert(idx);
            }
        }
        Self { by_query, by_join }
    }

    /// Position of the server whose query address is `addr`.
    pub fn by_query_port(&self, addr: SocketAddrV4) -> Option<usize> {
        self.by_query.get(&addr).map(|&i| i as usize)
    }

    /// Position of the server whose address to join is `addr`.
    pub fn by_game_port(&self, addr: SocketAddrV4) -> Option<usize> {
        self.by_join.get(&addr).map(|&i| i as usize)
    }

    /// Query port first, then game port.
    pub fn flexible(&self, addr: SocketAddrV4) -> Option<usize> {
        self.by_query_port(addr).or_else(|| self.by_game_port(addr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(ip: &str, qport: u16, gport: u16) -> Server {
        Server {
            endpoint: SocketAddrV4::new(ip.parse().unwrap(), qport),
            game_port: gport,
            ..Default::default()
        }
    }

    fn at(s: &str) -> SocketAddrV4 {
        s.parse().unwrap()
    }

    #[test]
    fn lookups() {
        let list = [
            server("1.1.1.1", 27016, 2302),
            server("1.1.1.1", 27017, 2402),
            server("2.2.2.2", 27016, 2302),
            server("1.1.1.1", 27016, 9999),
            server("3.3.3.3", 27016, 0),
        ];
        let ix = ServerIndex::build(&list);
        assert_eq!(ix.by_query_port(at("1.1.1.1:27017")), Some(1));
        // First occurrence wins.
        assert_eq!(ix.by_query_port(at("1.1.1.1:27016")), Some(0));
        assert_eq!(ix.by_game_port(at("1.1.1.1:2402")), Some(1));
        assert_eq!(ix.flexible(at("2.2.2.2:2302")), Some(2));
        assert_eq!(ix.flexible(at("3.3.3.3:2302")), None);
        // An unknown game port is no address to join.
        assert_eq!(ix.by_game_port(at("3.3.3.3:0")), None);
    }
}
