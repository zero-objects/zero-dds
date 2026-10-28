// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket opening handshake — RFC 6455 §4.
//!
//! Spec §4.1 (client) + §4.2 (server): HTTP/1.1 upgrade sequence.
//!
//! Client → Server:
//! ```text
//! GET /chat HTTP/1.1
//! Host: server.example.com
//! Upgrade: websocket
//! Connection: Upgrade
//! Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
//! Sec-WebSocket-Version: 13
//! ```
//!
//! Server → Client:
//! ```text
//! HTTP/1.1 101 Switching Protocols
//! Upgrade: websocket
//! Connection: Upgrade
//! Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
//! ```
//!
//! `Sec-WebSocket-Accept` = base64(SHA1(Sec-WebSocket-Key || GUID)).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// RFC 6455 §1.3 — magic GUID for the accept hash.
pub const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// RFC 6455 §4.1 — spec value for `Sec-WebSocket-Version`.
pub const WEBSOCKET_VERSION: &str = "13";

/// Handshake error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeError {
    /// HTTP header parsing failed.
    MalformedRequest,
    /// `Sec-WebSocket-Key` is missing.
    MissingKey,
    /// `Upgrade: websocket` is missing.
    NotWebSocketUpgrade,
    /// `Connection: Upgrade` is missing.
    NotUpgradeConnection,
    /// `Sec-WebSocket-Version` != 13.
    UnsupportedVersion(String),
    /// The server response had an unexpected status code.
    UnexpectedStatus(u16),
    /// `Sec-WebSocket-Accept` does not match the expected hash.
    AcceptMismatch,
    /// The server selected no subprotocol, where the client requires one.
    NoProtocolSelected,
    /// The server selected a subprotocol the client did not offer
    /// (RFC 6455 §4.1, client step 6).
    ProtocolNotOffered(String),
    /// The server agreed to an extension the client did not offer
    /// (RFC 6455 §4.1, client step 5).
    ExtensionNotOffered(String),
}

impl core::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MalformedRequest => f.write_str("malformed handshake request"),
            Self::MissingKey => f.write_str("missing Sec-WebSocket-Key"),
            Self::NotWebSocketUpgrade => f.write_str("Upgrade header is not websocket"),
            Self::NotUpgradeConnection => f.write_str("Connection header is not Upgrade"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported version: {v}"),
            Self::UnexpectedStatus(s) => write!(f, "unexpected status: {s}"),
            Self::AcceptMismatch => f.write_str("Sec-WebSocket-Accept mismatch"),
            Self::NoProtocolSelected => f.write_str("server selected no subprotocol"),
            Self::ProtocolNotOffered(p) => write!(f, "server selected subprotocol {p} not offered"),
            Self::ExtensionNotOffered(e) => write!(f, "server agreed to extension {e} not offered"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HandshakeError {}

/// Parsed client request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientHandshake {
    /// Request path (e.g. `/chat`).
    pub path: String,
    /// Host header.
    pub host: String,
    /// Sec-WebSocket-Key (base64).
    pub key: String,
    /// Optional `Sec-WebSocket-Protocol`.
    pub protocols: Vec<String>,
    /// Optional `Sec-WebSocket-Extensions`.
    pub extensions: Vec<String>,
}

/// Server response builder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerHandshake {
    /// Status code (always 101 for success).
    pub status: u16,
    /// `Sec-WebSocket-Accept` hash.
    pub accept: String,
    /// Optional `Sec-WebSocket-Protocol` (one of those offered by the client).
    pub protocol: Option<String>,
    /// Optional `Sec-WebSocket-Extensions`.
    pub extensions: Vec<String>,
}

/// Computes `Sec-WebSocket-Accept` from the client key. RFC 6455 §4.2.2.
#[must_use]
pub fn compute_accept(client_key: &str) -> String {
    let mut concatenated = String::with_capacity(client_key.len() + WEBSOCKET_GUID.len());
    concatenated.push_str(client_key.trim());
    concatenated.push_str(WEBSOCKET_GUID);
    let digest = sha1(concatenated.as_bytes());
    base64_encode(&digest)
}

