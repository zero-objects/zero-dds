// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `corbaloc:` and `corbaname:` URL parsers — CORBA 3.3 Part 2 §7.6.10.
//!
//! ## `corbaloc:` (§7.6.10.1 to §7.6.10.3, §7.6.10.6)
//! ```text
//! <corbaloc>       = "corbaloc:" <obj_addr_list> ["/" <key_string>]
//! <obj_addr_list>  = [<obj_addr> ","]* <obj_addr>
//! <obj_addr>       = <rir_prot_addr> | <iiop_prot_addr> | <future_prot_addr>
//! <rir_prot_addr>  = "rir:"
//! <iiop_prot_addr> = (":" | "iiop:") [<version> <host> [":" <port>]]
//! <host>           = DNS_style_host_name | IPv4_address | "[" IPv6_address "]"
//! <version>        = <major> "." <minor> "@" | empty_string
//! ```
//!
//! Without a host the local host is meant, without a port 2809, without a
//! version IIOP 1.0. `rir` stands alone in its URL and has no address; an
//! empty key of a `rir` URL is `NameService`. Another protocol token is
//! kept with its address as written (§7.6.10.6). The key string uses the
//! escapes of RFC 2396 (`%XX`).
//!
//! ## `corbaname:` (§7.6.10.5, CosNaming §2.5.3)
//! ```text
//! <corbaname> = "corbaname:" <obj_addr_list> ["/" <key_string>] ["#" <string_name>]
//! ```
//! The key string defaults to `NameService`; the stringified name is
//! unescaped like the key string.
//!
//! Every error maps to the `BAD_PARAM` minor code `string_to_object`
//! raises for it ([`IorError::bad_param_minor`]).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_corba_iiop::IiopVersion;

use crate::error::{IorError, IorResult};

/// The default port of an IIOP address (§7.6.10.3).
pub const DEFAULT_IIOP_PORT: u16 = 2809;

/// The host an IIOP address without a host names (§7.6.10.3: "If not
/// present, the local host is assumed").
pub const LOCAL_HOST: &str = "localhost";

/// The key of a `rir` URL with an empty key string and the default key of
/// a `corbaname` URL (§7.6.10.2, §7.6.10.5).
pub const DEFAULT_NAMING_KEY: &str = "NameService";

/// A `corbaloc:` address: the list of addresses and the object key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbalocAddress {
    /// The addresses in the order of the URL.
    pub endpoints: Vec<CorbalocEndpoint>,
    /// The object key after `%XX` decoding; empty when the URL has no key
    /// string, `NameService` for a `rir` URL with an empty key.
    pub object_key: Vec<u8>,
}

impl CorbalocAddress {
    /// `true` when the address is the `rir` protocol.
    #[must_use]
    pub fn is_rir(&self) -> bool {
        self.endpoints
            .iter()
            .any(|e| matches!(e, CorbalocEndpoint::Rir))
    }

    /// The IIOP addresses in the order of the URL.
    pub fn iiop_addresses(&self) -> impl Iterator<Item = &IiopAddress> {
        self.endpoints.iter().filter_map(|e| match e {
            CorbalocEndpoint::Iiop(a) => Some(a),
            _ => None,
        })
    }
}

/// One `<obj_addr>` of a `corbaloc:` address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorbalocEndpoint {
    /// `iiop:` or the default token `:` (§7.6.10.3).
    Iiop(IiopAddress),
    /// `rir:` (§7.6.10.2): the key string names an initial reference.
    Rir,
    /// A protocol this parser does not define (§7.6.10.6): the token and
    /// the protocol specific address as written.
    Other {
        /// The protocol token without the `:`.
        protocol: String,
        /// The address after `<token>:`.
        address: String,
    },
}

