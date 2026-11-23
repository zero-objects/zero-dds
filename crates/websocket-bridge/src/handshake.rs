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
    /// The request method is not `GET` (RFC 6455 §4.2.1 item 1).
    MethodNotGet(String),
    /// The request is not HTTP/1.1 or higher (RFC 6455 §4.2.1 item 1).
    HttpVersion(String),
    /// The `Host` header field is missing (RFC 6455 §4.2.1 item 2).
    MissingHost,
    /// `Sec-WebSocket-Key` is not the base64 of 16 octets (RFC 6455
    /// §4.2.1 item 5).
    InvalidKey,
    /// The `Origin` of the request is not one the server expects (RFC 6455
    /// §10.2, §4.2.2 /origin/); the field's value.
    OriginNotExpected(String),
    /// The request has no `Origin` and the server requires one (RFC 6455
    /// §10.2).
    MissingOrigin,
}

impl HandshakeError {
    /// The HTTP status code a server answers a client's opening handshake
    /// with for this error (RFC 6455 §4.2.1: "the server MUST stop
    /// processing the client's handshake and return an HTTP response with
    /// an appropriate error code"; §4.2.2 /version/; §10.7): 426 for a
    /// version the server does not speak, 405 for a method other than GET,
    /// 403 for an origin the server does not expect (§10.2), 400 for every
    /// other invalid request.
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        match self {
            Self::UnsupportedVersion(_) => 426,
            Self::MethodNotGet(_) => 405,
            Self::OriginNotExpected(_) | Self::MissingOrigin => 403,
            _ => 400,
        }
    }

    /// The HTTP response of [`Self::http_status`], complete with the empty
    /// line and the body. A 426 names the version the server speaks in
    /// `Sec-WebSocket-Version` (RFC 6455 §4.2.2 /version/, §4.4) and the
    /// protocol in `Upgrade` (RFC 9110 §15.5.22); a 405 names `GET` in
    /// `Allow` (RFC 9110 §15.5.6). The connection is closed after it.
    #[must_use]
    pub fn render_error_response(&self) -> String {
        let body = alloc::format!("{self}\n");
        let (reason, extra) = match self.http_status() {
            426 => (
                "Upgrade Required",
                alloc::format!(
                    "Sec-WebSocket-Version: {WEBSOCKET_VERSION}\r\nUpgrade: websocket\r\nConnection: close, Upgrade\r\n"
                ),
            ),
            405 => (
                "Method Not Allowed",
                String::from("Allow: GET\r\nConnection: close\r\n"),
            ),
            403 => ("Forbidden", String::from("Connection: close\r\n")),
            _ => ("Bad Request", String::from("Connection: close\r\n")),
        };
        alloc::format!(
            "HTTP/1.1 {} {reason}\r\n{extra}Content-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}",
            self.http_status(),
            body.len()
        )
    }
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
            Self::MethodNotGet(m) => write!(f, "the opening handshake is a GET request, not {m}"),
            Self::HttpVersion(v) => {
                write!(f, "the opening handshake is HTTP/1.1 or higher, not {v}")
            }
            Self::MissingHost => f.write_str("missing Host"),
            Self::InvalidKey => f.write_str("Sec-WebSocket-Key is not the base64 of 16 octets"),
            // The value is the client's; the response does not repeat it.
            Self::OriginNotExpected(_) => f.write_str("the Origin is not one this server expects"),
            Self::MissingOrigin => f.write_str("this server requires an Origin"),
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
    /// The `Origin` header field (RFC 6455 §4.2.1 item 7): the origin of
    /// the page whose script opens the connection, as a browser names it.
    /// `None` when the request has none. A server that expects certain
    /// origins checks it with
    /// [`OriginPolicy::check`](crate::origin::OriginPolicy::check) (§10.2).
    pub origin: Option<String>,
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

