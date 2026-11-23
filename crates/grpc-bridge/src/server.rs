// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC-over-HTTP/2 server connection.
//!
//! Connects [`zerodds_http2`] (RFC 9113 framing) + [`zerodds_hpack`] (RFC
//! 7541 compression) + the local gRPC wire codec ([`crate::frame`]) into
//! the server side of one HTTP/2 connection. The connection hands the
//! caller [`CallEvent`]s — a call opened by its HEADERS, each
//! Length-Prefixed-Message of it, the client's half-close, a reset — and
//! takes the caller's answers into per-stream queues that
//! [`GrpcServer::take_output`] writes within the client's flow-control
//! windows.
//!
//! Receiving (`zerodds-grpc-bridge-1.0` §3, §4.1, §4.3; RFC 9113):
//! - a header block in HEADERS and CONTINUATION frames, with the padding
//!   and the priority fields of HEADERS removed (§6.2, §6.10);
//! - several messages in one DATA frame and one message across DATA
//!   frames, without their padding (§6.1); a client stream delivers each
//!   message and its half-close;
//! - the limits of [`ServerLimits`]: SETTINGS, `RST_STREAM(REFUSED_STREAM)`
//!   above `max_concurrent_streams`, `RESOURCE_EXHAUSTED` above
//!   `max_recv_message_size`, the receive windows kept at
//!   `initial_window_size` with WINDOW_UPDATE (a stream window below
//!   65 535 applies once the client acknowledged the SETTINGS, §6.9.3),
//!   frames up to `max_frame_size`, client PINGs without a call
//!   (`keepalive.permit_without_calls`);
//! - frames of an unknown type are ignored (§4.1, §5.5), PRIORITY is read
//!   and ignored (§5.3.2), PUSH_PROMISE from a client is a connection
//!   error (§8.4).
//!
//! Sending: every DATA frame waits for room in the connection window and
//! in the stream's window the client grants (§6.9; SETTINGS
//! `INITIAL_WINDOW_SIZE` changes apply to the open streams, §6.9.2) and
//! stays within the client's `SETTINGS_MAX_FRAME_SIZE`; a header block
//! larger than it continues in CONTINUATION frames. Header blocks are
//! encoded when they go out, so the HPACK context of the client sees them
//! in the order of encoding. A smaller `SETTINGS_HEADER_TABLE_SIZE` of the
//! client is announced in the next header block (RFC 7541 §4.2, §6.3).
//!
//! A connection error (RFC 9113 §5.4.1) queues `GOAWAY` with its error
//! code; the caller writes [`GrpcServer::take_output`] and closes.
//!
//! # Usage skeleton
//!
//! ```ignore
//! let mut server = GrpcServer::with_limits(limits);
//! socket.write_all(&server.handshake())?;
//! let consumed = server.process_frame(&bytes)?;
//! for event in server.take_events() { /* answer */ }
//! socket.write_all(&server.take_output())?;
//! ```

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_hpack::{Decoder as HpackDecoder, Encoder as HpackEncoder, HeaderField};
use zerodds_http2::flow::encode_window_update;
use zerodds_http2::settings::{decode_settings, encode_settings};
use zerodds_http2::{
    ErrorCode, Flags, FlowControl, FrameHeader, FrameType, Setting, SettingId, Settings, StreamId,
    encode_frame,
};

use crate::frame::HEADER_LEN;
use crate::path::parse_path;
use crate::status::Status;

/// The largest header block (HEADERS and its CONTINUATION frames) the
/// server assembles; a larger one is a connection error
/// (`ENHANCE_YOUR_CALM`, RFC 9113 §10.5.1).
pub const MAX_HEADER_BLOCK: usize = 1024 * 1024;

/// The largest decoded header list of a request, in octets as RFC 9113
/// §6.5.2 counts them (name, value and 32 per field): the server's
/// `SETTINGS_MAX_HEADER_LIST_SIZE`, the hard limit of gRPC core's metadata.
/// A larger request is refused with `RESOURCE_EXHAUSTED`; its block is
/// decoded to the end without holding its fields, so the connection's HPACK
/// state stays in step (RFC 9113 §10.5.1).
pub const MAX_HEADER_LIST_SIZE: u32 = 16 * 1024;

/// A call opened by its request HEADERS (gRPC over HTTP/2, "Requests").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcCall {
    /// Stream id.
    pub stream_id: StreamId,
    /// `:path` e.g. `/dds.demo.Trader/PlaceOrder`.
    pub path: String,
    /// Service name (extracted from `:path`).
    pub service: String,
    /// Method name (extracted from `:path`).
    pub method: String,
    /// `:method`.
    pub http_method: String,
    /// `:authority`: the host and port the client addressed.
    pub authority: Option<String>,
    /// `content-type`.
    pub content_type: Option<String>,
    /// `grpc-encoding`: how the request messages are compressed.
    pub encoding: Option<String>,
    /// `grpc-accept-encoding`: the encodings the client takes.
    pub accept_encoding: Option<String>,
    /// `grpc-timeout`, as sent ([`crate::decode_timeout`] reads it).
    pub timeout: Option<String>,
    /// Custom metadata: every header that is not a pseudo-header
    /// (`authorization` for §7.2 among them), in arrival order.
    pub metadata: Vec<(String, String)>,
    /// The HEADERS carried END_STREAM: no request message follows.
    pub end_stream: bool,
}

impl GrpcCall {
    /// gRPC over HTTP/2, "Requests": `content-type` is `application/grpc`,
    /// optionally `+proto` (the only message format of the daemon), with
    /// optional parameters after `;`.
    #[must_use]
    pub fn content_type_is_grpc_proto(&self) -> bool {
        let Some(ct) = &self.content_type else {
            return false;
        };
        let media = ct.split(';').next().unwrap_or("").trim();
        media.eq_ignore_ascii_case("application/grpc")
            || media.eq_ignore_ascii_case("application/grpc+proto")
    }

    /// `content-type` begins with `application/grpc` (a gRPC request,
    /// perhaps of another message format). The octets are compared: a
    /// value with a multi-byte UTF-8 character across the 16th octet is
    /// not a gRPC request.
    #[must_use]
    pub fn content_type_is_grpc(&self) -> bool {
        self.content_type.as_deref().is_some_and(|ct| {
            ct.as_bytes()
                .get(..16)
                .is_some_and(|b| b.eq_ignore_ascii_case(b"application/grpc"))
        })
    }
}

/// What happened on a call (in the order of the frames).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallEvent {
    /// The request HEADERS of a new call.
    Open(GrpcCall),
    /// One Length-Prefixed-Message of the call.
    Message {
        /// Stream id.
        stream_id: StreamId,
        /// The Compressed-Flag was 1 (§4.4).
        compressed: bool,
        /// The message octets, without the 5-octet prefix.
        message: Vec<u8>,
    },
    /// The client ended its side of the stream after its last message.
    HalfClose {
        /// Stream id.
        stream_id: StreamId,
    },
    /// The stream ended without an answer: the client reset it, or the
    /// server did for a stream error (RFC 9113 §5.4.2). The caller drops
    /// the call.
    Reset {
        /// Stream id.
        stream_id: StreamId,
        /// The RST_STREAM error code.
        code: u32,
    },
    /// The server refused the call before a message was whole; the caller
    /// answers with [`GrpcServer::close_call`] and the status.
    Refused(Refusal),
}

/// Outgoing gRPC response of a unary call (helper form of the queue API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcResponse {
    /// Stream id.
    pub stream_id: StreamId,
    /// gRPC status code (0 = OK).
    pub status: Status,
    /// Optional status message (`grpc-message` header).
    pub message: Option<String>,
    /// Response message (without the length prefix); empty is none.
    pub body: Vec<u8>,
}

