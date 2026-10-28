// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC-over-HTTP/2 server skeleton.
//!
//! Connects [`zerodds_http2`] (RFC 7540 framing) + [`zerodds_hpack`] (RFC
//! 7541 compression) + the local gRPC wire codec ([`crate::frame`]) into
//! a callback-driven server that routes gRPC calls to a caller-layer
//! handler.
//!
//! # Usage skeleton
//!
//! ```ignore
//! let mut server = GrpcServer::new();
//! server.handle(stream_id, http2_frame_payload, |req| {
//!     // req: GrpcRequest with method, path, body
//!     // returns GrpcResponse
//! });
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_hpack::{Decoder as HpackDecoder, Encoder as HpackEncoder, HeaderField};
use zerodds_http2::{
    Flags, Frame, FrameHeader, FrameType, Settings, StreamId, StreamState, decode_frame,
    encode_frame,
};

use crate::frame::{decode_message, encode_message};
use crate::path::parse_path;
use crate::status::Status;

/// Incoming gRPC request (post-HTTP/2-decode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcRequest {
    /// Stream id.
    pub stream_id: StreamId,
    /// `:path` e.g. `/dds.demo.Trader/PlaceOrder`.
    pub path: String,
    /// Service name (extracted from `:path`).
    pub service: String,
    /// Method name (extracted from `:path`).
    pub method: String,
    /// `grpc-encoding` header (`identity`/`gzip` etc.).
    pub encoding: Option<String>,
    /// Custom metadata: every header that is not a pseudo-header
    /// (`authorization` for §7.2 among them), in arrival order.
    pub metadata: Vec<(String, String)>,
    /// Length-Prefixed-Message bytes.
    pub body: Vec<u8>,
}

/// Outgoing gRPC response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcResponse {
    /// Stream id.
    pub stream_id: StreamId,
    /// gRPC status code (0 = OK).
    pub status: Status,
    /// Optional status message (`grpc-message` header).
    pub message: Option<String>,
    /// Response body (Length-Prefixed-Message).
    pub body: Vec<u8>,
}

/// Stream-tracking entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StreamSlot {
    state: StreamState,
    headers: Vec<HeaderField>,
    body: Vec<u8>,
}

/// gRPC server skeleton.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrpcServer {
    settings: Settings,
    decoder: HpackDecoder,
    encoder: HpackEncoder,
    streams: alloc::collections::BTreeMap<StreamId, StreamSlot>,
    /// Encoded control frames the peer is owed, in the order they became
    /// due. Drained by [`GrpcServer::take_control_frames`].
    pending_control: Vec<u8>,
}

