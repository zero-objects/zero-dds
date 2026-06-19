// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `corbaloc:` and `corbaname:` URL parsers — spec §13.6.10.
//!
//! ## `corbaloc:`
//! ```text
//! corbaloc:[iiop]:1.2@host:port/<object_key_str>
//! corbaloc:rir:/<object_key_str>           // references the default ORB
//! corbaloc::host:port/<object_key_str>     // protocol implicit IIOP
//! ```
//!
//! `<object_key_str>` is an ASCII form of the object key with
//! `%XX` encoding for non-printable bytes.
//!
//! ## `corbaname:`
//! ```text
//! corbaname:[iiop]:1.2@host:port/<object_key>#<stringified_name>
//! ```
//!
//! Extends `corbaloc:` with a NamingContext lookup specification
//! in the fragment part.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_iiop::IiopVersion;

use crate::error::{IorError, IorResult};

/// A `corbaloc:` address — list of alternative endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbalocAddress {
    /// Protocol endpoints (typically IIOP).
    pub endpoints: Vec<CorbalocEndpoint>,
    /// Object key in its raw octet form (after `%XX` decoding).
    pub object_key: Vec<u8>,
}

/// An endpoint within a `corbaloc:` address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbalocEndpoint {
    /// Protocol identifier (`iiop`/`rir`/vendor).
    pub protocol: String,
    /// IIOP version (defaults to 1.0 if not given).
    pub iiop_version: IiopVersion,
    /// Host (empty for `rir:`).
    pub host: String,
    /// Port (defaults to 2809 for IIOP if not given).
    pub port: u16,
}

/// A `corbaname:` address — `corbaloc:` address plus
/// naming-context path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbanameAddress {
    /// Endpoint + object key of the NamingContext.
    pub address: CorbalocAddress,
    /// Stringified name (naming-service path).
    pub stringified_name: String,
}

/// Parses a `corbaloc:` URL.
///
/// # Errors
/// `InvalidUrlScheme`, `InvalidCorbalocAddress`, or `InvalidHexChar`
/// in object-key decoding.
pub fn parse_corbaloc(url: &str) -> IorResult<CorbalocAddress> {
    let payload = url
        .strip_prefix("corbaloc:")
        .ok_or_else(|| IorError::InvalidUrlScheme(url.into()))?;
    parse_corbaloc_payload(payload)
}

/// Parses a `corbaname:` URL.
///
/// # Errors
/// Same as `parse_corbaloc` plus fragment validation.
pub fn parse_corbaname(url: &str) -> IorResult<CorbanameAddress> {
    let payload = url
        .strip_prefix("corbaname:")
        .ok_or_else(|| IorError::InvalidUrlScheme(url.into()))?;
    let (addr_part, name_part) = match payload.split_once('#') {
        Some((a, n)) => (a, n.to_string()),
        None => (payload, String::new()),
    };
    let address = parse_corbaloc_payload(addr_part)?;
    Ok(CorbanameAddress {
        address,
        stringified_name: name_part,
    })
}

fn parse_corbaloc_payload(payload: &str) -> IorResult<CorbalocAddress> {
    // Split the address list and object_key at the first unescaped `/`.
    let (addr_list, key_part) = match payload.split_once('/') {
        Some((a, k)) => (a, k),
        None => {
            return Err(IorError::InvalidCorbalocAddress(
                "missing object_key separator '/'".into(),
            ));
        }
    };
    let mut endpoints = Vec::new();
    for ep in addr_list.split(',') {
        endpoints.push(parse_endpoint(ep)?);
    }
    let object_key = decode_object_key(key_part)?;
    Ok(CorbalocAddress {
        endpoints,
        object_key,
    })
}

fn parse_endpoint(s: &str) -> IorResult<CorbalocEndpoint> {
    // Form: [<protocol>]:<options>@<host>:<port>
    // Spec §13.6.10.1: leading `:` = default protocol (iiop).
    let s = s.trim();
    if s.starts_with("rir:") {
        return Ok(CorbalocEndpoint {
            protocol: "rir".into(),
            iiop_version: IiopVersion::V1_0,
            host: String::new(),
            port: 0,
        });
    }
    let (protocol, rest) = match s.find(':') {
        Some(0) => ("iiop".to_string(), &s[1..]),
        Some(i) => (s[..i].to_string(), &s[i + 1..]),
        None => return Err(IorError::InvalidCorbalocAddress(s.into())),
    };
    // optional `<major>.<minor>@` IIOP version.
    let (iiop_version, rest) = match rest.find('@') {
        Some(i) => {
            let v = &rest[..i];
            let host_port = &rest[i + 1..];
            (parse_version(v)?, host_port)
        }
        None => (IiopVersion::V1_0, rest),
    };
    // host:port — default port 2809 when no port is given (spec §13.6.10.1).
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| IorError::InvalidCorbalocAddress(rest.into()))?,
        ),
        _ => (rest.to_string(), 2809),
    };
    Ok(CorbalocEndpoint {
        protocol,
        iiop_version,
        host,
        port,
    })
}