/// The connection limits of spec §3 `grpc.*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerLimits {
    /// `grpc.max_concurrent_streams`: `SETTINGS_MAX_CONCURRENT_STREAMS`;
    /// a stream above it is refused with `RST_STREAM(REFUSED_STREAM)`.
    pub max_concurrent_streams: u32,
    /// `grpc.max_recv_message_size`: the largest request message in
    /// octets; a larger one ends its call with `RESOURCE_EXHAUSTED`.
    pub max_recv_message_size: u32,
    /// `grpc.max_send_message_size`: the largest response message in
    /// octets; a larger one ends its call with `RESOURCE_EXHAUSTED`.
    pub max_send_message_size: u32,
    /// `grpc.initial_window_size`: `SETTINGS_INITIAL_WINDOW_SIZE`, and the
    /// receive window the server keeps open for the connection.
    pub initial_window_size: u32,
    /// `grpc.max_frame_size`: `SETTINGS_MAX_FRAME_SIZE`, the largest frame
    /// the server accepts.
    pub max_frame_size: u32,
    /// `grpc.keepalive.permit_without_calls`: whether a client may PING
    /// while no call is open. Without it the third PING without a call
    /// after the first (none of the server's HEADERS or DATA in between)
    /// ends the connection with `GOAWAY(ENHANCE_YOUR_CALM)`.
    pub permit_pings_without_calls: bool,
}

impl Default for ServerLimits {
    /// The defaults of spec §3.
    fn default() -> Self {
        Self {
            max_concurrent_streams: 1024,
            max_recv_message_size: 4 * 1024 * 1024,
            max_send_message_size: 4 * 1024 * 1024,
            initial_window_size: 1024 * 1024,
            max_frame_size: 16_384,
            permit_pings_without_calls: false,
        }
    }
}

impl ServerLimits {
    /// RFC 9113 §6.5.2: the ranges of the two settings a value outside
    /// of which no peer accepts.
    ///
    /// # Errors
    /// The name of the setting and why.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(16_384..=16_777_215).contains(&self.max_frame_size) {
            return Err("max_frame_size lies outside 16384..=16777215 (RFC 9113 §6.5.2)");
        }
        if self.initial_window_size > 0x7fff_ffff {
            return Err("initial_window_size lies above 2^31-1 (RFC 9113 §6.5.2)");
        }
        Ok(())
    }
}

/// A call the server refused before a message was whole (spec §3: a
/// message above `max_recv_message_size` is `RESOURCE_EXHAUSTED`; a
/// malformed `:path` is `UNIMPLEMENTED`; a stream that ends inside a
/// message is `INTERNAL`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Stream id.
    pub stream_id: StreamId,
    /// `:path` of the call.
    pub path: String,
    /// The status the call ends with.
    pub status: Status,
    /// The `grpc-message`.
    pub message: String,
}

/// A connection error (RFC 9113 §5.4.1): the server queued
/// `GOAWAY(code)`; the caller writes [`GrpcServer::take_output`] and
/// closes the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionError {
    /// The GOAWAY error code.
    pub code: ErrorCode,
    /// What happened.
    pub reason: &'static str,
}

impl core::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} ({:?})", self.reason, self.code)
    }
}

/// Why a response message was not queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// The message exceeds `max_send_message_size`.
    TooLarge,
    /// The stream is not open for the server's messages (unknown, reset,
    /// or its trailers are queued).
    Closed,
}

/// One item of a stream's output queue.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Out {
    /// A header block, encoded when it goes out.
    Headers {
        fields: Vec<HeaderField>,
        end_stream: bool,
    },
    /// Length-Prefixed-Messages (or the body of a plain HTTP response), of
    /// which `sent` octets went out; with `end` the last frame carries
    /// END_STREAM.
    Data {
        bytes: Vec<u8>,
        sent: usize,
        end: bool,
    },
    /// RST_STREAM with this code.
    Reset(ErrorCode),
}

/// One stream of the connection.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stream {
    /// `:path` (for refusals).
    path: String,
    /// The receive window (RFC 9113 §6.9) and the octets received since
    /// the stream's last WINDOW_UPDATE.
    recv_window: FlowControl,
    unacked: u32,
    /// The octets of a message not yet whole.
    buf: Vec<u8>,
    /// The client ended its side (END_STREAM received).
    client_done: bool,
    /// The server refused the call: further DATA is ignored.
    refused: bool,
    /// The window the client grants the server's DATA on this stream.
    send_window: i64,
    queue: VecDeque<Out>,
    /// The response HEADERS are queued.
    opened: bool,
    /// The trailers or a RST_STREAM are queued.
    server_done: bool,
    /// The RST_STREAM went out.
    reset_sent: bool,
    /// The caller holds the stream's messages ([`GrpcServer::hold_window`]):
    /// its receive window is not returned.
    held: bool,
}

impl Stream {
    /// A stream the client opened with `path`: the receive window
    /// `recv_window`, the window `send_window` the client grants.
    fn new(path: String, recv_window: i64, send_window: u32) -> Self {
        Self {
            path,
            recv_window: FlowControl::new(recv_window),
            unacked: 0,
            buf: Vec::new(),
            client_done: false,
            refused: false,
            send_window: i64::from(send_window),
            queue: VecDeque::new(),
            opened: false,
            server_done: false,
            reset_sent: false,
            held: false,
        }
    }
}

/// The `grpc-message` of a request whose header list exceeds
/// [`MAX_HEADER_LIST_SIZE`].
fn header_list_message() -> String {
    alloc::format!(
        "the request's header list exceeds {MAX_HEADER_LIST_SIZE} octets (SETTINGS_MAX_HEADER_LIST_SIZE)"
    )
}

/// A header block in assembly: HEADERS without END_HEADERS and the
/// CONTINUATION frames after it (RFC 9113 §6.10).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Continuation {
    stream_id: StreamId,
    block: Vec<u8>,
    end_stream: bool,
}

/// The server side of one gRPC connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcServer {
    limits: ServerLimits,
    /// The client's SETTINGS (RFC 9113 §6.5).
    peer: Settings,
    decoder: HpackDecoder,
    encoder: HpackEncoder,
    /// A smaller HPACK table size of the client, announced at the start of
    /// the next header block (RFC 7541 §4.2).
    table_update: Option<usize>,
    streams: BTreeMap<StreamId, Stream>,
    /// Streams the server reset while the client still sends: their DATA
    /// counts against the connection window and is ignored (RFC 9113
    /// §5.1, "closed": frames after a RST_STREAM the endpoint sent).
    discarding: BTreeSet<StreamId>,
    continuation: Option<Continuation>,
    /// The highest stream id the client opened.
    last_client_stream: StreamId,
    /// The connection's receive window and what was received since its
    /// last WINDOW_UPDATE.
    conn_window: FlowControl,
    conn_unacked: u32,
    /// The window the client grants the server's DATA on the connection.
    conn_send_window: i64,
    /// Encoded control frames the peer is owed, in the order they became
    /// due.
    pending_control: Vec<u8>,
    events: Vec<CallEvent>,
    /// A PING without a call arrived since the server last sent HEADERS
    /// or DATA, and the strikes after it.
    idle_ping_seen: bool,
    ping_strikes: u32,
    /// The opaque data of the last PING ACK received.
    last_ping_ack: Option<[u8; 8]>,
    /// A GOAWAY was queued: the caller ends the connection.
    closing: bool,
    /// The graceful GOAWAY of [`GrpcServer::begin_drain`] went out with
    /// this last-stream-id: the open streams are served, a new one is
    /// refused.
    draining: Option<StreamId>,
    /// The server's SETTINGS went out ([`GrpcServer::handshake`]), and the
    /// client acknowledged them: from then on a stream starts with the
    /// announced `initial_window_size` (RFC 9113 §6.9.3).
    settings_sent: bool,
    settings_acked: bool,
    /// Round-robin start of [`GrpcServer::take_output`].
    rotation: usize,
}

