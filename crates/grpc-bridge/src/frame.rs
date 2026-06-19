// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! gRPC Length-Prefixed-Message — Spec §"Requests" + §"Responses".

use alloc::vec::Vec;
use core::fmt;

/// Spec — `Length-Prefixed-Message` Header-Layout:
/// ```text
/// +-----+--------+--------+--------+--------+
/// | CF  |       Message-Length              |
/// +-----+--------+--------+--------+--------+
/// |          Message bytes...                |
/// +------------------------------------------+
/// ```
/// CF = Compressed-Flag (1 byte; 0 = uncompressed, 1 = compressed).
/// Message-Length = 4-byte big-endian unsigned integer.
///
/// gRPC-Web extension: CF with MSB=1 (`0x80`) marks a trailers frame
/// (LPM-encoded HTTP trailer section).
pub const HEADER_LEN: usize = 5;

/// Codec error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// Header < 5 bytes.
    HeaderTooShort,
    /// The message body does not fit into the available bytes.
    BodyTruncated,
    /// Message > u32::MAX.
    MessageTooLarge,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooShort => f.write_str("LPM header < 5 bytes"),
            Self::BodyTruncated => f.write_str("LPM body truncated"),
            Self::MessageTooLarge => f.write_str("message length exceeds u32"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameError {}

/// Encodes a message body as a Length-Prefixed-Message.
///
/// `compressed` sets the Compressed-Flag byte (Spec §"Compressed-
/// Flag"). If `true`, the `Message-Encoding` header (caller) MUST be set
/// to a non-identity encoding value.
///
/// # Errors
/// `MessageTooLarge` if `payload.len() > u32::MAX`.
pub fn encode_message(payload: &[u8], compressed: bool) -> Result<Vec<u8>, FrameError> {
    if payload.len() > u32::MAX as usize {
        return Err(FrameError::MessageTooLarge);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.push(if compressed { 1 } else { 0 });
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// Encodes a gRPC-Web trailers frame (CF-MSB=1).
///
/// `trailers` is the UTF-8-encoded trailer section
/// (`grpc-status: 0\r\ngrpc-message: \r\n` etc.).
///
/// # Errors
/// `MessageTooLarge` if `trailers.len() > u32::MAX`.
pub fn encode_web_trailers(trailers: &[u8]) -> Result<Vec<u8>, FrameError> {
    if trailers.len() > u32::MAX as usize {
        return Err(FrameError::MessageTooLarge);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + trailers.len());
    out.push(0x80); // gRPC-Web trailer marker.
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(trailers.len() as u32).to_be_bytes());
    out.extend_from_slice(trailers);
    Ok(out)
}

/// Decodes a Length-Prefixed-Message. Returns
/// `(compressed_flag, message_bytes, consumed_bytes)`.
///
/// # Errors
/// Siehe [`FrameError`].
pub fn decode_message(bytes: &[u8]) -> Result<(u8, Vec<u8>, usize), FrameError> {
    if bytes.len() < HEADER_LEN {
        return Err(FrameError::HeaderTooShort);
    }
    let flag = bytes[0];
    let len = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
    let total = HEADER_LEN + len;
    if bytes.len() < total {
        return Err(FrameError::BodyTruncated);
    }
    Ok((flag, bytes[HEADER_LEN..total].to_vec(), total))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_message_encodes_to_5_byte_header() {
        // Spec §"Length-Prefixed-Message" — 5-byte header, length=0.
        let bytes = encode_message(&[], false).expect("encode");
        assert_eq!(bytes, alloc::vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn uncompressed_message_has_compressed_flag_zero() {
        let bytes = encode_message(b"hello", false).expect("encode");
        assert_eq!(bytes[0], 0);
        assert_eq!(&bytes[1..5], &5u32.to_be_bytes());
        assert_eq!(&bytes[5..], b"hello");
    }

    #[test]
    fn compressed_message_has_compressed_flag_one() {
        let bytes = encode_message(b"compressed", true).expect("encode");
        assert_eq!(bytes[0], 1);
    }

    #[test]
    fn round_trip_message() {
        for payload in [
            alloc::vec![],
            alloc::vec![0u8],
            alloc::vec![1, 2, 3, 4],
            alloc::vec![0xAB; 1000],
        ] {
            let bytes = encode_message(&payload, false).expect("encode");
            let (flag, decoded, consumed) = decode_message(&bytes).expect("decode");
            assert_eq!(flag, 0);
            assert_eq!(decoded, payload);
            assert_eq!(consumed, bytes.len());
        }
    }

    #[test]
    fn message_length_uses_big_endian_4_bytes() {
        // Spec — 4-byte BE.
        let bytes = encode_message(&alloc::vec![0; 256], false).expect("encode");
        // length=256 -> 0x00 0x00 0x01 0x00.
        assert_eq!(&bytes[1..5], &[0x00, 0x00, 0x01, 0x00]);
    }

    #[test]
    fn header_too_short_decode_fails() {
        assert_eq!(decode_message(&[]), Err(FrameError::HeaderTooShort));
        assert_eq!(decode_message(&[0; 4]), Err(FrameError::HeaderTooShort));
    }

    #[test]
    fn body_truncated_decode_fails() {
        // Length=10, but only 3 body bytes.
        let bytes = [0u8, 0, 0, 0, 10, 1, 2, 3];
        assert_eq!(decode_message(&bytes), Err(FrameError::BodyTruncated));
    }

    #[test]
    fn web_trailers_encoded_with_msb_set() {
        // gRPC-Web — CF-MSB=1 (0x80).
        let trailers = b"grpc-status: 0\r\n";
        let bytes = encode_web_trailers(trailers).expect("encode");
        assert_eq!(bytes[0], 0x80);
        assert_eq!(&bytes[1..5], &(trailers.len() as u32).to_be_bytes());
    }

    #[test]
    fn back_to_back_messages_can_be_decoded_sequentially() {
        // Spec: "*Length-Prefixed-Message" — stream of messages.
        let m1 = encode_message(b"first", false).expect("encode");
        let m2 = encode_message(b"second", false).expect("encode");
        let mut combined = m1.clone();
        combined.extend_from_slice(&m2);

        let (_, decoded1, consumed1) = decode_message(&combined).expect("decode 1");
        assert_eq!(decoded1, b"first");
        let (_, decoded2, _) = decode_message(&combined[consumed1..]).expect("decode 2");
        assert_eq!(decoded2, b"second");
    }
}
