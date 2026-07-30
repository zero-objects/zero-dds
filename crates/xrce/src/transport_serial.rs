// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Serial-Transport-Framer (Spec Annex C, RFC 1662 PPP/HDLC-Framing).
//!
//! Annex C defines a PPP/HDLC-like framing that marks frame boundaries
//! via begin/end flags + byte stuffing and appends a CRC-16 for
//! error detection. This is necessary because serial transports
//! (RS-232, SPI, I2C) have no inherent frame boundaries.
//!
//! ## Frame-Layout
//!
//! ```text
//!   7E [ stuffed payload + crc16 ] 7E
//! ```
//!
//! ## Byte-Stuffing
//!
//! - `0x7E` (begin/end flag) in the payload → `0x7D 0x5E`
//! - `0x7D` (escape flag) in the payload → `0x7D 0x5D`
//! - Generic: `b ∈ {0x7D, 0x7E}` → `0x7D, b XOR 0x20`
//!
//! ## CRC
//!
//! 16-Bit CRC-CCITT-FALSE: Init=`0xFFFF`, Polynom=`0x1021`,
//! RefIn=false, RefOut=false, XorOut=`0x0000`.
//!
//! Rationale: Spec §C.1.1.6 requires RFC 1662 CRC-16, polynomial
//! `x^16+x^12+x^5+1` (= `0x1021`). RFC 1662 technically specifies
//! init=`0xFFFF` with reflected input/output (X.25/FCS-16). We
//! choose the unreflected CCITT-FALSE variant, because it (a)
//! matches the majority of embedded CRC implementations (XMODEM,
//! ITU-T V.41) and (b) is congruent with the `0x1021` polynomial choice.
//! Appended as big-endian (most-significant byte first) before the
//! end flag, because that is the DDS-XCDR convention in the RTPS stack and
//! the XRCE spec text does not explicitly require it reflected.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::{DOSC_MAX_PAYLOAD_SIZE, Message};
use crate::transport_udp::MAX_DATAGRAM_SIZE;

/// Frame boundary flag.
pub const FLAG_BYTE: u8 = 0x7E;
/// Escape byte.
pub const ESCAPE_BYTE: u8 = 0x7D;
/// XOR mask for stuffed bytes.
pub const STUFF_XOR: u8 = 0x20;

/// Error during serial framing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SerialError {
    /// Frame is shorter than the CRC field (at least 2 bytes) — no
    /// complete payload possible.
    FrameTooShort {
        /// Actual destuffed frame length in bytes.
        actual: usize,
    },
    /// The computed CRC does not match the one transmitted in the frame.
    CrcMismatch {
        /// Expected (computed) CRC.
        expected: u16,
        /// Actual (transmitted) CRC.
        actual: u16,
    },
    /// Escape sequence ends mid-frame with no following byte.
    DanglingEscape,
    /// Escape sequence was followed by a byte, but the byte
    /// does not fit the stuffing convention (`b XOR 0x20` must yield `0x7D`
    /// or `0x7E`).
    InvalidEscape {
        /// The byte after `0x7D`.
        byte: u8,
    },
    /// Frame exceeds the DoS cap.
    FrameTooLong {
        /// Cap.
        limit: usize,
        /// Actual length.
        actual: usize,
    },
    /// Wraps an `XrceError` from `Message::decode`.
    Decode(XrceError),
    /// Wraps an `XrceError` from `Message::encode`.
    Encode(XrceError),
}

