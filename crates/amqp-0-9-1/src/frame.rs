// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 0.9.1 frame format (§4.2.2). Every frame after the protocol header is:
//!
//! ```text
//! [type:octet][channel:short][size:long][payload:size][frame-end:0xCE]
//! ```
//!
//! Frame types: METHOD(1), HEADER(2), BODY(3), HEARTBEAT(8). Method-frame
//! payloads carry `[class-id:short][method-id:short][arguments...]`.

extern crate alloc;
use alloc::vec::Vec;

use crate::types::WireError;

/// The 8-byte AMQP 0.9.1 protocol header the client sends first (§4.2.2): the
/// literal `AMQP` + `0, 0, 9, 1`.
pub const PROTOCOL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0, 0, 9, 1];

/// `frame-end` octet (§4.2.3) — every frame ends with `0xCE`.
pub const FRAME_END: u8 = 0xCE;

/// Frame type octet (§4.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// Method frame.
    Method = 1,
    /// Content header frame.
    Header = 2,
    /// Content body frame.
    Body = 3,
    /// Heartbeat frame.
    Heartbeat = 8,
}

impl FrameType {
    /// Maps the wire octet to a type.
    #[must_use]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Method),
            2 => Some(Self::Header),
            3 => Some(Self::Body),
            8 => Some(Self::Heartbeat),
            _ => None,
        }
    }
}

/// A decoded frame: its type, channel, and raw payload (without the frame-end).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Frame type.
    pub frame_type: FrameType,
    /// Channel number (0 = connection-global).
    pub channel: u16,
    /// Payload bytes (method header + args, or content header/body).
    pub payload: Vec<u8>,
}

impl Frame {
    /// Encodes the frame to the wire (type + channel + size + payload + 0xCE).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.payload.len());
        out.push(self.frame_type as u8);
        out.extend_from_slice(&self.channel.to_be_bytes());
        out.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.payload);
        out.push(FRAME_END);
        out
    }

    /// Decodes one frame from `bytes`, returning the frame and the number of
    /// bytes consumed. Returns [`WireError::Truncated`] if the slice does not
    /// hold a full frame yet (the caller should read more).
    ///
    /// # Errors
    /// [`WireError`] on a malformed header, bad frame type, or missing
    /// frame-end octet.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), WireError> {
        if bytes.len() < 7 {
            return Err(WireError::Truncated);
        }
        let frame_type =
            FrameType::from_u8(bytes[0]).ok_or(WireError::UnsupportedFieldType(bytes[0]))?;
        let channel = u16::from_be_bytes([bytes[1], bytes[2]]);
        let size = u32::from_be_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]) as usize;
        let total = 7 + size + 1; // header + payload + frame-end
        if bytes.len() < total {
            return Err(WireError::Truncated);
        }
        if bytes[7 + size] != FRAME_END {
            return Err(WireError::UnsupportedFieldType(bytes[7 + size]));
        }
        let payload = bytes[7..7 + size].to_vec();
        Ok((
            Self {
                frame_type,
                channel,
                payload,
            },
            total,
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrips() {
        let f = Frame {
            frame_type: FrameType::Method,
            channel: 1,
            payload: alloc::vec![0x00, 0x0a, 0x00, 0x0b],
        };
        let wire = f.encode();
        // type=1, channel=0x0001, size=4, ... , 0xCE.
        assert_eq!(wire[0], 1);
        assert_eq!(&wire[1..3], &[0x00, 0x01]);
        assert_eq!(*wire.last().unwrap(), FRAME_END);
        let (decoded, n) = Frame::decode(&wire).unwrap();
        assert_eq!(decoded, f);
        assert_eq!(n, wire.len());
    }

    #[test]
    fn decode_reports_truncation() {
        let f = Frame {
            frame_type: FrameType::Body,
            channel: 1,
            payload: alloc::vec![1, 2, 3, 4, 5],
        };
        let wire = f.encode();
        assert_eq!(
            Frame::decode(&wire[..wire.len() - 2]),
            Err(WireError::Truncated)
        );
    }

    #[test]
    fn protocol_header_is_0_0_9_1() {
        assert_eq!(&PROTOCOL_HEADER[..4], b"AMQP");
        assert_eq!(&PROTOCOL_HEADER[4..], &[0, 0, 9, 1]);
    }
}
