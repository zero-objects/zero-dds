// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Frame layer — RFC 9113 §4 + §6.
//!
//! Frame header (9 bytes):
//! ```text
//!  +-----------------------------------------------+
//!  |                 Length (24)                   |
//!  +---------------+---------------+---------------+
//!  |   Type (8)    |   Flags (8)   |
//!  +-+-------------+---------------+-------------------------------+
//!  |R|                 Stream Identifier (31)                      |
//!  +=+=============================================================+
//!  |                   Frame Payload (0...)                      ...
//!  +---------------------------------------------------------------+
//! ```

use crate::error::Http2Error;

/// Frame header length (bytes).
pub const FRAME_HEADER_LEN: usize = 9;

/// Default Max-Frame-Size (RFC 9113 §6.5.2: SETTINGS_MAX_FRAME_SIZE
/// initial value).
pub const DEFAULT_MAX_FRAME_SIZE: u32 = 16_384;

/// Frame type — RFC 9113 §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// `DATA` (0x0).
    Data = 0x0,
    /// `HEADERS` (0x1).
    Headers = 0x1,
    /// `PRIORITY` (0x2).
    Priority = 0x2,
    /// `RST_STREAM` (0x3).
    RstStream = 0x3,
    /// `SETTINGS` (0x4).
    Settings = 0x4,
    /// `PUSH_PROMISE` (0x5).
    PushPromise = 0x5,
    /// `PING` (0x6).
    Ping = 0x6,
    /// `GOAWAY` (0x7).
    GoAway = 0x7,
    /// `WINDOW_UPDATE` (0x8).
    WindowUpdate = 0x8,
    /// `CONTINUATION` (0x9).
    Continuation = 0x9,
}

impl FrameType {
    /// Mapping `u8 -> FrameType`.
    ///
    /// # Errors
    /// `UnknownFrameType` if the code is not in 0x0..=0x9.
    pub fn from_u8(v: u8) -> Result<Self, Http2Error> {
        match v {
            0x0 => Ok(Self::Data),
            0x1 => Ok(Self::Headers),
            0x2 => Ok(Self::Priority),
            0x3 => Ok(Self::RstStream),
            0x4 => Ok(Self::Settings),
            0x5 => Ok(Self::PushPromise),
            0x6 => Ok(Self::Ping),
            0x7 => Ok(Self::GoAway),
            0x8 => Ok(Self::WindowUpdate),
            0x9 => Ok(Self::Continuation),
            other => Err(Http2Error::UnknownFrameType(other)),
        }
    }
}

/// Frame flags (8 bits, meaning depends on frame type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags(pub u8);

impl Flags {
    /// `END_STREAM` (DATA/HEADERS, Bit 0).
    pub const END_STREAM: u8 = 0x1;
    /// `END_HEADERS` (HEADERS/CONTINUATION/PUSH_PROMISE, Bit 2).
    pub const END_HEADERS: u8 = 0x4;
    /// `PADDED` (DATA/HEADERS/PUSH_PROMISE, Bit 3).
    pub const PADDED: u8 = 0x8;
    /// `PRIORITY` (HEADERS, Bit 5).
    pub const PRIORITY: u8 = 0x20;
    /// `ACK` (SETTINGS/PING, Bit 0).
    pub const ACK: u8 = 0x1;

    /// Checks whether a bit is set.
    #[must_use]
    pub fn has(self, bit: u8) -> bool {
        (self.0 & bit) == bit
    }
}

/// Frame header (9 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Payload length (24-bit unsigned).
    pub length: u32,
    /// Frame type.
    pub frame_type: FrameType,
    /// Flags.
    pub flags: Flags,
    /// Stream id (R-bit + 31-bit stream ID).
    pub stream_id: u32,
}

/// Frame with header + borrowed slice over the payload (zero-copy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<'a> {
    /// Header.
    pub header: FrameHeader,
    /// Payload slice.
    pub payload: &'a [u8],
}

/// Decodes a frame from a byte slice. Spec §4.1.
///
/// Returns the frame and the number of bytes consumed (header + payload).
///
/// # Errors
/// * `ShortFrameHeader` if fewer than 9 bytes are available.
/// * `ShortPayload` if the length is larger than the available
///   buffer.
/// * `FrameTooLarge` if the length exceeds `max_frame_size`.
/// * `UnknownFrameType` if the type byte is not recognized.
pub fn decode_frame(input: &[u8], max_frame_size: u32) -> Result<(Frame<'_>, usize), Http2Error> {
    if input.len() < FRAME_HEADER_LEN {
        return Err(Http2Error::ShortFrameHeader);
    }
    let length = (u32::from(input[0]) << 16) | (u32::from(input[1]) << 8) | u32::from(input[2]);
    if length > max_frame_size {
        return Err(Http2Error::FrameTooLarge {
            got: length,
            max: max_frame_size,
        });
    }
    let frame_type = FrameType::from_u8(input[3])?;
    let flags = Flags(input[4]);
    // Strip the R-bit (MSB) — Spec §4.1.
    let stream_id = ((u32::from(input[5]) & 0x7f) << 24)
        | (u32::from(input[6]) << 16)
        | (u32::from(input[7]) << 8)
        | u32::from(input[8]);
    let total = FRAME_HEADER_LEN + length as usize;
    if input.len() < total {
        return Err(Http2Error::ShortPayload);
    }
    let payload = &input[FRAME_HEADER_LEN..total];
    Ok((
        Frame {
            header: FrameHeader {
                length,
                frame_type,
                flags,
                stream_id,
            },
            payload,
        },
        total,
    ))
}

