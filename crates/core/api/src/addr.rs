//! Server addresses as numbers: an IPv4 and a port ([`SocketAddrV4`], six
//! bytes, `Copy`, hashed as two integers).
//!
//! The lists name a server by its IP as text and its port as a number. They
//! are read into numbers once, where the JSON is parsed, so the merges, the
//! scans and the lookups that run over ten thousand servers compare and hash
//! integers instead of copying and hashing strings, and a query goes to the
//! socket with no `"ip:port"` written and parsed again. Text is made only to
//! show an address, link to it, copy it, or write the profile.
//!
//! The launcher list and Steam's master carry IPv4 addresses only. An entry
//! that is not one (a host name, IPv6, garbage) or has no usable port cannot
//! be queried nor joined from the list: it is left out where the list is
//! read, and how many were is printed.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
pub use std::net::{Ipv4Addr, SocketAddrV4};

/// The address of `ip` and `port`, when they make one: an IPv4 written as
/// digits and a port in 1..=65535.
pub fn addr(ip: &str, port: i64) -> Option<SocketAddrV4> {
    Some(SocketAddrV4::new(
        ip.trim().parse().ok()?,
        nonzero_port(port)?,
    ))
}

/// `port` when it is one a server can listen on.
pub fn nonzero_port(port: i64) -> Option<u16> {
    u16::try_from(port).ok().filter(|p| *p != 0)
}

/// Whether `a` names a server: neither its IP nor its port is zero (what an
/// entry read without a usable address holds).
pub fn is_addressed(a: &SocketAddrV4) -> bool {
    !a.ip().is_unspecified() && a.port() != 0
}

/// The address no server has, which an unreadable entry reads as.
pub const NO_ADDR: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);

/// An IPv4 as text: anything else (a host name, IPv6, `null`) reads as
/// `None` instead of failing the whole file around it.
pub mod lenient_ip {
    use super::*;

    pub fn serialize<S: Serializer>(ip: &Option<Ipv4Addr>, s: S) -> Result<S::Ok, S::Error> {
        ip.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Ipv4Addr>, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Option<Ipv4Addr>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an IP address as text")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(v.trim().parse().ok())
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
        }
        d.deserialize_any(Visitor)
    }
}

/// Steam's `"ip:port"` as an address, read without a copy of the text;
/// anything else (a host name, IPv6, port 0) reads as `None`.
pub(crate) fn lenient_addr<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<SocketAddrV4>, D::Error> {
    struct Visitor;
    impl serde::de::Visitor<'_> for Visitor {
        type Value = Option<SocketAddrV4>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an address as \"ip:port\"")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.trim()
                .parse::<SocketAddrV4>()
                .ok()
                .filter(|a| a.port() != 0))
        }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }
    d.deserialize_any(Visitor)
}

/// A port as a number; one outside 1..=65535 reads as 0 (unknown), as the
/// list's own `0` does.
pub(crate) fn port_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    Ok(nonzero_port(i64::deserialize(d)?).unwrap_or(0))
}

/// The launcher list's `endpoint`, `{"ip": "1.2.3.4", "port": 27016}`, as an
/// address; one that is not an IPv4 and a port reads as [`NO_ADDR`].
pub(crate) mod endpoint {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Endpoint {
        #[serde(with = "lenient_ip", default)]
        ip: Option<Ipv4Addr>,
        #[serde(default)]
        port: i64,
    }

    pub fn serialize<S: Serializer>(a: &SocketAddrV4, s: S) -> Result<S::Ok, S::Error> {
        Endpoint {
            ip: Some(*a.ip()),
            port: i64::from(a.port()),
        }
        .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SocketAddrV4, D::Error> {
        let e = Endpoint::deserialize(d)?;
        Ok(e.ip
            .zip(nonzero_port(e.port))
            .map_or(NO_ADDR, |(ip, port)| SocketAddrV4::new(ip, port)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_ipv4_and_a_port_make_an_address() {
        assert_eq!(
            addr("1.2.3.4", 27016),
            Some("1.2.3.4:27016".parse().unwrap())
        );
        assert_eq!(addr(" 1.2.3.4 ", 1), Some("1.2.3.4:1".parse().unwrap()));
        for (ip, port) in [
            ("1.2.3.4", 0),
            ("1.2.3.4", 70_000),
            ("1.2.3.4", -1),
            ("::1", 2302),
            ("play.example.com", 2302),
            ("", 2302),
        ] {
            assert_eq!(addr(ip, port), None, "{ip}:{port}");
        }
    }

    #[test]
    fn an_endpoint_reads_and_writes_as_the_list_has_it() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct S {
            #[serde(with = "endpoint")]
            e: SocketAddrV4,
        }
        let s: S = serde_json::from_str(r#"{"e":{"ip":"1.2.3.4","port":27016}}"#).unwrap();
        assert_eq!(s.e, "1.2.3.4:27016".parse().unwrap());
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"e":{"ip":"1.2.3.4","port":27016}}"#
        );
        for bad in [
            r#"{"e":{"ip":"host.example","port":27016}}"#,
            r#"{"e":{"ip":"::1","port":27016}}"#,
            r#"{"e":{"ip":null,"port":27016}}"#,
            r#"{"e":{"ip":"1.2.3.4","port":99999}}"#,
            r#"{"e":{"ip":"1.2.3.4"}}"#,
        ] {
            let s: S = serde_json::from_str(bad).unwrap();
            assert!(!is_addressed(&s.e), "{bad}");
        }
    }
}
