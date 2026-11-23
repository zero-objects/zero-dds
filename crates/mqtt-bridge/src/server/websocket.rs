// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The WebSocket transport of the broker server (OASIS MQTT 5.0 §6,
//! RFC 6455), below `ws://` on TCP and `wss://` on TLS.
//!
//! [`accept`] takes the opening handshake of RFC 6455 §4.2 on a connection
//! the server accepted: the request must be valid (§4.2.1) and offer the
//! subprotocol `mqtt`, which the response selects ([MQTT-6.0.0-4]). A
//! request without it, or an invalid one, gets an HTTP error response and
//! the close. The resource name of the request is not looked at: the URI
//! has no impact on the MQTT protocol (MQTT 5.0 §6).
//!
//! [`WebSocketStream`] turns the frames of the client back into the byte
//! stream the MQTT protocol reads:
//!
//! * the payload of every binary frame and its continuation frames is one
//!   stream, read as it arrives: a frame may carry several control packets
//!   or part of one ([MQTT-6.0.0-2]), and no frame is buffered whole, so
//!   the only size limit is the Maximum Packet Size of the MQTT protocol;
//! * a text frame closes the Network Connection ([MQTT-6.0.0-1]) with a
//!   Close frame 1003, read from its header;
//! * a frame the client did not mask (§5.1), one with an RSV bit (§5.2,
//!   nothing is negotiated), a reserved opcode (§5.2), a data frame against
//!   the fragmentation rules (§5.4) and an invalid header (§5.2, §5.5) fail
//!   the connection (§7.1.7) with 1002, an invalid Close frame with the
//!   status code of its violation (§5.5.1, §7.4.1);
//! * a Ping is answered with a Pong (§5.5.2), a Close with a Close of the
//!   same status code (§5.5.1); the read then ends.
//!
//! Its writer sends each control packet in one unmasked binary frame
//! (§5.1, [MQTT-6.0.0-1]). When the server closes the Network Connection,
//! the writer sends a Close frame 1000 and ends its direction of the
//! connection; the client's Close frame is still read, and the TCP
//! connection is closed when it has come, the client has closed, or
//! [`CLOSE_WAIT`] has passed (§7.1.1). The reader reads it while it runs;
//! a reader that stopped before reads it when the stream is dropped.

use std::boxed::Box;
use std::io::{self, Read, Write};
use std::string::String;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use std::vec::Vec;

use zerodds_websocket_bridge::close::{CloseCode, parse_close_body};
use zerodds_websocket_bridge::codec::{decode, encode, peek_header};
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::handshake::{
    HandshakeError, build_server_response, parse_client_request, render_server_response,
};

use super::conn::DeadlineReader;
use super::{ClientStream, ClientWriter};
use crate::ws::{SUBPROTOCOL, close_reason, invalid, read_handshake_head};

/// How long the server waits for the client's Close frame after it sent
/// its own, before the TCP connection is closed (RFC 6455 §7.1.1).
pub const CLOSE_WAIT: Duration = Duration::from_secs(2);

const PROTOCOL_ERROR: u16 = CloseCode::ProtocolError.to_u16();
const UNSUPPORTED_DATA: u16 = CloseCode::UnsupportedData.to_u16();
const NORMAL: u16 = CloseCode::Normal.to_u16();