impl GrpcServer {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Processes an incoming HTTP/2 frame byte-stream slice.
    ///
    /// Returns an optionally decoded gRPC request once the stream is
    /// complete (`END_STREAM` received).
    ///
    /// # Errors
    /// Static string on protocol violation.
    pub fn process_frame(
        &mut self,
        input: &[u8],
    ) -> Result<(Option<GrpcRequest>, usize), &'static str> {
        let (frame, consumed) =
            decode_frame(input, self.settings.max_frame_size).map_err(|_| "decode frame failed")?;
        let request = match frame.header.frame_type {
            FrameType::Headers => self.handle_headers(&frame)?,
            FrameType::Data => self.handle_data(&frame)?,
            FrameType::Settings => {
                self.acknowledge_settings(&frame);
                None
            }
            FrameType::Ping => {
                self.acknowledge_ping(&frame);
                None
            }
            FrameType::WindowUpdate => None,
            FrameType::RstStream => {
                self.streams.remove(&frame.header.stream_id);
                None
            }
            _ => None,
        };
        Ok((request, consumed))
    }

    /// The control frames the peer is owed, taken out of the server.
    ///
    /// Empty until a SETTINGS or a PING has arrived that requires an
    /// acknowledgement. A caller that never drains this loses nothing it
    /// had before; a caller driving a socket has to write it, because
    /// RFC 7540 §6.5.3 and §6.7 make both acknowledgements mandatory and
    /// a conforming client tears the connection down without them.
    ///
    /// This was the whole of what `grpcurl` — and therefore every Go
    /// gRPC client — hit against `zerodds-grpc-bridged`: SETTINGS, PING
    /// and WINDOW_UPDATE were consumed and answered with nothing, and
    /// the connection ended in an EOF before a single request reached
    /// the dispatcher. `tests/bridge_e2e.rs` drives the same server with
    /// a client written in this tree that asks for no acknowledgement,
    /// so it could not see it.
    pub fn take_control_frames(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.pending_control)
    }

    /// RFC 7540 §6.5.3 — a SETTINGS without ACK is answered by an empty
    /// SETTINGS with ACK on stream 0. A SETTINGS that already carries
    /// ACK is the peer answering ours and is not answered again.
    fn acknowledge_settings(&mut self, frame: &Frame<'_>) {
        if frame.header.flags.has(Flags::ACK) {
            return;
        }
        self.push_control(FrameType::Settings, Flags::ACK, &[]);
    }

    /// RFC 7540 §6.7 — a PING without ACK is answered by a PING with ACK
    /// carrying the same eight opaque octets.
    fn acknowledge_ping(&mut self, frame: &Frame<'_>) {
        if frame.header.flags.has(Flags::ACK) {
            return;
        }
        self.push_control(FrameType::Ping, Flags::ACK, frame.payload);
    }

    /// Encodes one control frame on stream 0 and queues it.
    fn push_control(&mut self, frame_type: FrameType, flags: u8, payload: &[u8]) {
        let header = FrameHeader {
            length: u32::try_from(payload.len()).unwrap_or(0),
            frame_type,
            flags: Flags(flags),
            stream_id: 0,
        };
        let mut out = alloc::vec![0u8; 9 + payload.len()];
        if encode_frame(&header, payload, &mut out, self.settings.max_frame_size).is_ok() {
            self.pending_control.extend_from_slice(&out);
        }
    }

    fn handle_headers(&mut self, frame: &Frame<'_>) -> Result<Option<GrpcRequest>, &'static str> {
        let headers = self
            .decoder
            .decode(frame.payload)
            .map_err(|_| "hpack decode")?;
        let slot = self
            .streams
            .entry(frame.header.stream_id)
            .or_insert(StreamSlot {
                state: StreamState::Idle,
                headers: Vec::new(),
                body: Vec::new(),
            });
        slot.headers.extend(headers);
        if frame.header.flags.has(Flags::END_STREAM) {
            return Ok(Some(self.finalize_request(frame.header.stream_id)?));
        }
        Ok(None)
    }

    fn handle_data(&mut self, frame: &Frame<'_>) -> Result<Option<GrpcRequest>, &'static str> {
        let slot = self
            .streams
            .get_mut(&frame.header.stream_id)
            .ok_or("data on unknown stream")?;
        slot.body.extend_from_slice(frame.payload);
        if frame.header.flags.has(Flags::END_STREAM) {
            return Ok(Some(self.finalize_request(frame.header.stream_id)?));
        }
        // A complete Length-Prefixed-Message is a request even without
        // END_STREAM.
        //
        // END_STREAM alone was the trigger, which serves a unary call and
        // nothing else: on a client- or bidi-streaming call the client
        // sends its message and keeps the stream open for the reply, so
        // the flag never arrives and the request was never dispatched.
        // `grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo`
        // — the call behind `grpcurl list` — is bidi-streaming, so the
        // reflection service this bridge advertises could not be reached
        // by the client it exists for. Measured against grpcurl 1.9.3:
        // HEADERS(END_HEADERS) then DATA(len=8, no END_STREAM), then the
        // client waiting until it gave up.
        //
        // A unary call reaches the same point at the same frame, because
        // its single DATA carries both the whole message and END_STREAM.
        if decode_message(&slot.body).is_ok() {
            return Ok(Some(self.finalize_request(frame.header.stream_id)?));
        }
        Ok(None)
    }

    fn finalize_request(&mut self, stream_id: StreamId) -> Result<GrpcRequest, &'static str> {
        let slot = self.streams.remove(&stream_id).ok_or("unknown stream")?;
        let path = slot
            .headers
            .iter()
            .find(|h| h.name == ":path")
            .map(|h| h.value.clone())
            .ok_or(":path missing")?;
        let (service, method) = parse_path(&path).map_err(|_| "bad path")?;
        let encoding = slot
            .headers
            .iter()
            .find(|h| h.name == "grpc-encoding")
            .map(|h| h.value.clone());
        let metadata = slot
            .headers
            .iter()
            .filter(|h| !h.name.starts_with(':'))
            .map(|h| (h.name.clone(), h.value.clone()))
            .collect();
        Ok(GrpcRequest {
            stream_id,
            path: path.clone(),
            service,
            method,
            encoding,
            metadata,
            body: slot.body,
        })
    }

    /// Encodes a `GrpcResponse` into an HTTP/2 byte stream
    /// (HEADERS + DATA + trailer HEADERS).
    ///
    /// # Errors
    /// Static string on encode errors.
    pub fn encode_response(&mut self, resp: &GrpcResponse) -> Result<Vec<u8>, &'static str> {
        let mut out = Vec::new();
        // 1) HEADERS frame: :status 200, content-type application/grpc.
        let headers = alloc::vec![
            HeaderField {
                name: ":status".into(),
                value: "200".into(),
            },
            HeaderField {
                name: "content-type".into(),
                value: "application/grpc".into(),
            },
        ];
        let h_payload = self.encoder.encode(&headers);
        let h = FrameHeader {
            length: h_payload.len() as u32,
            frame_type: FrameType::Headers,
            flags: Flags(Flags::END_HEADERS),
            stream_id: resp.stream_id,
        };
        let mut buf = alloc::vec![0u8; 9 + h_payload.len()];
        encode_frame(&h, &h_payload, &mut buf, self.settings.max_frame_size)
            .map_err(|_| "headers encode")?;
        out.extend_from_slice(&buf);

        // 2) DATA frame with LPM-encoded body.
        if !resp.body.is_empty() {
            let lpm = encode_message(&resp.body, false).map_err(|_| "lpm encode")?;
            let d = FrameHeader {
                length: lpm.len() as u32,
                frame_type: FrameType::Data,
                flags: Flags(0),
                stream_id: resp.stream_id,
            };
            let mut dbuf = alloc::vec![0u8; 9 + lpm.len()];
            encode_frame(&d, &lpm, &mut dbuf, self.settings.max_frame_size)
                .map_err(|_| "data encode")?;
            out.extend_from_slice(&dbuf);
        }

        // 3) Trailer-HEADERS with grpc-status (+ grpc-message).
        let mut trailers = alloc::vec![HeaderField {
            name: "grpc-status".into(),
            value: alloc::format!("{}", resp.status.code()),
        }];
        if let Some(msg) = &resp.message {
            trailers.push(HeaderField {
                name: "grpc-message".into(),
                value: msg.clone(),
            });
        }
        let t_payload = self.encoder.encode(&trailers);
        let t = FrameHeader {
            length: t_payload.len() as u32,
            frame_type: FrameType::Headers,
            flags: Flags(Flags::END_HEADERS | Flags::END_STREAM),
            stream_id: resp.stream_id,
        };
        let mut tbuf = alloc::vec![0u8; 9 + t_payload.len()];
        encode_frame(&t, &t_payload, &mut tbuf, self.settings.max_frame_size)
            .map_err(|_| "trailer encode")?;
        out.extend_from_slice(&tbuf);

        Ok(out)
    }

    /// Attempts to decode a request body as an LPM frame.
    /// Convenience for callers.
    ///
    /// # Errors
    /// Static string if the Length-Prefixed-Message is corrupt.
    pub fn decode_request_body(&self, req: &GrpcRequest) -> Result<Vec<u8>, &'static str> {
        let (_, msg, _) = decode_message(&req.body).map_err(|_| "lpm decode")?;
        Ok(msg)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn server_starts_with_no_streams() {
        let s = GrpcServer::new();
        assert!(s.streams.is_empty());
    }

    #[test]
    fn encode_response_includes_status_and_trailers() {
        let mut s = GrpcServer::new();
        let resp = GrpcResponse {
            stream_id: 1,
            status: Status::Ok,
            message: None,
            body: alloc::vec![1, 2, 3],
        };
        let bytes = s.encode_response(&resp).unwrap();
        assert!(bytes.len() > 9 * 3, "should have at least 3 frames");
    }

    #[test]
    fn encode_response_with_status_message_includes_it() {
        let mut s = GrpcServer::new();
        let resp = GrpcResponse {
            stream_id: 1,
            status: Status::Internal,
            message: Some("boom".into()),
            body: Vec::new(),
        };
        let _bytes = s.encode_response(&resp).unwrap();
        // Spec: Trailer should contain grpc-status:13 and grpc-message:boom.
        // Our test verifies encode succeeds end-to-end.
    }

    #[test]
    fn rst_stream_clears_state() {
        let mut s = GrpcServer::new();
        s.streams.insert(
            1,
            StreamSlot {
                state: StreamState::Open,
                headers: alloc::vec![],
                body: alloc::vec![],
            },
        );
        // Build a minimal RST_STREAM frame (length 4, type 3, stream 1).
        let buf = alloc::vec![
            0x00, 0x00, 0x04, // length 4
            0x03, // RST_STREAM
            0x00, // flags
            0x00, 0x00, 0x00, 0x01, // stream id 1
            0x00, 0x00, 0x00, 0x08, // error code CANCEL
        ];
        s.process_frame(&buf).unwrap();
        assert!(s.streams.is_empty());
    }
}