/// Whether `version` is `HTTP/<major>.<minor>` of 1.1 or higher.
fn http_1_1_or_higher(version: &str) -> bool {
    let Some((major, minor)) = version
        .strip_prefix("HTTP/")
        .and_then(|v| v.split_once('.'))
    else {
        return false;
    };
    match (major.parse::<u32>(), minor.parse::<u32>()) {
        (Ok(major), Ok(minor)) => major > 1 || (major == 1 && minor >= 1),
        _ => false,
    }
}

/// Whether `key` is the base64 of 16 octets (RFC 4648 §4): 22 characters
/// of the alphabet and `==`.
fn is_key_of_16_octets(key: &str) -> bool {
    let k = key.as_bytes();
    k.len() == 24
        && k[22..] == *b"=="
        && k[..22]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/')
}

/// Parses a client-handshake HTTP request and checks it against RFC 6455
/// §4.2.1: an HTTP/1.1 or higher GET request with `Host`, `Upgrade:
/// websocket`, `Connection: Upgrade`, a `Sec-WebSocket-Key` that is the
/// base64 of 16 octets, and `Sec-WebSocket-Version: 13`.
///
/// # Errors
/// See [`HandshakeError`]; [`HandshakeError::render_error_response`] is
/// the HTTP response for it.
pub fn parse_client_request(input: &str) -> Result<ClientHandshake, HandshakeError> {
    let mut lines = input.split("\r\n");
    let request_line = lines.next().ok_or(HandshakeError::MalformedRequest)?;
    let mut req_parts = request_line.split(' ');
    let method = req_parts.next().ok_or(HandshakeError::MalformedRequest)?;
    let path = req_parts
        .next()
        .filter(|p| !p.is_empty())
        .ok_or(HandshakeError::MalformedRequest)?
        .to_string();
    let version = req_parts.next().ok_or(HandshakeError::MalformedRequest)?;
    if req_parts.next().is_some() || !version.starts_with("HTTP/") {
        return Err(HandshakeError::MalformedRequest);
    }
    if method != "GET" {
        return Err(HandshakeError::MethodNotGet(method.to_string()));
    }
    if !http_1_1_or_higher(version) {
        return Err(HandshakeError::HttpVersion(version.to_string()));
    }

    let mut hs = ClientHandshake {
        path,
        ..Default::default()
    };
    let mut upgrade_ok = false;
    let mut connection_ok = false;
    let mut host_seen = false;
    let mut version: Option<String> = None;
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
            "host" => {
                host_seen = true;
                hs.host = v.to_string();
            }
            // RFC 9110 §7.8: a list of protocols; `websocket` is one of
            // them, compared without case (RFC 6455 §4.2.1 item 3).
            "upgrade" => {
                upgrade_ok |= v
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("websocket"));
            }
            "connection" => {
                connection_ok |= v
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("upgrade"));
            }
            "sec-websocket-key" => hs.key = v.to_string(),
            // RFC 6454 §7.3 allows one `Origin` field; the origins of a
            // second one are read as more origins of the same list, so
            // each of them is checked.
            "origin" => {
                hs.origin = Some(match hs.origin.take() {
                    Some(first) => alloc::format!("{first} {v}"),
                    None => v.to_string(),
                });
            }
            "sec-websocket-version" => version = Some(v.to_string()),
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
    // §4.2.2 /version/: a request for the upgrade whose version is not the
    // server's is answered with the versions the server speaks, whatever
    // else the request lacks.
    match version {
        Some(v) if v == WEBSOCKET_VERSION => {}
        other => {
            return Err(HandshakeError::UnsupportedVersion(
                other.unwrap_or_default(),
            ));
        }
    }
    if !host_seen {
        return Err(HandshakeError::MissingHost);
    }
    if hs.key.is_empty() {
        return Err(HandshakeError::MissingKey);
    }
    if !is_key_of_16_octets(&hs.key) {
        return Err(HandshakeError::InvalidKey);
    }
    Ok(hs)
}

