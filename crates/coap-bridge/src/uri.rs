// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP URI scheme parser per RFC 7252 §6.
//!
//! Spec: `coap-URI = "coap:" "//" host [ ":" port ] path-abempty [ "?" query ]`,
//! `coaps-URI = "coaps:" ...`. Default ports: 5683 (coap) / 5684 (coaps).

use alloc::string::String;
use alloc::vec::Vec;

/// Parsed CoAP URI per RFC 7252 §6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoapUri {
    /// `true` if `coaps://` (DTLS).
    pub secure: bool,
    /// Host: an IP address without brackets, or a registered name,
    /// percent-decoded and in lowercase (RFC 3986 §3.2.2, §6.2.2.1).
    pub host: String,
    /// Port (default 5683 / 5684).
    pub port: u16,
    /// Path segments, percent-decoded (RFC 7252 §6.4 step 8 — each one a
    /// `Uri-Path` option, an empty one included).
    pub path_segments: Vec<String>,
    /// Query arguments, percent-decoded (§6.4 step 9 — `Uri-Query`
    /// options).
    pub query_params: Vec<String>,
}

/// URI parser errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriError {
    /// Scheme is neither `coap://` nor `coaps://`.
    InvalidScheme,
    /// Host is missing.
    MissingHost,
    /// Port not parseable as u16.
    InvalidPort,
    /// Spec §6.1: fragment is not allowed.
    FragmentNotAllowed,
    /// A host the grammar of §6.1 does not allow: user information, an
    /// unclosed or invalid IP literal.
    InvalidHost,
    /// A percent-encoding that is not `%` and two hex digits, or one that
    /// does not decode to UTF-8 (§6.4 steps 5, 8, 9).
    InvalidEncoding,
}

impl core::fmt::Display for UriError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidScheme => write!(f, "InvalidScheme"),
            Self::MissingHost => write!(f, "MissingHost"),
            Self::InvalidPort => write!(f, "InvalidPort"),
            Self::FragmentNotAllowed => write!(f, "FragmentNotAllowed"),
            Self::InvalidHost => write!(f, "InvalidHost"),
            Self::InvalidEncoding => write!(f, "InvalidEncoding"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UriError {}

/// Parses a `coap://` or `coaps://` URI and decomposes it as RFC 7252 §6.4
/// does: the host without the brackets of an IP literal and
/// percent-decoded, the default port of the scheme when none is given, one
/// path segment per `Uri-Path` option (none for an empty path or "/") and
/// one argument per `Uri-Query` option, each percent-decoded. The scheme
/// and a registered host name are case-insensitive (RFC 3986 §3.1,
/// §3.2.2).
///
/// # Errors
/// See [`UriError`].
pub fn parse_coap_uri(input: &str) -> Result<CoapUri, UriError> {
    let (secure, rest) = if has_prefix_ignore_case(input, "coap://") {
        (false, &input[7..])
    } else if has_prefix_ignore_case(input, "coaps://") {
        (true, &input[8..])
    } else {
        return Err(UriError::InvalidScheme);
    };

    if rest.contains('#') {
        return Err(UriError::FragmentNotAllowed);
    }

    let authority_end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, path_query) = rest.split_at(authority_end);
    if authority.is_empty() {
        return Err(UriError::MissingHost);
    }
    if authority.contains('@') {
        return Err(UriError::InvalidHost);
    }

    let (host, port_text) = if let Some(literal) = authority.strip_prefix('[') {
        let close = literal.find(']').ok_or(UriError::InvalidHost)?;
        let address = &literal[..close];
        if address.parse::<core::net::Ipv6Addr>().is_err() {
            return Err(UriError::InvalidHost);
        }
        let after = &literal[close + 1..];
        let port = match after.strip_prefix(':') {
            Some(p) => Some(p),
            None if after.is_empty() => None,
            None => return Err(UriError::InvalidHost),
        };
        (address.to_ascii_lowercase(), port)
    } else {
        let (name, port) = match authority.rfind(':') {
            Some(colon) => (&authority[..colon], Some(&authority[colon + 1..])),
            None => (authority, None),
        };
        if name.contains('[') || name.contains(']') || name.contains(':') {
            return Err(UriError::InvalidHost);
        }
        (percent_decode(name)?.to_lowercase(), port)
    };
    if host.is_empty() {
        return Err(UriError::MissingHost);
    }
    let port = match port_text {
        // RFC 3986 §3.2.3: an empty port is the scheme's default.
        None | Some("") => default_port(secure),
        Some(p) if p.bytes().all(|b| b.is_ascii_digit()) => {
            p.parse().map_err(|_| UriError::InvalidPort)?
        }
        Some(_) => return Err(UriError::InvalidPort),
    };

    let (path, query) = match path_query.find('?') {
        Some(q) => (&path_query[..q], Some(&path_query[q + 1..])),
        None => (path_query, None),
    };
    // §6.4 step 8: "If |url| has a path component that is not "" or "/",
    // then for each segment, include a Uri-Path option".
    let path_segments = if path.is_empty() || path == "/" {
        Vec::new()
    } else {
        path[1..]
            .split('/')
            .map(percent_decode)
            .collect::<Result<Vec<_>, _>>()?
    };
    // §6.4 step 9: "for each argument in the query component".
    let query_params = match query {
        None => Vec::new(),
        Some(q) => q
            .split('&')
            .map(percent_decode)
            .collect::<Result<Vec<_>, _>>()?,
    };

    Ok(CoapUri {
        secure,
        host,
        port,
        path_segments,
        query_params,
    })
}

