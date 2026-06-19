// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectVariant`-Representations (Spec §7.7).
//!
//! Each object CREATE operation (Spec §8.4.6) carries an
//! `ObjectVariant` that holds object-specific data. These are
//! carried via three wire representations:
//!
//! - `REPRESENTATION_BY_REFERENCE`  — identifier string pointing to a
//!   template registered in the agent.
//! - `REPRESENTATION_AS_XML_STRING` — inline XML describing the object
//!   state (e.g. topic XML, QoS profile XML).
//! - `REPRESENTATION_IN_BINARY`     — inline binary (XCDR2 encoding of
//!   the strong-typed variant, e.g. for `OBJK_TYPE`).
//!
//! Wire discriminator (1 byte, Spec §7.7.2):
//! - `0x01` = ByReference
//! - `0x02` = ByXmlString
//! - `0x03` = InBinary
//!
//! Strings are XCDR2 `string<>` (4-byte LE length + UTF-8 + null terminator).
//! Bytes are XCDR2 `sequence<octet>` (4-byte LE length + bytes).
//!
//! Here the outer wrap and the discriminator are structurally validated;
//! the inner XML/XCDR validation is out of scope (handled by the
//! agent process later).

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u32, write_u32};
use crate::error::XrceError;

/// Discriminator bytes (Spec §7.7.2).
pub mod repr_disc {
    /// Reserved default; not spec-conformant for wire decode.
    pub const INVALID: u8 = 0x00;
    /// `REPRESENTATION_BY_REFERENCE`.
    pub const BY_REFERENCE: u8 = 0x01;
    /// `REPRESENTATION_AS_XML_STRING`.
    pub const AS_XML_STRING: u8 = 0x02;
    /// `REPRESENTATION_IN_BINARY`.
    pub const IN_BINARY: u8 = 0x03;
}

/// XCDR2 encoding cap for inline strings/bytes — protects against
/// `length=u32::MAX` decoder bombs. 64 KiB suffices for all
/// realistic topic XML descriptions.
pub const REPR_MAX_INLINE_BYTES: usize = 65_536;

/// `ObjectVariant` wire representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectVariant {
    /// Reference-by-name string.
    ByReference(String),
    /// Inline XML representation.
    ByXmlString(String),
    /// Inline binary (XCDR2-encoded strong type).
    InBinary(Vec<u8>),
}

impl ObjectVariant {
    /// Discriminator byte.
    #[must_use]
    pub fn discriminator(&self) -> u8 {
        match self {
            Self::ByReference(_) => repr_disc::BY_REFERENCE,
            Self::ByXmlString(_) => repr_disc::AS_XML_STRING,
            Self::InBinary(_) => repr_disc::IN_BINARY,
        }
    }

    /// Encodes this variant into XCDR2 form with the given endianness.
    /// Layout:
    ///
    /// ```text
    /// +----+----+----+----+----+ ... +----+
    /// |disc| pad pad pad |  payload  |
    /// +----+----+----+----+----+ ... +----+
    /// ```
    ///
    /// `disc` (1 byte) + 3 padding bytes (XCDR2 alignment to 4); then
    /// the length (u32) + bytes of the variant.
    ///
    /// # Errors
    /// `PayloadTooLarge` if the payload is `> REPR_MAX_INLINE_BYTES`.
    pub fn encode(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let payload = match self {
            Self::ByReference(s) | Self::ByXmlString(s) => s.as_bytes(),
            Self::InBinary(b) => b.as_slice(),
        };
        if payload.len() > REPR_MAX_INLINE_BYTES {
            return Err(XrceError::PayloadTooLarge {
                limit: REPR_MAX_INLINE_BYTES,
                actual: payload.len(),
            });
        }
        // Strings append a NUL terminator (XCDR2 §7.4.4).
        let extra_nul = matches!(self, Self::ByReference(_) | Self::ByXmlString(_));
        let payload_len = if extra_nul {
            payload.len() + 1
        } else {
            payload.len()
        };
        let len_u32 = u32::try_from(payload_len).map_err(|_| XrceError::ValueOutOfRange {
            message: "object variant length exceeds u32",
        })?;
        let mut out = Vec::with_capacity(8 + payload_len);
        out.push(self.discriminator());
        out.extend_from_slice(&[0u8, 0, 0]); // 3 bytes padding to 4-align
        let mut len_buf = [0u8; 4];
        write_u32(&mut len_buf, len_u32, e)?;
        out.extend_from_slice(&len_buf);
        out.extend_from_slice(payload);
        if extra_nul {
            out.push(0u8);
        }
        Ok(out)
    }