/// Produces the server response. RFC 6455 §4.2.2.
///
/// The response names no subprotocol and no extension: §4.2.2 item 8
/// allows only a subprotocol "the server has selected" from those it
/// supports, item 9 only extensions the server has agreed to use, and
/// which those are is the caller's knowledge. Echoing the client's offer
/// would turn every offer into an agreement. A caller writes its choice
/// into the returned [`ServerHandshake`] before rendering it:
///
/// * a subprotocol with
///   [`negotiation::select_subprotocol`](crate::negotiation::select_subprotocol)
///   against its own list;
/// * `permessage-deflate` with
///   [`permessage_deflate::accept_offers`](crate::permessage_deflate::accept_offers)
///   on `req.extensions` and
///   [`render_accept`](crate::permessage_deflate::render_accept) for the
///   response element (RFC 7692 §7.1; a bare `client_max_window_bits` of
///   the offer is not echoed, a response needs a value), and it gives the
///   agreed parameters to
///   [`Receiver::with_permessage_deflate`](crate::connection::Receiver::with_permessage_deflate)
///   and
///   [`Sender::with_permessage_deflate`](crate::connection::Sender::with_permessage_deflate).
#[must_use]
pub fn build_server_response(req: &ClientHandshake) -> ServerHandshake {
    ServerHandshake {
        status: 101,
        accept: compute_accept(&req.key),
        // §4.2.2 item 8 — a subprotocol this server does not support must
        // not be named, and omitting the header is how "none" is said.
        protocol: None,
        // §4.2.2 item 9 — likewise for extensions.
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
    render_client_request_with_extensions(host, path, key, protocols, &[])
}

/// [`render_client_request`] with the extensions the client offers in
/// `Sec-WebSocket-Extensions`, in order of preference (RFC 6455 §9.1);
/// each element is an extension with its parameters, e.g. the result of
/// [`permessage_deflate::render_offer`](crate::permessage_deflate::render_offer).
#[must_use]
pub fn render_client_request_with_extensions(
    host: &str,
    path: &str,
    key: &str,
    protocols: &[&str],
    extensions: &[&str],
) -> String {
    let mut out = alloc::format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: {WEBSOCKET_VERSION}\r\n"
    );
    if !protocols.is_empty() {
        out.push_str(&alloc::format!(
            "Sec-WebSocket-Protocol: {}\r\n",
            protocols.join(", ")
        ));
    }
    if !extensions.is_empty() {
        out.push_str(&alloc::format!(
            "Sec-WebSocket-Extensions: {}\r\n",
            extensions.join(", ")
        ));
    }
    out.push_str("\r\n");
    out
}

/// What the server agreed to in its handshake response.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerAnswer {
    /// The subprotocol the server selected.
    pub protocol: Option<String>,
    /// The elements of `Sec-WebSocket-Extensions`: the extensions the
    /// server agreed to, each with its parameters.
    pub extensions: Vec<String>,
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
    validate_server_response_with_extensions(response, key, offered, require, &[])
        .map(|answer| answer.protocol)
}

