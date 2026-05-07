// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket Wire-Codec — RFC 6455 §5.2 + §5.3.

use alloc::vec::Vec;
use core::fmt;

use crate::frame::{Frame, Opcode};
use crate::masking::apply_mask;

/// Codec-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Header zu kurz.
    HeaderTooShort,
    /// Spec §5.2 — payload length 126 mit Wert <= 125 oder 127 mit
    /// Wert <= 65535. "minimal number of bytes MUST be used".
    NonMinimalLength,
    /// Spec §5.2 — payload length 127 mit gesetztem MSB.
    PayloadLengthMsbSet,
    /// Frame-Body reicht nicht in die verfuegbaren Bytes.
    PayloadTruncated,
    /// Masking-Key reicht nicht in die verfuegbaren Bytes.
    MaskingKeyTruncated,
    /// Spec §5.5 — Control Frame mit payload > 125 Bytes ist illegal.
    ControlFrameTooLong,
    /// Spec §5.5 — Control Frame muss FIN=1 (kein Fragmentieren).
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

/// Encodiert einen [`Frame`] zum WebSocket-Wire-Byte-Slice.
///
/// Wenn `frame.masking_key` gesetzt ist, wird das Payload waehrend
/// des Encode XOR-maskiert (Spec §5.3).
///
/// # Errors
/// * [`CodecError::ControlFrameTooLong`] wenn Control-Frame mit
///   Payload > 125 Bytes (Spec §5.5).
/// * [`CodecError::FragmentedControlFrame`] wenn Control-Frame mit
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

    // Byte 1: MASK | Payload-Length (7 bits).
    let payload_len = frame.payload.len();
    let masked = frame.masking_key.is_some();
    let (len7, ext_len) = encode_payload_length(payload_len);
    let byte1 = (if masked { 0x80 } else { 0x00 }) | (len7 & 0x7F);
    out.push(byte1);
    out.extend_from_slice(&ext_len);

    // Masking-Key (4 bytes wenn MASK=1).
    if let Some(key) = frame.masking_key {
        out.extend_from_slice(&key);
        // Payload-Daten XOR-maskiert ausgeben.
        let mut masked_payload = frame.payload.clone();
        apply_mask(&mut masked_payload, key);
        out.extend_from_slice(&masked_payload);
    } else {
        out.extend_from_slice(&frame.payload);
    }

    Ok(out)
}

/// Spec §5.2 — Payload-Length-Encoding mit "minimal number of bytes".
/// Liefert `(7-bit-Wert, Extended-Bytes)`.
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