/// Parses a client-handshake HTTP request.
///
/// # Errors
/// See [`HandshakeError`].
pub fn parse_client_request(input: &str) -> Result<ClientHandshake, HandshakeError> {
    let mut lines = input.split("\r\n");
    let request_line = lines.next().ok_or(HandshakeError::MalformedRequest)?;
    let mut req_parts = request_line.split_whitespace();
    let _method = req_parts.next().ok_or(HandshakeError::MalformedRequest)?;
    let path = req_parts
        .next()
        .ok_or(HandshakeError::MalformedRequest)?
        .to_string();

    let mut hs = ClientHandshake {
        path,
        ..Default::default()
    };
    let mut upgrade_ok = false;
    let mut connection_ok = false;
    let mut version_seen = false;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or(HandshakeError::MalformedRequest)?;
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        match k.as_str() {
            "host" => hs.host = v.to_string(),
            "upgrade" => upgrade_ok = v.eq_ignore_ascii_case("websocket"),
            "connection" => {
                connection_ok = v
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("upgrade"));
            }
            "sec-websocket-key" => hs.key = v.to_string(),
            "sec-websocket-version" => {
                version_seen = true;
                if v != WEBSOCKET_VERSION {
                    return Err(HandshakeError::UnsupportedVersion(v.to_string()));
                }
            }
            "sec-websocket-protocol" => {
                hs.protocols
                    .extend(v.split(',').map(|s| s.trim().to_string()));
            }
            "sec-websocket-extensions" => {
                hs.extensions
                    .extend(v.split(',').map(|s| s.trim().to_string()));
            }
            _ => {}
        }
    }
    if !upgrade_ok {
        return Err(HandshakeError::NotWebSocketUpgrade);
    }
    if !connection_ok {
        return Err(HandshakeError::NotUpgradeConnection);
    }
    if hs.key.is_empty() {
        return Err(HandshakeError::MissingKey);
    }
    if !version_seen {
        return Err(HandshakeError::UnsupportedVersion(String::new()));
    }
    Ok(hs)
}

/// Produces the server response bytes. RFC 6455 §4.2.2.
///
/// The response agrees to a subprotocol or an extension only if this crate
/// can actually perform it, which today means neither.
///
/// It used to echo the client's offer back: `protocol` was
/// `req.protocols.first()` and `extensions` was `req.extensions.clone()`.
/// Both are what §4.2.2 forbids — item 8 allows only a subprotocol "the
/// server has selected" from those it supports, item 9 only extensions the
/// server has "agreed" to use — and echoing turns every offer into an
/// agreement the frame layer never made:
///
/// * **No extension is implemented in a frame path.**
///   `permessage_deflate` supplies §7.1 parameter parsing and the §7.2.2
///   tail helpers, and neither [`codec::encode`](crate::codec::encode) nor
///   [`codec::decode`](crate::codec::decode) consults them; `rsv1` is
///   carried through untouched and the crate has no DEFLATE dependency at
///   all. A `Sec-WebSocket-Extensions: permessage-deflate` in the response
///   therefore promised compression that never arrived, and a client that
///   believed it would fail on the first message.
/// * **The echo was not even well-formed.** RFC 7692 §7.1.2.1 lets a client
///   offer a bare `client_max_window_bits`; §7.1.2.2 requires a *value* in
///   the server's reply. Echoing the offer verbatim reproduced the bare
///   form, and Python's `websockets` rejects the handshake with
///   `NegotiationError: Unsupported extension` — which is how this was
///   found. `tests/websockets_py_interop_e2e.rs` holds that measurement;
///   no client written from this source tree had contradicted it, because
///   `tests/cross_vendor.rs` offers no extensions at all.
/// * **No subprotocol is defined.** The bridge carries DDS samples over
///   `ws_path`, not over a subprotocol token, so accepting whatever the
///   client happened to list first claimed a protocol this server does not
///   speak.
///
/// A caller that does support one selects it itself —
/// [`negotiation::select_subprotocol`](crate::negotiation::select_subprotocol)
/// against its own preference list, and
/// [`permessage_deflate::parse_offer`](crate::permessage_deflate::parse_offer)
/// plus [`render_accept`](crate::permessage_deflate::render_accept) for the
/// §7.1.2.2 reply form — and writes the result into the returned
/// [`ServerHandshake`] before rendering it.
#[must_use]
pub fn build_server_response(req: &ClientHandshake) -> ServerHandshake {
    ServerHandshake {
        status: 101,
        accept: compute_accept(&req.key),
        // §4.2.2 item 8 — a subprotocol this server does not support must
        // not be named, and omitting the header is how "none" is said.
        protocol: None,
        // §4.2.2 item 9 — likewise for extensions, and none of them reaches
        // the frame path.
        extensions: Vec::new(),
    }
}