fn parse_version(s: &str) -> IorResult<IiopVersion> {
    let (a, b) = s
        .split_once('.')
        .ok_or_else(|| IorError::InvalidCorbalocAddress(s.into()))?;
    let major = a
        .parse::<u8>()
        .map_err(|_| IorError::InvalidCorbalocAddress(s.into()))?;
    let minor = b
        .parse::<u8>()
        .map_err(|_| IorError::InvalidCorbalocAddress(s.into()))?;
    Ok(IiopVersion::new(major, minor))
}

fn decode_object_key(s: &str) -> IorResult<Vec<u8>> {
    // `%XX` hex decoding for non-printable bytes; printable bytes
    // stay as-is.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(IorError::InvalidCorbalocAddress(
                    "truncated %-escape in object_key".into(),
                ));
            }
            let h = hex_value(bytes[i + 1] as char)?;
            let l = hex_value(bytes[i + 2] as char)?;
            out.push((h << 4) | l);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

fn hex_value(c: char) -> IorResult<u8> {
    match c {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        'A'..='F' => Ok(c as u8 - b'A' + 10),
        other => Err(IorError::InvalidHexChar(other)),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_iiop_corbaloc() {
        let url = "corbaloc:iiop:1.2@host.example:1234/MyKey";
        let a = parse_corbaloc(url).unwrap();
        assert_eq!(a.endpoints.len(), 1);
        assert_eq!(a.endpoints[0].protocol, "iiop");
        assert_eq!(a.endpoints[0].iiop_version, IiopVersion::V1_2);
        assert_eq!(a.endpoints[0].host, "host.example");
        assert_eq!(a.endpoints[0].port, 1234);
        assert_eq!(a.object_key, b"MyKey".to_vec());
    }

    #[test]
    fn default_port_2809_when_missing() {
        let a = parse_corbaloc("corbaloc::host.example/Key").unwrap();
        assert_eq!(a.endpoints[0].port, 2809);
        assert_eq!(a.endpoints[0].iiop_version, IiopVersion::V1_0);
    }

    #[test]
    fn rir_endpoint() {
        let a = parse_corbaloc("corbaloc:rir:/NameService").unwrap();
        assert_eq!(a.endpoints[0].protocol, "rir");
    }

    #[test]
    fn multi_endpoint_failover() {
        let url = "corbaloc:iiop:1.2@a:1,iiop:1.0@b:2,:c:3/Key";
        let a = parse_corbaloc(url).unwrap();
        assert_eq!(a.endpoints.len(), 3);
        assert_eq!(a.endpoints[0].host, "a");
        assert_eq!(a.endpoints[0].port, 1);
        assert_eq!(a.endpoints[1].iiop_version, IiopVersion::V1_0);
        assert_eq!(a.endpoints[2].protocol, "iiop"); // ":host:port" -> default
        assert_eq!(a.endpoints[2].port, 3);
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
    }

    #[test]
    fn missing_slash_is_diagnostic() {
        let err = parse_corbaloc("corbaloc::host:1").unwrap_err();
        assert!(matches!(err, IorError::InvalidCorbalocAddress(_)));
    }

    #[test]
    fn corbaname_with_fragment() {
        let url = "corbaname::host:2809/NameService#PerimeterApp/Trader";
        let n = parse_corbaname(url).unwrap();
        assert_eq!(n.address.endpoints[0].host, "host");
        assert_eq!(n.address.object_key, b"NameService".to_vec());
        assert_eq!(n.stringified_name, "PerimeterApp/Trader");
    }

    #[test]
    fn corbaname_without_fragment_has_empty_name() {
        let n = parse_corbaname("corbaname::host:2809/NS").unwrap();
        assert!(n.stringified_name.is_empty());
    }

    #[test]
    fn truncated_percent_escape_is_diagnostic() {
        let err = parse_corbaloc("corbaloc::host:1/key%a").unwrap_err();
        assert!(matches!(err, IorError::InvalidCorbalocAddress(_)));
    }
}