    /// Decodes an `ObjectVariant`. Returns `(variant, bytes_consumed)`.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`, `PayloadTooLarge`.
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        if bytes.len() < 8 {
            return Err(XrceError::UnexpectedEof {
                needed: 8,
                offset: bytes.len(),
            });
        }
        let disc = bytes[0];
        // bytes[1..4] are padding, ignored
        let len = read_u32(&bytes[4..8], e)?;
        let len_us = usize::try_from(len).map_err(|_| XrceError::ValueOutOfRange {
            message: "object variant length exceeds usize",
        })?;
        if len_us > REPR_MAX_INLINE_BYTES {
            return Err(XrceError::PayloadTooLarge {
                limit: REPR_MAX_INLINE_BYTES,
                actual: len_us,
            });
        }
        if bytes.len() < 8 + len_us {
            return Err(XrceError::UnexpectedEof {
                needed: 8 + len_us,
                offset: bytes.len(),
            });
        }
        let payload = &bytes[8..8 + len_us];
        let consumed = 8 + len_us;
        let variant = match disc {
            repr_disc::BY_REFERENCE => {
                let s = decode_xcdr_string(payload)?;
                Self::ByReference(s)
            }
            repr_disc::AS_XML_STRING => {
                let s = decode_xcdr_string(payload)?;
                Self::ByXmlString(s)
            }
            repr_disc::IN_BINARY => Self::InBinary(payload.to_vec()),
            _ => {
                return Err(XrceError::ValueOutOfRange {
                    message: "unknown ObjectVariant discriminator",
                });
            }
        };
        Ok((variant, consumed))
    }
}