/// An `<iiop_addr>` (§7.6.10.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IiopAddress {
    /// The IIOP version, 1.0 when the URL gives none.
    pub version: IiopVersion,
    /// A DNS name, an IPv4 address or an IPv6 address without the
    /// brackets of the URL; [`LOCAL_HOST`] when the URL gives no host.
    pub host: String,
    /// The port, [`DEFAULT_IIOP_PORT`] when the URL gives none.
    pub port: u16,
}

/// A `corbaname:` address: the naming context and the name to resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbanameAddress {
    /// The addresses and key of the naming context; the key is
    /// `NameService` when the URL gives none.
    pub address: CorbalocAddress,
    /// The stringified name after `%XX` decoding; empty when the URL
    /// denotes the naming context itself.
    pub stringified_name: String,
}

/// Parses a `corbaloc:` URL. The scheme name is matched without regard
/// to case.
///
/// # Errors
/// `InvalidUrlScheme` for another scheme; `InvalidCorbalocAddress` for an
/// address that does not follow the grammar; `RirWithOtherProtocol` for
/// `rir` beside another address; `InvalidKeyString` for a bad escape in
/// the key string.
pub fn parse_corbaloc(url: &str) -> IorResult<CorbalocAddress> {
    let payload = strip_scheme(url, "corbaloc:")?;
    let (addr_list, key) = match payload.split_once('/') {
        Some((a, k)) => (a, Some(k)),
        None => (payload, None),
    };
    parse_address(addr_list, key, false)
}

/// Parses a `corbaname:` URL. The scheme name is matched without regard
/// to case.
///
/// # Errors
/// As [`parse_corbaloc`]; `InvalidKeyString` for a bad escape in the
/// stringified name or a name that is not UTF-8 after unescaping.
pub fn parse_corbaname(url: &str) -> IorResult<CorbanameAddress> {
    let payload = strip_scheme(url, "corbaname:")?;
    let (loc, name) = match payload.split_once('#') {
        Some((l, n)) => (l, n),
        None => (payload, ""),
    };
    let (addr_list, key) = match loc.split_once('/') {
        Some((a, k)) => (a, Some(k)),
        None => (loc, None),
    };
    let address = parse_address(addr_list, key, true)?;
    let name = unescape(name, "stringified name")?;
    let stringified_name = String::from_utf8(name)
        .map_err(|_| IorError::InvalidKeyString("stringified name is not UTF-8".into()))?;
    Ok(CorbanameAddress {
        address,
        stringified_name,
    })
}

/// Parses an `<iiop_addr>` (§7.6.10.3): `[<major>.<minor>@]<host>[:<port>]`
/// or the empty string for the local host. Also the address syntax of the
/// IIOP-like protocols other components accept (`ssliop:`).
///
/// # Errors
/// `InvalidCorbalocAddress` for a version, host or port that does not
/// follow the grammar, an IPv6 address outside brackets or one RFC 2373
/// does not allow, port 0.
pub fn parse_iiop_address(s: &str) -> IorResult<IiopAddress> {
    if s.is_empty() {
        return Ok(IiopAddress {
            version: IiopVersion::V1_0,
            host: LOCAL_HOST.into(),
            port: DEFAULT_IIOP_PORT,
        });
    }
    let (version, rest) = match s.split_once('@') {
        Some((v, rest)) => (parse_version(v)?, rest),
        None => (IiopVersion::V1_0, s),
    };
    let (host, port) = if let Some(bracketed) = rest.strip_prefix('[') {
        let (v6, after) = bracketed
            .split_once(']')
            .ok_or_else(|| bad_address(s, "an IPv6 address without `]`"))?;
        if v6.parse::<core::net::Ipv6Addr>().is_err() {
            return Err(bad_address(s, "not an IPv6 address"));
        }
        let port = match after {
            "" => None,
            p => Some(
                p.strip_prefix(':')
                    .ok_or_else(|| bad_address(s, "text after the IPv6 address"))?,
            ),
        };
        (v6, port)
    } else {
        let (host, port) = match rest.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (rest, None),
        };
        if host.is_empty() {
            return Err(bad_address(s, "no host"));
        }
        if !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
        {
            return Err(bad_address(s, "not a host name or IPv4 address"));
        }
        (host, port)
    };
    let port = match port {
        None => DEFAULT_IIOP_PORT,
        Some(p) => parse_port(p).ok_or_else(|| bad_address(s, "port"))?,
    };
    Ok(IiopAddress {
        version,
        host: host.to_string(),
        port,
    })
}