/// Encodes a frame into an output buffer. Spec §4.1.
///
/// # Errors
/// * `FrameTooLarge` if `payload.len() > max_frame_size`.
/// * `ShortPayload` if the output buffer is too small.
pub fn encode_frame(
    header: &FrameHeader,
    payload: &[u8],
    out: &mut [u8],
    max_frame_size: u32,
) -> Result<usize, Http2Error> {
    let len = payload.len();
    if len > max_frame_size as usize {
        return Err(Http2Error::FrameTooLarge {
            got: len as u32,
            max: max_frame_size,
        });
    }
    let total = FRAME_HEADER_LEN + len;
    if out.len() < total {
        return Err(Http2Error::ShortPayload);
    }
    let l = len as u32;
    out[0] = ((l >> 16) & 0xff) as u8;
    out[1] = ((l >> 8) & 0xff) as u8;
    out[2] = (l & 0xff) as u8;
    out[3] = header.frame_type as u8;
    out[4] = header.flags.0;
    let sid = header.stream_id & 0x7fff_ffff;
    out[5] = ((sid >> 24) & 0xff) as u8;
    out[6] = ((sid >> 16) & 0xff) as u8;
    out[7] = ((sid >> 8) & 0xff) as u8;
    out[8] = (sid & 0xff) as u8;
    out[FRAME_HEADER_LEN..total].copy_from_slice(payload);
    Ok(total)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn from_u8_round_trip() {
        for ft in [
            FrameType::Data,
            FrameType::Headers,
            FrameType::Priority,
            FrameType::RstStream,
            FrameType::Settings,
            FrameType::PushPromise,
            FrameType::Ping,
            FrameType::GoAway,
            FrameType::WindowUpdate,
            FrameType::Continuation,
        ] {
            assert_eq!(FrameType::from_u8(ft as u8).unwrap(), ft);
        }
    }

    #[test]
    fn unknown_frame_type_rejected() {
        assert!(matches!(
            FrameType::from_u8(0xff),
            Err(Http2Error::UnknownFrameType(0xff))
        ));
    }

    #[test]
    fn flags_has_detects_bits() {
        let f = Flags(Flags::END_STREAM | Flags::END_HEADERS);
        assert!(f.has(Flags::END_STREAM));
        assert!(f.has(Flags::END_HEADERS));
        assert!(!f.has(Flags::PADDED));
    }

    #[test]
    fn encode_decode_round_trip() {
        let h = FrameHeader {
            length: 5,
            frame_type: FrameType::Data,
            flags: Flags(Flags::END_STREAM),
            stream_id: 7,
        };
        let payload = vec![1, 2, 3, 4, 5];
        let mut buf = vec![0u8; 32];
        let n = encode_frame(&h, &payload, &mut buf, DEFAULT_MAX_FRAME_SIZE).unwrap();
        assert_eq!(n, FRAME_HEADER_LEN + 5);
        let (frame, consumed) = decode_frame(&buf[..n], DEFAULT_MAX_FRAME_SIZE).unwrap();
        assert_eq!(consumed, n);
        assert_eq!(frame.header.length, 5);
        assert_eq!(frame.header.stream_id, 7);
        assert_eq!(frame.header.frame_type, FrameType::Data);
        assert!(frame.header.flags.has(Flags::END_STREAM));
        assert_eq!(frame.payload, &payload[..]);
    }

    #[test]
    fn r_bit_stripped_from_stream_id() {
        // Construct frame with R-bit set (MSB of stream id byte 5).
        let mut buf = vec![0u8; 9];
        buf[3] = FrameType::Settings as u8;
        buf[5] = 0x80; // R-bit + stream 0
        let (frame, _) = decode_frame(&buf, DEFAULT_MAX_FRAME_SIZE).unwrap();
        assert_eq!(frame.header.stream_id, 0, "R-bit must be stripped");
    }

    #[test]
    fn frame_too_large_rejected() {
        let mut buf = vec![0u8; FRAME_HEADER_LEN + 100];
        // Length = 100
        buf[2] = 100;
        let r = decode_frame(&buf, 50);
        assert!(matches!(
            r,
            Err(Http2Error::FrameTooLarge { got: 100, max: 50 })
        ));
    }

    #[test]
    fn short_header_rejected() {
        let buf = vec![0u8; 5];
        assert!(matches!(
            decode_frame(&buf, DEFAULT_MAX_FRAME_SIZE),
            Err(Http2Error::ShortFrameHeader)
        ));
    }

    #[test]
    fn short_payload_rejected() {
        // Header says 100 bytes but buffer only has 9 + 50.
        let mut buf = vec![0u8; FRAME_HEADER_LEN + 50];
        buf[2] = 100;
        assert!(matches!(
            decode_frame(&buf, DEFAULT_MAX_FRAME_SIZE),
            Err(Http2Error::ShortPayload)
        ));
    }

    #[test]
    fn encode_into_too_small_buffer_rejected() {
        let h = FrameHeader {
            length: 0,
            frame_type: FrameType::Ping,
            flags: Flags(0),
            stream_id: 0,
        };
        let mut buf = vec![0u8; 5];
        assert!(encode_frame(&h, b"PINGPING", &mut buf, DEFAULT_MAX_FRAME_SIZE).is_err());
    }
}