impl Default for GrpcServer {
    fn default() -> Self {
        Self::with_limits(ServerLimits::default())
    }
}

/// The receive window of the connection the server keeps open: the
/// stream window it announces, at least the protocol's initial 65 535.
fn connection_target(limits: &ServerLimits) -> u32 {
    limits.initial_window_size.max(65_535)
}

fn conn_err(code: ErrorCode, reason: &'static str) -> ConnectionError {
    ConnectionError { code, reason }
}

impl GrpcServer {
    /// A connection with the defaults of spec §3.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A connection with `limits`.
    #[must_use]
    pub fn with_limits(limits: ServerLimits) -> Self {
        let mut decoder = HpackDecoder::default();
        decoder.set_max_header_list_size(Some(MAX_HEADER_LIST_SIZE as usize));
        Self {
            limits,
            peer: Settings::default(),
            decoder,
            encoder: HpackEncoder::default(),
            table_update: None,
            streams: BTreeMap::new(),
            discarding: BTreeSet::new(),
            continuation: None,
            last_client_stream: 0,
            conn_window: FlowControl::default(),
            conn_unacked: 0,
            conn_send_window: 65_535,
            pending_control: Vec::new(),
            events: Vec::new(),
            idle_ping_seen: false,
            ping_strikes: 0,
            last_ping_ack: None,
            closing: false,
            draining: None,
            settings_sent: false,
            settings_acked: false,
            rotation: 0,
        }
    }

    /// The limits of this connection.
    #[must_use]
    pub fn limits(&self) -> &ServerLimits {
        &self.limits
    }

    /// The server's connection preface (RFC 9113 §3.4): its SETTINGS with
    /// `MAX_CONCURRENT_STREAMS`, `INITIAL_WINDOW_SIZE` and `MAX_FRAME_SIZE`
    /// of the limits and `MAX_HEADER_LIST_SIZE` ([`MAX_HEADER_LIST_SIZE`]),
    /// then a WINDOW_UPDATE that opens the connection window (fixed at
    /// 65 535 by the protocol) to `initial_window_size`.
    #[must_use]
    pub fn handshake(&mut self) -> Vec<u8> {
        let payload = encode_settings(&[
            Setting {
                id: SettingId::MaxConcurrentStreams,
                value: self.limits.max_concurrent_streams,
            },
            Setting {
                id: SettingId::InitialWindowSize,
                value: self.limits.initial_window_size,
            },
            Setting {
                id: SettingId::MaxFrameSize,
                value: self.limits.max_frame_size,
            },
            Setting {
                id: SettingId::MaxHeaderListSize,
                value: MAX_HEADER_LIST_SIZE,
            },
        ]);
        let mut out = encode_one(FrameType::Settings, 0, 0, &payload);
        self.settings_sent = true;
        let target = connection_target(&self.limits);
        if target > 65_535 {
            let increment = target - 65_535;
            out.extend_from_slice(&encode_one(
                FrameType::WindowUpdate,
                0,
                0,
                &encode_window_update(increment),
            ));
            self.conn_window = FlowControl::new(i64::from(target));
        }
        out
    }

    /// Processes the next HTTP/2 frame of `input`: returns the octets it
    /// consumed, 0 while the frame is not whole. The calls it changed are
    /// in [`GrpcServer::take_events`].
    ///
    /// # Errors
    /// A connection error (RFC 9113 §5.4.1); `GOAWAY` with its code is
    /// queued in the output.
    pub fn process_frame(&mut self, input: &[u8]) -> Result<usize, ConnectionError> {
        let result = self.process_inner(input);
        if let Err(e) = result {
            let goaway = self.encode_goaway(e.code, e.reason.as_bytes());
            self.pending_control.extend_from_slice(&goaway);
        }
        result
    }

    fn process_inner(&mut self, input: &[u8]) -> Result<usize, ConnectionError> {
        if input.len() < 9 {
            return Ok(0);
        }
        let length =
            (usize::from(input[0]) << 16) | (usize::from(input[1]) << 8) | usize::from(input[2]);
        if length > self.limits.max_frame_size as usize {
            return Err(conn_err(
                ErrorCode::FrameSizeError,
                "a frame above max_frame_size",
            ));
        }
        if input.len() < 9 + length {
            return Ok(0);
        }
        let consumed = 9 + length;
        let flags = Flags(input[4]);
        let id = u32::from_be_bytes([input[5], input[6], input[7], input[8]]) & 0x7fff_ffff;
        let payload = &input[9..consumed];
        // RFC 9113 §6.10: within a header block only CONTINUATION frames
        // of its stream may follow.
        if let Some(c) = &self.continuation
            && (input[3] != FrameType::Continuation as u8 || id != c.stream_id)
        {
            return Err(conn_err(
                ErrorCode::ProtocolError,
                "a frame inside a header block that is not its CONTINUATION",
            ));
        }
        // RFC 9113 §4.1, §5.5: a frame of an unknown type is ignored.
        let Ok(frame_type) = FrameType::from_u8(input[3]) else {
            return Ok(consumed);
        };
        match frame_type {
            FrameType::Headers => self.handle_headers(id, flags, payload)?,
            FrameType::Continuation => self.handle_continuation(id, flags, payload)?,
            FrameType::Data => self.handle_data(id, flags, payload, length as u32)?,
            FrameType::Settings => self.handle_settings(id, flags, payload)?,
            FrameType::Ping => self.handle_ping(id, flags, payload)?,
            FrameType::WindowUpdate => self.handle_window_update(id, payload)?,
            FrameType::RstStream => self.handle_rst_stream(id, payload)?,
            FrameType::Priority => {
                // RFC 9113 §6.3: read and ignored; a wrong length is a
                // stream error.
                if id == 0 {
                    return Err(conn_err(ErrorCode::ProtocolError, "PRIORITY on stream 0"));
                }
                if length != 5 {
                    self.stream_error(id, ErrorCode::FrameSizeError);
                }
            }
            FrameType::PushPromise => {
                return Err(conn_err(
                    ErrorCode::ProtocolError,
                    "PUSH_PROMISE from a client (RFC 9113 §8.4)",
                ));
            }
            FrameType::GoAway => {
                if id != 0 {
                    return Err(conn_err(ErrorCode::ProtocolError, "GOAWAY on a stream"));
                }
            }
        }
        Ok(consumed)
    }

    /// The events of the frames processed since the last call.
    pub fn take_events(&mut self) -> Vec<CallEvent> {
        core::mem::take(&mut self.events)
    }

    /// The streams open now: calls being received or answered.
    #[must_use]
    pub fn active_streams(&self) -> usize {
        self.streams.len()
    }

    /// Whether the stream is open for the server's messages.
    #[must_use]
    pub fn is_open(&self, stream_id: StreamId) -> bool {
        self.streams.get(&stream_id).is_some_and(|s| !s.server_done)
    }

    /// Whether the client ended its side of the stream.
    #[must_use]
    pub fn client_done(&self, stream_id: StreamId) -> bool {
        self.streams.get(&stream_id).is_some_and(|s| s.client_done)
    }

    /// RFC 9113 §6.9: while `held`, the server returns none of the receive
    /// window of `stream_id`, so the client sends at most what is left of
    /// it; the messages that arrive are still events. The caller holds the
    /// stream while it does not take further messages of it (a reflection
    /// stream whose answers wait for the client's window). Released, the
    /// window used meanwhile goes back once it is half the window.
    pub fn hold_window(&mut self, stream_id: StreamId, held: bool) {
        let window_half = (self.limits.initial_window_size / 2).max(1);
        let Some(s) = self.streams.get_mut(&stream_id) else {
            return;
        };
        s.held = held;
        if held || s.client_done || s.unacked < window_half {
            return;
        }
        let increment = s.unacked;
        s.unacked = 0;
        let _ = s.recv_window.apply_window_update(increment);
        self.push_control(
            FrameType::WindowUpdate,
            0,
            stream_id,
            &encode_window_update(increment),
        );
    }

