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

    /// AMQP 0-9-1 §4.2.3: `type:octet channel:short size:long`, the
    /// payload, then the frame-end octet `%xCE`; frame type 1 is
    /// METHOD (`frame-method` in the protocol's XML definitions).
    #[test]
    fn amqp091_4_2_3_a_method_frame_is_type_channel_size_payload_and_frame_end() {
        let wire = [
            0x01, // frame-method
            0x00, 0x01, // channel 1
            0x00, 0x00, 0x00, 0x04, // size
            0x00, 0x0a, 0x00, 0x0b, // connection.start-ok (10/11)
            0xce, // frame-end
        ];
        let f = Frame {
            frame_type: FrameType::Method,
            channel: 1,
            payload: alloc::vec![0x00, 0x0a, 0x00, 0x0b],
        };
        assert_eq!(f.encode(), wire);
        assert_eq!(Frame::decode(&wire).unwrap(), (f, wire.len()));
    }

    /// §4.2.3: the four frame types of the XML definitions, by number.
    #[test]
    fn amqp091_4_2_3_the_frame_types_are_one_two_three_and_eight() {
        for (octet, kind) in [
            (1, FrameType::Method),
            (2, FrameType::Header),
            (3, FrameType::Body),
            (8, FrameType::Heartbeat),
        ] {
            assert_eq!(FrameType::from_u8(octet), Some(kind));
            assert_eq!(kind as u8, octet);
        }
        assert_eq!(FrameType::from_u8(4), None);
    }

    /// §4.2.3: a frame whose size runs past the octets read is not a
    /// frame yet; one whose last octet is not `%xCE` is a frame error.
    #[test]
    fn amqp091_4_2_3_a_frame_cut_short_or_without_frame_end_is_refused() {
        let wire = [0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0xaa, 0xbb, 0xce];
        assert_eq!(Frame::decode(&wire[..8]), Err(WireError::Truncated));
        let mut bad = wire;
        bad[9] = 0x00;
        assert!(Frame::decode(&bad).is_err());
    }

    /// §4.2.2: the protocol header is `"AMQP" %d0 %d0 %d9 %d1`.
    #[test]
    fn amqp091_4_2_2_the_protocol_header_is_amqp_0_0_9_1() {
        assert_eq!(PROTOCOL_HEADER, *b"AMQP\x00\x00\x09\x01");
    }
}