impl CoapUri {
    /// The URI again (RFC 7252 §6.5): the host in brackets when it is an
    /// IPv6 address, the port only when it is not the default, every path
    /// segment and query argument percent-encoded where RFC 3986 asks for
    /// it. Two URIs that name the same resource give the same string.
    #[must_use]
    pub fn to_uri(&self) -> String {
        let mut out = String::from(if self.secure { "coaps://" } else { "coap://" });
        if self.host.contains(':') {
            out.push('[');
            out.push_str(&self.host);
            out.push(']');
        } else {
            out.push_str(&percent_encode(&self.host, b""));
        }
        if self.port != default_port(self.secure) {
            out.push(':');
            out.push_str(&alloc::format!("{}", self.port));
        }
        for segment in &self.path_segments {
            out.push('/');
            out.push_str(&percent_encode(segment, b":@"));
        }
        for (i, arg) in self.query_params.iter().enumerate() {
            out.push(if i == 0 { '?' } else { '&' });
            out.push_str(&percent_encode(arg, b":@/?"));
        }
        out
    }
}

/// `true` when `text` starts with `prefix`, ASCII case ignored.
fn has_prefix_ignore_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Decodes the percent-encodings of `text` (RFC 3986 §2.1); the result
/// must be UTF-8 (the `string` format of the Uri-* options).
///
/// # Errors
/// [`UriError::InvalidEncoding`].
pub fn percent_decode(text: &str) -> Result<String, UriError> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3).ok_or(UriError::InvalidEncoding)?;
            let digit = |b: u8| (b as char).to_digit(16).ok_or(UriError::InvalidEncoding);
            out.push((digit(hex[0])? * 16 + digit(hex[1])?) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| UriError::InvalidEncoding)
}

/// Percent-encodes every byte of `text` outside the unreserved and
/// sub-delims sets of RFC 3986 and `extra`, with uppercase hex digits
/// (RFC 7252 §6.5 steps 7 and 8).
#[must_use]
pub fn percent_encode(text: &str, extra: &[u8]) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        let keep =
            b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=".contains(&b) || extra.contains(&b);
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&alloc::format!("%{b:02X}"));
        }
    }
    out
}