    /// The octets of messages queued on the stream that have not gone out
    /// (they wait for the client's window).
    #[must_use]
    pub fn queued(&self, stream_id: StreamId) -> usize {
        self.streams.get(&stream_id).map_or(0, |s| {
            s.queue
                .iter()
                .map(|o| match o {
                    Out::Data { bytes, sent, .. } => bytes.len() - sent,
                    _ => 0,
                })
                .sum()
        })
    }

    /// The window the client grants the server on the connection and on
    /// `stream_id` (RFC 9113 §6.9).
    #[must_use]
    pub fn send_windows(&self, stream_id: StreamId) -> (i64, Option<i64>) {
        (
            self.conn_send_window,
            self.streams.get(&stream_id).map(|s| s.send_window),
        )
    }

    /// The highest stream id the client opened (the last-stream-id of a
    /// GOAWAY).
    #[must_use]
    pub fn last_stream_id(&self) -> StreamId {
        self.last_client_stream
    }

    /// A GOAWAY was queued; the caller writes the output and ends the
    /// connection.
    #[must_use]
    pub fn is_closing(&self) -> bool {
        self.closing
    }

    /// Queues a keepalive PING with `opaque` (spec §6 Liveliness, §3
    /// `grpc.keepalive`).
    pub fn keepalive_ping(&mut self, opaque: [u8; 8]) {
        self.push_control(FrameType::Ping, 0, 0, &opaque);
    }

    /// Whether the PING with `opaque` was acknowledged.
    #[must_use]
    pub fn ping_acked(&self, opaque: [u8; 8]) -> bool {
        self.last_ping_ack == Some(opaque)
    }

    /// A GOAWAY with `code` and the last stream the client opened
    /// (RFC 9113 §6.8), encoded; the connection is closing from now on.
    pub fn encode_goaway(&mut self, code: ErrorCode, debug: &[u8]) -> Vec<u8> {
        self.closing = true;
        let mut payload = Vec::with_capacity(8 + debug.len());
        payload.extend_from_slice(&(self.last_client_stream & 0x7fff_ffff).to_be_bytes());
        payload.extend_from_slice(&(code as u32).to_be_bytes());
        payload.extend_from_slice(debug);
        encode_one(FrameType::GoAway, 0, 0, &payload)
    }

