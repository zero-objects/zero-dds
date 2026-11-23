// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT over WebSocket, the `ws://` and `wss://` broker schemes (Spec §2,
//! OASIS MQTT 5.0 §6, RFC 6455).
//!
//! [`WsStream`] turns the frames of a WebSocket connection back into the
//! byte stream the MQTT client reads and writes. MQTT 5.0 §6 fixes the
//! rules this layer holds to:
//!
//! * control packets travel in **binary** data frames; any other data
//!   frame closes the connection;
//! * a frame may carry several or part of a control packet, so the
//!   payloads of consecutive frames are one stream;
//! * the client offers the subprotocol `mqtt`, and the server must select
//!   it.
//!
//! RFC 6455 adds the client's duties: every frame it sends is masked with
//! a key from a strong source (§5.3), it refuses a masked frame from the
//! server (§5.1), answers a Ping with a Pong (§5.5.2) and a Close with a
//! Close (§5.5.1). Nothing is negotiated, so a frame with an RSV bit set is
//! refused (§5.2).
//!
//! Invalid data of the server fails the connection (RFC 6455 §7.1.7,
//! §10.7): the client sends a Close frame with the status code of the
//! violation and shuts the network connection down. The codes are 1002 for
//! a violation of the framing rules or a Close frame with an invalid status
//! code, 1003 for a data frame that is not binary, 1007 for a Close reason
//! that is not UTF-8, 1009 for a frame over the limit.

use std::io::{self, Read, Write};
use std::string::String;
use std::vec::Vec;

use zerodds_websocket_bridge::close::parse_close_body;
use zerodds_websocket_bridge::codec::{decode, encode};
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::handshake::{
    client_key, render_client_request, validate_server_response,
};

use super::client::MqttStream;
use crate::ws::{SUBPROTOCOL, close_reason, invalid, read_handshake_head};

/// The largest frame payload accepted: one maximal MQTT packet with its
/// fixed header. A larger announcement is refused before it is buffered.
const MAX_FRAME_PAYLOAD: u64 = crate::net::MAX_MQTT_PACKET_SIZE as u64 + 5;

/// RFC 6455 §7.4.1 — "a protocol error".
const PROTOCOL_ERROR: u16 = 1002;
/// RFC 6455 §7.4.1 — "a type of data it cannot accept".
const UNSUPPORTED_DATA: u16 = 1003;
/// RFC 6455 §7.4.1 — "a message that is too big for it to process".
const MESSAGE_TOO_BIG: u16 = 1009;

/// Fills `buf` from the TLS provider's random source, the strong source
/// RFC 6455 §4.1 and §5.3 require for the nonce and the masking keys.
fn secure_fill(buf: &mut [u8]) -> io::Result<()> {
    zerodds_bridge_security::tls_provider()
        .secure_random
        .fill(buf)
        .map_err(|_| io::Error::other("no random source for the WebSocket key"))
}

/// The payload length a frame header announces, once enough of the header
/// is buffered to say (RFC 6455 §5.2).
fn announced_payload_len(raw: &[u8]) -> Option<u64> {
    let len7 = raw.get(1)? & 0x7F;
    match len7 {
        126 => raw
            .get(2..4)
            .map(|b| u64::from(u16::from_be_bytes([b[0], b[1]]))),
        127 => raw.get(2..10).map(|b| {
            let mut a = [0u8; 8];
            a.copy_from_slice(b);
            u64::from_be_bytes(a)
        }),
        n => Some(u64::from(n)),
    }
}

/// The MQTT byte stream over a WebSocket connection.
pub(crate) struct WsStream {
    /// The TCP or TLS connection below.
    wire: MqttStream,
    /// Received bytes not yet decoded into a frame.
    raw: Vec<u8>,
    /// Payload bytes of data frames not yet read by MQTT.
    payload: Vec<u8>,
    /// How much of `payload` MQTT has read.
    read_pos: usize,
    /// A binary message is open and continuation frames may follow.
    in_message: bool,
    /// The connection is closed: a Close frame arrived or was sent.
    closed: bool,
}

