// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP URI-Scheme-Parser nach RFC 7252 §6.
//!
//! Spec: `coap-URI = "coap:" "//" host [ ":" port ] path-abempty [ "?" query ]`,
//! `coaps-URI = "coaps:" ...`. Default-Ports: 5683 (coap) / 5684 (coaps).

use alloc::string::String;
use alloc::vec::Vec;

/// Parsed CoAP URI nach RFC 7252 §6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoapUri {
    /// `true` wenn `coaps://` (DTLS).
    pub secure: bool,
    /// Host (DNS-Name oder IP-Literal).
    pub host: String,
    /// Port (default 5683 / 5684).
    pub port: u16,
    /// Path-Segmente (RFC 7252 §6.4 — werden zu `Uri-Path`-Options).
    pub path_segments: Vec<String>,
    /// Query-Parameter (RFC 7252 §6.4 — `Uri-Query`-Options).
    pub query_params: Vec<String>,
}

/// URI-Parser-Errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriError {
    /// Scheme ist nicht `coap://` oder `coaps://`.
    InvalidScheme,
    /// Host fehlt.
    MissingHost,
    /// Port nicht parseable als u16.
    InvalidPort,
    /// Spec §6.1: Fragment ist nicht erlaubt.
    FragmentNotAllowed,
}

impl core::fmt::Display for UriError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidScheme => write!(f, "InvalidScheme"),
            Self::MissingHost => write!(f, "MissingHost"),
            Self::InvalidPort => write!(f, "InvalidPort"),
            Self::FragmentNotAllowed => write!(f, "FragmentNotAllowed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UriError {}

/// Parsed eine `coap://` oder `coaps://` URI nach RFC 7252 §6.
///
/// # Errors
/// Siehe [`UriError`].
pub fn parse_coap_uri(input: &str) -> Result<CoapUri, UriError> {
    let (secure, rest) = if let Some(r) = input.strip_prefix("coap://") {
        (false, r)
    } else if let Some(r) = input.strip_prefix("coaps://") {
        (true, r)
    } else {
        return Err(UriError::InvalidScheme);
    };

    if rest.contains('#') {
        return Err(UriError::FragmentNotAllowed);
    }

    let (authority, path_query) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };

    if authority.is_empty() {
        return Err(UriError::MissingHost);
    }

    let (host, port) = if let Some(colon) = authority.rfind(':') {
        let host_part = &authority[..colon];
        let port_str = &authority[colon + 1..];
        let port_num: u16 = port_str.parse().map_err(|_| UriError::InvalidPort)?;
        if host_part.is_empty() {
            return Err(UriError::MissingHost);
        }
        (host_part.to_string(), port_num)
    } else {
        (authority.to_string(), default_port(secure))
    };

    let (path_part, query_part) = match path_query.find('?') {
        Some(q) => (&path_query[..q], &path_query[q + 1..]),
        None => (path_query, ""),
    };

    let path_segments: Vec<String> = if path_part.is_empty() {
        Vec::new()
    } else {
        path_part
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    };
    let query_params: Vec<String> = if query_part.is_empty() {
        Vec::new()
    } else {
        query_part
            .split('&')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    };

    Ok(CoapUri {
        secure,
        host,
        port,
        path_segments,
        query_params,
    })
}

/// Default-Port nach Scheme.
#[must_use]
pub const fn default_port(secure: bool) -> u16 {
    if secure { 5684 } else { 5683 }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_coap_uri() {
        let u = parse_coap_uri("coap://example.com/.well-known/core").expect("ok");
        assert!(!u.secure);
        assert_eq!(u.host, "example.com");
        assert_eq!(u.port, 5683);
        assert_eq!(
            u.path_segments,
            vec![".well-known".to_string(), "core".into()]
        );
    }

    #[test]
    fn parses_coaps_default_port() {
        let u = parse_coap_uri("coaps://example.com/foo").expect("ok");
        assert!(u.secure);
        assert_eq!(u.port, 5684);
    }

    #[test]
    fn parses_explicit_port() {
        let u = parse_coap_uri("coap://example.com:7777/foo").expect("ok");
        assert_eq!(u.port, 7777);
    }

    #[test]
    fn parses_query_params() {
        let u = parse_coap_uri("coap://e.com/foo?a=1&b=2").expect("ok");
        assert_eq!(u.query_params, vec!["a=1".to_string(), "b=2".into()]);
    }

    #[test]
    fn parses_empty_path_when_no_slash() {
        let u = parse_coap_uri("coap://e.com").expect("ok");
        assert!(u.path_segments.is_empty());
    }

    #[test]
    fn rejects_unknown_scheme() {
        assert_eq!(parse_coap_uri("http://e.com"), Err(UriError::InvalidScheme));
    }

    #[test]
    fn rejects_missing_host() {
        assert_eq!(parse_coap_uri("coap://"), Err(UriError::MissingHost));
    }

    #[test]
    fn rejects_invalid_port() {
        assert_eq!(
            parse_coap_uri("coap://e.com:abc/"),
            Err(UriError::InvalidPort)
        );
    }

    #[test]
    fn rejects_fragment() {
        assert_eq!(
            parse_coap_uri("coap://e.com/#frag"),
            Err(UriError::FragmentNotAllowed)
        );
    }

    #[test]
    fn default_port_returns_5684_for_coaps() {
        assert_eq!(default_port(true), 5684);
        assert_eq!(default_port(false), 5683);
    }

    #[test]
    fn round_trip_path_filtering_strips_empty_segments() {
        let u = parse_coap_uri("coap://e.com//foo//bar/").expect("ok");
        assert_eq!(u.path_segments, vec!["foo".to_string(), "bar".into()]);
    }
}