    /// The graceful shutdown of RFC 9113 §6.8 (bridge spec §9.2): queues
    /// `GOAWAY(NO_ERROR)` with the last stream the client opened, without
    /// ending the connection. The streams up to it are served as before;
    /// a stream the client opens after it is refused with
    /// `RST_STREAM(REFUSED_STREAM)` (§8.7: not processed, safe to retry
    /// elsewhere). A second call queues nothing.
    pub fn begin_drain(&mut self) {
        if self.draining.is_some() || self.closing {
            return;
        }
        self.draining = Some(self.last_client_stream);
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&(self.last_client_stream & 0x7fff_ffff).to_be_bytes());
        payload.extend_from_slice(&(ErrorCode::NoError as u32).to_be_bytes());
        let frame = encode_one(FrameType::GoAway, 0, 0, &payload);
        self.pending_control.extend_from_slice(&frame);
    }

    /// The last-stream-id of the graceful GOAWAY, once
    /// [`Self::begin_drain`] sent it.
    #[must_use]
    pub fn draining(&self) -> Option<StreamId> {
        self.draining
    }

    /// Queues a GOAWAY behind the frames already due (RFC 9113 §6.8).
    pub fn goaway(&mut self, code: ErrorCode, debug: &[u8]) {
        let frame = self.encode_goaway(code, debug);
        self.pending_control.extend_from_slice(&frame);
    }

    // ------------------------------------------------------------------
    // Receiving
    // ------------------------------------------------------------------

    /// RFC 9113 §6.5.3 — a SETTINGS without ACK is applied to the
    /// client's settings and answered by an empty SETTINGS with ACK.
    ///
    /// The ACK of the server's own SETTINGS applies a stream window below
    /// the protocol's 65 535 to the open streams (RFC 9113 §6.9.2): until
    /// then the client may send up to 65 535 on a stream (§6.9.3).
    fn handle_settings(
        &mut self,
        id: StreamId,
        flags: Flags,
        payload: &[u8],
    ) -> Result<(), ConnectionError> {
        if id != 0 {
            return Err(conn_err(ErrorCode::ProtocolError, "SETTINGS on a stream"));
        }
        if flags.has(Flags::ACK) {
            if !payload.is_empty() {
                return Err(conn_err(
                    ErrorCode::FrameSizeError,
                    "SETTINGS ACK with a payload",
                ));
            }
            if self.settings_sent && !self.settings_acked {
                let before = self.stream_initial_window();
                self.settings_acked = true;
                let after = self.stream_initial_window();
                for s in self.streams.values_mut() {
                    s.recv_window
                        .apply_initial_window_size_change(before, after);
                }
            }
            return Ok(());
        }
        let settings = decode_settings(payload)
            .map_err(|_| conn_err(ErrorCode::FrameSizeError, "SETTINGS length"))?;
        for s in settings {
            let old_window = i64::from(self.peer.initial_window_size);
            self.peer.apply(s).map_err(|_| {
                if s.id == SettingId::InitialWindowSize {
                    conn_err(
                        ErrorCode::FlowControlError,
                        "SETTINGS_INITIAL_WINDOW_SIZE above 2^31-1",
                    )
                } else {
                    conn_err(
                        ErrorCode::ProtocolError,
                        "a SETTINGS value outside its range",
                    )
                }
            })?;
            match s.id {
                SettingId::InitialWindowSize => {
                    // §6.9.2: the change applies to every open stream.
                    let delta = i64::from(self.peer.initial_window_size) - old_window;
                    for st in self.streams.values_mut() {
                        st.send_window += delta;
                        if st.send_window > 0x7fff_ffff {
                            return Err(conn_err(
                                ErrorCode::FlowControlError,
                                "a stream window above 2^31-1 after SETTINGS",
                            ));
                        }
                    }
                }
                SettingId::HeaderTableSize => {
                    let size = (s.value as usize).min(4096);
                    if size != self.encoder.table().max_size() {
                        self.encoder = HpackEncoder::with_max_size(size);
                        self.table_update = Some(size);
                    }
                }
                _ => {}
            }
        }
        self.push_control(FrameType::Settings, Flags::ACK, 0, &[]);
        Ok(())
    }

    /// RFC 9113 §6.7 — a PING without ACK is answered by a PING with ACK
    /// carrying the same eight opaque octets; an ACK is recorded for
    /// [`GrpcServer::ping_acked`]. Without `permit_pings_without_calls` a
    /// PING while no call is open is allowed once until the server sends
    /// HEADERS or DATA again; each further one is a strike, and the third
    /// strike is `GOAWAY(ENHANCE_YOUR_CALM, "too_many_pings")`, as the
    /// keepalive enforcement of grpc-go does.
    fn handle_ping(
        &mut self,
        id: StreamId,
        flags: Flags,
        payload: &[u8],
    ) -> Result<(), ConnectionError> {
        if id != 0 {
            return Err(conn_err(ErrorCode::ProtocolError, "PING on a stream"));
        }
        let Ok(opaque) = <[u8; 8]>::try_from(payload) else {
            return Err(conn_err(
                ErrorCode::FrameSizeError,
                "PING of another length than 8",
            ));
        };
        if flags.has(Flags::ACK) {
            self.last_ping_ack = Some(opaque);
            return Ok(());
        }
        if !self.limits.permit_pings_without_calls && self.active_streams() == 0 {
            if self.idle_ping_seen {
                self.ping_strikes += 1;
            }
            self.idle_ping_seen = true;
            if self.ping_strikes > 2 {
                self.goaway(ErrorCode::EnhanceYourCalm, b"too_many_pings");
                return Ok(());
            }
        }
        self.push_control(FrameType::Ping, Flags::ACK, 0, payload);
        Ok(())
    }

    /// RFC 9113 §6.9: the client grants the server's DATA more room.
    fn handle_window_update(
        &mut self,
        id: StreamId,
        payload: &[u8],
    ) -> Result<(), ConnectionError> {
        let Ok(raw) = <[u8; 4]>::try_from(payload) else {
            return Err(conn_err(
                ErrorCode::FrameSizeError,
                "WINDOW_UPDATE of another length than 4",
            ));
        };
        let increment = i64::from(u32::from_be_bytes(raw) & 0x7fff_ffff);
        if id == 0 {
            if increment == 0 {
                return Err(conn_err(ErrorCode::ProtocolError, "WINDOW_UPDATE of 0"));
            }
            self.conn_send_window += increment;
            if self.conn_send_window > 0x7fff_ffff {
                return Err(conn_err(
                    ErrorCode::FlowControlError,
                    "the connection window above 2^31-1",
                ));
            }
            return Ok(());
        }
        let Some(s) = self.streams.get_mut(&id) else {
            // A stream that ended (RFC 9113 §6.9: allowed on a closed
            // stream) or one never opened (a connection error, §5.1).
            if id > self.last_client_stream {
                return Err(conn_err(
                    ErrorCode::ProtocolError,
                    "WINDOW_UPDATE on an idle stream",
                ));
            }
            return Ok(());
        };
        if increment == 0 {
            self.stream_error(id, ErrorCode::ProtocolError);
            return Ok(());
        }
        s.send_window += increment;
        if s.send_window > 0x7fff_ffff {
            self.stream_error(id, ErrorCode::FlowControlError);
        }
        Ok(())
    }

    fn handle_rst_stream(&mut self, id: StreamId, payload: &[u8]) -> Result<(), ConnectionError> {
        let Ok(raw) = <[u8; 4]>::try_from(payload) else {
            return Err(conn_err(
                ErrorCode::FrameSizeError,
                "RST_STREAM of another length than 4",
            ));
        };
        if id == 0 || id > self.last_client_stream {
            return Err(conn_err(
                ErrorCode::ProtocolError,
                "RST_STREAM on stream 0 or an idle stream",
            ));
        }
        self.discarding.remove(&id);
        if self.streams.remove(&id).is_some() {
            self.events.push(CallEvent::Reset {
                stream_id: id,
                code: u32::from_be_bytes(raw),
            });
        }
        Ok(())
    }

    /// RFC 9113 §6.2: the header block of a HEADERS frame without its
    /// padding and priority fields; without END_HEADERS it continues in
    /// CONTINUATION frames.
    fn handle_headers(
        &mut self,
        id: StreamId,
        flags: Flags,
        payload: &[u8],
    ) -> Result<(), ConnectionError> {
        if id == 0 {
            return Err(conn_err(ErrorCode::ProtocolError, "HEADERS on stream 0"));
        }
        let mut block = payload;
        let mut pad = 0usize;
        if flags.has(Flags::PADDED) {
            let (&p, rest) = block.split_first().ok_or(conn_err(
                ErrorCode::FrameSizeError,
                "PADDED HEADERS without a pad length",
            ))?;
            pad = usize::from(p);
            block = rest;
        }
        if flags.has(Flags::PRIORITY) {
            if block.len() < 5 {
                return Err(conn_err(
                    ErrorCode::FrameSizeError,
                    "HEADERS shorter than its priority fields",
                ));
            }
            block = &block[5..];
        }
        let block = block
            .len()
            .checked_sub(pad)
            .map(|end| &block[..end])
            .ok_or(conn_err(
                ErrorCode::ProtocolError,
                "padding longer than the HEADERS frame",
            ))?;
        if block.len() > MAX_HEADER_BLOCK {
            return Err(conn_err(
                ErrorCode::EnhanceYourCalm,
                "a header block above 1 MiB",
            ));
        }
        let end_stream = flags.has(Flags::END_STREAM);
        if !flags.has(Flags::END_HEADERS) {
            self.continuation = Some(Continuation {
                stream_id: id,
                block: block.to_vec(),
                end_stream,
            });
            return Ok(());
        }
        self.header_block(id, block, end_stream)
    }

    fn handle_continuation(
        &mut self,
        id: StreamId,
        flags: Flags,
        payload: &[u8],
    ) -> Result<(), ConnectionError> {
        let Some(mut c) = self.continuation.take() else {
            return Err(conn_err(
                ErrorCode::ProtocolError,
                "CONTINUATION without a header block",
            ));
        };
        if c.block.len() + payload.len() > MAX_HEADER_BLOCK {
            return Err(conn_err(
                ErrorCode::EnhanceYourCalm,
                "a header block above 1 MiB",
            ));
        }
        c.block.extend_from_slice(payload);
        if !flags.has(Flags::END_HEADERS) {
            self.continuation = Some(c);
            return Ok(());
        }
        self.header_block(id, &c.block, c.end_stream)
    }

    /// A whole header block of stream `id`.
    fn header_block(
        &mut self,
        id: StreamId,
        block: &[u8],
        end_stream: bool,
    ) -> Result<(), ConnectionError> {
        // The HPACK block is decoded in every case: the decoder's dynamic
        // table lives as long as the connection (RFC 7541 §2.3.2). A list
        // above MAX_HEADER_LIST_SIZE was decoded to its end without its
        // fields; another decoding error is COMPRESSION_ERROR (RFC 9113
        // §4.3), a dynamic table size update above the 4 096 of the
        // server's SETTINGS_HEADER_TABLE_SIZE among them (RFC 7541 §6.3).
        let (headers, too_large) = match self.decoder.decode(block) {
            Ok(headers) => (headers, false),
            Err(zerodds_hpack::DecoderError::HeaderListTooLarge { .. }) => (Vec::new(), true),
            Err(_) => return Err(conn_err(ErrorCode::CompressionError, "HPACK decode")),
        };
        if self.discarding.contains(&id) {
            if end_stream {
                self.discarding.remove(&id);
            }
            return Ok(());
        }
        if let Some(s) = self.streams.get_mut(&id) {
            // The client's trailers end its side (gRPC sends none; a
            // HEADERS without END_STREAM here is malformed, RFC 9113 §8.1).
            if !end_stream || s.client_done {
                self.stream_error(id, ErrorCode::ProtocolError);
                return Ok(());
            }
            if too_large && !s.refused {
                self.refuse(id, Status::ResourceExhausted, header_list_message());
            }
            self.client_ended(id);
            return Ok(());
        }
        if id <= self.last_client_stream || id.is_multiple_of(2) {
            if id <= self.last_client_stream {
                // HEADERS on a stream that ended: a stream error (§5.1).
                self.push_control(
                    FrameType::RstStream,
                    0,
                    id,
                    &(ErrorCode::StreamClosed as u32).to_be_bytes(),
                );
                return Ok(());
            }
            return Err(conn_err(
                ErrorCode::ProtocolError,
                "HEADERS opening an even stream id",
            ));
        }
        self.last_client_stream = id;
        // RFC 9113 §5.1.2: a stream above SETTINGS_MAX_CONCURRENT_STREAMS
        // is refused; the client may retry it. RFC 9113 §6.8: so is a
        // stream opened after the graceful GOAWAY.
        if self.draining.is_some()
            || self.active_streams() >= self.limits.max_concurrent_streams as usize
        {
            self.push_control(
                FrameType::RstStream,
                0,
                id,
                &(ErrorCode::RefusedStream as u32).to_be_bytes(),
            );
            if !end_stream {
                self.discarding.insert(id);
            }
            return Ok(());
        }
        if too_large {
            // The request is answered without its headers: `:path` is
            // unknown.
            self.streams.insert(
                id,
                Stream::new(
                    String::new(),
                    self.stream_initial_window(),
                    self.peer.initial_window_size,
                ),
            );
            self.refuse(id, Status::ResourceExhausted, header_list_message());
            if end_stream && let Some(s) = self.streams.get_mut(&id) {
                s.client_done = true;
            }
            return Ok(());
        }
        // RFC 9113 §8.2.1, §8.3: lowercase names, pseudo-headers first,
        // `:method` and `:path` present; otherwise a malformed request.
        let mut regular_seen = false;
        let mut malformed = false;
        for h in &headers {
            if h.name.bytes().any(|b| b.is_ascii_uppercase()) {
                malformed = true;
            }
            if h.name.starts_with(':') {
                malformed |= regular_seen;
            } else {
                regular_seen = true;
            }
        }
        let http_method = header_value(&headers, ":method");
        let path = header_value(&headers, ":path");
        let (Some(http_method), Some(path), false) = (http_method, path, malformed) else {
            self.push_control(
                FrameType::RstStream,
                0,
                id,
                &(ErrorCode::ProtocolError as u32).to_be_bytes(),
            );
            if !end_stream {
                self.discarding.insert(id);
            }
            return Ok(());
        };
        self.streams.insert(
            id,
            Stream::new(
                path.clone(),
                self.stream_initial_window(),
                self.peer.initial_window_size,
            ),
        );
        // A GET is no gRPC call; its path is the caller's (spec §5.2: the
        // IDL of a topic at `/idl/<dds_name>`).
        let parsed = if http_method == "GET" {
            Ok(parse_path(&path).unwrap_or_default())
        } else {
            parse_path(&path)
        };
        let Ok((service, method)) = parsed else {
            // grpc-go: "malformed method name", UNIMPLEMENTED.
            self.refuse(
                id,
                Status::Unimplemented,
                alloc::format!("malformed method name {path:?}"),
            );
            if end_stream && let Some(s) = self.streams.get_mut(&id) {
                s.client_done = true;
            }
            return Ok(());
        };
        self.events.push(CallEvent::Open(GrpcCall {
            stream_id: id,
            path,
            service,
            method,
            http_method,
            authority: header_value(&headers, ":authority"),
            content_type: header_value(&headers, "content-type"),
            encoding: header_value(&headers, "grpc-encoding"),
            accept_encoding: header_value(&headers, "grpc-accept-encoding"),
            timeout: header_value(&headers, "grpc-timeout"),
            metadata: headers
                .iter()
                .filter(|h| !h.name.starts_with(':'))
                .map(|h| (h.name.clone(), h.value.clone()))
                .collect(),
            end_stream,
        }));
        if end_stream {
            self.client_ended(id);
        }
        Ok(())
    }

    /// The receive window a new stream starts with: `initial_window_size`
    /// once the client acknowledged the server's SETTINGS, before that at
    /// least the 65 535 the client assumes (RFC 9113 §6.9.3).
    fn stream_initial_window(&self) -> i64 {
        let announced = i64::from(self.limits.initial_window_size);
        if self.settings_acked {
            announced
        } else {
            announced.max(65_535)
        }
    }

    fn handle_data(
        &mut self,
        id: StreamId,
        flags: Flags,
        payload: &[u8],
        len: u32,
    ) -> Result<(), ConnectionError> {
        if id == 0 {
            return Err(conn_err(ErrorCode::ProtocolError, "DATA on stream 0"));
        }
        let end_stream = flags.has(Flags::END_STREAM);
        // RFC 9113 §6.9.1: the whole frame, padding included, counts
        // against the connection window.
        self.conn_window.consume(len).map_err(|_| {
            conn_err(
                ErrorCode::FlowControlError,
                "DATA beyond the connection window",
            )
        })?;
        self.conn_unacked += len;
        let target = connection_target(&self.limits);
        if self.conn_unacked >= target / 2 {
            let increment = self.conn_unacked;
            self.conn_unacked = 0;
            let _ = self.conn_window.apply_window_update(increment);
            self.push_control(
                FrameType::WindowUpdate,
                0,
                0,
                &encode_window_update(increment),
            );
        }
        let data = unpadded(flags, payload)?;
        if self.discarding.contains(&id) {
            if end_stream {
                self.discarding.remove(&id);
            }
            return Ok(());
        }
        let window_half = (self.limits.initial_window_size / 2).max(1);
        let Some(s) = self.streams.get_mut(&id) else {
            if id > self.last_client_stream {
                return Err(conn_err(ErrorCode::ProtocolError, "DATA on an idle stream"));
            }
            // RFC 9113 §5.1: DATA on a closed stream is a stream error.
            self.push_control(
                FrameType::RstStream,
                0,
                id,
                &(ErrorCode::StreamClosed as u32).to_be_bytes(),
            );
            return Ok(());
        };
        if s.client_done {
            // "half-closed (remote)": STREAM_CLOSED (RFC 9113 §5.1).
            self.stream_error(id, ErrorCode::StreamClosed);
            return Ok(());
        }
        if s.recv_window.consume(len).is_err() {
            // A stream error (RFC 9113 §6.9.1).
            self.stream_error(id, ErrorCode::FlowControlError);
            return Ok(());
        }
        s.unacked += len;
        let stream_update = if !end_stream && !s.held && s.unacked >= window_half {
            let increment = s.unacked;
            s.unacked = 0;
            let _ = s.recv_window.apply_window_update(increment);
            Some(increment)
        } else {
            None
        };
        if !s.refused && !s.server_done {
            s.buf.extend_from_slice(data);
        }
        if let Some(increment) = stream_update {
            self.push_control(
                FrameType::WindowUpdate,
                0,
                id,
                &encode_window_update(increment),
            );
        }
        self.drain_messages(id);
        if end_stream {
            self.client_ended(id);
        }
        Ok(())
    }

    /// Every whole Length-Prefixed-Message in the buffer of `id` as an
    /// event; spec §3 `max_recv_message_size` on the length a prefix
    /// declares, before the message's octets are held.
    fn drain_messages(&mut self, id: StreamId) {
        let max_recv = self.limits.max_recv_message_size;
        loop {
            let Some(s) = self.streams.get_mut(&id) else {
                return;
            };
            if s.refused || s.buf.len() < HEADER_LEN {
                return;
            }
            let declared = u32::from_be_bytes([s.buf[1], s.buf[2], s.buf[3], s.buf[4]]);
            if declared > max_recv {
                s.buf = Vec::new();
                self.refuse(
                    id,
                    Status::ResourceExhausted,
                    alloc::format!(
                        "request message of {declared} octets exceeds max_recv_message_size {max_recv}"
                    ),
                );
                return;
            }
            let total = HEADER_LEN + declared as usize;
            if s.buf.len() < total {
                return;
            }
            let compressed = s.buf[0] & 1 == 1;
            let message = s.buf[HEADER_LEN..total].to_vec();
            s.buf.drain(..total);
            self.events.push(CallEvent::Message {
                stream_id: id,
                compressed,
                message,
            });
        }
    }

    /// The client ended its side of `id`.
    fn client_ended(&mut self, id: StreamId) {
        let Some(s) = self.streams.get_mut(&id) else {
            return;
        };
        s.client_done = true;
        // A reset after the trailers is no longer needed (RFC 9113 §8.1).
        if matches!(s.queue.back(), Some(Out::Reset(ErrorCode::NoError))) {
            s.queue.pop_back();
        }
        let partial = !s.buf.is_empty() && !s.refused;
        let refused = s.refused;
        s.buf = Vec::new();
        if partial {
            self.refuse(
                id,
                Status::Internal,
                "the client ended the stream inside a message".into(),
            );
        } else if !refused {
            self.events.push(CallEvent::HalfClose { stream_id: id });
        }
        self.reap(id);
    }

    /// The server refuses the call `id` before its answer: an event for
    /// the caller, and no further messages of it.
    fn refuse(&mut self, id: StreamId, status: Status, message: String) {
        let Some(s) = self.streams.get_mut(&id) else {
            return;
        };
        s.refused = true;
        self.events.push(CallEvent::Refused(Refusal {
            stream_id: id,
            path: s.path.clone(),
            status,
            message,
        }));
    }

    /// A stream error (RFC 9113 §5.4.2): RST_STREAM with `code`, the call
    /// ends without an answer.
    fn stream_error(&mut self, id: StreamId, code: ErrorCode) {
        let client_done = match self.streams.remove(&id) {
            Some(s) => {
                self.events.push(CallEvent::Reset {
                    stream_id: id,
                    code: code as u32,
                });
                s.client_done
            }
            None => true,
        };
        if !client_done {
            self.discarding.insert(id);
        }
        self.push_control(FrameType::RstStream, 0, id, &(code as u32).to_be_bytes());
    }

    // ------------------------------------------------------------------
    // Answering
    // ------------------------------------------------------------------

    /// Queues the response HEADERS of a call (`:status 200`, `content-type
    /// application/grpc`, then `extra`, e.g. `grpc-encoding`): the start of
    /// the answer (gRPC over HTTP/2, "Responses"). A second call does
    /// nothing.
    pub fn open_response(&mut self, stream_id: StreamId, extra: &[(&str, &str)]) {
        let Some(s) = self.streams.get_mut(&stream_id) else {
            return;
        };
        if s.opened || s.server_done {
            return;
        }
        let mut fields = alloc::vec![
            field(":status", "200"),
            field("content-type", "application/grpc"),
        ];
        fields.extend(extra.iter().map(|(n, v)| field(n, v)));
        s.queue.push_back(Out::Headers {
            fields,
            end_stream: false,
        });
        s.opened = true;
    }

    /// Queues one response message of an open call: its
    /// Length-Prefixed-Message with the Compressed-Flag `compressed` (the
    /// octets of `message` are compressed already). Opens the response
    /// first when it is not.
    ///
    /// # Errors
    /// [`SendError::TooLarge`] above `max_send_message_size`,
    /// [`SendError::Closed`] when the stream takes no more messages.
    pub fn send_message(
        &mut self,
        stream_id: StreamId,
        message: &[u8],
        compressed: bool,
    ) -> Result<(), SendError> {
        if !self.fits_send(message.len()) {
            return Err(SendError::TooLarge);
        }
        if !self.is_open(stream_id) {
            return Err(SendError::Closed);
        }
        self.open_response(stream_id, &[]);
        let mut lpm = Vec::with_capacity(HEADER_LEN + message.len());
        lpm.push(u8::from(compressed));
        lpm.extend_from_slice(&(message.len() as u32).to_be_bytes());
        lpm.extend_from_slice(message);
        if let Some(s) = self.streams.get_mut(&stream_id) {
            s.queue.push_back(Out::Data {
                bytes: lpm,
                sent: 0,
                end: false,
            });
        }
        Ok(())
    }

    /// Ends the call with `status` (and `grpc-message`) in the trailers,
    /// after its queued messages; opens the response first when it is not.
    /// When the client has not ended its side, `RST_STREAM(NO_ERROR)`
    /// follows the trailers (RFC 9113 §8.1) and its further DATA is
    /// ignored. Nothing for a stream that is not open.
    pub fn close_call(&mut self, stream_id: StreamId, status: Status, message: Option<&str>) {
        self.close_call_with(stream_id, status, message, &[]);
    }

    /// [`Self::close_call`] with further trailers after `grpc-status` and
    /// `grpc-message` (custom metadata, spec §6: `zerodds-deadline-missed`).
    pub fn close_call_with(
        &mut self,
        stream_id: StreamId,
        status: Status,
        message: Option<&str>,
        extra: &[(&str, &str)],
    ) {
        if !self.is_open(stream_id) {
            return;
        }
        self.open_response(stream_id, &[]);
        let mut trailers = alloc::vec![field("grpc-status", &alloc::format!("{}", status.code()))];
        if let Some(msg) = message {
            trailers.push(field("grpc-message", &percent_encode(msg)));
        }
        trailers.extend(extra.iter().map(|(n, v)| field(n, v)));
        if let Some(s) = self.streams.get_mut(&stream_id) {
            s.queue.push_back(Out::Headers {
                fields: trailers,
                end_stream: true,
            });
            if !s.client_done {
                s.queue.push_back(Out::Reset(ErrorCode::NoError));
            }
            s.server_done = true;
        }
    }

    /// Answers a request that is not a gRPC request with the HTTP status
    /// `http_status` (gRPC over HTTP/2: a `content-type` that does not
    /// begin with `application/grpc` SHOULD get 415) and, for a gRPC
    /// client, `grpc-status` in the same HEADERS frame, which ends the
    /// stream.
    pub fn reject(&mut self, stream_id: StreamId, http_status: u16, status: Status, message: &str) {
        let Some(s) = self.streams.get_mut(&stream_id) else {
            return;
        };
        if s.opened || s.server_done {
            return;
        }
        s.queue.push_back(Out::Headers {
            fields: alloc::vec![
                field(":status", &alloc::format!("{http_status}")),
                field("content-type", "application/grpc"),
                field("grpc-status", &alloc::format!("{}", status.code())),
                field("grpc-message", &percent_encode(message)),
            ],
            end_stream: true,
        });
        if !s.client_done {
            s.queue.push_back(Out::Reset(ErrorCode::NoError));
        }
        s.opened = true;
        s.server_done = true;
    }

    /// Answers a request that is not a gRPC call (spec §5.2, an HTTP GET)
    /// with a plain HTTP response: `:status`, `content-type`,
    /// `content-length` and `body` in DATA frames, the last with
    /// END_STREAM. When the client has not ended its side,
    /// `RST_STREAM(NO_ERROR)` follows (RFC 9113 §8.1).
    pub fn respond_http(
        &mut self,
        stream_id: StreamId,
        http_status: u16,
        content_type: &str,
        body: &[u8],
    ) {
        let Some(s) = self.streams.get_mut(&stream_id) else {
            return;
        };
        if s.opened || s.server_done {
            return;
        }
        s.queue.push_back(Out::Headers {
            fields: alloc::vec![
                field(":status", &alloc::format!("{http_status}")),
                field("content-type", content_type),
                field("content-length", &alloc::format!("{}", body.len())),
            ],
            end_stream: body.is_empty(),
        });
        if !body.is_empty() {
            s.queue.push_back(Out::Data {
                bytes: body.to_vec(),
                sent: 0,
                end: true,
            });
        }
        if !s.client_done {
            s.queue.push_back(Out::Reset(ErrorCode::NoError));
        }
        s.opened = true;
        s.server_done = true;
    }

    /// Queues a whole unary answer: response HEADERS, the message (when
    /// `resp.body` is not empty), the trailers; a message above
    /// `max_send_message_size` becomes `RESOURCE_EXHAUSTED` (spec §3).
    pub fn respond(&mut self, resp: &GrpcResponse) {
        let resp = self.fit_response(resp.clone());
        if !resp.body.is_empty() {
            let _ = self.send_message(resp.stream_id, &resp.body, false);
        }
        self.close_call(resp.stream_id, resp.status, resp.message.as_deref());
    }

    /// Resets a stream with `code` (RFC 9113 §6.4) and drops what was
    /// queued on it.
    pub fn reset_stream(&mut self, stream_id: StreamId, code: ErrorCode) {
        let Some(s) = self.streams.get_mut(&stream_id) else {
            return;
        };
        s.queue.clear();
        s.queue.push_back(Out::Reset(code));
        s.server_done = true;
    }

    /// Whether a response message of `len` octets may be sent (spec §3
    /// `max_send_message_size`).
    #[must_use]
    pub fn fits_send(&self, len: usize) -> bool {
        len <= self.limits.max_send_message_size as usize
    }

    /// `resp`, or — when its message exceeds `max_send_message_size` — the
    /// `RESOURCE_EXHAUSTED` the call ends with instead (spec §3, §4.5).
    #[must_use]
    pub fn fit_response(&self, resp: GrpcResponse) -> GrpcResponse {
        if self.fits_send(resp.body.len()) {
            return resp;
        }
        GrpcResponse {
            stream_id: resp.stream_id,
            status: Status::ResourceExhausted,
            message: Some(alloc::format!(
                "response message of {} octets exceeds max_send_message_size {}",
                resp.body.len(),
                self.limits.max_send_message_size
            )),
            body: Vec::new(),
        }
    }

    /// The frames that may go out now: the control frames, then each
    /// stream's queue as far as the client's windows allow (DATA in frames
    /// within the client's `SETTINGS_MAX_FRAME_SIZE`; a header block in
    /// HEADERS and CONTINUATION frames).
    pub fn take_output(&mut self) -> Vec<u8> {
        let mut out = core::mem::take(&mut self.pending_control);
        let mut ids: Vec<StreamId> = self.streams.keys().copied().collect();
        if !ids.is_empty() {
            let start = self.rotation % ids.len();
            ids.rotate_left(start);
            self.rotation = self.rotation.wrapping_add(1);
        }
        let max_frame = self.peer.max_frame_size as usize;
        for id in ids {
            while let Some(s) = self.streams.get_mut(&id) {
                let Some(front) = s.queue.front_mut() else {
                    break;
                };
                match front {
                    Out::Headers { .. } => {
                        let Some(Out::Headers { fields, end_stream }) = s.queue.pop_front() else {
                            break;
                        };
                        let mut block = Vec::new();
                        if let Some(size) = self.table_update.take() {
                            // RFC 7541 §6.3: to 0, then to the new size.
                            block.push(0x20);
                            block.extend_from_slice(&zerodds_hpack::encode_integer(
                                size as u64,
                                5,
                                0x20,
                            ));
                        }
                        block.extend_from_slice(&self.encoder.encode(&fields));
                        encode_header_block(&mut out, id, &block, end_stream, max_frame);
                        self.idle_ping_seen = false;
                        self.ping_strikes = 0;
                    }
                    Out::Data { bytes, sent, end } => {
                        let left = bytes.len() - *sent;
                        let room = s.send_window.min(self.conn_send_window);
                        if left > 0 && room <= 0 {
                            break;
                        }
                        let n = left.min(max_frame).min(usize::try_from(room).unwrap_or(0));
                        let n = if left == 0 { 0 } else { n };
                        let flags = if *end && *sent + n == bytes.len() {
                            Flags::END_STREAM
                        } else {
                            0
                        };
                        out.extend_from_slice(&encode_raw(
                            FrameType::Data,
                            flags,
                            id,
                            &bytes[*sent..*sent + n],
                        ));
                        *sent += n;
                        s.send_window -= n as i64;
                        self.conn_send_window -= n as i64;
                        if *sent == bytes.len() {
                            s.queue.pop_front();
                        }
                        self.idle_ping_seen = false;
                        self.ping_strikes = 0;
                    }
                    Out::Reset(code) => {
                        let code = *code;
                        s.queue.pop_front();
                        out.extend_from_slice(&encode_one(
                            FrameType::RstStream,
                            0,
                            id,
                            &(code as u32).to_be_bytes(),
                        ));
                        s.reset_sent = true;
                    }
                }
            }
            self.reap(id);
        }
        out
    }

    /// Removes a stream both sides are done with.
    fn reap(&mut self, id: StreamId) {
        let Some(s) = self.streams.get(&id) else {
            return;
        };
        if !(s.server_done && s.queue.is_empty()) {
            return;
        }
        if s.client_done {
            self.streams.remove(&id);
        } else if s.reset_sent {
            self.streams.remove(&id);
            self.discarding.insert(id);
        }
    }

    /// Encodes one control frame and queues it.
    fn push_control(&mut self, frame_type: FrameType, flags: u8, stream_id: u32, payload: &[u8]) {
        self.pending_control
            .extend_from_slice(&encode_one(frame_type, flags, stream_id, payload));
    }
}