impl WsStream {
    /// The opening handshake of RFC 6455 §4.1 over `wire`: `host` is the
    /// `Host` header value, `path` the resource name. The server must
    /// answer 101 with the matching accept value and select `mqtt`.
    ///
    /// # Errors
    /// I/O errors of the connection; `InvalidData` naming what the server's
    /// answer lacks.
    pub(crate) fn handshake(mut wire: MqttStream, host: &str, path: &str) -> io::Result<Self> {
        let mut nonce = [0u8; 16];
        secure_fill(&mut nonce)?;
        let key = client_key(&nonce);
        wire.write_all(render_client_request(host, path, &key, &[SUBPROTOCOL]).as_bytes())?;
        wire.flush()?;
        let (head, raw) = read_handshake_head(&mut wire, "WebSocket handshake response")?;
        validate_server_response(&head, &key, &[SUBPROTOCOL], true)
            .map_err(|e| invalid(format!("WebSocket handshake: {e}")))?;
        Ok(Self {
            wire,
            // Frames the server sent right behind its response.
            raw,
            payload: Vec::new(),
            read_pos: 0,
            in_message: false,
            closed: false,
        })
    }

    /// The TCP or TLS connection below, for timeouts and shutdown.
    pub(crate) fn wire(&self) -> &MqttStream {
        &self.wire
    }

    /// Whether bytes are buffered above the socket: undecoded frame bytes,
    /// unread payload, or what the wire below holds.
    pub(crate) fn has_buffered_input(&mut self) -> bool {
        !self.raw.is_empty() || self.read_pos < self.payload.len() || self.wire.has_buffered_input()
    }

    /// Sends a Close frame (status 1000) and marks the connection closed;
    /// the caller then shuts the socket down.
    pub(crate) fn close(&mut self) {
        if !self.closed {
            let _ = self.send(Frame::close(1000, ""));
            self.closed = true;
        }
    }

    fn send(&mut self, mut frame: Frame) -> io::Result<()> {
        let mut key = [0u8; 4];
        secure_fill(&mut key)?;
        frame.masking_key = Some(key);
        let bytes = encode(&frame).map_err(|e| invalid(format!("WebSocket encode: {e}")))?;
        self.wire.write_all(&bytes)?;
        self.wire.flush()
    }

    /// Fails the connection for invalid data of the server (RFC 6455
    /// §7.1.7, §10.7; MQTT 5.0 §6 [MQTT-6.0.0-1]): the Close frame with
    /// `status` and `message` as its reason, unless a Close frame went out
    /// before, then the network connection is shut down. Returns the error
    /// the read ends with.
    fn fail(&mut self, status: u16, message: impl Into<String>) -> io::Error {
        let message = message.into();
        if !self.closed {
            // The connection ends either way; a Close frame that cannot be
            // written any more changes nothing about the error reported.
            let _ = self.send(Frame::close(status, close_reason(&message)));
            self.closed = true;
        }
        self.wire.shutdown_both();
        invalid(message)
    }

    fn on_frame(&mut self, frame: Frame) -> io::Result<()> {
        if frame.masking_key.is_some() {
            return Err(self.fail(
                PROTOCOL_ERROR,
                "the server sent a masked frame (RFC 6455 §5.1)",
            ));
        }
        if frame.rsv1 || frame.rsv2 || frame.rsv3 {
            return Err(self.fail(
                PROTOCOL_ERROR,
                "a frame with an RSV bit set, and no extension was negotiated (RFC 6455 §5.2)",
            ));
        }
        match frame.opcode {
            Opcode::Binary => {
                if self.in_message {
                    return Err(self.fail(
                        PROTOCOL_ERROR,
                        "a new binary frame inside an unfinished message (RFC 6455 §5.4)",
                    ));
                }
                self.in_message = !frame.fin;
                self.push_payload(frame.payload);
            }
            Opcode::Continuation => {
                if !self.in_message {
                    return Err(self.fail(
                        PROTOCOL_ERROR,
                        "a continuation frame outside a message (RFC 6455 §5.4)",
                    ));
                }
                self.in_message = !frame.fin;
                self.push_payload(frame.payload);
            }
            Opcode::Text => {
                return Err(self.fail(
                    UNSUPPORTED_DATA,
                    "a text frame; MQTT travels in binary frames only (MQTT 5.0 §6)",
                ));
            }
            Opcode::Ping => {
                let mut pong = Frame::binary(frame.payload);
                pong.opcode = Opcode::Pong;
                self.send(pong)?;
            }
            Opcode::Pong => {}
            Opcode::Close => self.on_close(&frame.payload)?,
            Opcode::Reserved(op) => {
                return Err(self.fail(
                    PROTOCOL_ERROR,
                    format!("a frame with the reserved opcode {op:#x} (RFC 6455 §5.2)"),
                ));
            }
        }
        Ok(())
    }