/// Decodiert einen [`Frame`] aus dem WebSocket-Wire-Byte-Slice.
/// Wenn das MASK-Bit gesetzt ist, wird das Payload waehrend des Decode
/// automatisch demaskiert.
///
/// Liefert `(frame, consumed_bytes)`.
///
/// # Errors
/// Siehe [`CodecError`].
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
    fn smallest_text_frame_encodes_to_2_byte_header_plus_payload() {
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
    fn medium_payload_uses_extended_16_bit_length() {
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
    fn large_payload_uses_extended_64_bit_length() {
        // Spec §5.2 — len > 65535 ⇒ marker 127 + 8 byte BE mit MSB=0.
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
    fn round_trip_unmasked_text() {
        let f = Frame::text("hello world");
        let bytes = encode(&f).expect("encode");
        let (parsed, consumed) = decode(&bytes).expect("decode");
        assert_eq!(parsed, f);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn round_trip_masked_payload_unmasked_on_decode() {
        // Spec §5.3 — payload XOR'd with key on encode, XOR'd back on
        // decode.
        let f = Frame::text("masked!").with_mask([0x12, 0x34, 0x56, 0x78]);
        let bytes = encode(&f).expect("encode");
        // Wire-Bytes ungleich Plaintext.
        assert_ne!(&bytes[6..], b"masked!");
        let (parsed, _) = decode(&bytes).expect("decode");
        assert_eq!(parsed.payload, b"masked!");
        assert_eq!(parsed.masking_key, Some([0x12, 0x34, 0x56, 0x78]));
    }

    #[test]
    fn round_trip_medium_and_large_payloads() {
        for size in [126, 200, 65535, 65536, 100_000] {
            let f = Frame::binary(alloc::vec![0xAB; size]);
            let bytes = encode(&f).expect("encode");
            let (parsed, _) = decode(&bytes).expect("decode");
            assert_eq!(parsed.payload.len(), size);
        }
    }

    #[test]
    fn ping_frame_round_trip() {
        let f = Frame::ping(alloc::vec![1, 2, 3]);
        let bytes = encode(&f).expect("encode");
        let (parsed, _) = decode(&bytes).expect("decode");
        assert_eq!(parsed.opcode, Opcode::Ping);
        assert_eq!(parsed.payload, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn close_frame_carries_status_code() {
        let f = Frame::close(1000, "");
        let bytes = encode(&f).expect("encode");
        let (parsed, _) = decode(&bytes).expect("decode");
        assert_eq!(parsed.opcode, Opcode::Close);
        assert_eq!(&parsed.payload[..2], &1000u16.to_be_bytes());
    }

    #[test]
    fn header_too_short_decode_fails() {
        assert_eq!(decode(&[]), Err(CodecError::HeaderTooShort));
        assert_eq!(decode(&[0x81]), Err(CodecError::HeaderTooShort));
    }

    #[test]
    fn extended_16_bit_length_truncated_fails() {
        // Marker 126 ohne 2 Length-Bytes.
        assert_eq!(decode(&[0x81, 0x7E]), Err(CodecError::HeaderTooShort));
    }

    #[test]
    fn extended_64_bit_length_msb_set_rejected() {
        // Spec §5.2 — MSB MUST be 0.
        let bytes = [0x82u8, 0x7F, 0x80, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(decode(&bytes), Err(CodecError::PayloadLengthMsbSet));
    }

    #[test]
    fn non_minimal_16_bit_length_rejected() {
        // Spec §5.2 — "minimal number of bytes MUST be used".
        // Marker 126 mit Wert 100 ist non-minimal.
        let bytes = [0x82u8, 0x7E, 0, 100, 0xAA, 0xBB];
        assert_eq!(decode(&bytes), Err(CodecError::NonMinimalLength));
    }

    #[test]
    fn non_minimal_64_bit_length_rejected() {
        // Marker 127 mit Wert 65000 ist non-minimal (waere 16-bit).
        let mut bytes = alloc::vec![0x82u8, 0x7F];
        bytes.extend_from_slice(&65000u64.to_be_bytes());
        assert_eq!(decode(&bytes), Err(CodecError::NonMinimalLength));
    }

    #[test]
    fn control_frame_with_long_payload_rejected_on_encode() {
        // Spec §5.5 — control frame payload <= 125.
        let f = Frame::ping(alloc::vec![0; 200]);
        assert_eq!(encode(&f), Err(CodecError::ControlFrameTooLong));
    }

    #[test]
    fn fragmented_control_frame_rejected_on_encode() {
        // Spec §5.5 — control frames MUST NOT be fragmented (FIN=1).
        let mut f = Frame::ping(alloc::vec![1, 2]);
        f.fin = false;
        assert_eq!(encode(&f), Err(CodecError::FragmentedControlFrame));
    }

    #[test]
    fn masked_frame_without_key_bytes_decode_fails() {
        // MASK=1 aber nur 2 Header-Bytes vorhanden.
        let bytes = [0x81u8, 0x80];
        assert_eq!(decode(&bytes), Err(CodecError::MaskingKeyTruncated));
    }

    #[test]
    fn payload_truncation_decode_fails() {
        // FIN+Text+len=10, aber nur 2 Payload-Bytes.
        let bytes = [0x81u8, 0x0A, 0xAA, 0xBB];
        assert_eq!(decode(&bytes), Err(CodecError::PayloadTruncated));
    }

    #[test]
    fn rsv_bits_propagate_to_decoded_frame() {
        // Spec §5.2 — RSV1-3 sind kein Codec-Validation-Topic
        // (Extension-Negotiation), aber muessen 1:1 durchgereicht
        // werden.
        let mut f = Frame::binary(alloc::vec![1]);
        f.rsv1 = true;
        f.rsv3 = true;
        let bytes = encode(&f).expect("encode");
        let (parsed, _) = decode(&bytes).expect("decode");
        assert!(parsed.rsv1);
        assert!(!parsed.rsv2);
        assert!(parsed.rsv3);
    }

    #[test]
    fn fin_zero_text_frame_round_trip() {
        // Spec §5.4 — Fragmentierung: FIN=0 + Text-Opcode → Caller
        // sendet Continuation-Frames mit FIN=1 fuer letzten.
        let mut f = Frame::text("part-1");
        f.fin = false;
        let bytes = encode(&f).expect("encode");
        let (parsed, _) = decode(&bytes).expect("decode");
        assert!(!parsed.fin);
        assert_eq!(parsed.opcode, Opcode::Text);
    }
}
