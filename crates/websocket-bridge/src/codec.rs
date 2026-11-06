// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket wire codec — RFC 6455 §5.2 + §5.3.

use alloc::vec::Vec;
use core::fmt;

use crate::frame::{Frame, Opcode};
use crate::masking::apply_mask;

/// Codec error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Header too short.
    HeaderTooShort,
    /// Spec §5.2 — payload length 126 with value <= 125 or 127 with
    /// value <= 65535. "minimal number of bytes MUST be used".
    NonMinimalLength,
    /// Spec §5.2 — payload length 127 with the MSB set.
    PayloadLengthMsbSet,
    /// The frame body does not fit into the available bytes.
    PayloadTruncated,
    /// The masking key does not fit into the available bytes.
    MaskingKeyTruncated,
    /// Spec §5.5 — a control frame with payload > 125 bytes is illegal.
    ControlFrameTooLong,
    /// Spec §5.5 — a control frame must have FIN=1 (no fragmentation).
    FragmentedControlFrame,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooShort => f.write_str("header too short"),
            Self::NonMinimalLength => f.write_str("non-minimal payload length encoding"),
            Self::PayloadLengthMsbSet => f.write_str("64-bit payload length MSB set"),
            Self::PayloadTruncated => f.write_str("payload truncated"),
            Self::MaskingKeyTruncated => f.write_str("masking key truncated"),
            Self::ControlFrameTooLong => f.write_str("control frame payload > 125 bytes"),
            Self::FragmentedControlFrame => f.write_str("control frame with FIN=0"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CodecError {}

/// Encodes a [`Frame`] into the WebSocket wire byte slice.
///
/// If `frame.masking_key` is set, the payload is XOR-masked during
/// encoding (Spec §5.3).
///
/// # Errors
/// * [`CodecError::ControlFrameTooLong`] if a control frame has a
///   payload > 125 bytes (Spec §5.5).
/// * [`CodecError::FragmentedControlFrame`] if a control frame has
///   FIN=0 (Spec §5.5).
pub fn encode(frame: &Frame) -> Result<Vec<u8>, CodecError> {
    if frame.opcode.is_control() {
        if !frame.fin {
            return Err(CodecError::FragmentedControlFrame);
        }
        if frame.payload.len() > 125 {
            return Err(CodecError::ControlFrameTooLong);
        }
    }
    let mut out = Vec::with_capacity(2 + 8 + 4 + frame.payload.len());

    // Byte 0: FIN | RSV1 | RSV2 | RSV3 | Opcode (4 bits).
    let mut byte0 = frame.opcode.to_bits() & 0x0F;
    if frame.fin {
        byte0 |= 0x80;
    }
    if frame.rsv1 {
        byte0 |= 0x40;
    }
    if frame.rsv2 {
        byte0 |= 0x20;
    }
    if frame.rsv3 {
        byte0 |= 0x10;
    }
    out.push(byte0);

    // Byte 1: MASK | payload length (7 bits).
    let payload_len = frame.payload.len();
    let masked = frame.masking_key.is_some();
    let (len7, ext_len) = encode_payload_length(payload_len);
    let byte1 = (if masked { 0x80 } else { 0x00 }) | (len7 & 0x7F);
    out.push(byte1);
    out.extend_from_slice(&ext_len);

    // Masking key (4 bytes if MASK=1).
    if let Some(key) = frame.masking_key {
        out.extend_from_slice(&key);
        // Emit the payload data XOR-masked.
        let mut masked_payload = frame.payload.clone();
        apply_mask(&mut masked_payload, key);
        out.extend_from_slice(&masked_payload);
    } else {
        out.extend_from_slice(&frame.payload);
    }

    Ok(out)
}

/// Spec §5.2 — payload length encoding with "minimal number of bytes".
/// Returns `(7-bit value, extended bytes)`.
fn encode_payload_length(len: usize) -> (u8, Vec<u8>) {
    if len <= 125 {
        #[allow(clippy::cast_possible_truncation)]
        (len as u8, Vec::new())
    } else if len <= 0xFFFF {
        #[allow(clippy::cast_possible_truncation)]
        (126, (len as u16).to_be_bytes().to_vec())
    } else {
        // Spec §5.2 — 64-bit length with MSB=0.
        let bytes = (len as u64).to_be_bytes();
        (127, bytes.to_vec())
    }
}

/// Decodes a [`Frame`] from the WebSocket wire byte slice.
/// If the MASK bit is set, the payload is automatically unmasked
/// during decoding.
///
/// Returns `(frame, consumed_bytes)`.
///
/// # Errors
/// See [`CodecError`].
pub fn decode(bytes: &[u8]) -> Result<(Frame, usize), CodecError> {
    if bytes.len() < 2 {
        return Err(CodecError::HeaderTooShort);
    }
    let byte0 = bytes[0];
    let fin = (byte0 & 0x80) != 0;
    let rsv1 = (byte0 & 0x40) != 0;
    let rsv2 = (byte0 & 0x20) != 0;
    let rsv3 = (byte0 & 0x10) != 0;
    let opcode = Opcode::from_bits(byte0 & 0x0F);

    let byte1 = bytes[1];
    let masked = (byte1 & 0x80) != 0;
    let len7 = byte1 & 0x7F;

    let mut cursor = 2usize;
    let payload_len = match len7.cmp(&126) {
        core::cmp::Ordering::Less => usize::from(len7),
        core::cmp::Ordering::Equal => {
            if bytes.len() < cursor + 2 {
                return Err(CodecError::HeaderTooShort);
            }
            let v = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]);
            cursor += 2;
            if v <= 125 {
                return Err(CodecError::NonMinimalLength);
            }
            usize::from(v)
        }
        core::cmp::Ordering::Greater => {
            // len7 == 127.
            if bytes.len() < cursor + 8 {
                return Err(CodecError::HeaderTooShort);
            }
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[cursor..cursor + 8]);
            let v = u64::from_be_bytes(buf);
            cursor += 8;
            if (v & 0x8000_0000_0000_0000) != 0 {
                return Err(CodecError::PayloadLengthMsbSet);
            }
            if v <= 0xFFFF {
                return Err(CodecError::NonMinimalLength);
            }
            usize::try_from(v).map_err(|_| CodecError::PayloadTruncated)?
        }
    };

    if opcode.is_control() {
        if !fin {
            return Err(CodecError::FragmentedControlFrame);
        }
        if payload_len > 125 {
            return Err(CodecError::ControlFrameTooLong);
        }
    }

    let masking_key = if masked {
        if bytes.len() < cursor + 4 {
            return Err(CodecError::MaskingKeyTruncated);
        }
        let key = [
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ];
        cursor += 4;
        Some(key)
    } else {
        None
    };

    if bytes.len() < cursor + payload_len {
        return Err(CodecError::PayloadTruncated);
    }
    let mut payload = bytes[cursor..cursor + payload_len].to_vec();
    cursor += payload_len;

    if let Some(key) = masking_key {
        apply_mask(&mut payload, key);
    }

    Ok((
        Frame {
            fin,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            masking_key,
            payload,
        },
        cursor,
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ws_5_2_smallest_text_frame_encodes_to_2_byte_header_plus_payload() {
        // RFC 6455 §5.2 — payload-len <= 125 ⇒ 2 byte header.
        let bytes = encode(&Frame::text("hi")).expect("encode");
        assert_eq!(bytes.len(), 4);
        // FIN=1, opcode=1.
        assert_eq!(bytes[0], 0x81);
        // MASK=0, len=2.
        assert_eq!(bytes[1], 0x02);
        assert_eq!(&bytes[2..], b"hi");
    }

    #[test]
    fn ws_5_2_medium_payload_uses_extended_16_bit_length() {
        // Spec §5.2 — len 126..=65535 ⇒ marker 126 + 2 byte BE.
        let payload = alloc::vec![0xAA; 200];
        let f = Frame::binary(payload.clone());
        let bytes = encode(&f).expect("encode");
        assert_eq!(bytes[0], 0x82);
        assert_eq!(bytes[1] & 0x7F, 126);
        assert_eq!(&bytes[2..4], &200u16.to_be_bytes());
        assert_eq!(&bytes[4..], &payload[..]);
    }

    #[test]
    fn ws_5_2_large_payload_uses_extended_64_bit_length() {
        // Spec §5.2 — len > 65535 ⇒ marker 127 + 8 byte BE with MSB=0.
        let payload = alloc::vec![0xBB; 70_000];
        let f = Frame::binary(payload.clone());
        let bytes = encode(&f).expect("encode");
        assert_eq!(bytes[1] & 0x7F, 127);
        let mut len_buf = [0u8; 8];
        len_buf.copy_from_slice(&bytes[2..10]);
        assert_eq!(u64::from_be_bytes(len_buf), 70_000);
        // MSB=0.
        assert_eq!(bytes[2] & 0x80, 0);
    }

    #[test]
    fn ws_5_5_1_close_frame_carries_status_code() {
        // §5.5.1: the body of a Close frame starts with the status code,
        // two octets in network byte order; 1000 is 0x03e8 (§7.4.1).
        let wire = [0x88, 0x02, 0x03, 0xe8];
        assert_eq!(encode(&Frame::close(1000, "")).expect("encode"), wire);
        let (parsed, consumed) = decode(&wire).expect("decode");
        assert_eq!(parsed.opcode, Opcode::Close);
        assert_eq!(parsed.payload, [0x03, 0xe8]);
        assert_eq!(consumed, wire.len());
    }

    // RFC 6455 §5.7: the examples, decoded and encoded.
    const HELLO: &[u8] = b"Hello";
    const MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];
    const MASKED_HELLO: [u8; 5] = [0x7f, 0x9f, 0x4d, 0x51, 0x58];

    fn decoded(wire: &[u8]) -> Frame {
        let (frame, consumed) = decode(wire).expect("decode");
        assert_eq!(consumed, wire.len());
        frame
    }

    #[test]
    fn ws_5_7_a_single_frame_text_message_unmasked_and_masked() {
        let unmasked = [&[0x81, 0x05][..], HELLO].concat();
        assert_eq!(encode(&Frame::text("Hello")).expect("encode"), unmasked);
        assert_eq!(decoded(&unmasked), Frame::text("Hello"));
        // §5.3: the masked payload is the payload XOR the key.
        let masked = [&[0x81, 0x85][..], &MASK[..], &MASKED_HELLO[..]].concat();
        let frame = Frame::text("Hello").with_mask(MASK);
        assert_eq!(encode(&frame).expect("encode"), masked);
        assert_eq!(decoded(&masked), frame);
    }

    #[test]
    fn ws_5_7_a_fragmented_text_message() {
        // §5.4: "Hel" with FIN 0 and the text opcode, "lo" with FIN 1 and
        // the continuation opcode.
        let first = [0x01, 0x03, 0x48, 0x65, 0x6c];
        let last = [0x80, 0x02, 0x6c, 0x6f];
        let mut hel = Frame::text("Hel");
        hel.fin = false;
        let mut lo = Frame::text("lo");
        lo.opcode = Opcode::Continuation;
        assert_eq!(encode(&hel).expect("encode"), first);
        assert_eq!(encode(&lo).expect("encode"), last);
        assert_eq!(decoded(&first), hel);
        assert_eq!(decoded(&last), lo);
    }

    #[test]
    fn ws_5_7_an_unmasked_ping_and_the_masked_pong_with_its_body() {
        // §5.5.2, §5.5.3: the Pong carries the Application data of the
        // Ping it answers.
        let ping_wire = [&[0x89, 0x05][..], HELLO].concat();
        let pong_wire = [&[0x8a, 0x85][..], &MASK[..], &MASKED_HELLO[..]].concat();
        let ping = decoded(&ping_wire);
        assert_eq!(ping, Frame::ping(HELLO.to_vec()));
        assert_eq!(encode(&ping).expect("encode"), ping_wire);
        let pong = Frame::pong(ping.payload.clone()).with_mask(MASK);
        assert_eq!(encode(&pong).expect("encode"), pong_wire);
        assert_eq!(decoded(&pong_wire), pong);
    }

    #[test]
    fn ws_5_7_a_256_and_a_65536_octet_binary_frame() {
        // §5.2: 126 and a 16-bit length, 127 and a 64-bit length.
        for (header, size) in [
            (alloc::vec![0x82, 0x7e, 0x01, 0x00], 256usize),
            (
                alloc::vec![0x82, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00],
                65_536,
            ),
        ] {
            let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
            let wire = [header, data.clone()].concat();
            assert_eq!(encode(&Frame::binary(data.clone())).expect("encode"), wire);
            assert_eq!(decoded(&wire), Frame::binary(data));
        }
    }

    #[test]
    fn header_too_short_decode_fails() {
        assert_eq!(decode(&[]), Err(CodecError::HeaderTooShort));
        assert_eq!(decode(&[0x81]), Err(CodecError::HeaderTooShort));
    }

    #[test]
    fn ws_5_2_extended_16_bit_length_truncated_fails() {
        // Marker 126 without the 2 length bytes.
        assert_eq!(decode(&[0x81, 0x7E]), Err(CodecError::HeaderTooShort));
    }

    #[test]
    fn ws_5_2_extended_64_bit_length_msb_set_rejected() {
        // Spec §5.2 — MSB MUST be 0.
        let bytes = [0x82u8, 0x7F, 0x80, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(decode(&bytes), Err(CodecError::PayloadLengthMsbSet));
    }

    #[test]
    fn ws_5_2_non_minimal_16_bit_length_rejected() {
        // Spec §5.2 — "minimal number of bytes MUST be used".
        // Marker 126 with value 100 is non-minimal.
        let bytes = [0x82u8, 0x7E, 0, 100, 0xAA, 0xBB];
        assert_eq!(decode(&bytes), Err(CodecError::NonMinimalLength));
    }

    #[test]
    fn ws_5_2_non_minimal_64_bit_length_rejected() {
        // Marker 127 with value 65000 is non-minimal (would be 16-bit).
        let mut bytes = alloc::vec![0x82u8, 0x7F];
        bytes.extend_from_slice(&65000u64.to_be_bytes());
        assert_eq!(decode(&bytes), Err(CodecError::NonMinimalLength));
    }

    #[test]
    fn ws_5_5_control_frame_with_long_payload_rejected_on_encode() {
        // Spec §5.5 — control frame payload <= 125.
        let f = Frame::ping(alloc::vec![0; 200]);
        assert_eq!(encode(&f), Err(CodecError::ControlFrameTooLong));
    }

    #[test]
    fn ws_5_5_fragmented_control_frame_rejected_on_encode() {
        // Spec §5.5 — control frames MUST NOT be fragmented (FIN=1).
        let mut f = Frame::ping(alloc::vec![1, 2]);
        f.fin = false;
        assert_eq!(encode(&f), Err(CodecError::FragmentedControlFrame));
    }

    #[test]
    fn masked_frame_without_key_bytes_decode_fails() {
        // MASK=1 but only 2 header bytes present.
        let bytes = [0x81u8, 0x80];
        assert_eq!(decode(&bytes), Err(CodecError::MaskingKeyTruncated));
    }

    #[test]
    fn payload_truncation_decode_fails() {
        // FIN+Text+len=10, but only 2 payload bytes.
        let bytes = [0x81u8, 0x0A, 0xAA, 0xBB];
        assert_eq!(decode(&bytes), Err(CodecError::PayloadTruncated));
    }

    #[test]
    fn ws_5_2_rsv_bits_propagate_to_decoded_frame() {
        // Spec §5.2 — RSV1-3 are not a codec validation topic
        // (extension negotiation), but must be passed through 1:1.
        // FIN 0x80, RSV1 0x40, RSV3 0x10, opcode 0x2.
        let wire = [0xd2, 0x01, 0x01];
        let mut f = Frame::binary(alloc::vec![1]);
        f.rsv1 = true;
        f.rsv3 = true;
        assert_eq!(encode(&f).expect("encode"), wire);
        let (parsed, _) = decode(&wire).expect("decode");
        assert!(parsed.rsv1);
        assert!(!parsed.rsv2);
        assert!(parsed.rsv3);
    }
}