/// Render to HTTP/1.1-101-Response-Bytes.
#[must_use]
pub fn render_server_response(resp: &ServerHandshake) -> String {
    let mut out = alloc::format!(
        "HTTP/1.1 {} Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n",
        resp.status,
        resp.accept
    );
    if let Some(p) = &resp.protocol {
        out.push_str(&alloc::format!("Sec-WebSocket-Protocol: {p}\r\n"));
    }
    if !resp.extensions.is_empty() {
        out.push_str(&alloc::format!(
            "Sec-WebSocket-Extensions: {}\r\n",
            resp.extensions.join(", ")
        ));
    }
    out.push_str("\r\n");
    out
}

/// The `Sec-WebSocket-Key` of a client: base64 of a 16-byte nonce, which
/// RFC 6455 §4.1 requires to be randomly selected for each connection. The
/// caller supplies the nonce from a strong source.
#[must_use]
pub fn client_key(nonce: &[u8; 16]) -> String {
    base64_encode(nonce)
}

/// The client's opening handshake request, RFC 6455 §4.1. `host` is the
/// `Host` header value (with `:port` when it is not the scheme's default),
/// `path` the resource name, `protocols` the subprotocols offered in order
/// of preference. No extension is offered.
#[must_use]
pub fn render_client_request(host: &str, path: &str, key: &str, protocols: &[&str]) -> String {
    let mut out = alloc::format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: {WEBSOCKET_VERSION}\r\n"
    );
    if !protocols.is_empty() {
        out.push_str(&alloc::format!(
            "Sec-WebSocket-Protocol: {}\r\n",
            protocols.join(", ")
        ));
    }
    out.push_str("\r\n");
    out
}

/// Validates the server's handshake response on the client side, RFC 6455
/// §4.1 steps 1 to 6: status 101, `Upgrade: websocket`, `Connection:
/// Upgrade`, `Sec-WebSocket-Accept` for `key`, no extension (the client
/// offered none), and a subprotocol only out of `offered`. With `require`
/// set, the server must have selected one. Returns the selected
/// subprotocol. `response` is the header block up to and including the
/// empty line.
///
/// # Errors
/// See [`HandshakeError`].
pub fn validate_server_response(
    response: &str,
    key: &str,
    offered: &[&str],
    require: bool,
) -> Result<Option<String>, HandshakeError> {
    let mut lines = response.split("\r\n");
    let status_line = lines.next().ok_or(HandshakeError::MalformedRequest)?;
    let mut parts = status_line.split_whitespace();
    let version = parts.next().ok_or(HandshakeError::MalformedRequest)?;
    if !version.starts_with("HTTP/1.") {
        return Err(HandshakeError::MalformedRequest);
    }
    let status: u16 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or(HandshakeError::MalformedRequest)?;
    if status != 101 {
        return Err(HandshakeError::UnexpectedStatus(status));
    }
    let mut upgrade_ok = false;
    let mut connection_ok = false;
    let mut accept: Option<String> = None;
    let mut protocol: Option<String> = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or(HandshakeError::MalformedRequest)?;
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "upgrade" => upgrade_ok = v.eq_ignore_ascii_case("websocket"),
            "connection" => {
                connection_ok = v
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("upgrade"));
            }
            "sec-websocket-accept" => accept = Some(v.to_string()),
            "sec-websocket-protocol" => protocol = Some(v.to_string()),
            "sec-websocket-extensions" if !v.is_empty() => {
                return Err(HandshakeError::ExtensionNotOffered(v.to_string()));
            }
            _ => {}
        }
    }
    if !upgrade_ok {
        return Err(HandshakeError::NotWebSocketUpgrade);
    }
    if !connection_ok {
        return Err(HandshakeError::NotUpgradeConnection);
    }
    if accept.as_deref() != Some(compute_accept(key).as_str()) {
        return Err(HandshakeError::AcceptMismatch);
    }
    match protocol {
        Some(p) if !offered.contains(&p.as_str()) => Err(HandshakeError::ProtocolNotOffered(p)),
        None if require => Err(HandshakeError::NoProtocolSelected),
        other => Ok(other),
    }
}

// ============================================================================
// Minimal SHA-1 + Base64 (no external deps for no_std).
// ============================================================================