/// [`validate_server_response`] for a client that offered the extensions
/// `offered_extensions` (the elements it sent): the response may name
/// extensions out of them, and one the client did not offer fails the
/// handshake (RFC 6455 §4.1 step 5). The parameters of an agreed
/// extension are the extension's to check, for `permessage-deflate`
/// [`permessage_deflate::accept_response`](crate::permessage_deflate::accept_response).
///
/// # Errors
/// See [`HandshakeError`].
pub fn validate_server_response_with_extensions(
    response: &str,
    key: &str,
    offered: &[&str],
    require: bool,
    offered_extensions: &[&str],
) -> Result<ServerAnswer, HandshakeError> {
    let name_of = |element: &str| {
        element
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
    };
    let mut extensions: Vec<String> = Vec::new();
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
            "sec-websocket-extensions" => {
                for element in v.split(',').map(str::trim).filter(|e| !e.is_empty()) {
                    let name = name_of(element);
                    if !offered_extensions.iter().any(|o| name_of(o) == name) {
                        return Err(HandshakeError::ExtensionNotOffered(element.to_string()));
                    }
                    extensions.push(element.to_string());
                }
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
        protocol => Ok(ServerAnswer {
            protocol,
            extensions,
        }),
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
    fn ws_4_1_rfc6455_section_1_3_accept_test_vector() {
        // Spec §1.3 — "the sample nonce" → expected accept value.
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
        assert_eq!(compute_accept(key), expected);
    }

    /// The client side of §4.1 against the §1.3 sample: the request carries
    /// the key and the offered subprotocol, the sample response validates,
    /// and each deviation step 1 to 6 names is refused.
    #[test]
    fn mqttbridge_2_the_client_side_validates_the_server_response() {
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
    fn ws_4_1_parses_minimal_client_handshake() {
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
    fn ws_4_1_parses_protocols_and_extensions() {
        let req = "GET / HTTP/1.1\r\n\
                   Host: x\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
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
    fn ws_4_1_rejects_missing_upgrade() {
        let req = "GET / HTTP/1.1\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   \r\n";
        assert_eq!(
            parse_client_request(req),
            Err(HandshakeError::NotWebSocketUpgrade)
        );
    }

    #[test]
    fn ws_4_1_rejects_wrong_version() {
        let req = "GET / HTTP/1.1\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                   Sec-WebSocket-Version: 8\r\n\
                   \r\n";
        assert!(matches!(
            parse_client_request(req),
            Err(HandshakeError::UnsupportedVersion(_))
        ));
    }

    #[test]
    fn ws_4_1_rejects_missing_key() {
        let req = "GET / HTTP/1.1\r\n\
                   Host: x\r\n\
                   Upgrade: websocket\r\n\
                   Connection: Upgrade\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   \r\n";
        assert_eq!(parse_client_request(req), Err(HandshakeError::MissingKey));
    }

    #[test]
    fn ws_4_1_server_response_includes_accept() {
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
    fn ws_4_1_server_agrees_to_no_extension_it_cannot_perform() {
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
    fn ws_4_1_server_selects_no_subprotocol_it_does_not_support() {
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
    fn ws_4_1_render_server_response_format() {
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
    fn ws_4_1_base64_round_trip_known_vectors() {
        // RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn ws_4_1_sha1_known_vector_abc() {
        // FIPS-180-4 Appendix A.1.
        let h = sha1(b"abc");
        let expected: [u8; 20] = [
            0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50,
            0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
        ];
        assert_eq!(h, expected);
    }

    #[test]
    fn ws_4_1_connection_header_with_keep_alive_still_detects_upgrade() {
        // Spec §4.1: Connection MUST contain the `Upgrade` token (case-
        // insensitive). Browsers often send `Connection: keep-alive,
        // Upgrade`.
        let req = "GET / HTTP/1.1\r\n\
                   Host: x\r\n\
                   Upgrade: WebSocket\r\n\
                   Connection: keep-alive, Upgrade\r\n\
                   Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                   Sec-WebSocket-Version: 13\r\n\
                   \r\n";
        assert!(parse_client_request(req).is_ok());
    }

    /// A valid request with one line replaced or removed.
    fn request(change: impl Fn(&str) -> Option<String>) -> String {
        let lines = [
            "GET /chat HTTP/1.1",
            "Host: server.example.com",
            "Upgrade: websocket",
            "Connection: Upgrade",
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
            "Sec-WebSocket-Version: 13",
        ];
        let mut out = String::new();
        for line in lines {
            if let Some(l) = change(line) {
                out.push_str(&l);
                out.push_str("\r\n");
            }
        }
        out.push_str("\r\n");
        out
    }

    fn replace(what: &'static str, with: &'static str) -> impl Fn(&str) -> Option<String> {
        move |line| {
            if line.starts_with(what) {
                (!with.is_empty()).then(|| with.to_string())
            } else {
                Some(line.to_string())
            }
        }
    }

    #[test]
    fn ws_4_2_2_an_unsupported_version_gets_426_and_the_version_the_server_speaks() {
        // §4.2.2 /version/: "the server MUST abort the WebSocket handshake
        // [...] and instead send an appropriate HTTP error code (such as
        // 426 Upgrade Required) and a |Sec-WebSocket-Version| header field
        // indicating the version(s) the server is capable of
        // understanding." 25 is the version of the example in §4.4.
        for line in ["Sec-WebSocket-Version: 25", "Sec-WebSocket-Version: 8", ""] {
            let err = parse_client_request(&request(replace("Sec-WebSocket-Version", line)))
                .expect_err("not version 13");
            assert!(matches!(err, HandshakeError::UnsupportedVersion(_)));
            assert_eq!(err.http_status(), 426);
            let response = err.render_error_response();
            assert!(
                response.starts_with("HTTP/1.1 426 Upgrade Required\r\n"),
                "{response}"
            );
            assert!(response.contains("\r\nSec-WebSocket-Version: 13\r\n"));
            // RFC 9110 §15.5.22: a 426 names the protocol in Upgrade.
            assert!(response.contains("\r\nUpgrade: websocket\r\n"));
        }
    }

    #[test]
    fn ws_4_2_1_a_request_that_is_not_get_gets_405() {
        // §4.2.1 item 1: "An HTTP/1.1 or higher GET request".
        let err = parse_client_request(&request(replace("GET", "POST /chat HTTP/1.1")))
            .expect_err("a POST");
        assert_eq!(err, HandshakeError::MethodNotGet("POST".to_string()));
        assert_eq!(err.http_status(), 405);
        let response = err.render_error_response();
        assert!(response.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
        assert!(response.contains("\r\nAllow: GET\r\n"));
        // The method is case-sensitive (RFC 9110 §9.1).
        assert!(matches!(
            parse_client_request(&request(replace("GET", "get /chat HTTP/1.1"))),
            Err(HandshakeError::MethodNotGet(_))
        ));
    }

    #[test]
    fn ws_4_2_1_a_request_below_http_1_1_is_refused() {
        for (line, ok) in [
            ("GET /chat HTTP/1.0", false),
            ("GET /chat HTTP/0.9", false),
            ("GET /chat HTTP/1.1", true),
            ("GET /chat HTTP/1.2", true),
            ("GET /chat HTTP/2.0", true),
        ] {
            let result = parse_client_request(&request(replace("GET", line)));
            assert_eq!(result.is_ok(), ok, "{line}: {result:?}");
            if let Err(err) = result {
                assert!(matches!(err, HandshakeError::HttpVersion(_)));
                assert_eq!(err.http_status(), 400);
            }
        }
        // A request line that is none.
        for line in [
            "GET /chat",
            "GET  HTTP/1.1",
            "GET /chat FTP/1.1",
            "GET /a b HTTP/1.1",
        ] {
            assert_eq!(
                parse_client_request(&request(replace("GET", line))),
                Err(HandshakeError::MalformedRequest),
                "{line}"
            );
        }
    }

    #[test]
    fn ws_4_2_1_a_request_without_host_or_with_a_wrong_key_gets_400() {
        // §4.2.1 items 2 and 5.
        let err = parse_client_request(&request(replace("Host", ""))).expect_err("no Host");
        assert_eq!(err, HandshakeError::MissingHost);
        for key in [
            "Sec-WebSocket-Key: a",
            // 15 octets and 17 octets.
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25j",
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZWU=",
            // 24 characters, one outside the alphabet.
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25j*Q==",
        ] {
            let err = parse_client_request(&request(replace("Sec-WebSocket-Key", key)))
                .expect_err("no 16 octets");
            assert_eq!(err, HandshakeError::InvalidKey, "{key}");
            assert_eq!(err.http_status(), 400);
            let response = err.render_error_response();
            assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
            assert!(response.contains("\r\nConnection: close\r\n"));
            // The body is as long as Content-Length says.
            let (head, body) = response.split_once("\r\n\r\n").expect("head and body");
            assert!(head.contains(&alloc::format!("Content-Length: {}", body.len())));
        }
    }

    #[test]
    fn ws_4_2_1_upgrade_may_list_several_protocols() {
        // RFC 9110 §7.8: Upgrade is a list.
        let req = request(replace("Upgrade", "Upgrade: h2c, WebSocket"));
        assert!(parse_client_request(&req).is_ok());
    }

    #[test]
    fn ws_4_1_the_client_accepts_only_extensions_it_offered() {
        // §4.1 step 5 of the response check.
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let offer = "permessage-deflate; client_max_window_bits";
        let req = render_client_request_with_extensions("h", "/", key, &[], &[offer]);
        assert_eq!(
            parse_client_request(&req).unwrap().extensions,
            alloc::vec![offer.to_string()]
        );
        let response = |ext: &str| {
            alloc::format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
                 Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nSec-WebSocket-Extensions: {ext}\r\n\r\n"
            )
        };
        let answer = validate_server_response_with_extensions(
            &response("permessage-deflate; server_max_window_bits=10"),
            key,
            &[],
            false,
            &[offer],
        )
        .expect("offered");
        assert_eq!(
            answer.extensions,
            alloc::vec!["permessage-deflate; server_max_window_bits=10".to_string()]
        );
        assert_eq!(
            validate_server_response_with_extensions(
                &response("x-webkit-deflate-frame"),
                key,
                &[],
                false,
                &[offer]
            ),
            Err(HandshakeError::ExtensionNotOffered(
                "x-webkit-deflate-frame".to_string()
            ))
        );
    }

    #[test]
    fn ws_4_2_1_the_origin_field_is_read() {
        // §4.2.1 item 7: "Optionally, an |Origin| header field."
        assert_eq!(
            parse_client_request(&request(|l| Some(l.to_string())))
                .unwrap()
                .origin,
            None
        );
        let with = |origin: &'static str| request(replace("Host", origin));
        let req = parse_client_request(&with(
            "Host: server.example.com\r\nOrigin: http://example.com",
        ))
        .unwrap();
        assert_eq!(req.origin.as_deref(), Some("http://example.com"));
        // The field name has no case (RFC 9110 §5.1).
        let req = parse_client_request(&with("Host: h\r\nORIGIN:  null ")).unwrap();
        assert_eq!(req.origin.as_deref(), Some("null"));
        // A second field adds to the list of the first, so that a check of
        // the list sees both.
        let req = parse_client_request(&with(
            "Host: h\r\nOrigin: https://a.example\r\nOrigin: https://b.example",
        ))
        .unwrap();
        assert_eq!(
            req.origin.as_deref(),
            Some("https://a.example https://b.example")
        );
    }

    #[test]
    fn ws_10_2_an_origin_the_server_does_not_expect_gets_403() {
        // §10.2: "it SHOULD respond to the WebSocket handshake with a reply
        // containing HTTP 403 Forbidden status code."
        for err in [
            HandshakeError::OriginNotExpected("https://evil.example".to_string()),
            HandshakeError::MissingOrigin,
        ] {
            assert_eq!(err.http_status(), 403);
            let response = err.render_error_response();
            assert!(
                response.starts_with("HTTP/1.1 403 Forbidden\r\n"),
                "{response}"
            );
            assert!(response.contains("\r\nConnection: close\r\n"));
            let (head, body) = response.split_once("\r\n\r\n").expect("head and body");
            assert!(head.contains(&alloc::format!("Content-Length: {}", body.len())));
            // The response does not carry the client's text back.
            assert!(!response.contains("evil.example"));
        }
    }
}
