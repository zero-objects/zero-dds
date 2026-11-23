// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The origins a server expects — RFC 6455 §10.2 and §4.2.2 /origin/,
//! RFC 6454.
//!
//! A browser names the origin of the page that opens a connection in the
//! `Origin` header field of the opening handshake (RFC 6455 §4.1 item 8). A
//! server that is meant for certain sites only checks that field against
//! the origins it expects and answers a handshake from any other with HTTP
//! 403 (§10.2). [`OriginPolicy`] is that check: a list of origins in the
//! ASCII serialization of RFC 6454 §6.2 (`scheme://host[:port]`), compared
//! as RFC 6454 §5 says (scheme, host and port; scheme and host without
//! case, the default port of the scheme and no port being the same).
//!
//! A client outside a browser sends any `Origin` or none (§10.1), so the
//! check is no authentication; it keeps the script of a foreign page in a
//! user's browser from using the server with that user's standing.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::handshake::HandshakeError;

/// One origin as a scheme, host and port triple (RFC 6454 §4), lowercase,
/// with the port spelled out where the scheme has a default one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    scheme: String,
    host: String,
    port: Option<u16>,
}

/// The default port of the schemes a WebSocket page or client comes from.
fn default_port(scheme: &str) -> Option<u16> {
    match scheme {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        _ => None,
    }
}

/// Whether `host` is a host as an origin serializes it (RFC 6454 §6.2, RFC
/// 3986 §3.2.2): a name or an IPv4 address of letters, digits, `-`, `.` and
/// `_`, or an IPv6 address in brackets. Any other character is in no host a
/// browser names; `*` among them, which is no pattern for a part of a host.
fn is_host(host: &str) -> bool {
    match host.strip_prefix('[') {
        Some(rest) => rest.strip_suffix(']').is_some_and(|address| {
            !address.is_empty()
                && address
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() || matches!(c, b':' | b'.'))
        }),
        None => {
            !host.is_empty()
                && host
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_'))
        }
    }
}

impl Origin {
    /// Parses the ASCII serialization of an origin (RFC 6454 §6.2):
    /// `scheme "://" host [ ":" port ]`. `None` for anything else: text
    /// with a path, a query, user information, a character outside ASCII
    /// (a host with such characters is written in its `xn--` form) or a
    /// character that is in no host, such as `*`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (scheme, rest) = text.split_once("://")?;
        let mut scheme_chars = scheme.bytes();
        if !scheme_chars.next()?.is_ascii_alphabetic()
            || !scheme_chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
        {
            return None;
        }
        if rest
            .bytes()
            .any(|c| !c.is_ascii_graphic() || matches!(c, b'/' | b'?' | b'#' | b'@' | b'\\'))
        {
            return None;
        }
        // An IPv6 host is in brackets, which hold its colons (RFC 3986
        // §3.2.2).
        let (host, port) = if rest.starts_with('[') {
            let end = rest.find(']')?;
            let (host, after) = rest.split_at(end + 1);
            match after.strip_prefix(':') {
                Some(port) => (host, Some(port)),
                None if after.is_empty() => (host, None),
                None => return None,
            }
        } else {
            match rest.split_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (rest, None),
            }
        };
        if !is_host(host) {
            return None;
        }
        let scheme = scheme.to_ascii_lowercase();
        let port = match port {
            Some(digits) => {
                if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                Some(digits.parse::<u16>().ok()?)
            }
            None => default_port(&scheme),
        };
        Some(Self {
            host: host.to_ascii_lowercase(),
            scheme,
            port,
        })
    }
}

/// One entry of the list of expected origins.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expected {
    /// `*`: every origin.
    Any,
    /// `null`: the origin a browser names for a page without one (RFC 6454
    /// §6.2 and §7.1: a sandboxed frame, a `file:` page).
    Null,
    /// This origin.
    Exact(Origin),
}

/// A setting of the origin check that cannot be served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginPolicyError {
    /// An entry of the list is not `*`, `null` or `scheme://host[:port]`;
    /// an entry with a pattern in its host (`https://*.example.org`) is
    /// among them.
    InvalidEntry(String),
    /// The `Origin` field is required and no origin is expected: every
    /// origin would pass. `*` says so in the list.
    RequiredWithoutList,
}