/// Minimal SHA-1 over `bytes`. Spec FIPS-180-4 §6.1.
fn sha1(bytes: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let bit_len = (bytes.len() as u64) * 8;
    let mut msg = Vec::with_capacity(bytes.len() + 64);
    msg.extend_from_slice(bytes);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wv) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wv);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, w) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&w.to_be_bytes());
    }
    out
}

/// Base64 encode per RFC 4648 (standard alphabet).
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for c in &mut chunks {
        let v = (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]);
        out.push(ALPHA[((v >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((v >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((v >> 6) & 0x3f) as usize] as char);
        out.push(ALPHA[(v & 0x3f) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let v = u32::from(rem[0]) << 16;
            out.push(ALPHA[((v >> 18) & 0x3f) as usize] as char);
            out.push(ALPHA[((v >> 12) & 0x3f) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let v = (u32::from(rem[0]) << 16) | (u32::from(rem[1]) << 8);
            out.push(ALPHA[((v >> 18) & 0x3f) as usize] as char);
            out.push(ALPHA[((v >> 12) & 0x3f) as usize] as char);
            out.push(ALPHA[((v >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn rfc6455_section_1_3_accept_test_vector() {
        // Spec §1.3 — "the sample nonce" → expected accept value.
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
        assert_eq!(compute_accept(key), expected);
    }

    /// The client side of §4.1 against the §1.3 sample: the request carries
    /// the key and the offered subprotocol, the sample response validates,
    /// and each deviation step 1 to 6 names is refused.
    #[test]
    fn the_client_side_validates_the_server_response() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let req = render_client_request("server.example.com", "/mqtt", key, &["mqtt"]);
        let parsed = parse_client_request(&req).unwrap();
        assert_eq!(parsed.path, "/mqtt");
        assert_eq!(parsed.key, key);
        assert_eq!(parsed.protocols, alloc::vec!["mqtt".to_string()]);

        let ok = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\
                  Connection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\
                  Sec-WebSocket-Protocol: mqtt\r\n\r\n";
        assert_eq!(
            validate_server_response(ok, key, &["mqtt"], true),
            Ok(Some("mqtt".to_string()))
        );
        let cases: [(&str, HandshakeError); 6] = [
            (
                "HTTP/1.1 400 Bad Request\r\n\r\n",
                HandshakeError::UnexpectedStatus(400),
            ),
            (
                "HTTP/1.1 101 x\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
                HandshakeError::NotWebSocketUpgrade,
            ),
            (
                "HTTP/1.1 101 x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: AAAA\r\nSec-WebSocket-Protocol: mqtt\r\n\r\n",
                HandshakeError::AcceptMismatch,
            ),
            (
                "HTTP/1.1 101 x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
                HandshakeError::NoProtocolSelected,
            ),
            (
                "HTTP/1.1 101 x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nSec-WebSocket-Protocol: chat\r\n\r\n",
                HandshakeError::ProtocolNotOffered("chat".to_string()),
            ),
            (
                "HTTP/1.1 101 x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nSec-WebSocket-Extensions: permessage-deflate\r\nSec-WebSocket-Protocol: mqtt\r\n\r\n",
                HandshakeError::ExtensionNotOffered("permessage-deflate".to_string()),
            ),
        ];
        for (resp, want) in cases {
            assert_eq!(
                validate_server_response(resp, key, &["mqtt"], true),
                Err(want)
            );
        }
        assert_eq!(client_key(&[0u8; 16]), "AAAAAAAAAAAAAAAAAAAAAA==");
    }

    #[test]
    fn parses_minimal_client_handshake() {
        let req = "GET /chat HTTP/1.1\r\n\
                   Host: server.example.com\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   \r\n";
        let h = parse_client_request(req).unwrap();
        assert_eq!(h.path, "/chat");
        assert_eq!(h.host, "server.example.com");
        assert_eq!(h.key, "dGhlIHNhbXBsZSBub25jZQ==");
    }

    #[test]
    fn parses_protocols_and_extensions() {
        let req = "GET / HTTP/1.1\r\n\
                   Host: x\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Key: a\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   Sec-WebSocket-Protocol: chat, superchat\r\n\
                   Sec-WebSocket-Extensions: permessage-deflate\r\n\
                   \r\n";
        let h = parse_client_request(req).unwrap();
        assert_eq!(
            h.protocols,
            alloc::vec!["chat".to_string(), "superchat".into()]
        );
        assert_eq!(h.extensions, alloc::vec!["permessage-deflate".to_string()]);
    }

    #[test]
    fn rejects_missing_upgrade() {
        let req = "GET / HTTP/1.1\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Key: a\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   \r\n";
        assert_eq!(
            parse_client_request(req),
            Err(HandshakeError::NotWebSocketUpgrade)
        );
    }

    #[test]
    fn rejects_wrong_version() {
        let req = "GET / HTTP/1.1\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Key: a\r\n\
                   Sec-WebSocket-Version: 8\r\n\
                   \r\n";
        assert!(matches!(
            parse_client_request(req),
            Err(HandshakeError::UnsupportedVersion(_))
        ));
    }

    #[test]
    fn rejects_missing_key() {
        let req = "GET / HTTP/1.1\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   \r\n";
        assert_eq!(parse_client_request(req), Err(HandshakeError::MissingKey));
    }

    #[test]
    fn server_response_includes_accept() {
        let req = ClientHandshake {
            key: "dGhlIHNhbXBsZSBub25jZQ==".into(),
            ..Default::default()
        };
        let resp = build_server_response(&req);
        assert_eq!(resp.status, 101);
        assert_eq!(resp.accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    /// §4.2.2 item 9 — an extension the frame path cannot perform must not
    /// be agreed to. The bare `client_max_window_bits` this offer carries is
    /// legal in a client offer (RFC 7692 §7.1.2.1) and illegal in a server
    /// reply (§7.1.2.2), so echoing it produced a response Python's
    /// `websockets` rejects outright.
    #[test]
    fn server_agrees_to_no_extension_it_cannot_perform() {
        let req = ClientHandshake {
            key: "dGhlIHNhbXBsZSBub25jZQ==".into(),
            extensions: alloc::vec!["permessage-deflate; client_max_window_bits".to_string()],
            ..Default::default()
        };
        let resp = build_server_response(&req);
        assert!(
            resp.extensions.is_empty(),
            "no extension reaches codec::encode/decode, so none may be agreed: {:?}",
            resp.extensions
        );
        let rendered = render_server_response(&resp);
        assert!(
            !rendered.contains("Sec-WebSocket-Extensions"),
            "an omitted agreement must omit the header: {rendered}"
        );
    }

    /// §4.2.2 item 8 — the bridge defines no subprotocol, so it must select
    /// none rather than accept whatever the client listed first.
    #[test]
    fn server_selects_no_subprotocol_it_does_not_support() {
        let req = ClientHandshake {
            key: "dGhlIHNhbXBsZSBub25jZQ==".into(),
            protocols: alloc::vec!["chat".to_string(), "superchat".to_string()],
            ..Default::default()
        };
        let resp = build_server_response(&req);
        assert_eq!(
            resp.protocol, None,
            "an unsupported subprotocol must not be echoed back as selected"
        );
        let rendered = render_server_response(&resp);
        assert!(
            !rendered.contains("Sec-WebSocket-Protocol"),
            "an omitted selection must omit the header: {rendered}"
        );
    }

    #[test]
    fn render_server_response_format() {
        let resp = ServerHandshake {
            status: 101,
            accept: "abc".into(),
            protocol: Some("chat".into()),
            extensions: alloc::vec![],
        };
        let s = render_server_response(&resp);
        assert!(s.contains("HTTP/1.1 101"));
        assert!(s.contains("Upgrade: websocket"));
        assert!(s.contains("Sec-WebSocket-Accept: abc"));
        assert!(s.contains("Sec-WebSocket-Protocol: chat"));
    }

    #[test]
    fn base64_round_trip_known_vectors() {
        // RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn sha1_known_vector_abc() {
        // FIPS-180-4 Appendix A.1.
        let h = sha1(b"abc");
        let expected: [u8; 20] = [
            0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50,
            0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
        ];
        assert_eq!(h, expected);
    }

    #[test]
    fn connection_header_with_keep_alive_still_detects_upgrade() {
        // Spec §4.1: Connection MUST contain the `Upgrade` token (case-
        // insensitive). Browsers often send `Connection: keep-alive,
        // Upgrade`.
        let req = "GET / HTTP/1.1\r\n\
                   Host: x\r\n\
                   Upgrade: WebSocket\r\n\
                   Connection: keep-alive, Upgrade\r\n\
                   Sec-WebSocket-Key: a\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   \r\n";
        assert!(parse_client_request(req).is_ok());
    }
}
