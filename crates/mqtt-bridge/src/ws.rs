// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the two WebSocket ends of MQTT share (OASIS MQTT 5.0 §6, RFC 6455):
//! the subprotocol name, the reading of the HTTP header block of the
//! opening handshake and the reason of a Close frame.
//!
//! The client is `daemon::ws_transport` (the `ws://` and `wss://` broker
//! schemes), the server `server::websocket` (the WebSocket listener of the
//! broker server).

use std::io::{self, Read};
use std::string::String;
use std::vec::Vec;

/// The subprotocol MQTT 5.0 §6 requires: the client offers it
/// ([MQTT-6.0.0-3]), the server selects it ([MQTT-6.0.0-4]).
pub(crate) const SUBPROTOCOL: &str = "mqtt";

/// The largest header block of an opening handshake read before it must
/// end with the empty line.
pub(crate) const MAX_HANDSHAKE_HEAD: usize = 16 * 1024;

/// The longest reason of a Close frame: a control frame carries at most
/// 125 octets, two of them the status code (RFC 6455 §5.5).
const MAX_CLOSE_REASON: usize = 123;

pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Reads the header block of an opening handshake from `wire`, up to and
/// including the empty line that ends it (RFC 6455 §4.1, §4.2.1); `what`
/// names it in the errors. Returns the block and the octets read behind
/// it, which are the first frame octets.
///
/// # Errors
/// I/O errors of `wire`; `UnexpectedEof` when the connection ends before
/// the empty line; `InvalidData` for a block over [`MAX_HANDSHAKE_HEAD`]
/// octets or one that is not UTF-8.
pub(crate) fn read_handshake_head<R: Read>(
    wire: &mut R,
    what: &str,
) -> io::Result<(String, Vec<u8>)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > MAX_HANDSHAKE_HEAD {
            return Err(invalid(format!(
                "{what} exceeds {MAX_HANDSHAKE_HEAD} bytes"
            )));
        }
        let n = wire.read(&mut chunk)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed during the WebSocket handshake",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let rest = buf.split_off(end);
    let head = String::from_utf8(buf).map_err(|_| invalid(format!("{what} is not UTF-8")))?;
    Ok((head, rest))
}

/// `message` cut to the longest reason a Close frame carries, at a
/// character boundary.
pub(crate) fn close_reason(message: &str) -> &str {
    let mut end = message.len().min(MAX_CLOSE_REASON);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    &message[..end]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_head_ends_at_the_empty_line_and_keeps_what_follows() {
        let mut wire: &[u8] = b"GET / HTTP/1.1\r\nHost: a\r\n\r\n\x82\x80";
        let (head, rest) = read_handshake_head(&mut wire, "request").unwrap();
        assert_eq!(head, "GET / HTTP/1.1\r\nHost: a\r\n\r\n");
        assert_eq!(rest, [0x82, 0x80]);
    }

    #[test]
    fn a_head_without_end_is_refused_at_the_limit_and_at_the_end_of_the_connection() {
        let long = vec![b'a'; MAX_HANDSHAKE_HEAD + 2048];
        let e = read_handshake_head(&mut long.as_slice(), "request").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
        let e = read_handshake_head(&mut &b"GET / HTTP/1.1\r\n"[..], "request").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::UnexpectedEof);
        let e = read_handshake_head(&mut &b"GET \xff HTTP/1.1\r\n\r\n"[..], "request").unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_close_reason_is_cut_at_a_character_boundary() {
        let s = "ä".repeat(70);
        let r = close_reason(&s);
        assert!(r.len() <= MAX_CLOSE_REASON);
        assert_eq!(r.len(), 122);
        assert_eq!(close_reason("short"), "short");
    }
}