/// The key string of `key` (§7.6.10.1): US-ASCII alphanumerics and the
/// characters the clause lists (`; / : ? @ & = + $ , - _ ! ~ * ' ( )`)
/// stay, every other octet is written `%XX`.
#[must_use]
pub fn escape_key_string(key: &[u8]) -> String {
    const KEEP: &[u8] = b";/:?@&=+$,-_!~*'()";
    let mut out = String::with_capacity(key.len());
    for &b in key {
        if b.is_ascii_alphanumeric() || KEEP.contains(&b) {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(hex_digit(b >> 4));
            out.push(hex_digit(b & 0x0f));
        }
    }
    out
}

fn hex_digit(n: u8) -> char {
    char::from(if n < 10 { b'0' + n } else { b'a' + n - 10 })
}

/// `url` without its scheme `scheme` (which ends in `:`), compared without
/// regard to case.
fn strip_scheme<'a>(url: &'a str, scheme: &str) -> IorResult<&'a str> {
    match url.get(..scheme.len()) {
        Some(s) if s.eq_ignore_ascii_case(scheme) => Ok(&url[scheme.len()..]),
        _ => Err(IorError::InvalidUrlScheme(url.into())),
    }
}

/// The address list and key of a `corbaloc` or `corbaname` URL.
/// `corbaname` makes an empty key `NameService`.
fn parse_address(
    addr_list: &str,
    key: Option<&str>,
    corbaname: bool,
) -> IorResult<CorbalocAddress> {
    let mut endpoints = Vec::new();
    for ep in addr_list.split(',') {
        endpoints.push(parse_endpoint(ep)?);
    }
    let rir = endpoints.iter().any(|e| matches!(e, CorbalocEndpoint::Rir));
    if rir && endpoints.len() > 1 {
        return Err(IorError::RirWithOtherProtocol(addr_list.into()));
    }
    let mut object_key = unescape(key.unwrap_or(""), "key string")?;
    if object_key.is_empty() && (rir || corbaname) {
        object_key = DEFAULT_NAMING_KEY.as_bytes().to_vec();
    }
    Ok(CorbalocAddress {
        endpoints,
        object_key,
    })
}

fn parse_endpoint(s: &str) -> IorResult<CorbalocEndpoint> {
    let (token, address) = s
        .split_once(':')
        .ok_or_else(|| bad_address(s, "no protocol token"))?;
    if token.is_empty() || token.eq_ignore_ascii_case("iiop") {
        return parse_iiop_address(address).map(CorbalocEndpoint::Iiop);
    }
    if token.eq_ignore_ascii_case("rir") {
        if !address.is_empty() {
            return Err(bad_address(s, "rir has no address"));
        }
        return Ok(CorbalocEndpoint::Rir);
    }
    if !token
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'+'))
    {
        return Err(bad_address(s, "protocol token"));
    }
    Ok(CorbalocEndpoint::Other {
        protocol: token.to_string(),
        address: address.to_string(),
    })
}

fn parse_version(s: &str) -> IorResult<IiopVersion> {
    let number = |t: &str| {
        if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        t.parse::<u8>().ok()
    };
    let (a, b) = s.split_once('.').ok_or_else(|| bad_address(s, "version"))?;
    match (number(a), number(b)) {
        (Some(major), Some(minor)) => Ok(IiopVersion::new(major, minor)),
        _ => Err(bad_address(s, "version")),
    }
}