fn field(name: &str, value: &str) -> HeaderField {
    HeaderField {
        name: name.into(),
        value: value.into(),
    }
}

/// gRPC over HTTP/2, "Responses": `grpc-message` is percent-encoded —
/// every octet outside `%x20-%x24 / %x26-%x7E` as `%XX` of its UTF-8.
#[must_use]
pub fn percent_encode(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    for b in message.bytes() {
        if (0x20..=0x7e).contains(&b) && b != b'%' {
            out.push(char::from(b));
        } else {
            out.push_str(&alloc::format!("%{b:02X}"));
        }
    }
    out
}

/// A header block in a HEADERS frame and, when larger than `max_frame`, in
/// CONTINUATION frames after it (RFC 9113 §6.2, §6.10).
fn encode_header_block(
    out: &mut Vec<u8>,
    id: StreamId,
    block: &[u8],
    end_stream: bool,
    max_frame: usize,
) {
    let mut chunks = block.chunks(max_frame.max(1)).peekable();
    let first = chunks.next().unwrap_or(&[]);
    let mut flags = if end_stream { Flags::END_STREAM } else { 0 };
    if chunks.peek().is_none() {
        flags |= Flags::END_HEADERS;
    }
    out.extend_from_slice(&encode_raw(FrameType::Headers, flags, id, first));
    while let Some(chunk) = chunks.next() {
        let flags = if chunks.peek().is_none() {
            Flags::END_HEADERS
        } else {
            0
        };
        out.extend_from_slice(&encode_raw(FrameType::Continuation, flags, id, chunk));
    }
}

