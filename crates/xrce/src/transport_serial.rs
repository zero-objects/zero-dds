// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE serial transport framing (DDS-XRCE 1.0 Annex C.1, an HDLC-like
//! framing after RFC 1662).
//!
//! Serial transports (RS-232, SPI, I2C) have no frame boundaries of their
//! own; Annex C frames each XRCE Message.
//!
//! ## Frame format (Annex C.1.1)
//!
//! ```text
//!   7E | sourceAddress | remoteAddress | payloadLength (LE) | payload | frameCheck (LE)
//!   1    1               1               2                    N         2
//! ```
//!
//! The frame begins with `BEGIN_FLAG` (0x7E); its end follows from
//! `payloadLength`. `frameCheck` is the 16-bit CRC of RFC 1662 (FCS-16,
//! polynomial `x^16 + x^12 + x^5 + 1`, reflected, initial value 0xFFFF,
//! complemented) of the frame "without including the BEGIN_FLAG or
//! transparency stuffing", little endian (C.1.1.6).
//!
//! ## Transparency (Annex C.1.2)
//!
//! Each byte after the flag that is 0x7E or 0x7D is sent as 0x7D followed by
//! the byte XOR 0x20.
//!
//! ## Frame integrity (Annex C.1.3)
//!
//! A frame is dropped when its `frameCheck` is not the computed CRC, and when
//! a non-escaped `BEGIN_FLAG` comes before its payload is completely read;
//! that flag then begins the next frame.
//!
//! ## The frame check of Micro-XRCE-DDS
//!
//! eProsima Micro-XRCE-DDS 3.0.1 (agent `StreamFramingProtocol.cpp`, client
//! `stream_framing_protocol.c`) frames as Annex C does but computes
//! `frameCheck` as CRC-16/ARC (polynomial `x^16 + x^15 + x^2 + 1`,
//! reflected, initial value 0, not complemented) over the payload alone.
//! [`FrameCheck::Crc16ArcPayload`] takes and makes those frames; the default
//! is [`FrameCheck::Rfc1662`].

extern crate alloc;
use alloc::vec::Vec;

use crate::error::XrceError;
use crate::submessages::Message;

/// `BEGIN_FLAG` (Annex C.1.1.1).
pub const FLAG_BYTE: u8 = 0x7E;
/// `ESCAPE_FLAG` (Annex C.1.2).
pub const ESCAPE_BYTE: u8 = 0x7D;
/// `XOR_FLAG` (Annex C.1.2).
pub const STUFF_XOR: u8 = 0x20;
/// The octets of a frame before the payload, after the flag: the two
/// addresses and `payloadLength`.
pub const FRAME_HEADER_LEN: usize = 4;

/// Error during serial framing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SerialError {
    /// The frame ends before its header, payload and `frameCheck`.
    FrameTooShort {
        /// The destuffed octets there are.
        actual: usize,
    },
    /// `frameCheck` is not the computed CRC (Annex C.1.3).
    CrcMismatch {
        /// The computed CRC.
        expected: u16,
        /// The transmitted `frameCheck`.
        actual: u16,
    },
    /// A non-escaped `BEGIN_FLAG` before the payload was completely read
    /// (Annex C.1.3); it begins the next frame.
    BeginFlagInFrame,
    /// An escape sequence ends with the input.
    DanglingEscape,
    /// The byte after `ESCAPE_FLAG` does not stuff 0x7E or 0x7D.
    InvalidEscape {
        /// The byte after `0x7D`.
        byte: u8,
    },
    /// A payload longer than `payloadLength` can say (64 KiB).
    FrameTooLong {
        /// The limit.
        limit: usize,
        /// The length.
        actual: usize,
    },
    /// The payload is not an XRCE Message.
    Decode(XrceError),
    /// The Message does not encode.
    Encode(XrceError),
}

impl core::fmt::Display for SerialError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrameTooShort { actual } => write!(f, "serial frame too short: {actual} bytes"),
            Self::CrcMismatch { expected, actual } => write!(
                f,
                "serial frameCheck mismatch: expected 0x{expected:04x}, actual 0x{actual:04x}"
            ),
            Self::BeginFlagInFrame => {
                write!(
                    f,
                    "serial BEGIN_FLAG before the payload was completely read"
                )
            }
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

/// How `frameCheck` is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FrameCheck {
    /// Annex C.1.1.6: the FCS-16 of RFC 1662 over the frame without the
    /// `BEGIN_FLAG`, that is addresses, `payloadLength` and payload.
    #[default]
    Rfc1662,
    /// CRC-16/ARC over the payload alone, what Micro-XRCE-DDS 3.0.1
    /// computes (see the module doc). Not Annex C; opt-in for its peers.
    Crc16ArcPayload,
}