impl core::fmt::Display for SerialError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrameTooShort { actual } => write!(f, "serial frame too short: {actual} bytes"),
            Self::CrcMismatch { expected, actual } => write!(
                f,
                "serial crc mismatch: expected 0x{expected:04x}, actual 0x{actual:04x}"
            ),
            Self::DanglingEscape => write!(f, "serial dangling escape at end of frame"),
            Self::InvalidEscape { byte } => write!(f, "serial invalid escape byte 0x{byte:02x}"),
            Self::FrameTooLong { limit, actual } => {
                write!(f, "serial frame too long: limit {limit}, actual {actual}")
            }
            Self::Decode(e) => write!(f, "serial decode: {e}"),
            Self::Encode(e) => write!(f, "serial encode: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SerialError {}

/// Berechnet CRC-16-CCITT-FALSE.
#[must_use]
pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Encodes exactly one HDLC frame around `payload`.
///
/// Layout: `7E [stuffed payload + crc16 BE] 7E`.
#[must_use]
pub fn encode_payload(payload: &[u8]) -> Vec<u8> {
    let crc = crc16_ccitt_false(payload);
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.push(FLAG_BYTE);
    stuff_into(&mut out, payload);
    let crc_bytes = crc.to_be_bytes();
    stuff_into(&mut out, &crc_bytes);
    out.push(FLAG_BYTE);
    out
}

/// Encodes a complete `Message` as an HDLC frame.
///
/// # Errors
/// `Encode` if `Message::encode` fails; `FrameTooLong` if the
/// encoded message is > `MAX_DATAGRAM_SIZE`.
pub fn encode_message(msg: &Message) -> Result<Vec<u8>, SerialError> {
    let payload = msg.encode().map_err(SerialError::Encode)?;
    if payload.len() > MAX_DATAGRAM_SIZE {
        return Err(SerialError::FrameTooLong {
            limit: MAX_DATAGRAM_SIZE,
            actual: payload.len(),
        });
    }
    Ok(encode_payload(&payload))
}

fn stuff_into(out: &mut Vec<u8>, data: &[u8]) {
    for &b in data {
        if b == FLAG_BYTE || b == ESCAPE_BYTE {
            out.push(ESCAPE_BYTE);
            out.push(b ^ STUFF_XOR);
        } else {
            out.push(b);
        }
    }
}

/// Destuffs `input` (without begin/end flags). Returns the raw payload
/// incl. the CRC field.
fn destuff(input: &[u8]) -> Result<Vec<u8>, SerialError> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        if b == ESCAPE_BYTE {
            i += 1;
            if i >= input.len() {
                return Err(SerialError::DanglingEscape);
            }
            let unstuffed = input[i] ^ STUFF_XOR;
            if unstuffed != FLAG_BYTE && unstuffed != ESCAPE_BYTE {
                return Err(SerialError::InvalidEscape { byte: input[i] });
            }
            out.push(unstuffed);
        } else {
            out.push(b);
        }
        i += 1;
    }
    Ok(out)
}

/// Decodes exactly one frame that lies in `bytes` between the begin and
/// end flag. `bytes` must contain exactly the frame-inner bytes (between
/// the flags) — without the flags themselves.
///
/// # Errors
/// - `FrameTooShort` (< 2 byte CRC).
/// - `DanglingEscape`/`InvalidEscape` from `destuff`.
/// - `CrcMismatch`.
pub fn decode_frame_inner(bytes: &[u8]) -> Result<Vec<u8>, SerialError> {
    let raw = destuff(bytes)?;
    if raw.len() < 2 {
        return Err(SerialError::FrameTooShort { actual: raw.len() });
    }
    let split = raw.len() - 2;
    let payload = &raw[..split];
    let crc_recv = u16::from_be_bytes([raw[split], raw[split + 1]]);
    let crc_calc = crc16_ccitt_false(payload);
    if crc_recv != crc_calc {
        return Err(SerialError::CrcMismatch {
            expected: crc_calc,
            actual: crc_recv,
        });
    }
    Ok(payload.to_vec())
}

