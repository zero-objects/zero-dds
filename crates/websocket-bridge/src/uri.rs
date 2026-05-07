// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket URI-Scheme-Parser nach RFC 6455 §3.
//!
//! Spec: `ws-URI = "ws:" "//" host [ ":" port ] path [ "?" query ]`,
//! `wss-URI = "wss:" "//" host [ ":" port ] path [ "?" query ]`.
//! Default-Ports: 80 (ws), 443 (wss).
//! Fragment ist explizit verboten in §3.

use alloc::string::String;

/// Parsed WebSocket URI nach RFC 6455 §3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketUri {
    /// `true` wenn `wss://` (TLS).
    pub secure: bool,
    /// Host (IP oder DNS-Name; nicht percent-decoded).
    pub host: String,
    /// Port; default 80 (ws) oder 443 (wss).
    pub port: u16,
    /// Path; default "/".
    pub resource_name: String,
    /// Optional Query-String (ohne fuehrendes "?").
    pub query: Option<String>,
}

/// URI-Parser-Errors nach RFC 6455 §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriError {
    /// Scheme ist nicht `ws://` oder `wss://`.
    InvalidScheme,
    /// Host fehlt.
    MissingHost,
    /// Port ist nicht parseable als u16.
    InvalidPort,
    /// Spec §3: "fragment identifier MUST NOT be used".
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

/// Parsed eine `ws://` oder `wss://` URI nach RFC 6455 §3.
///
/// # Errors
/// Siehe [`UriError`].
pub fn parse_websocket_uri(input: &str) -> Result<WebSocketUri, UriError> {
    let (secure, rest) = if let Some(r) = input.strip_prefix("ws://") {
        (false, r)
    } else if let Some(r) = input.strip_prefix("wss://") {
        (true, r)
    } else {
        return Err(UriError::InvalidScheme);
    };

    if rest.contains('#') {
        return Err(UriError::FragmentNotAllowed);
    }

    // Aufteilen Authority vs Path/Query.
    let (authority, path_query) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    if authority.is_empty() {
        return Err(UriError::MissingHost);
    }

    // Authority: host[:port]. Note: IPv6-Literale [::1] werden hier
    // nicht unterstuetzt (RFC 6874 — Caller-Layer); wir akzeptieren
    // Hostnames + IPv4-Literale.
    let (host, port) = if let Some(colon) = authority.rfind(':') {
        let host_part = &authority[..colon];
        let port_str = &authority[colon + 1..];
        let port_num: u16 = port_str.parse().map_err(|_| UriError::InvalidPort)?;
        if host_part.is_empty() {
            return Err(UriError::MissingHost);
        }
        (host_part.to_string(), port_num)
    } else {
        (authority.to_string(), if secure { 443 } else { 80 })
    };

    // Path/Query splitten.
    let (path, query) = match path_query.find('?') {
        Some(q) => (
            path_query[..q].to_string(),
            Some(path_query[q + 1..].to_string()),
        ),
        None => (path_query.to_string(), None),
    };

    Ok(WebSocketUri {
        secure,
        host,
        port,
        resource_name: path,
        query,
    })
}

/// Default-Port nach Scheme.
#[must_use]
pub fn default_port(secure: bool) -> u16 {
    if secure { 443 } else { 80 }
}

/// Sammelt die `Resource-Name`-Form nach Spec (`<path> [ "?" <query> ]`).
#[must_use]
pub fn resource_name(uri: &WebSocketUri) -> String {
    match &uri.query {
        Some(q) => {
            let mut s = uri.resource_name.clone();
            s.push('?');
            s.push_str(q);
            s
        }
        None => uri.resource_name.clone(),
    }
}

/// `true` wenn `host` als bekannter Local-Loopback-Wert akzeptiert
/// werden darf (Same-Origin-Policy-Hint fuer Browser-Pfad).
#[must_use]
pub fn is_local_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_ws_uri() {
        let u = parse_websocket_uri("ws://example.com/chat").expect("ok");
        assert!(!u.secure);
        assert_eq!(u.host, "example.com");
        assert_eq!(u.port, 80);
        assert_eq!(u.resource_name, "/chat");
        assert!(u.query.is_none());
    }

    #[test]
    fn parses_basic_wss_uri() {
        let u = parse_websocket_uri("wss://example.com/").expect("ok");
        assert!(u.secure);
        assert_eq!(u.port, 443);
    }

    #[test]
    fn parses_explicit_port() {
        let u = parse_websocket_uri("ws://example.com:8080/foo").expect("ok");
        assert_eq!(u.port, 8080);
    }

    #[test]
    fn parses_query_string() {
        let u = parse_websocket_uri("wss://e.com:443/p?token=abc").expect("ok");
        assert_eq!(u.query.as_deref(), Some("token=abc"));
        assert_eq!(u.resource_name, "/p");
    }

    #[test]
    fn parses_default_path_when_missing() {
        let u = parse_websocket_uri("ws://e.com").expect("ok");
        assert_eq!(u.resource_name, "/");
    }

    #[test]
    fn rejects_unknown_scheme() {
        assert_eq!(
            parse_websocket_uri("http://e.com"),
            Err(UriError::InvalidScheme)
        );
    }

    #[test]
    fn rejects_missing_host() {
        assert_eq!(parse_websocket_uri("ws://"), Err(UriError::MissingHost));
    }

    #[test]
    fn rejects_missing_host_before_port() {
        assert_eq!(
            parse_websocket_uri("ws://:8080/"),
            Err(UriError::MissingHost)
        );
    }

    #[test]
    fn rejects_invalid_port() {
        assert_eq!(
            parse_websocket_uri("ws://e.com:abc/"),
            Err(UriError::InvalidPort)
        );
    }

    #[test]
    fn rejects_fragment() {
        assert_eq!(
            parse_websocket_uri("ws://e.com/#anchor"),
            Err(UriError::FragmentNotAllowed)
        );
    }

    #[test]
    fn default_port_returns_443_for_wss() {
        assert_eq!(default_port(true), 443);
        assert_eq!(default_port(false), 80);
    }

    #[test]
    fn resource_name_combines_path_and_query() {
        let u = WebSocketUri {
            secure: false,
            host: "e.com".into(),
            port: 80,
            resource_name: "/foo".into(),
            query: Some("a=1".into()),
        };
        assert_eq!(resource_name(&u), "/foo?a=1");
    }

    #[test]
    fn resource_name_without_query_is_path() {
        let u = WebSocketUri {
            secure: false,
            host: "e.com".into(),
            port: 80,
            resource_name: "/foo".into(),
            query: None,
        };
        assert_eq!(resource_name(&u), "/foo");
    }

    #[test]
    fn is_local_loopback_recognizes_localhost() {
        assert!(is_local_loopback("localhost"));
        assert!(is_local_loopback("127.0.0.1"));
        assert!(is_local_loopback("::1"));
        assert!(!is_local_loopback("example.com"));
    }
}