/// Default port per scheme.
#[must_use]
pub const fn default_port(secure: bool) -> u16 {
    if secure { 5684 } else { 5683 }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn coap_6_1_parses_basic_coap_uri() {
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
    fn coap_6_1_parses_coaps_default_port() {
        let u = parse_coap_uri("coaps://example.com/foo").expect("ok");
        assert!(u.secure);
        assert_eq!(u.port, 5684);
    }

    #[test]
    fn coap_6_1_parses_explicit_port() {
        let u = parse_coap_uri("coap://example.com:7777/foo").expect("ok");
        assert_eq!(u.port, 7777);
    }

    #[test]
    fn coap_6_1_parses_query_params() {
        let u = parse_coap_uri("coap://e.com/foo?a=1&b=2").expect("ok");
        assert_eq!(u.query_params, vec!["a=1".to_string(), "b=2".into()]);
    }

    #[test]
    fn coap_6_1_parses_empty_path_when_no_slash() {
        let u = parse_coap_uri("coap://e.com").expect("ok");
        assert!(u.path_segments.is_empty());
    }

    #[test]
    fn coap_6_1_rejects_unknown_scheme() {
        assert_eq!(parse_coap_uri("http://e.com"), Err(UriError::InvalidScheme));
    }

    #[test]
    fn coap_6_1_rejects_missing_host() {
        assert_eq!(parse_coap_uri("coap://"), Err(UriError::MissingHost));
    }

    #[test]
    fn coap_6_1_rejects_invalid_port() {
        assert_eq!(
            parse_coap_uri("coap://e.com:abc/"),
            Err(UriError::InvalidPort)
        );
    }

    #[test]
    fn coap_6_1_rejects_fragment() {
        assert_eq!(
            parse_coap_uri("coap://e.com/#frag"),
            Err(UriError::FragmentNotAllowed)
        );
    }

    #[test]
    fn coap_1_1_default_port_returns_5684_for_coaps() {
        assert_eq!(default_port(true), 5684);
        assert_eq!(default_port(false), 5683);
    }

    #[test]
    fn coap_6_4_an_empty_segment_is_a_uri_path_option() {
        // RFC 7252 §6.4 step 8: every segment of a path other than "" and
        // "/" is a Uri-Path option, an empty one too.
        let u = parse_coap_uri("coap://e.com//foo//bar/").expect("ok");
        let want: Vec<String> = ["", "foo", "", "bar", ""]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(u.path_segments, want);
        assert!(
            parse_coap_uri("coap://e.com/")
                .expect("ok")
                .path_segments
                .is_empty()
        );
    }

    #[test]
    fn coap_6_4_the_segments_and_arguments_are_percent_decoded() {
        // §6.4 steps 8 and 9: "after converting each percent-encoding".
        let u = parse_coap_uri("coap://e.com/a%2Fb/%C3%A4?x%26y=1&z").expect("ok");
        assert_eq!(u.path_segments, vec!["a/b".to_string(), "ä".into()]);
        assert_eq!(u.query_params, vec!["x&y=1".to_string(), "z".into()]);
        assert_eq!(
            parse_coap_uri("coap://e.com/%zz"),
            Err(UriError::InvalidEncoding)
        );
        assert_eq!(
            parse_coap_uri("coap://e.com/%FF"),
            Err(UriError::InvalidEncoding)
        );
    }

    #[test]
    fn coap_6_1_an_ipv6_literal_is_in_brackets() {
        // RFC 7252 §6.1 with RFC 3986 §3.2.2: host = IP-literal / ...
        let u = parse_coap_uri("coap://[2001:db8::1]:61616/s").expect("ok");
        assert_eq!(u.host, "2001:db8::1");
        assert_eq!(u.port, 61616);
        let d = parse_coap_uri("coaps://[::1]/").expect("ok");
        assert_eq!((d.host.as_str(), d.port), ("::1", 5684));
        assert_eq!(
            parse_coap_uri("coap://[2001:db8::1/s"),
            Err(UriError::InvalidHost)
        );
        assert_eq!(
            parse_coap_uri("coap://2001:db8::1/s"),
            Err(UriError::InvalidHost)
        );
        assert_eq!(
            parse_coap_uri("coap://user@e.com/"),
            Err(UriError::InvalidHost)
        );
    }

    #[test]
    fn coap_6_3_scheme_and_host_ignore_case_and_the_default_port_is_implied() {
        // §6.3: "coap://EXAMPLE.com:5683/~sensors/temp.xml" and
        // "coap://example.com/%7Esensors/temp.xml" are equivalent.
        let a = parse_coap_uri("CoAP://EXAMPLE.com:5683/~sensors/temp.xml").expect("ok");
        let b = parse_coap_uri("coap://example.com/%7Esensors/temp.xml").expect("ok");
        assert_eq!(a, b);
        assert_eq!(a.to_uri(), "coap://example.com/~sensors/temp.xml");
        assert_eq!(
            parse_coap_uri("coap://[::1]:7/a%2Fb?q=%20")
                .expect("ok")
                .to_uri(),
            "coap://[::1]:7/a%2Fb?q=%20"
        );
    }
}