impl FrameCheck {
    /// The check of `frame`: the four header octets after the flag and the
    /// payload, destuffed.
    #[must_use]
    pub fn compute(self, frame: &[u8]) -> u16 {
        match self {
            Self::Rfc1662 => fcs16(frame),
            Self::Crc16ArcPayload => crc16_arc(frame.get(FRAME_HEADER_LEN..).unwrap_or(&[])),
        }
    }
}

/// A frame of Annex C.1.1, taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialFrame {
    /// `sourceAddress`: the address of the sender.
    pub source: u8,
    /// `remoteAddress`: the address of the recipient.
    pub remote: u8,
    /// The XRCE Message.
    pub payload: Vec<u8>,
}

/// The 16-bit frame check of RFC 1662 (FCS-16): polynomial
/// `x^16 + x^12 + x^5 + 1` in reflected form (0x8408), initial value
/// 0xFFFF, complemented. Its value for "123456789" is 0x906E. The CRC was
/// CCITT-FALSE (not reflected, not complemented), 0x29B1 for the same input,
/// and every frame of a peer after Annex C was refused.
#[must_use]
pub fn fcs16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            if crc & 0x0001 != 0 {
                crc = (crc >> 1) ^ 0x8408;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// CRC-16/ARC: polynomial `x^16 + x^15 + x^2 + 1` in reflected form
/// (0xA001), initial value 0, not complemented; 0xBB3D for "123456789".
#[must_use]
pub fn crc16_arc(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            if crc & 0x0001 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

/// One frame around `payload` from `source` to `remote` (Annex C.1.1).
///
/// # Errors
/// `FrameTooLong` for a payload of more than 65 535 octets.
pub fn encode_frame(source: u8, remote: u8, payload: &[u8]) -> Result<Vec<u8>, SerialError> {
    encode_frame_with(FrameCheck::Rfc1662, source, remote, payload)
}

/// [`encode_frame`] with the frame check `check`.
///
/// # Errors
/// `FrameTooLong` for a payload of more than 65 535 octets.
pub fn encode_frame_with(
    check: FrameCheck,
    source: u8,
    remote: u8,
    payload: &[u8],
) -> Result<Vec<u8>, SerialError> {
    let length = u16::try_from(payload.len()).map_err(|_| SerialError::FrameTooLong {
        limit: usize::from(u16::MAX),
        actual: payload.len(),
    })?;
    let mut raw = Vec::with_capacity(FRAME_HEADER_LEN + payload.len() + 2);
    raw.push(source);
    raw.push(remote);
    raw.extend_from_slice(&length.to_le_bytes());
    raw.extend_from_slice(payload);
    let frame_check = check.compute(&raw);
    raw.extend_from_slice(&frame_check.to_le_bytes());
    let mut out = Vec::with_capacity(raw.len() + 8);
    out.push(FLAG_BYTE);
    for &b in &raw {
        if b == FLAG_BYTE || b == ESCAPE_BYTE {
            out.push(ESCAPE_BYTE);
            out.push(b ^ STUFF_XOR);
        } else {
            out.push(b);
        }
    }
    Ok(out)
}

/// A Message as one frame from `source` to `remote`.
///
/// # Errors
/// `Encode` if the Message does not encode; `FrameTooLong` if it is longer
/// than `payloadLength` can say.
pub fn encode_message(msg: &Message, source: u8, remote: u8) -> Result<Vec<u8>, SerialError> {
    encode_message_with(FrameCheck::Rfc1662, msg, source, remote)
}

/// [`encode_message`] with the frame check `check`.
///
/// # Errors
/// As [`encode_message`].
pub fn encode_message_with(
    check: FrameCheck,
    msg: &Message,
    source: u8,
    remote: u8,
) -> Result<Vec<u8>, SerialError> {
    let payload = msg.encode().map_err(SerialError::Encode)?;
    encode_frame_with(check, source, remote, &payload)
}

/// The frame in `bytes`, the octets after its `BEGIN_FLAG`, stuffed. Octets
/// after the frame are ignored.
///
/// # Errors
/// `BeginFlagInFrame`, `DanglingEscape`, `InvalidEscape`, `FrameTooShort`,
/// `CrcMismatch`.
pub fn decode_frame_inner(bytes: &[u8]) -> Result<SerialFrame, SerialError> {
    decode_frame_inner_with(FrameCheck::Rfc1662, bytes)
}

/// [`decode_frame_inner`] with the frame check `check`.
///
/// # Errors
/// As [`decode_frame_inner`].
pub fn decode_frame_inner_with(
    check: FrameCheck,
    bytes: &[u8],
) -> Result<SerialFrame, SerialError> {
    let mut assembler = FrameAssembler::new(check);
    for &b in bytes {
        if b == FLAG_BYTE {
            return Err(SerialError::BeginFlagInFrame);
        }
        if let Some(done) = assembler.push(b) {
            return done;
        }
    }
    if assembler.escaped {
        return Err(SerialError::DanglingEscape);
    }
    Err(SerialError::FrameTooShort {
        actual: assembler.raw.len(),
    })
}

/// The first frame in `input`, its Message, and the octets after it.
///
/// # Errors
/// `FrameTooShort` without a `BEGIN_FLAG` or a complete frame, the errors of
/// [`decode_frame_inner`], and `Decode` if the payload is no XRCE Message.
pub fn decode_frame(input: &[u8]) -> Result<(SerialFrame, Message, &[u8]), SerialError> {
    decode_frame_with(FrameCheck::Rfc1662, input)
}

/// [`decode_frame`] with the frame check `check`.
///
/// # Errors
/// As [`decode_frame`].
pub fn decode_frame_with(
    check: FrameCheck,
    input: &[u8],
) -> Result<(SerialFrame, Message, &[u8]), SerialError> {
    let begin = input
        .iter()
        .position(|&b| b == FLAG_BYTE)
        .ok_or(SerialError::FrameTooShort { actual: 0 })?;
    let mut assembler = FrameAssembler::new(check);
    for (i, &b) in input[begin + 1..].iter().enumerate() {
        if b == FLAG_BYTE {
            return Err(SerialError::BeginFlagInFrame);
        }
        if let Some(done) = assembler.push(b) {
            let frame = done?;
            let msg = Message::decode(&frame.payload).map_err(SerialError::Decode)?;
            return Ok((frame, msg, &input[begin + 2 + i..]));
        }
    }
    Err(SerialError::FrameTooShort {
        actual: assembler.raw.len(),
    })
}

/// Collects the destuffed octets of one frame after its flag; the frame is
/// complete when header, payload and `frameCheck` are there.
#[derive(Debug)]
struct FrameAssembler {
    check: FrameCheck,
    raw: Vec<u8>,
    escaped: bool,
}

impl FrameAssembler {
    fn new(check: FrameCheck) -> Self {
        Self {
            check,
            raw: Vec::new(),
            escaped: false,
        }
    }

    /// Takes one stuffed octet (not a `BEGIN_FLAG`); the frame, or why it
    /// is refused, once complete.
    fn push(&mut self, b: u8) -> Option<Result<SerialFrame, SerialError>> {
        let octet = if self.escaped {
            self.escaped = false;
            let unstuffed = b ^ STUFF_XOR;
            if unstuffed != FLAG_BYTE && unstuffed != ESCAPE_BYTE {
                return Some(Err(SerialError::InvalidEscape { byte: b }));
            }
            unstuffed
        } else if b == ESCAPE_BYTE {
            self.escaped = true;
            return None;
        } else {
            b
        };
        self.raw.push(octet);
        if self.raw.len() < FRAME_HEADER_LEN {
            return None;
        }
        let length = usize::from(u16::from_le_bytes([self.raw[2], self.raw[3]]));
        let total = FRAME_HEADER_LEN + length + 2;
        if self.raw.len() < total {
            return None;
        }
        let checked = &self.raw[..FRAME_HEADER_LEN + length];
        let expected = self.check.compute(checked);
        let actual = u16::from_le_bytes([self.raw[total - 2], self.raw[total - 1]]);
        if expected != actual {
            return Some(Err(SerialError::CrcMismatch { expected, actual }));
        }
        Some(Ok(SerialFrame {
            source: self.raw[0],
            remote: self.raw[1],
            payload: self.raw[FRAME_HEADER_LEN..FRAME_HEADER_LEN + length].to_vec(),
        }))
    }
}

/// Streaming framer: takes the octets of a serial line as they come and
/// returns the complete frames with their Messages.
#[derive(Debug, Default)]
pub struct SerialFramer {
    /// How `frameCheck` is computed.
    check: FrameCheck,
    /// The frame being read, after its `BEGIN_FLAG`.
    current: Option<FrameAssembler>,
}

/// A frame the framer read: its addresses and Message.
pub type SerialFrameMessage = (SerialFrame, Message);

impl SerialFramer {
    /// Fresh framer with the frame check of Annex C.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fresh framer with the frame check `check`.
    #[must_use]
    pub fn with_frame_check(check: FrameCheck) -> Self {
        Self {
            check,
            current: None,
        }
    }

    /// Back to waiting for a `BEGIN_FLAG`.
    pub fn reset(&mut self) {
        self.current = None;
    }

    /// Takes `data` and returns the frames completed within it, and the
    /// errors of the frames refused. Octets before a `BEGIN_FLAG` are
    /// ignored; a `BEGIN_FLAG` inside a frame refuses that frame and begins
    /// the next (Annex C.1.3).
    pub fn push_bytes(&mut self, data: &[u8]) -> Vec<Result<SerialFrameMessage, SerialError>> {
        let mut out = Vec::new();
        for &b in data {
            if b == FLAG_BYTE {
                if let Some(current) = &self.current {
                    if !current.raw.is_empty() || current.escaped {
                        out.push(Err(SerialError::BeginFlagInFrame));
                    }
                }
                self.current = Some(FrameAssembler::new(self.check));
                continue;
            }
            let Some(current) = self.current.as_mut() else {
                continue;
            };
            if let Some(done) = current.push(b) {
                self.current = None;
                out.push(done.and_then(|frame| {
                    let msg = Message::decode(&frame.payload).map_err(SerialError::Decode)?;
                    Ok((frame, msg))
                }));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::handshake::{ClientRepresentation, XRCE_VERSION};
    use crate::header::{ClientKey, MessageHeader, SessionId, StreamId};
    use crate::serial_number::SerialNumber16;
    use crate::submessages::{CreateClientPayload, HeartbeatPayload, Submessage};

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

    fn create_client() -> Message {
        message_with(
            CreateClientPayload {
                representation: ClientRepresentation {
                    timestamp: None,
                    version: XRCE_VERSION,
                    vendor_id: [0, 0],
                    client_key: ClientKey([0x7E, 0x7D, 0, 0]),
                    session_id: 0,
                    properties: None,
                    requested_mtu: None,
                },
            }
            .into_submessage()
            .unwrap(),
        )
    }

    /// RFC 1662 FCS-16 (CRC-16/X-25): 0x906E for "123456789", the check
    /// value of RFC 1662 Appendix C; it was 0x29B1, CCITT-FALSE.
    #[test]
    fn the_frame_check_is_the_fcs16_of_rfc_1662() {
        assert_eq!(fcs16(b"123456789"), 0x906E);
        assert_eq!(fcs16(&[]), 0x0000);
    }

    /// The CRC of Micro-XRCE-DDS: CRC-16/ARC, 0xBB3D for "123456789", over
    /// the payload alone; a frame of the one check is refused by the other.
    #[test]
    fn the_frame_check_of_micro_xrce_dds_is_taken_when_chosen() {
        assert_eq!(crc16_arc(b"123456789"), 0xBB3D);
        let payload = [0x10, 0x20, 0x30];
        let frame = encode_frame_with(FrameCheck::Crc16ArcPayload, 0, 1, &payload).unwrap();
        let check = crc16_arc(&payload).to_le_bytes();
        assert_eq!(&frame[frame.len() - 2..], &check);
        let back = decode_frame_inner_with(FrameCheck::Crc16ArcPayload, &frame[1..]).unwrap();
        assert_eq!(back.payload, payload);
        assert!(matches!(
            decode_frame_inner(&frame[1..]),
            Err(SerialError::CrcMismatch { .. })
        ));
        let mut framer = SerialFramer::with_frame_check(FrameCheck::Crc16ArcPayload);
        let ours = encode_message(&create_client(), 1, 0).unwrap();
        assert!(matches!(
            framer.push_bytes(&ours).as_slice(),
            [Err(SerialError::CrcMismatch { .. })]
        ));
    }

    /// Annex C.1.1: 7E, sourceAddress, remoteAddress, payloadLength little
    /// endian, payload, frameCheck little endian over the four header octets
    /// and the payload.
    #[test]
    fn a_frame_carries_the_addresses_the_length_and_a_little_endian_check() {
        let frame = encode_frame(0x01, 0x02, &[0x10, 0x20, 0x30]).unwrap();
        let check = fcs16(&[0x01, 0x02, 0x03, 0x00, 0x10, 0x20, 0x30]).to_le_bytes();
        assert_eq!(
            frame,
            alloc::vec![
                0x7E, 0x01, 0x02, 0x03, 0x00, 0x10, 0x20, 0x30, check[0], check[1]
            ]
        );
        let back = decode_frame_inner(&frame[1..]).unwrap();
        assert_eq!(
            back,
            SerialFrame {
                source: 1,
                remote: 2,
                payload: alloc::vec![0x10, 0x20, 0x30]
            }
        );
    }

    /// Annex C.1.2: 0x7E and 0x7D after the flag are stuffed, also in the
    /// addresses, the length and the check.
    #[test]
    fn flag_and_escape_octets_are_stuffed_everywhere_after_the_flag() {
        let payload = alloc::vec![0x7E, 0x7D, 0x00, 0x7E];
        let frame = encode_frame(0x7E, 0x7D, &payload).unwrap();
        assert_eq!(&frame[..5], &[0x7E, 0x7D, 0x5E, 0x7D, 0x5D]);
        assert!(!frame[1..].contains(&FLAG_BYTE));
        let back = decode_frame_inner(&frame[1..]).unwrap();
        assert_eq!(
            (back.source, back.remote, back.payload),
            (0x7E, 0x7D, payload)
        );
    }

    /// Annex C.1.3: a frame whose frameCheck is not the computed CRC is
    /// dropped.
    #[test]
    fn a_frame_with_a_wrong_check_is_refused() {
        let mut frame = encode_frame(1, 2, &[1, 2, 3, 4]).unwrap();
        let last = frame.len() - 1;
        frame[last] ^= 0x01;
        assert!(matches!(
            decode_frame_inner(&frame[1..]),
            Err(SerialError::CrcMismatch { .. })
        ));
    }

    /// Annex C.1.3: a non-escaped BEGIN_FLAG before the payload is read
    /// drops the frame, and begins the next one.
    #[test]
    fn a_begin_flag_inside_a_frame_drops_it_and_begins_the_next() {
        let good = encode_message(&create_client(), 3, 0).unwrap();
        let mut bytes = good[..6].to_vec();
        bytes.extend_from_slice(&good);
        let mut framer = SerialFramer::new();
        let out = framer.push_bytes(&bytes);
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0], Err(SerialError::BeginFlagInFrame));
        let (frame, msg) = out[1].clone().unwrap();
        assert_eq!(frame.source, 3);
        assert_eq!(msg, create_client());
    }

    #[test]
    fn the_framer_takes_frames_split_and_back_to_back_and_skips_noise() {
        let a = encode_message(&create_client(), 1, 0).unwrap();
        let hb = message_with(
            HeartbeatPayload {
                first_unacked_seq_nr: 1,
                last_unacked_seq_nr: 7,
                stream_id: StreamId::BUILTIN_RELIABLE.0,
            }
            .into_submessage()
            .unwrap(),
        );
        let b = encode_message(&hb, 2, 0).unwrap();
        let mut stream = alloc::vec![0x00, 0x55];
        stream.extend_from_slice(&a);
        stream.extend_from_slice(&b);
        let mut framer = SerialFramer::new();
        let mut got = Vec::new();
        for chunk in stream.chunks(3) {
            got.extend(framer.push_bytes(chunk));
        }
        let got: Vec<_> = got.into_iter().map(Result::unwrap).collect();
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].0.source, &got[0].1), (1, &create_client()));
        assert_eq!((got[1].0.source, &got[1].1), (2, &hb));
    }

    #[test]
    fn decode_frame_returns_the_rest_after_the_frame() {
        let mut bytes = encode_message(&create_client(), 5, 6).unwrap();
        bytes.extend_from_slice(&[0xAA, 0xBB]);
        let (frame, msg, rest) = decode_frame(&bytes).unwrap();
        assert_eq!((frame.source, frame.remote), (5, 6));
        assert_eq!(msg, create_client());
        assert_eq!(rest, &[0xAA, 0xBB]);
    }

    #[test]
    fn escape_errors_and_short_frames_are_refused() {
        assert!(matches!(
            decode_frame_inner(&[0x01, ESCAPE_BYTE]),
            Err(SerialError::DanglingEscape)
        ));
        assert!(matches!(
            decode_frame_inner(&[ESCAPE_BYTE, 0xFF]),
            Err(SerialError::InvalidEscape { byte: 0xFF })
        ));
        assert!(matches!(
            decode_frame_inner(&[0x01, 0x02, 0x05]),
            Err(SerialError::FrameTooShort { actual: 3 })
        ));
        assert!(matches!(
            encode_frame(0, 0, &alloc::vec![0u8; 70_000]),
            Err(SerialError::FrameTooLong { .. })
        ));
    }
}