impl core::fmt::Display for OriginPolicyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidEntry(entry) => write!(
                f,
                "got `{entry}`, expected `*`, `null` or an origin `scheme://host[:port]` \
                 in ASCII, without a path; `*` is an entry of its own and no pattern \
                 inside a host"
            ),
            Self::RequiredWithoutList => f.write_str(
                "the Origin field is required and no origin is expected; \
                 name the origins, or `*` for any",
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OriginPolicyError {}

/// RFC 6455 §10.2 — the origins a server expects. The default expects
/// nothing and accepts every opening handshake, with or without `Origin`
/// (§4.2.2 /origin/: "If the server does not validate the origin, it will
/// accept connections from anywhere").
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OriginPolicy {
    expected: Vec<Expected>,
    required: bool,
}

impl OriginPolicy {
    /// The policy of a list of expected origins. Each entry is an origin
    /// in its ASCII serialization (`https://app.example.org`,
    /// `http://localhost:3000`), `null`, or `*` for every origin. An empty
    /// list is no check. `*` is an entry of its own: an entry with `*` in
    /// its host (`https://*.example.org`) is refused, each origin is named.
    ///
    /// `required` says what a handshake without `Origin` gets: `false`
    /// accepts it (a client outside a browser, which may leave the field
    /// out, §4.1 item 8), `true` refuses it.
    ///
    /// # Errors
    /// [`OriginPolicyError`] for an entry that is no origin, and for
    /// `required` with an empty list.
    pub fn new<S: AsRef<str>>(expected: &[S], required: bool) -> Result<Self, OriginPolicyError> {
        let mut list = Vec::with_capacity(expected.len());
        for entry in expected {
            let entry = entry.as_ref().trim();
            list.push(match entry {
                "*" => Expected::Any,
                "null" => Expected::Null,
                other => Expected::Exact(
                    Origin::parse(other)
                        .ok_or_else(|| OriginPolicyError::InvalidEntry(other.to_string()))?,
                ),
            });
        }
        if required && list.is_empty() {
            return Err(OriginPolicyError::RequiredWithoutList);
        }
        Ok(Self {
            expected: list,
            required,
        })
    }

    /// Whether the policy checks anything.
    #[must_use]
    pub fn is_checking(&self) -> bool {
        !self.expected.is_empty()
    }

    fn expects(&self, origin: &str) -> bool {
        let parsed = Origin::parse(origin);
        self.expected.iter().any(|entry| match entry {
            Expected::Any => true,
            Expected::Null => origin == "null",
            Expected::Exact(expected) => parsed.as_ref() == Some(expected),
        })
    }

    /// Checks the `Origin` of an opening handshake
    /// ([`ClientHandshake::origin`](crate::handshake::ClientHandshake::origin)).
    /// The field may list several origins separated by spaces (RFC 6454
    /// §7.1); every one of them has to be expected.
    ///
    /// # Errors
    /// [`HandshakeError::OriginNotExpected`] for an origin outside the
    /// list, [`HandshakeError::MissingOrigin`] for a handshake without the
    /// field when it is required. Both are answered with HTTP 403
    /// ([`HandshakeError::render_error_response`]).
    pub fn check(&self, origin: Option<&str>) -> Result<(), HandshakeError> {
        if !self.is_checking() {
            return Ok(());
        }
        let Some(origin) = origin else {
            return if self.required {
                Err(HandshakeError::MissingOrigin)
            } else {
                Ok(())
            };
        };
        let mut listed = origin.split(' ').filter(|o| !o.is_empty()).peekable();
        // A field that is there and names nothing is no expected origin.
        if listed.peek().is_none() || !listed.all(|o| self.expects(o)) {
            return Err(HandshakeError::OriginNotExpected(origin.to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn policy(expected: &[&str], required: bool) -> OriginPolicy {
        OriginPolicy::new(expected, required).expect("a valid list")
    }

    #[test]
    fn ws_10_2_an_origin_is_scheme_host_and_port() {
        let o = Origin::parse("HTTPS://App.Example.ORG").expect("an origin");
        assert_eq!(o.scheme, "https");
        assert_eq!(o.host, "app.example.org");
        assert_eq!(o.port, Some(443));
        assert_eq!(
            Origin::parse("http://localhost:3000")
                .expect("an origin")
                .port,
            Some(3000)
        );
        let v6 = Origin::parse("http://[::1]:8080").expect("an origin");
        assert_eq!((v6.host.as_str(), v6.port), ("[::1]", Some(8080)));
        assert_eq!(
            Origin::parse("http://[::1]").expect("an origin").port,
            Some(80)
        );
        // A scheme without a default port keeps "no port" apart from a port.
        assert_eq!(Origin::parse("app://bundle").expect("an origin").port, None);
    }

    #[test]
    fn ws_10_2_text_that_is_no_serialized_origin_is_refused() {
        for text in [
            "",
            "example.org",
            "https://",
            "https://example.org/",
            "https://example.org/path",
            "https://example.org?x=1",
            "https://user@example.org",
            "https://example.org:",
            "https://example.org:port",
            "https://example.org:70000",
            "https://example.org:443:443",
            "https://exa mple.org",
            "https://bücher.example",
            "1http://example.org",
            "https://[::1",
            "https://[::1]x",
            "https://[]",
            "https://a]b",
            // `*` is no pattern for a part of a host, and no other
            // character outside letters, digits, `-`, `.` and `_` is in one.
            "https://*.example.org",
            "https://*",
            "https://app.*.example.org",
            "https://*:443",
            "https://exa%6dple.org",
            "https://example.org,https://other.example",
            "https://<example.org>",
            "https://[::g]",
            "https://[*]",
        ] {
            assert_eq!(Origin::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn ws_10_2_an_expected_origin_passes_and_any_other_is_refused() {
        let p = policy(&["https://app.example.org", "http://localhost:3000"], false);
        assert_eq!(p.check(Some("https://app.example.org")), Ok(()));
        assert_eq!(p.check(Some("http://localhost:3000")), Ok(()));
        for other in [
            "https://evil.example",
            // Another scheme, another port, a host that only starts or
            // ends like the expected one.
            "http://app.example.org",
            "https://app.example.org:8443",
            "http://localhost",
            "https://app.example.org.evil.example",
            "https://evilapp.example.org",
            "null",
            "not an origin",
        ] {
            assert_eq!(
                p.check(Some(other)),
                Err(HandshakeError::OriginNotExpected(other.to_string())),
                "{other:?}"
            );
        }
    }

    #[test]
    fn ws_10_2_scheme_and_host_compare_without_case_and_the_default_port_is_no_port() {
        let p = policy(
            &["https://App.Example.org:443", "http://intra.example"],
            false,
        );
        assert_eq!(p.check(Some("https://app.example.org")), Ok(()));
        assert_eq!(p.check(Some("HTTPS://APP.EXAMPLE.ORG")), Ok(()));
        assert_eq!(p.check(Some("https://app.example.org:443")), Ok(()));
        assert_eq!(p.check(Some("http://intra.example:80")), Ok(()));
        assert!(p.check(Some("http://intra.example:8080")).is_err());
    }

    #[test]
    fn ws_10_2_a_handshake_without_origin_passes_unless_the_field_is_required() {
        let lenient = policy(&["https://app.example.org"], false);
        assert_eq!(lenient.check(None), Ok(()));
        let strict = policy(&["https://app.example.org"], true);
        assert_eq!(strict.check(None), Err(HandshakeError::MissingOrigin));
        assert_eq!(strict.check(Some("https://app.example.org")), Ok(()));
        // The field is there and empty: no origin, and not "no field".
        assert!(lenient.check(Some("")).is_err());
    }

    #[test]
    fn ws_10_2_without_a_list_nothing_is_checked() {
        let p = OriginPolicy::default();
        assert!(!p.is_checking());
        assert_eq!(p.check(None), Ok(()));
        assert_eq!(p.check(Some("https://anything.example")), Ok(()));
        assert_eq!(p.check(Some("not an origin")), Ok(()));
        assert_eq!(policy(&[], false), p);
    }

    #[test]
    fn ws_10_2_the_star_expects_every_origin_and_null_is_its_own_entry() {
        let any = policy(&["*"], true);
        assert_eq!(any.check(Some("https://anything.example")), Ok(()));
        assert_eq!(any.check(Some("null")), Ok(()));
        assert_eq!(any.check(None), Err(HandshakeError::MissingOrigin));
        let null = policy(&["null"], false);
        assert_eq!(null.check(Some("null")), Ok(()));
        assert!(null.check(Some("https://anything.example")).is_err());
    }

    #[test]
    fn ws_10_2_every_origin_of_a_list_in_the_field_has_to_be_expected() {
        let p = policy(&["https://a.example", "https://b.example"], false);
        assert_eq!(p.check(Some("https://a.example https://b.example")), Ok(()));
        assert!(
            p.check(Some("https://a.example https://evil.example"))
                .is_err()
        );
    }

    #[test]
    fn ws_10_2_a_list_entry_that_is_no_origin_and_required_without_a_list_are_refused() {
        assert_eq!(
            OriginPolicy::new(&["https://app.example.org/"], false),
            Err(OriginPolicyError::InvalidEntry(
                "https://app.example.org/".to_string()
            ))
        );
        assert_eq!(
            OriginPolicy::new(&["app.example.org"], false),
            Err(OriginPolicyError::InvalidEntry(
                "app.example.org".to_string()
            ))
        );
        assert_eq!(
            OriginPolicy::new::<&str>(&[], true),
            Err(OriginPolicyError::RequiredWithoutList)
        );
    }

    #[test]
    fn ws_10_2_a_list_entry_with_a_pattern_in_its_host_is_refused() {
        // `*` stands for every origin as an entry of its own. No browser
        // names an origin with `*` in its host, so an entry with one would
        // refuse every handshake it was written for: it is an error.
        for entry in [
            "https://*.example.org",
            "https://app.*.example.org",
            "https://*",
            "*.example.org",
            "**",
        ] {
            let err = OriginPolicy::new(&[entry], false).expect_err(entry);
            assert_eq!(err, OriginPolicyError::InvalidEntry(entry.to_string()));
            assert!(err.to_string().contains("no pattern inside a host"));
        }
        // The names a browser does send pass: a host with `_` or `-`, an
        // IPv4 and an IPv6 address.
        let p = policy(
            &[
                "https://my_app-1.example.org",
                "http://192.0.2.7:8080",
                "http://[2001:db8::1]",
            ],
            false,
        );
        assert_eq!(p.check(Some("https://my_app-1.example.org")), Ok(()));
        assert_eq!(p.check(Some("http://192.0.2.7:8080")), Ok(()));
        assert_eq!(p.check(Some("http://[2001:DB8::1]:80")), Ok(()));
    }
}