/// A port: decimal digits, 1 to 65535.
fn parse_port(p: &str) -> Option<u16> {
    if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    p.parse::<u16>().ok().filter(|&n| n != 0)
}

fn bad_address(s: &str, what: &str) -> IorError {
    IorError::InvalidCorbalocAddress(alloc::format!("{what}: `{s}`"))
}

/// RFC 2396 unescaping of `%XX`; other characters stay as their UTF-8
/// octets.
fn unescape(s: &str, what: &str) -> IorResult<Vec<u8>> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s
                .get(i + 1..i + 3)
                .filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or_else(|| IorError::InvalidKeyString(alloc::format!("{what}: `{s}`")))?;
            out.push(
                u8::from_str_radix(hex, 16)
                    .map_err(|_| IorError::InvalidKeyString(alloc::format!("{what}: `{s}`")))?,
            );
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn iiop(a: &CorbalocAddress, i: usize) -> &IiopAddress {
        match &a.endpoints[i] {
            CorbalocEndpoint::Iiop(addr) => addr,
            other => panic!("not iiop: {other:?}"),
        }
    }

    #[test]
    fn parse_simple_iiop_corbaloc() {
        let a = parse_corbaloc("corbaloc:iiop:1.2@host.example:1234/MyKey").unwrap();
        assert_eq!(a.endpoints.len(), 1);
        assert_eq!(
            iiop(&a, 0),
            &IiopAddress {
                version: IiopVersion::V1_2,
                host: "host.example".into(),
                port: 1234
            }
        );
        assert_eq!(a.object_key, b"MyKey".to_vec());
    }

    #[test]
    fn default_port_2809_when_missing() {
        let a = parse_corbaloc("corbaloc::host.example/Key").unwrap();
        assert_eq!(iiop(&a, 0).port, 2809);
        assert_eq!(iiop(&a, 0).version, IiopVersion::V1_0);
    }

    #[test]
    fn rir_endpoint() {
        let a = parse_corbaloc("corbaloc:rir:/NameService").unwrap();
        assert_eq!(a.endpoints, alloc::vec![CorbalocEndpoint::Rir]);
        assert!(a.is_rir());
    }

    #[test]
    fn multi_endpoint_failover() {
        let a = parse_corbaloc("corbaloc:iiop:1.2@a:1,iiop:1.0@b:2,:c:3/Key").unwrap();
        assert_eq!(a.endpoints.len(), 3);
        assert_eq!(iiop(&a, 0).host, "a");
        assert_eq!(iiop(&a, 0).port, 1);
        assert_eq!(iiop(&a, 1).version, IiopVersion::V1_0);
        assert_eq!(iiop(&a, 2).host, "c");
        assert_eq!(iiop(&a, 2).port, 3);
    }

    #[test]
    fn percent_escape_in_object_key_is_decoded() {
        let a = parse_corbaloc("corbaloc::host:1/key%00%ff").unwrap();
        assert_eq!(a.object_key, alloc::vec![b'k', b'e', b'y', 0x00, 0xff]);
    }

    #[test]
    fn missing_url_scheme_is_diagnostic() {
        let err = parse_corbaloc("http://x").unwrap_err();
        assert!(matches!(err, IorError::InvalidUrlScheme(_)));
        assert_eq!(err.bad_param_minor(), 7);
    }

    #[test]
    fn corbaname_with_fragment() {
        let n = parse_corbaname("corbaname::host:2809/NameService#PerimeterApp/Trader").unwrap();
        assert_eq!(iiop(&n.address, 0).host, "host");
        assert_eq!(n.address.object_key, b"NameService".to_vec());
        assert_eq!(n.stringified_name, "PerimeterApp/Trader");
    }

    #[test]
    fn corbaname_without_fragment_has_empty_name() {
        let n = parse_corbaname("corbaname::host:2809/NS").unwrap();
        assert!(n.stringified_name.is_empty());
        assert_eq!(n.address.object_key, b"NS".to_vec());
    }

    #[test]
    fn truncated_percent_escape_is_diagnostic() {
        for url in ["corbaloc::host:1/key%a", "corbaloc::host:1/key%zz"] {
            let err = parse_corbaloc(url).unwrap_err();
            assert!(matches!(err, IorError::InvalidKeyString(_)), "{url}");
            assert_eq!(err.bad_param_minor(), 9);
        }
    }

    /// §7.6.10.1: `<key_string>` is optional; without it the key is empty.
    #[test]
    fn corba_p2_7_6_10_corbaloc_without_key_string_has_an_empty_key() {
        for url in ["corbaloc::host:1", "corbaloc::host:1/"] {
            let a = parse_corbaloc(url).unwrap();
            assert!(a.object_key.is_empty(), "{url}");
            assert_eq!(iiop(&a, 0).port, 1);
        }
    }

    /// §7.6.10.1, §7.6.10.5, §7.6.10.6: every example URL of the clause.
    #[test]
    fn corba_p2_7_6_10_every_example_url_of_the_clause_parses() {
        let a = parse_corbaloc("corbaloc::555xyz.com/Prod/TradingService").unwrap();
        assert_eq!(iiop(&a, 0).host, "555xyz.com");
        assert_eq!(iiop(&a, 0).port, 2809);
        assert_eq!(a.object_key, b"Prod/TradingService");

        let a = parse_corbaloc("corbaloc:iiop:1.1@555xyz.com/Prod/TradingService").unwrap();
        assert_eq!(iiop(&a, 0).version, IiopVersion::V1_1);
        assert_eq!(a.object_key, b"Prod/TradingService");

        let a = parse_corbaloc("corbaloc::555xyz.com,:556xyz.com:80/Dev/NameService").unwrap();
        assert_eq!(iiop(&a, 0).host, "555xyz.com");
        assert_eq!(iiop(&a, 0).port, 2809);
        assert_eq!(iiop(&a, 1).host, "556xyz.com");
        assert_eq!(iiop(&a, 1).port, 80);
        assert_eq!(a.object_key, b"Dev/NameService");

        assert_eq!(
            parse_corbaloc("corbaloc:rir:/TradingService")
                .unwrap()
                .object_key,
            b"TradingService"
        );
        assert_eq!(
            parse_corbaloc("corbaloc:rir:/NameService")
                .unwrap()
                .object_key,
            b"NameService"
        );

        let a = parse_corbaloc("corbaloc:iiop:192.168.14.25:555/NameService").unwrap();
        assert_eq!(iiop(&a, 0).host, "192.168.14.25");
        assert_eq!(iiop(&a, 0).port, 555);

        let a = parse_corbaloc("corbaloc::[1080::8:800:200C:417A]:88/DefaultEventChannel").unwrap();
        assert_eq!(iiop(&a, 0).host, "1080::8:800:200C:417A");
        assert_eq!(iiop(&a, 0).port, 88);
        assert_eq!(a.object_key, b"DefaultEventChannel");

        let n = parse_corbaname("corbaname::555objs.com#a/string/path/to/obj").unwrap();
        assert_eq!(iiop(&n.address, 0).host, "555objs.com");
        assert_eq!(n.address.object_key, b"NameService");
        assert_eq!(n.stringified_name, "a/string/path/to/obj");

        let n = parse_corbaname("corbaname:rir:#a/local/obj").unwrap();
        assert!(n.address.is_rir());
        assert_eq!(n.address.object_key, b"NameService");
        assert_eq!(n.stringified_name, "a/local/obj");

        let a = parse_corbaloc("corbaloc:iiop:xyz.com,atm:E.164:358.400.1234567/dev/test/objectX")
            .unwrap();
        assert_eq!(iiop(&a, 0).host, "xyz.com");
        assert_eq!(
            a.endpoints[1],
            CorbalocEndpoint::Other {
                protocol: "atm".into(),
                address: "E.164:358.400.1234567".into()
            }
        );
        assert_eq!(a.object_key, b"dev/test/objectX");
    }

    /// §7.6.10.6: a protocol this ORB does not define is kept with its
    /// address; the IIOP addresses beside it stay usable.
    #[test]
    fn corba_p2_7_6_10_6_a_future_protocol_address_is_kept() {
        let a = parse_corbaloc("corbaloc:dce:cell.example:9,:h:1/k").unwrap();
        assert_eq!(
            a.endpoints[0],
            CorbalocEndpoint::Other {
                protocol: "dce".into(),
                address: "cell.example:9".into()
            }
        );
        assert_eq!(a.iiop_addresses().count(), 1);
    }

    /// §7.6.10.2: an empty key string of a `rir` URL is `NameService`.
    #[test]
    fn corba_p2_7_6_10_2_rir_with_an_empty_key_is_nameservice() {
        for url in ["corbaloc:rir:", "corbaloc:rir:/", "corbaloc:RIR:/"] {
            assert_eq!(
                parse_corbaloc(url).unwrap().object_key,
                b"NameService",
                "{url}"
            );
        }
    }

    /// §7.6.10.3: no host is the local host with port 2809 and IIOP 1.0.
    #[test]
    fn corba_p2_7_6_10_3_an_empty_iiop_address_is_the_local_host() {
        for url in ["corbaloc::/Key", "corbaloc:iiop:/Key", "corbaloc:IIOP:"] {
            let a = parse_corbaloc(url).unwrap();
            assert_eq!(
                iiop(&a, 0),
                &IiopAddress {
                    version: IiopVersion::V1_0,
                    host: LOCAL_HOST.into(),
                    port: 2809
                },
                "{url}"
            );
        }
    }

    /// §7.6.10.3: `"[" IPv6_address "]"` without a port takes 2809; the
    /// brackets are not part of the host.
    #[test]
    fn corba_p2_7_6_10_ipv6_host_without_port_takes_2809() {
        for (url, host) in [
            ("corbaloc::[::1]/key", "::1"),
            ("corbaloc:iiop:1.2@[fe80::1:2]/key", "fe80::1:2"),
            (
                "corbaloc::[::ffff:192.168.14.25]/key",
                "::ffff:192.168.14.25",
            ),
        ] {
            let a = parse_corbaloc(url).unwrap();
            assert_eq!(iiop(&a, 0).host, host, "{url}");
            assert_eq!(iiop(&a, 0).port, 2809, "{url}");
        }
    }

    #[test]
    fn corba_p2_7_6_10_ipv6_host_with_port() {
        let a = parse_corbaloc("corbaloc::[1080::8:800:200C:417A]:88/DefaultEventChannel").unwrap();
        assert_eq!(
            iiop(&a, 0),
            &IiopAddress {
                version: IiopVersion::V1_0,
                host: "1080::8:800:200C:417A".into(),
                port: 88
            }
        );
    }

    /// §7.6.10.2: "The rir protocol cannot be used with any other protocol
    /// in a URL", and it has no address.
    #[test]
    fn corba_p2_7_6_10_rir_with_another_protocol_is_refused() {
        for url in [
            "corbaloc:rir:,:host:1/NameService",
            "corbaloc::host:1,rir:/NameService",
            "corbaloc:rir:,rir:/NameService",
        ] {
            let err = parse_corbaloc(url).unwrap_err();
            assert!(matches!(err, IorError::RirWithOtherProtocol(_)), "{url}");
            assert_eq!(err.bad_param_minor(), 8);
        }
        let err = parse_corbaname("corbaname:rir:,iiop:h#a").unwrap_err();
        assert!(matches!(err, IorError::RirWithOtherProtocol(_)));
        let err = parse_corbaloc("corbaloc:rir:host:1/NameService").unwrap_err();
        assert!(matches!(err, IorError::InvalidCorbalocAddress(_)));
    }

    /// §7.6.10: one case per minor code of the table.
    #[test]
    fn corba_p2_7_6_10_failures_carry_bad_param_minor_7_to_10() {
        let minor = |url: &str| parse_corbaloc(url).unwrap_err().bad_param_minor();
        assert_eq!(minor("corbalocx::h/k"), 7);
        // Text without the `IOR:` scheme where a stringified IOR is read.
        for not_an_ior in ["corbaloc::h/k", "x", "IORX:00"] {
            assert_eq!(
                crate::from_stringified(not_an_ior)
                    .unwrap_err()
                    .bad_param_minor(),
                7,
                "{not_an_ior}"
            );
        }
        assert_eq!(
            crate::from_stringified("ior:0")
                .unwrap_err()
                .bad_param_minor(),
            9
        );
        for bad in [
            "corbaloc::[::1/k",
            "corbaloc::[not-v6]:1/k",
            "corbaloc::[::1]x/k",
            "corbaloc::::1/k",
            "corbaloc::h:/k",
            "corbaloc::h:0/k",
            "corbaloc::h:65536/k",
            "corbaloc::h:+1/k",
            "corbaloc:iiop:1.x@h/k",
            "corbaloc:iiop:1.2@/k",
            "corbaloc::h:1,/k",
            "corbaloc:h/k",
            "corbaloc::h h/k",
        ] {
            assert_eq!(minor(bad), 8, "{bad}");
        }
        assert_eq!(minor("corbaloc::h/%4"), 9);
        assert_eq!(
            parse_corbaname("corbaname::h#a%2")
                .unwrap_err()
                .bad_param_minor(),
            9
        );
        assert_eq!(
            parse_corbaname("corbaname::h#%ff%fe")
                .unwrap_err()
                .bad_param_minor(),
            9
        );
        assert_eq!(
            IorError::Malformed("no profile".into()).bad_param_minor(),
            10
        );
    }

    /// The stringified name of a `corbaname` URL is unescaped.
    #[test]
    fn corba_p2_7_6_10_5_the_stringified_name_is_unescaped() {
        let n = parse_corbaname("corbaname::h#a%2fb.kind/c%5c.d").unwrap();
        assert_eq!(n.stringified_name, "a/b.kind/c\\.d");
    }

    #[test]
    fn a_key_round_trips_through_escape_key_string() {
        let key: Vec<u8> = (0u8..=255).collect();
        let url = alloc::format!("corbaloc::h/{}", escape_key_string(&key));
        assert_eq!(parse_corbaloc(&url).unwrap().object_key, key);
        assert_eq!(
            escape_key_string(b"Prod/Trading Service"),
            "Prod/Trading%20Service"
        );
    }

    /// §7.6.10.1: alphanumerics and the listed characters stay; `.`,
    /// which the list does not name, and every other octet are escaped.
    #[test]
    fn corba_p2_7_6_10_1_only_the_listed_characters_stay_unescaped() {
        let listed = b";/:?@&=+$,-_!~*'()";
        assert_eq!(
            escape_key_string(listed),
            core::str::from_utf8(listed).unwrap()
        );
        assert_eq!(escape_key_string(b"azAZ09"), "azAZ09");
        assert_eq!(escape_key_string(b"a.b"), "a%2eb");
        for b in 0u8..=255 {
            let kept = b.is_ascii_alphanumeric() || listed.contains(&b);
            let escaped = escape_key_string(&[b]);
            assert_eq!(escaped.len() == 1, kept, "{b:#04x}: {escaped}");
        }
    }

    #[test]
    fn parse_iiop_address_reads_the_ssliop_style_address() {
        assert_eq!(
            parse_iiop_address("1.2@b:2").unwrap(),
            IiopAddress {
                version: IiopVersion::V1_2,
                host: "b".into(),
                port: 2
            }
        );
    }
}