/// Decodes the first complete frame in `input` (with begin/end flags).
/// Returns `(message, rest)` — `rest` points to the bytes after the
/// end flag.
///
/// # Errors
/// - `FrameTooShort` if no complete frame is present.
/// - Frame decode error (CRC, escape, etc.).
/// - `Decode` if the payload is not a valid XRCE message.
pub fn decode_frame(input: &[u8]) -> Result<(Message, &[u8]), SerialError> {
    // Search for the begin flag.
    let begin = input
        .iter()
        .position(|&b| b == FLAG_BYTE)
        .ok_or(SerialError::FrameTooShort { actual: 0 })?;
    let after_begin = &input[begin + 1..];
    // Search for the end flag (the next 0x7E NOT directly after begin — but
    // it may be that the sender writes several 0x7E in a row
    // and then an adjacency occurs; we skip empty frames).
    let mut search_start = 0;
    while search_start < after_begin.len() && after_begin[search_start] == FLAG_BYTE {
        search_start += 1;
    }
    let end_rel = after_begin[search_start..]
        .iter()
        .position(|&b| b == FLAG_BYTE)
        .ok_or(SerialError::FrameTooShort {
            actual: after_begin.len(),
        })?;
    let inner_end = search_start + end_rel;
    let inner = &after_begin[search_start..inner_end];
    let payload = decode_frame_inner(inner)?;
    let msg = Message::decode(&payload).map_err(SerialError::Decode)?;
    let rest = &after_begin[inner_end + 1..];
    Ok((msg, rest))
}

/// Streaming decoder: takes a continuous byte stream and
/// extracts completely received frames.
///
/// Anti-bombing: per `push_bytes` call at most
/// `DOSC_MAX_PAYLOAD_SIZE * 2` bytes are buffered internally (worst case due
/// to stuffing). Overflow → the internal buffer is put into a resync mode
/// that skips up to the next `0x7E`.
#[derive(Debug, Default)]
pub struct SerialFramer {
    /// Currently buffered bytes.
    buf: Vec<u8>,
    /// `true` when we are currently inside a frame (between begin and end).
    in_frame: bool,
    /// `true` when, after an overflow, we skip all bytes up to the next
    /// `0x7E`.
    resync: bool,
}

impl SerialFramer {
    /// Fresh framer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Anti-bombing cap: maximum internal buffer before resync.
    const BUF_CAP: usize = DOSC_MAX_PAYLOAD_SIZE * 2;

