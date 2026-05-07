// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `corbaloc:` und `corbaname:` URL-Parser — Spec §13.6.10.
//!
//! ## `corbaloc:`
//! ```text
//! corbaloc:[iiop]:1.2@host:port/<object_key_str>
//! corbaloc:rir:/<object_key_str>           // referenziert Default-ORB
//! corbaloc::host:port/<object_key_str>     // protocol implicit IIOP
//! ```
//!
//! `<object_key_str>` ist eine ASCII-Form des Object-Keys mit
//! `%XX`-Encoding fuer non-printable.
//!
//! ## `corbaname:`
//! ```text
//! corbaname:[iiop]:1.2@host:port/<object_key>#<stringified_name>
//! ```
//!
//! Erweitert `corbaloc:` um eine NamingContext-Lookup-Spezifikation
//! im Fragment-Teil.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_corba_iiop::IiopVersion;

use crate::error::{IorError, IorResult};

/// Eine `corbaloc:`-Adresse — Liste alternativer Endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbalocAddress {
    /// Protokoll-Endpoints (typisch IIOP).
    pub endpoints: Vec<CorbalocEndpoint>,
    /// Object-Key in seiner Roh-Octet-Form (nach `%XX`-Decoding).
    pub object_key: Vec<u8>,
}

/// Ein Endpoint innerhalb einer `corbaloc:`-Adresse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbalocEndpoint {
    /// Protokoll-Identifier (`iiop`/`rir`/Vendor).
    pub protocol: String,
    /// IIOP-Version (default 1.0 wenn nicht angegeben).
    pub iiop_version: IiopVersion,
    /// Host (leer bei `rir:`).
    pub host: String,
    /// Port (Default 2809 fuer IIOP wenn nicht angegeben).
    pub port: u16,
}

/// Eine `corbaname:`-Adresse — `corbaloc:`-Adresse plus
/// Naming-Context-Pfad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorbanameAddress {
    /// Endpoint + Object-Key des NamingContext.
    pub address: CorbalocAddress,
    /// Stringified-Name (Naming-Service-Pfad).
    pub stringified_name: String,
}

/// Parsiert eine `corbaloc:`-URL.
///
/// # Errors
/// `InvalidUrlScheme`, `InvalidCorbalocAddress`, oder `InvalidHexChar`
/// im Object-Key-Decoding.
pub fn parse_corbaloc(url: &str) -> IorResult<CorbalocAddress> {
    let payload = url
        .strip_prefix("corbaloc:")
        .ok_or_else(|| IorError::InvalidUrlScheme(url.into()))?;
    parse_corbaloc_payload(payload)
}

/// Parsiert eine `corbaname:`-URL.
///
/// # Errors
/// Wie `parse_corbaloc` plus Fragment-Validierung.
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
    // Split address-list und object_key am ersten unescaped `/`.
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
    // optionale `<major>.<minor>@` IIOP-Version.
    let (iiop_version, rest) = match rest.find('@') {
        Some(i) => {
            let v = &rest[..i];
            let host_port = &rest[i + 1..];
            (parse_version(v)?, host_port)
        }
        None => (IiopVersion::V1_0, rest),
    };
    // host:port — Default-Port 2809 wenn ohne Port (Spec §13.6.10.1).
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
    // `%XX`-Hex-Decoding fuer non-printable bytes; printable bytes
    // bleiben as-is.
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