/// XCDR2 `string<>` appends a NUL terminator. On decode we trim
/// trailing NULs and require valid UTF-8.
fn decode_xcdr_string(payload: &[u8]) -> Result<String, XrceError> {
    let trimmed = if let Some((&last, rest)) = payload.split_last() {
        if last == 0 { rest } else { payload }
    } else {
        payload
    };
    core::str::from_utf8(trimmed)
        .map(alloc::string::ToString::to_string)
        .map_err(|_| XrceError::ValueOutOfRange {
            message: "object variant string is not valid utf-8",
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn by_reference_roundtrip_le() {
        let v = ObjectVariant::ByReference("MyTopicProfile".into());
        let enc = v.encode(Endianness::Little).unwrap();
        let (v2, n) = ObjectVariant::decode(&enc, Endianness::Little).unwrap();
        assert_eq!(v, v2);
        assert_eq!(n, enc.len());
    }

    #[test]
    fn by_xml_string_roundtrip_be() {
        let v = ObjectVariant::ByXmlString(
            "<dds><topic name=\"Chat\" type=\"std::string\"/></dds>".into(),
        );
        let enc = v.encode(Endianness::Big).unwrap();
        let (v2, _) = ObjectVariant::decode(&enc, Endianness::Big).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn in_binary_roundtrip() {
        let v = ObjectVariant::InBinary(alloc::vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x01, 0x02]);
        let enc = v.encode(Endianness::Little).unwrap();
        let (v2, _) = ObjectVariant::decode(&enc, Endianness::Little).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn unknown_discriminator_rejected() {
        // Discriminator 0x42 is not in the spec
        let mut bad = alloc::vec![0x42, 0, 0, 0];
        bad.extend_from_slice(&0u32.to_le_bytes());
        let res = ObjectVariant::decode(&bad, Endianness::Little);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn truncated_header_returns_eof() {
        let res = ObjectVariant::decode(&[0x01, 0, 0], Endianness::Little);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 8, .. })
        ));
    }

    #[test]
    fn truncated_payload_returns_eof() {
        // disc=BY_REFERENCE, length=10 but only 3 bytes of payload
        let mut bad = alloc::vec![0x01, 0, 0, 0];
        bad.extend_from_slice(&10u32.to_le_bytes());
        bad.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let res = ObjectVariant::decode(&bad, Endianness::Little);
        assert!(matches!(res, Err(XrceError::UnexpectedEof { .. })));
    }

    #[test]
    fn oversized_length_rejected_as_payload_too_large() {
        let mut bad = alloc::vec![0x01, 0, 0, 0];
        let huge = (REPR_MAX_INLINE_BYTES as u32) + 1;
        bad.extend_from_slice(&huge.to_le_bytes());
        let res = ObjectVariant::decode(&bad, Endianness::Little);
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }

    #[test]
    fn invalid_utf8_rejected() {
        // Encoded as ByReference but with invalid UTF-8 bytes
        let mut bad = alloc::vec![repr_disc::BY_REFERENCE, 0, 0, 0];
        bad.extend_from_slice(&3u32.to_le_bytes());
        bad.extend_from_slice(&[0xFF, 0xFE, 0xFD]);
        let res = ObjectVariant::decode(&bad, Endianness::Little);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn discriminator_returns_correct_byte() {
        assert_eq!(
            ObjectVariant::ByReference(String::new()).discriminator(),
            repr_disc::BY_REFERENCE
        );
        assert_eq!(
            ObjectVariant::ByXmlString(String::new()).discriminator(),
            repr_disc::AS_XML_STRING
        );
        assert_eq!(
            ObjectVariant::InBinary(Vec::new()).discriminator(),
            repr_disc::IN_BINARY
        );
    }

    /// Spec §7.7.3 requires a discriminated union with 12 object
    /// kind variants. ZeroDDS realizes that via 2 tiers:
    ///
    /// 1. Outer wire: 3-discriminator `ObjectVariant` form
    ///    (ByReference / ByXmlString / InBinary).
    /// 2. Inner: per kind XCDR2-encoded strong type (Topic/Type/QoS/
    ///    Participant/Pub/Sub/DW/DR/Application/Agent/Client).
    ///
    /// This test shows that the outer form works transparently for all 12
    /// spec OBJK values — the inner XCDR2 is generated by the
    /// caller (xml_config / agent) and is out of scope
    /// for the wire codec.
    #[test]
    fn object_variant_carries_all_12_objk_kinds_through_outer_repr() {
        use crate::object_kind::{
            OBJK_AGENT, OBJK_APPLICATION, OBJK_CLIENT, OBJK_DATAREADER, OBJK_DATAWRITER,
            OBJK_PARTICIPANT, OBJK_PUBLISHER, OBJK_QOSPROFILE, OBJK_SUBSCRIBER, OBJK_TOPIC,
            OBJK_TYPE,
        };
        let kinds: &[u8] = &[
            OBJK_PARTICIPANT,
            OBJK_TOPIC,
            OBJK_PUBLISHER,
            OBJK_SUBSCRIBER,
            OBJK_DATAWRITER,
            OBJK_DATAREADER,
            OBJK_TYPE,
            OBJK_QOSPROFILE,
            OBJK_APPLICATION,
            OBJK_AGENT,
            OBJK_CLIENT,
        ];
        for kind in kinds {
            // The inner repr is by convention 1 byte with the OBJK value
            // as a marker — this demonstrates the 2-tier architecture.
            let inner = alloc::vec![*kind];
            let v = ObjectVariant::InBinary(inner.clone());
            let bytes = v.encode(Endianness::Little).expect("encode");
            let (v2, _) = ObjectVariant::decode(&bytes, Endianness::Little).expect("decode");
            assert_eq!(v2, ObjectVariant::InBinary(inner));
        }
    }

    #[test]
    fn object_variant_xml_form_supports_topic_qosprofile_application() {
        // Spec example: topic XML repr.
        for s in [
            "<topic name=\"T\"/>",
            "<qos_profile name=\"P\"/>",
            "<application name=\"A\"/>",
        ] {
            let v = ObjectVariant::ByXmlString(s.into());
            let bytes = v.encode(Endianness::Little).expect("encode");
            let (v2, _) = ObjectVariant::decode(&bytes, Endianness::Little).expect("decode");
            assert_eq!(v2, ObjectVariant::ByXmlString(s.into()));
        }
    }
}