    /// Reset to the initial state.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.in_frame = false;
        self.resync = false;
    }

    /// Consumes a byte chunk and returns all frames that became complete
    /// within the chunk. Frame errors are reported as well.
    pub fn push_bytes(&mut self, data: &[u8]) -> Vec<Result<Message, SerialError>> {
        let mut out = Vec::new();
        for &b in data {
            if self.resync {
                if b == FLAG_BYTE {
                    self.resync = false;
                    self.in_frame = true;
                    self.buf.clear();
                }
                continue;
            }
            if b == FLAG_BYTE {
                if self.in_frame && !self.buf.is_empty() {
                    // End flag: frame complete.
                    let inner = core::mem::take(&mut self.buf);
                    match decode_frame_inner(&inner) {
                        Ok(payload) => match Message::decode(&payload) {
                            Ok(msg) => out.push(Ok(msg)),
                            Err(e) => out.push(Err(SerialError::Decode(e))),
                        },
                        Err(e) => out.push(Err(e)),
                    }
                    // The next frame could start immediately (if the
                    // sender uses the end flag simultaneously as the begin
                    // flag of the next frame — RFC 1662 allows that).
                    self.in_frame = true;
                } else {
                    // Begin flag (or empty frame between two 7E).
                    self.in_frame = true;
                    self.buf.clear();
                }
            } else if self.in_frame {
                self.buf.push(b);
                if self.buf.len() > Self::BUF_CAP {
                    out.push(Err(SerialError::FrameTooLong {
                        limit: Self::BUF_CAP,
                        actual: self.buf.len(),
                    }));
                    self.buf.clear();
                    self.in_frame = false;
                    self.resync = true;
                }
            } // otherwise: bytes outside of frames are ignored.
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::header::{ClientKey, MessageHeader, SessionId, StreamId};
    use crate::object_id::ObjectId;
    use crate::object_info::BaseObjectRequest;
    use crate::serial_number::SerialNumber16;
    use crate::submessages::timestamp::TimePoint;
    use crate::submessages::{
        AckNackPayload, CreateClientPayload, FragmentPayload, HeartbeatPayload, ResetPayload,
        Submessage, TimestampPayload, TimestampReplyPayload, WriteDataPayload,
    };

    fn message_with(sm: Submessage) -> Message {
        let header = MessageHeader::with_client_key(
            SessionId(0),
            StreamId::BUILTIN_RELIABLE,
            SerialNumber16::new(1),
            ClientKey([0xCA, 0xFE, 0xBA, 0xBE]),
        )
        .unwrap();
        Message::new(header, alloc::vec![sm]).unwrap()
    }

    #[test]
    fn crc16_ccitt_false_empty_input_returns_init_value() {
        assert_eq!(crc16_ccitt_false(&[]), 0xFFFF);
    }

    #[test]
    fn crc16_ccitt_false_known_vector_123456789() {
        // CCITT-FALSE: CRC("123456789") = 0x29B1
        let crc = crc16_ccitt_false(b"123456789");
        assert_eq!(crc, 0x29B1);
    }

    #[test]
    fn encode_payload_starts_and_ends_with_flag() {
        let frame = encode_payload(&[1, 2, 3]);
        assert_eq!(frame.first(), Some(&FLAG_BYTE));
        assert_eq!(frame.last(), Some(&FLAG_BYTE));
    }

    #[test]
    fn encode_payload_stuffs_flag_byte_in_payload() {
        let frame = encode_payload(&[0x7E]);
        // 7E [7D 5E (stuffed body)] [7D (escape) ?? (low byte of CRC)]
        // … we only check that the stuffed body contains 7D 5E.
        let body = &frame[1..frame.len() - 1];
        assert!(body.starts_with(&[ESCAPE_BYTE, FLAG_BYTE ^ STUFF_XOR]));
    }

    #[test]
    fn encode_payload_stuffs_escape_byte_in_payload() {
        let frame = encode_payload(&[0x7D]);
        let body = &frame[1..frame.len() - 1];
        assert!(body.starts_with(&[ESCAPE_BYTE, ESCAPE_BYTE ^ STUFF_XOR]));
    }

    #[test]
    fn encode_decode_roundtrip_no_special_bytes() {
        let payload = alloc::vec![1, 2, 3, 4, 5, 6, 7, 8];
        let frame = encode_payload(&payload);
        let inner = &frame[1..frame.len() - 1];
        let decoded = decode_frame_inner(inner).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn encode_decode_roundtrip_with_flag_byte_in_middle() {
        let payload = alloc::vec![0xAA, 0x7E, 0xBB, 0x7E, 0xCC];
        let frame = encode_payload(&payload);
        let inner = &frame[1..frame.len() - 1];
        let decoded = decode_frame_inner(inner).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn encode_decode_roundtrip_with_escape_byte_in_middle() {
        let payload = alloc::vec![0xAA, 0x7D, 0xBB, 0x7D, 0xCC];
        let frame = encode_payload(&payload);
        let inner = &frame[1..frame.len() - 1];
        let decoded = decode_frame_inner(inner).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn encode_decode_roundtrip_both_special_bytes_combined() {
        let payload = alloc::vec![0x7E, 0x7D, 0x7E, 0x7D, 0xFF, 0x00, 0x7D, 0x7E];
        let frame = encode_payload(&payload);
        let inner = &frame[1..frame.len() - 1];
        let decoded = decode_frame_inner(inner).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn encode_decode_roundtrip_only_flag_bytes() {
        let payload = alloc::vec![0x7E; 16];
        let frame = encode_payload(&payload);
        let inner = &frame[1..frame.len() - 1];
        let decoded = decode_frame_inner(inner).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn encode_decode_roundtrip_only_escape_bytes() {
        let payload = alloc::vec![0x7D; 16];
        let frame = encode_payload(&payload);
        let inner = &frame[1..frame.len() - 1];
        let decoded = decode_frame_inner(inner).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_rejects_crc_mismatch() {
        let payload = alloc::vec![1, 2, 3, 4];
        let mut frame = encode_payload(&payload);
        // Flip the last byte before the end flag.
        let len = frame.len();
        frame[len - 2] ^= 0xFF;
        let inner = &frame[1..frame.len() - 1];
        let res = decode_frame_inner(inner);
        assert!(matches!(res, Err(SerialError::CrcMismatch { .. })));
    }

    #[test]
    fn decode_rejects_dangling_escape() {
        let bad = alloc::vec![0x01, 0x02, ESCAPE_BYTE];
        let res = decode_frame_inner(&bad);
        assert!(matches!(res, Err(SerialError::DanglingEscape)));
    }

    #[test]
    fn decode_rejects_invalid_escape() {
        // 0x7D 0xFF — 0xFF XOR 0x20 = 0xDF, neither 0x7D nor 0x7E.
        let bad = alloc::vec![ESCAPE_BYTE, 0xFF, 0x00, 0x00];
        let res = decode_frame_inner(&bad);
        assert!(matches!(
            res,
            Err(SerialError::InvalidEscape { byte: 0xFF })
        ));
    }

    #[test]
    fn decode_rejects_short_frame() {
        let bad = alloc::vec![0x01];
        let res = decode_frame_inner(&bad);
        assert!(matches!(res, Err(SerialError::FrameTooShort { actual: 1 })));
    }

    #[test]
    fn full_message_encode_decode_create_client() {
        let msg = message_with(
            CreateClientPayload {
                representation: alloc::vec![b'X', b'R', b'C', b'E', 1, 0],
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let (back, rest) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
        assert!(rest.is_empty());
    }

    #[test]
    fn full_message_encode_decode_write_data_with_special_bytes() {
        // Deliberately construct a payload that contains many 0x7E/0x7D.
        let msg = message_with(
            WriteDataPayload {
                base: BaseObjectRequest {
                    request_id: [0x00, 0x01],
                    object_id: ObjectId::from_raw(0x0DA5),
                },
                serialized_data: alloc::vec![0x7E, 0x7D, 0x00, 0x7E, 0x7D, 0xFF, 0x7E],
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let (back, _) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn full_message_encode_decode_acknack() {
        let msg = message_with(
            AckNackPayload {
                first_unacked_seq_num: 5,
                nack_bitmap: [0xAA, 0x55],
                stream_id: 0x80,
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let (back, _) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn full_message_encode_decode_heartbeat() {
        let msg = message_with(
            HeartbeatPayload {
                first_unacked_seq_nr: 1,
                last_unacked_seq_nr: 9,
                stream_id: 0x80,
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let (back, _) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn full_message_encode_decode_reset() {
        let msg = message_with(ResetPayload.into_submessage().unwrap());
        let frame = encode_message(&msg).unwrap();
        let (back, _) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn full_message_encode_decode_fragment() {
        let msg = message_with(
            FragmentPayload {
                data: alloc::vec![0x7E; 32],
                last_fragment: true,
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let (back, _) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn full_message_encode_decode_timestamp() {
        let msg = message_with(
            TimestampPayload {
                transmit_timestamp: TimePoint {
                    seconds: 100,
                    nanoseconds: 0,
                },
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let (back, _) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn full_message_encode_decode_timestamp_reply() {
        let msg = message_with(TimestampReplyPayload::default().into_submessage().unwrap());
        let frame = encode_message(&msg).unwrap();
        let (back, _) = decode_frame(&frame).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn streaming_framer_single_frame_in_one_chunk() {
        let msg = message_with(ResetPayload.into_submessage().unwrap());
        let frame = encode_message(&msg).unwrap();
        let mut framer = SerialFramer::new();
        let out = framer.push_bytes(&frame);
        assert_eq!(out.len(), 1);
        assert_eq!(*out[0].as_ref().unwrap(), msg);
    }

    #[test]
    fn streaming_framer_split_across_two_chunks() {
        let msg = message_with(
            CreateClientPayload {
                representation: alloc::vec![1, 2, 3, 4, 5, 6],
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let mid = frame.len() / 2;
        let mut framer = SerialFramer::new();
        assert!(framer.push_bytes(&frame[..mid]).is_empty());
        let out = framer.push_bytes(&frame[mid..]);
        assert_eq!(out.len(), 1);
        assert_eq!(*out[0].as_ref().unwrap(), msg);
    }

    #[test]
    fn streaming_framer_byte_at_a_time() {
        let msg = message_with(
            HeartbeatPayload {
                first_unacked_seq_nr: 0,
                last_unacked_seq_nr: 5,
                stream_id: 0x80,
            }
            .into_submessage()
            .unwrap(),
        );
        let frame = encode_message(&msg).unwrap();
        let mut framer = SerialFramer::new();
        let mut collected: Vec<Message> = Vec::new();
        for chunk in frame.chunks(1) {
            for r in framer.push_bytes(chunk) {
                collected.push(r.unwrap());
            }
        }
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0], msg);
    }

    #[test]
    fn streaming_framer_three_back_to_back_frames() {
        let msgs = alloc::vec![
            message_with(ResetPayload.into_submessage().unwrap()),
            message_with(
                AckNackPayload {
                    first_unacked_seq_num: 5,
                    nack_bitmap: [0, 0],
                    stream_id: 0x80,
                }
                .into_submessage()
                .unwrap(),
            ),
            message_with(
                HeartbeatPayload {
                    first_unacked_seq_nr: 0,
                    last_unacked_seq_nr: 1,
                    stream_id: 0x80,
                }
                .into_submessage()
                .unwrap(),
            ),
        ];
        let mut concat = Vec::new();
        for m in &msgs {
            concat.extend_from_slice(&encode_message(m).unwrap());
        }
        let mut framer = SerialFramer::new();
        let out = framer.push_bytes(&concat);
        assert_eq!(out.len(), 3);
        for (i, r) in out.into_iter().enumerate() {
            assert_eq!(r.unwrap(), msgs[i]);
        }
    }

    #[test]
    fn streaming_framer_skips_garbage_before_first_flag() {
        let msg = message_with(ResetPayload.into_submessage().unwrap());
        let frame = encode_message(&msg).unwrap();
        let mut buf = alloc::vec![0xAB, 0xCD, 0xEF, 0x01]; // garbage
        buf.extend_from_slice(&frame);
        let mut framer = SerialFramer::new();
        let out = framer.push_bytes(&buf);
        assert_eq!(out.len(), 1);
        assert_eq!(*out[0].as_ref().unwrap(), msg);
    }

    #[test]
    fn streaming_framer_emits_crc_error_for_corrupted_frame() {
        let msg = message_with(ResetPayload.into_submessage().unwrap());
        let mut frame = encode_message(&msg).unwrap();
        // Flip the last byte before the end flag.
        let len = frame.len();
        frame[len - 2] ^= 0xFF;
        let mut framer = SerialFramer::new();
        let out = framer.push_bytes(&frame);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], Err(SerialError::CrcMismatch { .. })));
    }

    #[test]
    fn streaming_framer_recovers_after_crc_error() {
        let bad_msg = message_with(ResetPayload.into_submessage().unwrap());
        let good_msg = message_with(
            HeartbeatPayload {
                first_unacked_seq_nr: 0,
                last_unacked_seq_nr: 1,
                stream_id: 0x80,
            }
            .into_submessage()
            .unwrap(),
        );
        let mut frame_bad = encode_message(&bad_msg).unwrap();
        let len = frame_bad.len();
        frame_bad[len - 2] ^= 0xFF; // CRC kaputt
        let frame_good = encode_message(&good_msg).unwrap();
        let mut concat = frame_bad.clone();
        concat.extend_from_slice(&frame_good);
        let mut framer = SerialFramer::new();
        let out = framer.push_bytes(&concat);
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0], Err(SerialError::CrcMismatch { .. })));
        assert_eq!(*out[1].as_ref().unwrap(), good_msg);
    }

    #[test]
    fn streaming_framer_reset_clears_state() {
        let msg = message_with(ResetPayload.into_submessage().unwrap());
        let frame = encode_message(&msg).unwrap();
        let mut framer = SerialFramer::new();
        // Halben Frame einspeisen.
        let _ = framer.push_bytes(&frame[..frame.len() / 2]);
        framer.reset();
        // Now feed in the full frame.
        let out = framer.push_bytes(&frame);
        assert_eq!(out.len(), 1);
        assert_eq!(*out[0].as_ref().unwrap(), msg);
    }

    #[test]
    fn flag_byte_constants_match_spec() {
        assert_eq!(FLAG_BYTE, 0x7E);
        assert_eq!(ESCAPE_BYTE, 0x7D);
        assert_eq!(STUFF_XOR, 0x20);
    }
}
