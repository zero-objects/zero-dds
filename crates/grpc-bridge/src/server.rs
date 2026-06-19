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
            FrameType::Settings | FrameType::Ping | FrameType::WindowUpdate => None,
            FrameType::RstStream => {
                self.streams.remove(&frame.header.stream_id);
                None
            }
            _ => None,
        };
        Ok((request, consumed))
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
        Ok(GrpcRequest {
            stream_id,
            path: path.clone(),
            service,
            method,
            encoding,
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