/// The answer to a request that does not offer `mqtt`: the server cannot
/// select it ([MQTT-6.0.0-4]), and RFC 6455 §4.2.2 allows it to select
/// only an offered subprotocol.
fn no_subprotocol_response() -> String {
    let body = "the client offers no WebSocket subprotocol mqtt (MQTT 5.0 section 6)\n";
    format!(
        "HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

/// Writes the HTTP error response `text` and closes the connection.
fn refuse(writer: &mut dyn ClientWriter, text: &str) {
    // The connection is closed either way; the response is a courtesy to
    // the client.
    let _ = writer
        .write_all(text.as_bytes())
        .and_then(|()| writer.flush());
    writer.close();
}

/// The opening handshake of RFC 6455 §4.2 on `stream`, within `timeout`.
///
/// # Errors
/// I/O errors of the connection; `InvalidData` for a request that is not a
/// valid opening handshake or offers no subprotocol `mqtt` — both answered
/// with an HTTP error response and the close.
pub(super) fn accept<S: ClientStream>(
    stream: S,
    timeout: Duration,
) -> io::Result<WebSocketStream<S>> {
    let mut writer = stream.writer()?;
    let mut reader = DeadlineReader {
        stream,
        deadline: Some(Instant::now() + timeout),
    };
    let read = read_handshake_head(&mut reader, "WebSocket opening handshake");
    let stream = reader.stream;
    let (head, raw) = match read {
        Ok(r) => r,
        Err(e) => {
            if e.kind() == io::ErrorKind::InvalidData {
                refuse(
                    &mut *writer,
                    &HandshakeError::MalformedRequest.render_error_response(),
                );
            } else {
                writer.close();
            }
            return Err(e);
        }
    };
    let req = match parse_client_request(&head) {
        Ok(r) => r,
        Err(e) => {
            refuse(&mut *writer, &e.render_error_response());
            return Err(invalid(format!("WebSocket opening handshake: {e}")));
        }
    };
    if !req.protocols.iter().any(|p| p == SUBPROTOCOL) {
        refuse(&mut *writer, &no_subprotocol_response());
        return Err(invalid(
            "the client offers no WebSocket subprotocol mqtt (MQTT 5.0 §6 [MQTT-6.0.0-3])",
        ));
    }
    let mut resp = build_server_response(&req);
    resp.protocol = Some(SUBPROTOCOL.into());
    writer.write_all(render_server_response(&resp).as_bytes())?;
    writer.flush()?;
    Ok(WebSocketStream {
        wire: stream,
        link: Arc::new(Link {
            out: Mutex::new(Out {
                w: writer,
                close_sent: false,
                peer_done: false,
                close_deadline: None,
            }),
            wake: Condvar::new(),
        }),
        raw,
        frame: None,
        in_message: false,
        peer_done: false,
    })
}

/// The sending side of a connection, shared by the reader (Pong, Close)
/// and the writer (the control packets), and the state of the closing
/// handshake.
struct Out {
    w: Box<dyn ClientWriter>,
    /// A Close frame went out; no frame follows it (RFC 6455 §5.5.1).
    close_sent: bool,
    /// The client's Close frame came, the client closed the connection, or
    /// the connection was failed.
    peer_done: bool,
    /// When the server stops waiting for the client's Close frame.
    close_deadline: Option<Instant>,
}

/// [`Out`] and the wake-up of the thread that closes the TCP connection.
struct Link {
    out: Mutex<Out>,
    wake: Condvar,
}

impl Link {
    fn lock(&self) -> MutexGuard<'_, Out> {
        self.out.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The client has ended its side: the TCP connection may close now.
    fn peer_done(&self) {
        self.lock().peer_done = true;
        self.wake.notify_all();
    }
}

impl Out {
    fn send(&mut self, frame: &Frame) -> io::Result<()> {
        let bytes = encode(frame).map_err(|e| invalid(format!("WebSocket encode: {e}")))?;
        self.w.write_all(&bytes)?;
        self.w.flush()
    }

    /// The Close frame with `status` and `reason`, unless one went out.
    fn close_frame(&mut self, status: u16, reason: &str) {
        if !self.close_sent {
            self.close_sent = true;
            // The connection ends either way; a Close frame that cannot be
            // written any more changes nothing about that.
            let _ = self.send(&Frame::close(status, reason));
        }
    }
}

/// The data frame whose payload is arriving.
struct DataFrame {
    /// Payload octets still to come.
    left: u64,
    key: [u8; 4],
    /// Payload octets unmasked so far, for the key index (§5.3).
    pos: usize,
    fin: bool,
}

impl DataFrame {
    fn unmask(&mut self, octets: &mut [u8]) {
        for o in octets.iter_mut() {
            *o ^= self.key[self.pos % 4];
            self.pos = self.pos.wrapping_add(1);
        }
        self.left -= octets.len() as u64;
    }
}

/// The MQTT byte stream of a WebSocket connection the server accepted.
pub struct WebSocketStream<S: ClientStream> {
    /// The TCP or TLS connection below.
    wire: S,
    link: Arc<Link>,
    /// Received octets not yet taken into a frame.
    raw: Vec<u8>,
    /// The data frame whose payload is arriving.
    frame: Option<DataFrame>,
    /// A binary message is open and continuation frames follow (§5.4).
    in_message: bool,
    /// The client sent a Close frame or ended the connection, or the
    /// connection was failed: nothing more is read.
    peer_done: bool,
}

impl<S: ClientStream> WebSocketStream<S> {
    /// Fails the connection for invalid data of the client (RFC 6455
    /// §7.1.7, §10.7; MQTT 5.0 [MQTT-6.0.0-1]): the Close frame with
    /// `status` and `message` as its reason, unless one went out before,
    /// then the network connection is closed. Returns the error the read
    /// ends with.
    fn fail(&mut self, status: u16, message: &str) -> io::Error {
        self.peer_done = true;
        let mut out = self.link.lock();
        out.close_frame(status, close_reason(message));
        out.w.close();
        out.peer_done = true;
        drop(out);
        self.link.wake.notify_all();
        invalid(message)
    }

    /// The client ended its side of the connection.
    fn ended(&mut self) {
        self.peer_done = true;
        self.link.peer_done();
    }

    /// Reads more octets of the connection into `raw`.
    fn fill(&mut self) -> io::Result<()> {
        let mut chunk = [0u8; 4096];
        let n = self.wire.read(&mut chunk)?;
        if n == 0 {
            self.ended();
        } else {
            self.raw.extend_from_slice(&chunk[..n]);
        }
        Ok(())
    }

    /// Takes the next frame header out of `raw`: starts a data frame or
    /// handles a whole control frame. `false`: `raw` holds too little.
    fn next_frame(&mut self) -> io::Result<bool> {
        let header = match peek_header(&self.raw) {
            Ok(h) => h,
            Err(e) if e.is_incomplete() => return Ok(false),
            Err(e) => {
                return Err(self.fail(e.close_code().to_u16(), &format!("WebSocket frame: {e}")));
            }
        };
        if !header.masked {
            return Err(self.fail(
                PROTOCOL_ERROR,
                "the client sent an unmasked frame (RFC 6455 §5.1)",
            ));
        }
        if header.rsv1 || header.rsv2 || header.rsv3 {
            return Err(self.fail(
                PROTOCOL_ERROR,
                "a frame with an RSV bit set, and no extension was negotiated (RFC 6455 §5.2)",
            ));
        }
        match header.opcode {
            Opcode::Text => Err(self.fail(
                UNSUPPORTED_DATA,
                "a text frame; MQTT travels in binary frames only (MQTT 5.0 §6)",
            )),
            Opcode::Reserved(op) => Err(self.fail(
                PROTOCOL_ERROR,
                &format!("a frame with the reserved opcode {op:#x} (RFC 6455 §5.2)"),
            )),
            Opcode::Binary | Opcode::Continuation => {
                if header.opcode == Opcode::Binary && self.in_message {
                    return Err(self.fail(
                        PROTOCOL_ERROR,
                        "a new binary frame inside an unfinished message (RFC 6455 §5.4)",
                    ));
                }
                if header.opcode == Opcode::Continuation && !self.in_message {
                    return Err(self.fail(
                        PROTOCOL_ERROR,
                        "a continuation frame outside a message (RFC 6455 §5.4)",
                    ));
                }
                if self.raw.len() < header.header_len {
                    return Ok(false);
                }
                let mut key = [0u8; 4];
                key.copy_from_slice(&self.raw[header.header_len - 4..header.header_len]);
                self.raw.drain(..header.header_len);
                self.frame = Some(DataFrame {
                    left: header.payload_len,
                    key,
                    pos: 0,
                    fin: header.fin,
                });
                Ok(true)
            }
            Opcode::Ping | Opcode::Pong | Opcode::Close => {
                let (frame, used) = match decode(&self.raw) {
                    Ok(v) => v,
                    Err(e) if e.is_incomplete() => return Ok(false),
                    Err(e) => {
                        return Err(
                            self.fail(e.close_code().to_u16(), &format!("WebSocket frame: {e}"))
                        );
                    }
                };
                self.raw.drain(..used);
                self.control(&frame)?;
                Ok(true)
            }
        }
    }

    /// A Ping, Pong or Close frame of the client.
    fn control(&mut self, frame: &Frame) -> io::Result<()> {
        match frame.opcode {
            Opcode::Ping => {
                let mut out = self.link.lock();
                if !out.close_sent {
                    out.send(&Frame::pong(frame.payload.clone()))?;
                }
            }
            Opcode::Close => {
                let answer = match parse_close_body(&frame.payload) {
                    Ok(Some((status, _reason))) => Frame::close(status, ""),
                    Ok(None) => Frame {
                        payload: Vec::new(),
                        ..Frame::close(NORMAL, "")
                    },
                    Err(e) => {
                        return Err(self.fail(
                            e.close_code().to_u16(),
                            &format!(
                                "the Close frame of the client is invalid: {e} (RFC 6455 §5.5.1)"
                            ),
                        ));
                    }
                };
                {
                    let mut out = self.link.lock();
                    if !out.close_sent {
                        out.close_sent = true;
                        // The client closes the connection whether the
                        // answer reaches it or not.
                        let _ = out.send(&answer);
                    }
                }
                self.ended();
            }
            // A Pong answers nothing (§5.5.3); data frames do not get here.
            _ => {}
        }
        Ok(())
    }
}

impl<S: ClientStream> Read for WebSocketStream<S> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if self.peer_done {
                return Ok(0);
            }
            if let Some(f) = self.frame.as_mut() {
                if f.left > 0 {
                    let cap = usize::try_from(f.left).unwrap_or(usize::MAX).min(out.len());
                    let n = if self.raw.is_empty() {
                        let n = self.wire.read(&mut out[..cap])?;
                        if n == 0 {
                            self.ended();
                            return Ok(0);
                        }
                        n
                    } else {
                        let n = cap.min(self.raw.len());
                        out[..n].copy_from_slice(&self.raw[..n]);
                        self.raw.drain(..n);
                        n
                    };
                    f.unmask(&mut out[..n]);
                    if f.left == 0 {
                        self.in_message = !f.fin;
                        self.frame = None;
                    }
                    return Ok(n);
                }
                // A frame without payload.
                self.in_message = !f.fin;
                self.frame = None;
                continue;
            }
            if !self.next_frame()? {
                self.fill()?;
            }
        }
    }
}