    /// The Close frame of the server with `body` (RFC 6455 §5.5.1). A body
    /// of one octet, a status code that must not appear in a Close frame
    /// (§7.4.1, §7.4.2) and a reason that is not UTF-8 are invalid data. A
    /// valid one is answered with a Close frame of the same status code, or
    /// one without a body when the server sent none.
    fn on_close(&mut self, body: &[u8]) -> io::Result<()> {
        let answer = match parse_close_body(body) {
            Ok(Some((status, _reason))) => Frame::close(status, ""),
            Ok(None) => Frame {
                payload: Vec::new(),
                ..Frame::close(1000, "")
            },
            Err(e) => {
                return Err(self.fail(
                    e.close_code().to_u16(),
                    format!("the Close frame of the server is invalid: {e} (RFC 6455 §5.5.1)"),
                ));
            }
        };
        // No frame is read after a Close frame went out (`read`), so this
        // end has not sent one yet. The server closes the network
        // connection whether the answer reaches it or not.
        let _ = self.send(answer);
        self.closed = true;
        Ok(())
    }

    fn push_payload(&mut self, bytes: Vec<u8>) {
        if self.read_pos == self.payload.len() {
            self.payload = bytes;
            self.read_pos = 0;
        } else {
            self.payload.extend_from_slice(&bytes);
        }
    }
}

impl Read for WsStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut chunk = [0u8; 4096];
        while self.read_pos == self.payload.len() {
            if self.closed {
                return Ok(0);
            }
            match decode(&self.raw) {
                Ok((frame, used)) => {
                    self.raw.drain(..used);
                    self.on_frame(frame)?;
                }
                Err(e) if e.is_incomplete() => {
                    if announced_payload_len(&self.raw).is_some_and(|n| n > MAX_FRAME_PAYLOAD) {
                        return Err(self.fail(
                            MESSAGE_TOO_BIG,
                            format!(
                                "a WebSocket frame announces more than {MAX_FRAME_PAYLOAD} bytes"
                            ),
                        ));
                    }
                    let n = self.wire.read(&mut chunk)?;
                    if n == 0 {
                        self.closed = true;
                        return Ok(0);
                    }
                    self.raw.extend_from_slice(&chunk[..n]);
                }
                Err(e) => {
                    return Err(self.fail(e.close_code().to_u16(), format!("WebSocket frame: {e}")));
                }
            }
        }
        let n = out.len().min(self.payload.len() - self.read_pos);
        out[..n].copy_from_slice(&self.payload[self.read_pos..self.read_pos + n]);
        self.read_pos += n;
        Ok(n)
    }
}

impl Write for WsStream {
    /// One binary frame per call, carrying all of `bytes`.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "the WebSocket connection is closed",
            ));
        }
        self.send(Frame::binary(bytes.to_vec()))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.wire.flush()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_announced_length_is_read_from_every_header_form() {
        assert_eq!(announced_payload_len(&[0x82]), None);
        assert_eq!(announced_payload_len(&[0x82, 5]), Some(5));
        assert_eq!(announced_payload_len(&[0x82, 126, 0x01]), None);
        assert_eq!(announced_payload_len(&[0x82, 126, 0x01, 0x00]), Some(256));
        assert_eq!(
            announced_payload_len(&[0x82, 127, 0, 0, 0, 1, 0, 0, 0, 0]),
            Some(1 << 32)
        );
    }
}
