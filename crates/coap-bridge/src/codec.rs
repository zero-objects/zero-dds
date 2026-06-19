// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CoAP wire codec — RFC 7252 §3 + §3.1.
//!
//! Implements encode/decode of the complete message including:
//! * 4-byte fixed header (Ver/T/TKL/Code/MID).
//! * Token (0..=8 bytes).
//! * Options with delta encoding + extended-length mechanism (13/14).
//! * 0xFF payload marker.
//! * Payload up to the end of the datagram.

use alloc::vec::Vec;
use core::fmt;

use crate::message::{CoapCode, CoapMessage, MessageType};
use crate::option::{CoapOption, OptionValue};

/// Codec error — spec-conformant "MUST be processed as a message format
/// error" cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Header too short (< 4 bytes).
    HeaderTooShort,
    /// Spec §3 — "Implementations MUST set this field to 1 (01 binary).
    /// Other values are reserved [...]. Messages with unknown version
    /// numbers MUST be silently ignored." We return an explicit error
    /// to the caller.
    UnsupportedVersion(u8),
    /// Spec §3 — "Lengths 9-15 are reserved, MUST NOT be sent, and
    /// MUST be processed as a message format error."
    ReservedTokenLength(u8),
    /// Token does not fit in the available bytes.
    TokenTruncated,
    /// Option header does not fit in the available bytes.
    OptionHeaderTruncated,
    /// Spec §3.1 — "15: Reserved for the Payload Marker. If the field
    /// is set to this value but the entire byte is not the payload
    /// marker, this MUST be processed as a message format error."
    OptionDeltaIs15,
    /// Spec §3.1 — Option length 15 reserved.
    OptionLengthIs15,
    /// Option value does not fit in the available bytes.
    OptionValueTruncated,
    /// Spec §3 — "The presence of a marker followed by a zero-length
    /// payload MUST be processed as a message format error."
    PayloadMarkerWithoutPayload,
    /// Token length is > 8 on encode (caller error).
    EncodeTokenTooLong,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderTooShort => f.write_str("header < 4 bytes"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported CoAP version {v}"),
            Self::ReservedTokenLength(l) => write!(f, "reserved token length {l}"),
            Self::TokenTruncated => f.write_str("token truncated"),
            Self::OptionHeaderTruncated => f.write_str("option header truncated"),
            Self::OptionDeltaIs15 => f.write_str("option delta 15 (reserved)"),
            Self::OptionLengthIs15 => f.write_str("option length 15 (reserved)"),
            Self::OptionValueTruncated => f.write_str("option value truncated"),
            Self::PayloadMarkerWithoutPayload => {
                f.write_str("payload marker with zero-length payload")
            }
            Self::EncodeTokenTooLong => f.write_str("token length > 8"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CodecError {}

/// Encodes a [`CoapMessage`] to a wire-format byte slice.
///
/// # Errors
/// Returns [`CodecError::EncodeTokenTooLong`] if `token.len() > 8`.
pub fn encode(msg: &CoapMessage) -> Result<Vec<u8>, CodecError> {
    if msg.token.len() > 8 {
        return Err(CodecError::EncodeTokenTooLong);
    }
    let mut out = Vec::with_capacity(4 + msg.token.len() + 16 + msg.payload.len());

    // Spec §3 Fixed-Header.
    let ver = 1u8;
    let t = msg.message_type.to_bits();
    #[allow(clippy::cast_possible_truncation)]
    let tkl = msg.token.len() as u8;
    out.push((ver << 6) | (t << 4) | (tkl & 0x0F));
    out.push(msg.code.to_byte());
    out.extend_from_slice(&msg.message_id.to_be_bytes());

    // Token.
    out.extend_from_slice(&msg.token);

    // Options — Spec §3.1 Delta-Encoding. Sort first.
    let mut opts: Vec<&CoapOption> = msg.options.iter().collect();
    opts.sort_by_key(|o| o.number);

    let mut prev_number: u16 = 0;
    for opt in opts {
        let delta = opt.number - prev_number;
        prev_number = opt.number;
        let value_bytes = opt.value.to_wire_bytes();
        #[allow(clippy::cast_possible_truncation)]
        let length = value_bytes.len() as u32;

        let (delta_nibble, delta_extended) = encode_extended(delta as u32);
        let (length_nibble, length_extended) = encode_extended(length);

        out.push((delta_nibble << 4) | (length_nibble & 0x0F));
        out.extend_from_slice(&delta_extended);
        out.extend_from_slice(&length_extended);
        out.extend_from_slice(&value_bytes);
    }

    // Payload — Spec §3 0xFF-Marker.
    if !msg.payload.is_empty() {
        out.push(0xFF);
        out.extend_from_slice(&msg.payload);
    }

    Ok(out)
}

/// Computes `(nibble, extended_bytes)` for delta or length
/// encoding (RFC 7252 §3.1).
fn encode_extended(value: u32) -> (u8, Vec<u8>) {
    if value < 13 {
        #[allow(clippy::cast_possible_truncation)]
        (value as u8, Vec::new())
    } else if value < 269 {
        // 13: 1-byte extended = value - 13.
        #[allow(clippy::cast_possible_truncation)]
        let ext = (value - 13) as u8;
        (13, alloc::vec![ext])
    } else {
        // 14: 2-byte extended = value - 269 (network byte order).
        #[allow(clippy::cast_possible_truncation)]
        let ext = (value - 269) as u16;
        (14, ext.to_be_bytes().to_vec())
    }
}

/// Decodes a [`CoapMessage`] from the wire-format byte slice.
///
/// # Errors
/// See [`CodecError`].
pub fn decode(bytes: &[u8]) -> Result<CoapMessage, CodecError> {
    if bytes.len() < 4 {
        return Err(CodecError::HeaderTooShort);
    }
    let h0 = bytes[0];
    let version = (h0 >> 6) & 0b11;
    if version != 1 {
        return Err(CodecError::UnsupportedVersion(version));
    }
    let t_bits = (h0 >> 4) & 0b11;
    // 2-bit value is always 0..=3 ⇒ from_bits always returns Some.
    let message_type = MessageType::from_bits(t_bits).ok_or(CodecError::HeaderTooShort)?;
    let tkl = h0 & 0x0F;
    if tkl > 8 {
        return Err(CodecError::ReservedTokenLength(tkl));
    }
    let code = CoapCode::from_byte(bytes[1]);
    let message_id = u16::from_be_bytes([bytes[2], bytes[3]]);

    let mut cursor = 4usize;
    let token_end = cursor.saturating_add(tkl as usize);
    if token_end > bytes.len() {
        return Err(CodecError::TokenTruncated);
    }
    let token = bytes[cursor..token_end].to_vec();
    cursor = token_end;

    // Options up to 0xFF or end.
    let mut options: Vec<CoapOption> = Vec::new();
    let mut current_number: u16 = 0;
    while cursor < bytes.len() {
        let b = bytes[cursor];
        if b == 0xFF {
            // Payload marker.
            cursor += 1;
            if cursor >= bytes.len() {
                return Err(CodecError::PayloadMarkerWithoutPayload);
            }
            break;
        }
        let delta_nibble = (b >> 4) & 0x0F;
        let length_nibble = b & 0x0F;
        cursor += 1;
        if delta_nibble == 15 {
            return Err(CodecError::OptionDeltaIs15);
        }
        if length_nibble == 15 {
            return Err(CodecError::OptionLengthIs15);
        }
        let delta = decode_extended(delta_nibble, bytes, &mut cursor)?;
        let length = decode_extended(length_nibble, bytes, &mut cursor)?;
        current_number = current_number
            .checked_add(u16::try_from(delta).map_err(|_| CodecError::OptionHeaderTruncated)?)
            .ok_or(CodecError::OptionHeaderTruncated)?;
        let len_us = length as usize;
        if cursor.saturating_add(len_us) > bytes.len() {
            return Err(CodecError::OptionValueTruncated);
        }
        let value_bytes = bytes[cursor..cursor + len_us].to_vec();
        cursor += len_us;
        options.push(CoapOption {
            number: current_number,
            value: OptionValue::Opaque(value_bytes),
        });
    }

    // Payload (if the marker was consumed).
    let payload = if cursor < bytes.len() {
        bytes[cursor..].to_vec()
    } else {
        Vec::new()
    };

    Ok(CoapMessage {
        version,
        message_type,
        code,
        message_id,
        token,
        options,
        payload,
    })
}

/// Reads extended bytes per Spec §3.1 (delta/length nibble 13/14).
fn decode_extended(nibble: u8, bytes: &[u8], cursor: &mut usize) -> Result<u32, CodecError> {
    match nibble {
        v if v < 13 => Ok(u32::from(v)),
        13 => {
            if *cursor >= bytes.len() {
                return Err(CodecError::OptionHeaderTruncated);
            }
            let v = u32::from(bytes[*cursor]) + 13;
            *cursor += 1;
            Ok(v)
        }
        14 => {
            if *cursor + 1 >= bytes.len() {
                return Err(CodecError::OptionHeaderTruncated);
            }
            let v = u32::from(u16::from_be_bytes([bytes[*cursor], bytes[*cursor + 1]])) + 269;
            *cursor += 2;
            Ok(v)
        }
        // 15 was already caught by the caller.
        _ => Err(CodecError::OptionHeaderTruncated),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;
    use crate::option::{CoapOption, numbers};

    #[test]
    fn encodes_minimum_get_request_to_4_byte_header() {
        // RFC 7252 §3 — smallest message: GET without token / options /
        // payload.
        let m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x1234);
        let bytes = encode(&m).expect("encode");
        assert_eq!(bytes.len(), 4);
        // Ver=1, T=0 (CON), TKL=0.
        assert_eq!(bytes[0], 0b0100_0000);
        assert_eq!(bytes[1], CoapCode::GET.to_byte());
        assert_eq!(bytes[2], 0x12);
        assert_eq!(bytes[3], 0x34);
    }

    #[test]
    fn encode_decode_round_trip_preserves_header_fields() {
        // Header round-trip.
        for t in [
            MessageType::Confirmable,
            MessageType::NonConfirmable,
            MessageType::Acknowledgement,
            MessageType::Reset,
        ] {
            let mut m = CoapMessage::new(t, CoapCode::POST, 0xAA55);
            m.token = alloc::vec![1, 2, 3, 4];
            let bytes = encode(&m).expect("encode");
            let parsed = decode(&bytes).expect("decode");
            assert_eq!(parsed.message_type, t);
            assert_eq!(parsed.code, CoapCode::POST);
            assert_eq!(parsed.message_id, 0xAA55);
            assert_eq!(parsed.token, alloc::vec![1, 2, 3, 4]);
        }
    }

    #[test]
    fn token_length_above_8_is_rejected_on_encode() {
        // Spec §3 TKL 0..=8.
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0);
        m.token = alloc::vec![0; 9];
        assert_eq!(encode(&m), Err(CodecError::EncodeTokenTooLong));
    }

    #[test]
    fn header_too_short_decode_fails() {
        assert_eq!(decode(&[]), Err(CodecError::HeaderTooShort));
        assert_eq!(decode(&[0; 3]), Err(CodecError::HeaderTooShort));
    }

    #[test]
    fn unsupported_version_decode_fails() {
        // Spec §3 — version != 1.
        let bytes = [0b1000_0000_u8, 0, 0, 0]; // Ver=2.
        assert_eq!(decode(&bytes), Err(CodecError::UnsupportedVersion(2)));
    }

    #[test]
    fn reserved_token_length_9_through_15_decode_fails() {
        // Spec §3.
        for tkl in 9..=15u8 {
            let bytes = [0b0100_0000 | tkl, 0, 0, 0];
            assert_eq!(decode(&bytes), Err(CodecError::ReservedTokenLength(tkl)));
        }
    }

    #[test]
    fn token_truncated_decode_fails() {
        // TKL=4 but only 2 token bytes present.
        let bytes = [0b0100_0100_u8, 1, 0, 0, 0xAA, 0xBB];
        assert_eq!(decode(&bytes), Err(CodecError::TokenTruncated));
    }

    #[test]
    fn single_short_option_encodes_in_one_byte_header() {
        // RFC 7252 §3.1 — Delta=1, Length=2 → 1 byte.
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        m.options = alloc::vec![CoapOption {
            number: 1,
            value: OptionValue::Opaque(alloc::vec![0xAB, 0xCD]),
        }];
        let bytes = encode(&m).expect("encode");
        // Header (4) + Option-Header (1) + Value (2) = 7.
        assert_eq!(bytes.len(), 7);
        assert_eq!(bytes[4], (1 << 4) | 2);
        assert_eq!(&bytes[5..7], &[0xAB, 0xCD]);
    }

    #[test]
    fn option_delta_extended_13_uses_one_extra_byte() {
        // Spec §3.1 — Delta = 20 → nibble=13, ext=7.
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        m.options = alloc::vec![CoapOption {
            number: 20,
            value: OptionValue::Empty,
        }];
        let bytes = encode(&m).expect("encode");
        // [hdr 4][nibble (13<<4|0)=0xD0][delta-ext = 7]
        assert_eq!(bytes[4], 0xD0);
        assert_eq!(bytes[5], 7);
        // Round-trip.
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(parsed.options.len(), 1);
        assert_eq!(parsed.options[0].number, 20);
    }

    #[test]
    fn option_delta_extended_14_uses_two_extra_bytes() {
        // Spec §3.1 — Delta = 1000 → nibble=14, ext = 1000-269 = 731.
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        m.options = alloc::vec![CoapOption {
            number: 1000,
            value: OptionValue::Empty,
        }];
        let bytes = encode(&m).expect("encode");
        assert_eq!(bytes[4], 0xE0); // nibble=14, length=0.
        assert_eq!(&bytes[5..7], &731u16.to_be_bytes());
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(parsed.options[0].number, 1000);
    }

    #[test]
    fn option_length_extended_13_uses_one_extra_byte() {
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        m.options = alloc::vec![CoapOption {
            number: 1,
            value: OptionValue::Opaque(alloc::vec![0; 50]),
        }];
        let bytes = encode(&m).expect("encode");
        // delta=1 nibble=1, length nibble=13 → 0x1D, then ext = 50-13=37.
        assert_eq!(bytes[4], 0x1D);
        assert_eq!(bytes[5], 37);
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(parsed.options[0].number, 1);
        if let OptionValue::Opaque(v) = &parsed.options[0].value {
            assert_eq!(v.len(), 50);
        } else {
            panic!("expected opaque");
        }
    }

    #[test]
    fn delta_encoding_sums_across_multiple_options() {
        // Spec §3.1 — delta-encoding basis.
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        m.options = alloc::vec![
            CoapOption {
                number: 1,
                value: OptionValue::Empty,
            },
            CoapOption {
                number: 5,
                value: OptionValue::Empty,
            },
            CoapOption {
                number: 11,
                value: OptionValue::Empty,
            },
        ];
        let bytes = encode(&m).expect("encode");
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(
            parsed.options.iter().map(|o| o.number).collect::<Vec<_>>(),
            alloc::vec![1, 5, 11]
        );
    }

    #[test]
    fn payload_marker_separates_options_from_payload() {
        // Spec §3 — 0xFF payload marker.
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::CONTENT, 1);
        m.payload = alloc::vec![1, 2, 3, 4];
        let bytes = encode(&m).expect("encode");
        // Last 5 bytes = 0xFF + payload.
        let n = bytes.len();
        assert_eq!(bytes[n - 5], 0xFF);
        assert_eq!(&bytes[n - 4..], &[1, 2, 3, 4]);
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(parsed.payload, alloc::vec![1, 2, 3, 4]);
    }

    #[test]
    fn payload_marker_without_payload_is_format_error() {
        // Spec §3 — "MUST be processed as a message format error".
        let bytes = [0b0100_0000_u8, CoapCode::GET.to_byte(), 0, 0, 0xFFu8];
        assert_eq!(decode(&bytes), Err(CodecError::PayloadMarkerWithoutPayload));
    }

    #[test]
    fn full_observe_request_encode_decode_round_trip() {
        // RFC 7641 §2 — Observe-register request with Uri-Path.
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0xBEEF);
        m.token = alloc::vec![0x42];
        m.options = alloc::vec![
            CoapOption::observe(0),
            CoapOption::uri_path("sensors"),
            CoapOption::uri_path("temp"),
        ];
        let bytes = encode(&m).expect("encode");
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(parsed.token, alloc::vec![0x42]);
        assert_eq!(parsed.options.len(), 3);
        // Order: Observe (6), Uri-Path (11), Uri-Path (11).
        assert_eq!(parsed.options[0].number, numbers::OBSERVE);
        assert_eq!(parsed.options[1].number, numbers::URI_PATH);
        assert_eq!(parsed.options[2].number, numbers::URI_PATH);
        // Decoded values are Opaque (the decoder does not know the
        // format semantics); the caller converts.
    }

    #[test]
    fn options_are_sorted_on_encode() {
        // Spec §3.1 — "instances MUST appear in order of their Option
        // Numbers".
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        m.options = alloc::vec![
            CoapOption::content_format(50),
            CoapOption::observe(0),
            CoapOption::uri_path("a"),
        ];
        let bytes = encode(&m).expect("encode");
        let parsed = decode(&bytes).expect("decode");
        let numbers: Vec<u16> = parsed.options.iter().map(|o| o.number).collect();
        // Observe(6) < Uri-Path(11) < Content-Format(12).
        assert_eq!(numbers, alloc::vec![6, 11, 12]);
    }

    #[test]
    fn empty_message_round_trip() {
        // Spec §4.1 — Code 0.00 = Empty Message (Reset/ACK).
        let m = CoapMessage::new(MessageType::Acknowledgement, CoapCode::EMPTY, 0xABCD);
        let bytes = encode(&m).expect("encode");
        assert_eq!(bytes.len(), 4);
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(parsed, m);
    }
}