impl<S: ClientStream> ClientStream for WebSocketStream<S> {
    fn writer(&self) -> io::Result<Box<dyn ClientWriter>> {
        Ok(Box::new(WebSocketWriter {
            link: Arc::clone(&self.link),
            pending: Vec::new(),
        }))
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.wire.set_read_timeout(timeout)
    }
}

impl<S: ClientStream> Drop for WebSocketStream<S> {
    /// A reader that stopped before the client's Close frame came reads it
    /// now, up to the deadline of the server's Close frame (RFC 6455
    /// §7.1.1): the client has then read all the server sent, and the TCP
    /// connection closes without unread octets.
    fn drop(&mut self) {
        let deadline = {
            let out = self.link.lock();
            if self.peer_done || out.peer_done || !out.close_sent {
                return;
            }
            out.close_deadline
                .unwrap_or_else(|| Instant::now() + CLOSE_WAIT)
        };
        let mut sink = [0u8; 4096];
        while let Some(left) = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
        {
            if self.wire.set_read_timeout(Some(left)).is_err() {
                break;
            }
            match self.read(&mut sink) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        self.link.peer_done();
    }
}

/// Closes the TCP connection when the client has ended its side or the
/// deadline of the server's Close frame has passed.
fn close_when_done(link: &Link) {
    let mut out = link.lock();
    loop {
        let left = out
            .close_deadline
            .and_then(|d| d.checked_duration_since(Instant::now()))
            .filter(|d| !d.is_zero());
        let Some(left) = left.filter(|_| !out.peer_done) else {
            break;
        };
        out = link
            .wake
            .wait_timeout(out, left)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
    out.w.close();
}

/// The writing side of a [`WebSocketStream`]: each control packet in one
/// unmasked binary frame.
struct WebSocketWriter {
    link: Arc<Link>,
    /// The packet written since the last flush.
    pending: Vec<u8>,
}

impl Write for WebSocketWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    /// Sends what was written since the last flush as one binary frame.
    fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let frame = Frame::binary(core::mem::take(&mut self.pending));
        let mut out = self.link.lock();
        if out.close_sent {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "the WebSocket connection is closed",
            ));
        }
        out.send(&frame)
    }
}

impl ClientWriter for WebSocketWriter {
    /// The Close frame 1000 and the end of the server's direction of the
    /// connection; a thread of its own closes the TCP connection when the
    /// client's Close frame has come or [`CLOSE_WAIT`] has passed.
    fn close(&mut self) {
        let mut out = self.link.lock();
        out.close_frame(NORMAL, "");
        if out.peer_done {
            out.w.close();
            return;
        }
        out.w.close_write();
        out.close_deadline = Some(Instant::now() + CLOSE_WAIT);
        drop(out);
        let link = Arc::clone(&self.link);
        thread::spawn(move || close_when_done(&link));
    }
}
