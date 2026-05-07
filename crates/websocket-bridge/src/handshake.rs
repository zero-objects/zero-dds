// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket Opening-Handshake — RFC 6455 §4.
//!
//! Spec §4.1 (Client) + §4.2 (Server): HTTP/1.1-Upgrade-Sequence.
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

/// RFC 6455 §1.3 — Magic-GUID fuer den Accept-Hash.
pub const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// RFC 6455 §4.1 — Spec-Wert fuer `Sec-WebSocket-Version`.
pub const WEBSOCKET_VERSION: &str = "13";

/// Handshake-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeError {
    /// HTTP-Header parsing fehlgeschlagen.
    MalformedRequest,
    /// `Sec-WebSocket-Key` fehlt.
    MissingKey,
    /// `Upgrade: websocket` fehlt.
    NotWebSocketUpgrade,
    /// `Connection: Upgrade` fehlt.
    NotUpgradeConnection,
    /// `Sec-WebSocket-Version` != 13.
    UnsupportedVersion(String),
    /// Server-Response hatte unerwarteten Status-Code.
    UnexpectedStatus(u16),
    /// `Sec-WebSocket-Accept` passt nicht zum erwarteten Hash.
    AcceptMismatch,
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
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HandshakeError {}

/// Geparste Client-Request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientHandshake {
    /// Request-Path (z.B. `/chat`).
    pub path: String,
    /// Host-Header.
    pub host: String,
    /// Sec-WebSocket-Key (Base64).
    pub key: String,
    /// Optional `Sec-WebSocket-Protocol`.
    pub protocols: Vec<String>,
    /// Optional `Sec-WebSocket-Extensions`.
    pub extensions: Vec<String>,
}

/// Server-Response-Builder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerHandshake {
    /// Status-Code (immer 101 fuer Success).
    pub status: u16,
    /// `Sec-WebSocket-Accept`-Hash.
    pub accept: String,
    /// Optional `Sec-WebSocket-Protocol` (eines der Client-angebotenen).
    pub protocol: Option<String>,
    /// Optional `Sec-WebSocket-Extensions`.
    pub extensions: Vec<String>,
}

/// Berechnet `Sec-WebSocket-Accept` aus dem Client-Key. RFC 6455 §4.2.2.
#[must_use]
pub fn compute_accept(client_key: &str) -> String {
    let mut concatenated = String::with_capacity(client_key.len() + WEBSOCKET_GUID.len());
    concatenated.push_str(client_key.trim());
    concatenated.push_str(WEBSOCKET_GUID);
    let digest = sha1(concatenated.as_bytes());
    base64_encode(&digest)
}

/// Parst eine Client-Handshake-HTTP-Request.
///
/// # Errors
/// Siehe [`HandshakeError`].
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

/// Erzeugt die Server-Response-Bytes. RFC 6455 §4.2.2.
#[must_use]
pub fn build_server_response(req: &ClientHandshake) -> ServerHandshake {
    ServerHandshake {
        status: 101,
        accept: compute_accept(&req.key),
        protocol: req.protocols.first().cloned(),
        extensions: req.extensions.clone(),
    }
}

/// Render zu HTTP/1.1-101-Response-Bytes.
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

// ============================================================================
// Minimal SHA-1 + Base64 (no external deps fuer no_std).
// ============================================================================

/// Minimaler SHA-1 ueber `bytes`. Spec FIPS-180-4 §6.1.
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

/// Base64-Encode laut RFC 4648 (Standard-Alphabet).
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
        // Spec §4.1: Connection MUST contain `Upgrade` token (case-
        // insensitive). Browser senden oft `Connection: keep-alive,
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