/// A frame of any payload length up to 2^24-1, encoded.
fn encode_raw(frame_type: FrameType, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
    let header = FrameHeader {
        length: u32::try_from(payload.len()).unwrap_or(0),
        frame_type,
        flags: Flags(flags),
        stream_id,
    };
    let mut out = alloc::vec![0u8; 9 + payload.len()];
    match encode_frame(&header, payload, &mut out, 16_777_215) {
        Ok(_) => out,
        Err(_) => Vec::new(),
    }
}

/// A frame of at most 16 384 octets of payload (the size every peer
/// accepts), encoded.
fn encode_one(frame_type: FrameType, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
    if payload.len() > 16_384 {
        return Vec::new();
    }
    encode_raw(frame_type, flags, stream_id, payload)
}

/// The data of a DATA frame without its padding (RFC 9113 §6.1).
fn unpadded(flags: Flags, payload: &[u8]) -> Result<&[u8], ConnectionError> {
    if !flags.has(Flags::PADDED) {
        return Ok(payload);
    }
    let (&pad, rest) = payload.split_first().ok_or(conn_err(
        ErrorCode::FrameSizeError,
        "PADDED DATA without a pad length",
    ))?;
    rest.len()
        .checked_sub(usize::from(pad))
        .map(|end| &rest[..end])
        .ok_or(conn_err(
            ErrorCode::ProtocolError,
            "padding longer than the DATA frame",
        ))
}

fn header_value(headers: &[HeaderField], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|h| h.name == name)
        .map(|h| h.value.clone())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests;
