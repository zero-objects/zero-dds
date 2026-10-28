// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `ObjectVariant`-Representations (Spec §7.7.3).
//!
//! Each object CREATE operation (Spec §8.4.6) carries an `ObjectVariant`.
//! Byte-oracled against a real 15-datagram sample of eProsima Micro-XRCE-
//! DDS Client/Agent 3.0.1's own `CREATE` traffic
//! (`tools/wire-capture/xrce-sessions/client-to-agent.hex`) — Annex A's
//! own IDL settles what this module used to get wrong (see below): the
//! wire form is a **two-level** discriminated union, not the single-level
//! `RepresentationFormat` byte this module modeled until it was checked
//! against real bytes:
//!
//! ```text
//! union ObjectVariant switch (ObjectKind) {   // outer: which kind of object
//!   case OBJK_PARTICIPANT: OBJK_PARTICIPANT_Representation { representation; short domain_id; };
//!   case OBJK_TOPIC:       OBJK_TOPIC_Representation       { representation; ObjectId participant_id; };
//!   case OBJK_PUBLISHER:   OBJK_PUBLISHER_Representation   { representation; ObjectId participant_id; };
//!   case OBJK_SUBSCRIBER:  OBJK_SUBSCRIBER_Representation  { representation; ObjectId participant_id; };
//!   case OBJK_DATAWRITER:  DATAWRITER_Representation       { representation; ObjectId publisher_id; };
//!   case OBJK_DATAREADER:  DATAREADER_Representation       { representation; ObjectId subscriber_id; };
//!   case OBJK_TYPE | OBJK_QOSPROFILE | OBJK_APPLICATION | OBJK_OTHER:
//!     ..._Representation { representation; }               // no trailing field
//! };
//! ```
//!
//! `representation` itself is the **inner** union this module's
//! [`ObjectVariant`] enum + [`ObjectVariant::encode`]/[`ObjectVariant::decode`]
//! model — discriminated by `RepresentationFormat` (1 byte, Spec §7.7.2):
//! - `0x01` = `ByReference` — identifier string pointing to a template
//!   registered in the agent.
//! - `0x02` = `ByXmlString` — inline XML describing the object state
//!   (e.g. topic XML, QoS profile XML).
//! - `0x03` = `InBinary` — inline binary (XCDR2 encoding of the
//!   strong-typed variant, e.g. for `OBJK_TYPE`).
//!
//! Strings are XCDR2 `string<>` (4-byte length + UTF-8 + null terminator).
//! Bytes are XCDR2 `sequence<octet>` (4-byte length + bytes).
//!
//! [`ObjectVariant::encode_for_kind`]/[`ObjectVariant::decode_for_kind`]
//! wrap the inner codec with the outer `ObjectKind` byte and the
//! kind-specific trailing field. `domain_id` (`short`, a genuine XCDR2
//! numeric primitive) is 2-byte-aligned relative to the start of this
//! structure — confirmed against the captured `OBJK_PARTICIPANT` CREATE,
//! whose 89-byte (odd) XML string leaves exactly one pad byte before it.
//! `participant_id`/`publisher_id`/`subscriber_id` are `ObjectId`, an
//! `octet[2]` array (Annex A), which XCDR2 does **not** align — confirmed
//! against the captured `OBJK_SUBSCRIBER` CREATE, whose 1-byte (odd)
//! empty-string payload leaves the trailing `ObjectId` sitting at an odd
//! offset with no pad byte before it.
//!
//! Every one of the capture's seven `CREATE(OBJK_TOPIC)` ends in the
//! `ObjectId` of its participant (`00 11`), XML and binary alike — the
//! topic's container is named on the wire the same way a publisher's is.
//!
//! Here the outer wrap and the discriminators are structurally validated;
//! the inner XML/XCDR validation is out of scope. The `representation`
//! payload itself — a topic/QoS XML document, or a binary blob such as
//! `OBJK_DataWriter_Binary` — is not decoded further by this module;
//! `zerodds-xrce-agent`'s `daemon::entity_repr` reads it.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_i16, read_u32, write_i16, write_u32};
use crate::error::XrceError;
use crate::object_id::ObjectId;
use crate::object_kind::ObjectKind;

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

/// `REFERENCE_MAX_LEN` of DDS-XRCE 1.0 §7.7.3.1: the bound of the
/// `object_reference` string of `REPRESENTATION_BY_REFERENCE`.
pub const REFERENCE_MAX_LEN: usize = 128;

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
    /// `disc` (1 byte) + 3 padding bytes (XCDR2 alignment to 4, counting
    /// from this structure's own start at offset 0); then the length
    /// (u32) + bytes of the variant. Only correct when this structure
    /// itself starts at a 4-aligned offset — [`Self::encode_for_kind`]
    /// is the real wire form (Spec §7.7.3), where a 1-byte `ObjectKind`
    /// precedes this one and only 2 padding bytes are needed, not 3; see
    /// its doc and the module doc.
    ///
    /// # Errors
    /// `PayloadTooLarge` if the payload is `> REPR_MAX_INLINE_BYTES`.
    pub fn encode(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::new();
        self.encode_into(&mut out, e)?;
        Ok(out)
    }

    /// Appends the variant to `out`, which starts the payload: the length is
    /// an `unsigned long` aligned to 4 counted from `out[0]`, so the padding
    /// after the discriminator byte depends on where the variant starts.
    ///
    /// # Errors
    /// `PayloadTooLarge` if the payload is `> REPR_MAX_INLINE_BYTES`.
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
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
        if matches!(self, Self::ByReference(_)) && payload.len() > REFERENCE_MAX_LEN {
            return Err(XrceError::PayloadTooLarge {
                limit: REFERENCE_MAX_LEN,
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
        out.push(self.discriminator());
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let mut len_buf = [0u8; 4];
        write_u32(&mut len_buf, len_u32, e)?;
        out.extend_from_slice(&len_buf);
        out.extend_from_slice(payload);
        if extra_nul {
            out.push(0u8);
        }
        Ok(())
    }

    /// Decodes an `ObjectVariant`. Returns `(variant, bytes_consumed)`.
    /// See [`Self::encode`]'s doc for when this (rather than
    /// [`Self::decode_for_kind`]) is the correct one to call.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`, `PayloadTooLarge`.
    pub fn decode(bytes: &[u8], e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_at(bytes, 0, e)
    }

    /// Decodes the variant that starts at `pos` of `bytes`, which starts the
    /// payload. Returns it and the position after it.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`, `PayloadTooLarge`.
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        let disc = *bytes.get(pos).ok_or(XrceError::UnexpectedEof {
            needed: pos + 1,
            offset: bytes.len(),
        })?;
        let len_at = (pos + 1).div_ceil(4) * 4;
        let header_end = len_at + 4;
        let len_bytes = bytes
            .get(len_at..header_end)
            .ok_or(XrceError::UnexpectedEof {
                needed: header_end,
                offset: bytes.len(),
            })?;
        let len = read_u32(len_bytes, e)?;
        // The Annex B.2 examples (S. 117 to 124) put the DHEADER of the
        // structure where the `sequence<octet>` of `binary_representation`
        // has its length, and write the DHEADER with bit 31 set
        // (`0x80000002`, `0x8000000C`). XCDR2 gives the sequence a length
        // of its own and a DHEADER without that bit; a length with bit 31
        // set is more than `REPR_MAX_INLINE_BYTES` and no sequence, so it
        // is read as the example's form: the blob is the DHEADER (bit 31
        // cleared) and the members it counts.
        if disc == repr_disc::IN_BINARY && len & 0x8000_0000 != 0 {
            let members =
                usize::try_from(len & 0x7FFF_FFFF).map_err(|_| XrceError::ValueOutOfRange {
                    message: "object variant length exceeds usize",
                })?;
            if members > REPR_MAX_INLINE_BYTES {
                return Err(XrceError::PayloadTooLarge {
                    limit: REPR_MAX_INLINE_BYTES,
                    actual: members,
                });
            }
            let end = len_at + 4 + members;
            let mut blob = bytes
                .get(len_at..end)
                .ok_or(XrceError::UnexpectedEof {
                    needed: end,
                    offset: bytes.len(),
                })?
                .to_vec();
            // Bit 31 of the 32-bit value: the last octet in little-endian,
            // the first in big-endian.
            match e {
                Endianness::Little => blob[3] &= 0x7F,
                Endianness::Big => blob[0] &= 0x7F,
            }
            return Ok((Self::InBinary(blob), end));
        }
        let len_us = usize::try_from(len).map_err(|_| XrceError::ValueOutOfRange {
            message: "object variant length exceeds usize",
        })?;
        if len_us > REPR_MAX_INLINE_BYTES {
            return Err(XrceError::PayloadTooLarge {
                limit: REPR_MAX_INLINE_BYTES,
                actual: len_us,
            });
        }
        let payload =
            bytes
                .get(header_end..header_end + len_us)
                .ok_or(XrceError::UnexpectedEof {
                    needed: header_end + len_us,
                    offset: bytes.len(),
                })?;
        let consumed = header_end + len_us;
        let variant = match disc {
            repr_disc::BY_REFERENCE => {
                let s = decode_xcdr_string(payload)?;
                // §7.7.3.1: `string<REFERENCE_MAX_LEN> object_reference`.
                if s.len() > REFERENCE_MAX_LEN {
                    return Err(XrceError::PayloadTooLarge {
                        limit: REFERENCE_MAX_LEN,
                        actual: s.len(),
                    });
                }
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

    /// Encodes the full wire `ObjectVariant` (Spec §7.7.3): the outer
    /// `ObjectKind` byte, this inner format-discriminated payload
    /// ([`Self::encode`]), and `extra`'s kind-specific trailing field —
    /// see the module doc for which kinds carry one and the alignment
    /// each one's wire type does or does not need.
    ///
    /// # Errors
    /// As [`Self::encode`]; `ValueOutOfRange` if `extra` carries a field
    /// `kind` does not (e.g. a `parent` on `OBJK_TOPIC`) or is missing
    /// one it must have (e.g. no `domain_id` on `OBJK_PARTICIPANT`).
    pub fn encode_for_kind(
        &self,
        kind: ObjectKind,
        extra: ObjectVariantExtra,
        e: Endianness,
    ) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::new();
        self.encode_for_kind_into(&mut out, kind, extra, e)?;
        Ok(out)
    }

    /// As [`Self::encode_for_kind`], appended to `out`, which starts the
    /// payload: the alignment of the length and of `domain_id` is counted
    /// from `out[0]`.
    ///
    /// # Errors
    /// As [`Self::encode_for_kind`].
    pub fn encode_for_kind_into(
        &self,
        out: &mut Vec<u8>,
        kind: ObjectKind,
        extra: ObjectVariantExtra,
        e: Endianness,
    ) -> Result<(), XrceError> {
        extra.validate_for(kind)?;
        out.push(kind.to_u8());
        self.encode_into(out, e)?;
        if let Some(domain_id) = extra.domain_id {
            if out.len() % 2 != 0 {
                out.push(0); // `short` aligns to 2 bytes; `ObjectId` (below) does not
            }
            let mut buf = [0u8; 2];
            write_i16(&mut buf, domain_id, e)?;
            out.extend_from_slice(&buf);
        }
        if let Some(parent) = extra.parent {
            out.extend_from_slice(&parent.to_bytes());
        }
        Ok(())
    }

    /// Decodes the full wire `ObjectVariant`. Returns the object kind
    /// this representation is for, the inner payload, the kind-specific
    /// trailing field (if that kind carries one), and the total bytes
    /// consumed.
    ///
    /// # Errors
    /// [`Self::decode`]'s errors; `ValueOutOfRange` if the leading byte
    /// is not a real `OBJK_*` value, or `UnexpectedEof` if a trailing
    /// field the kind requires is missing.
    pub fn decode_for_kind(
        bytes: &[u8],
        e: Endianness,
    ) -> Result<(ObjectKind, Self, ObjectVariantExtra, usize), XrceError> {
        Self::decode_for_kind_at(bytes, 0, e)
    }

    /// As [`Self::decode_for_kind`] for the variant that starts at `pos` of
    /// `bytes`, which starts the payload; the last value is the position
    /// after it.
    ///
    /// # Errors
    /// As [`Self::decode_for_kind`].
    pub fn decode_for_kind_at(
        bytes: &[u8],
        pos: usize,
        e: Endianness,
    ) -> Result<(ObjectKind, Self, ObjectVariantExtra, usize), XrceError> {
        let Some(&kind_byte) = bytes.get(pos) else {
            return Err(XrceError::UnexpectedEof {
                needed: pos + 1,
                offset: bytes.len(),
            });
        };
        let kind = ObjectKind::from_u8(kind_byte)?;
        let (variant, mut offset) = Self::decode_at(bytes, pos + 1, e)?;
        let mut extra = ObjectVariantExtra::default();
        if ObjectVariantExtra::has_domain_id(kind) {
            if offset % 2 != 0 {
                offset += 1; // skip the alignment pad byte `short` needs
            }
            let field = bytes
                .get(offset..offset + 2)
                .ok_or(XrceError::UnexpectedEof {
                    needed: offset + 2,
                    offset: bytes.len(),
                })?;
            extra.domain_id = Some(read_i16(field, e)?);
            offset += 2;
        } else if ObjectVariantExtra::has_parent(kind) {
            // Annex B.2.4 puts a padding octet between the end of
            // `OBJK_DataWriter_Binary` and the `publisher_id`, so that the id
            // starts on a multiple of four (an `octet[2]` has no alignment of
            // its own, and B.2.7 has none). A blob of that form is followed by
            // zero octets to the next multiple of four and then exactly the
            // two octets of the id, nothing else.
            if matches!(variant, Self::InBinary(_)) {
                let skip = offset.wrapping_neg() & 3;
                if skip != 0
                    && bytes.len() == offset + skip + 2
                    && bytes[offset..offset + skip].iter().all(|b| *b == 0)
                {
                    offset += skip;
                }
            }
            let field = bytes
                .get(offset..offset + 2)
                .ok_or(XrceError::UnexpectedEof {
                    needed: offset + 2,
                    offset: bytes.len(),
                })?;
            extra.parent = Some(ObjectId::from_bytes(field)?);
            offset += 2; // `ObjectId` is `octet[2]` -- no alignment
        }
        Ok((kind, variant, extra, offset))
    }
}

/// The `elementNameReference` of DDS-XML 1.0
/// (`dds-xml_qos_definitions_nonamespace.xsd`, also used in the domain and
/// domain participant files): a `xs:string` restricted by `whiteSpace
/// collapse` and the pattern `((::)?[a-zA-Z0-9_.])+`, i.e. one or more
/// characters of `[a-zA-Z0-9_.]`, each of which may be preceded by `::`.
/// DDS-XRCE 1.0 §7.7.3.1.1 has the `object_reference` formatted as that type.
///
/// Returns the collapsed string (leading and trailing white space removed, runs
/// of white space inside one space, as `whiteSpace collapse` has it) when it
/// matches, `None` when it does not. A space left inside, a single `:`, a `::`
/// at the end or a `-` do not match.
#[must_use]
pub fn element_name_reference(s: &str) -> Option<String> {
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let b = collapsed.as_bytes();
    let mut i = 0;
    let mut units = 0;
    while i < b.len() {
        if b[i] == b':' {
            if b.get(i + 1) != Some(&b':') {
                return None;
            }
            i += 2;
        }
        match b.get(i) {
            Some(c) if c.is_ascii_alphanumeric() || *c == b'_' || *c == b'.' => {
                i += 1;
                units += 1;
            }
            _ => return None,
        }
    }
    (units > 0).then_some(collapsed)
}

/// The trailing field several `ObjectKind`s' own `..._Representation`
/// struct appends after the inner `representation` union — see the
/// module doc. At most one of `domain_id`/`parent` is ever set for a
/// given, valid `ObjectKind`; [`ObjectVariant::decode_for_kind`] only
/// ever sets the one `kind` calls for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ObjectVariantExtra {
    /// `OBJK_PARTICIPANT_Representation.domain_id`.
    pub domain_id: Option<i16>,
    /// `OBJK_TOPIC_Representation.participant_id` /
    /// `OBJK_PUBLISHER_Representation.participant_id` /
    /// `OBJK_SUBSCRIBER_Representation.participant_id` /
    /// `DATAWRITER_Representation.publisher_id` /
    /// `DATAREADER_Representation.subscriber_id` — always "the id of the
    /// container this one is created under", whatever Annex A calls the
    /// field for this particular kind.
    pub parent: Option<ObjectId>,
}

impl ObjectVariantExtra {
    fn has_domain_id(kind: ObjectKind) -> bool {
        kind == ObjectKind::Participant
    }

    fn has_parent(kind: ObjectKind) -> bool {
        matches!(
            kind,
            ObjectKind::Topic
                | ObjectKind::Publisher
                | ObjectKind::Subscriber
                | ObjectKind::DataWriter
                | ObjectKind::DataReader
        )
    }

    fn validate_for(self, kind: ObjectKind) -> Result<(), XrceError> {
        let wrong = |what: &'static str| Err(XrceError::ValueOutOfRange { message: what });
        if Self::has_domain_id(kind) {
            if self.domain_id.is_none() {
                return wrong("OBJK_PARTICIPANT_Representation requires domain_id");
            }
        } else if self.domain_id.is_some() {
            return wrong("only OBJK_PARTICIPANT_Representation carries domain_id");
        }
        if Self::has_parent(kind) {
            if self.parent.is_none() {
                return wrong("this ObjectKind's representation requires a parent id");
            }
        } else if self.parent.is_some() {
            return wrong("this ObjectKind's representation carries no parent id");
        }
        Ok(())
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

    /// Annex B.2.3: the CREATE of a DataWriter in binary. After the kind and
    /// format octets and two octets of padding comes the DHEADER
    /// `0x8000000C` of `OBJK_DataWriter_Binary`, with no length of the
    /// `sequence<octet>` in front, and `publisher_id` {0xBB, 0xB3} after the
    /// blob.
    #[test]
    fn annex_b_2_3_writer_variant_reads_as_the_blob_and_its_publisher() {
        let mut v = alloc::vec![0x05, 0x03, 0x00, 0x00];
        v.extend_from_slice(&[0x0C, 0x00, 0x00, 0x80]);
        v.extend_from_slice(&[0x07, 0x00, 0x00, 0x00]);
        v.extend_from_slice(b"Square\0");
        v.push(0x00); // qos? = FALSE
        v.extend_from_slice(&[0xBB, 0xB3]);
        let (kind, variant, extra, end) =
            ObjectVariant::decode_for_kind_at(&v, 0, Endianness::Little).unwrap();
        assert_eq!(kind, ObjectKind::DataWriter);
        assert_eq!(extra.parent, Some(ObjectId::from_raw(0xBBB3)));
        assert_eq!(end, v.len());
        let blob = match variant {
            ObjectVariant::InBinary(blob) => blob,
            other => return assert_eq!(other, ObjectVariant::InBinary(alloc::vec![])),
        };
        // The DHEADER with bit 31 cleared, then its 12 members.
        assert_eq!(&blob[..4], &[0x0C, 0x00, 0x00, 0x00]);
        assert_eq!(blob.len(), 16);
    }

    /// The CREATE_Payload of the Annex B.2.4/B.2.7 examples from the
    /// ObjectVariant on (kind, format, padding, DHEADER with bit 31, members,
    /// the id of the container).
    fn annex_b_variant(dheader: u32, members: &[u8], kind: u8, parent: [u8; 2]) -> Vec<u8> {
        let mut v = alloc::vec![kind, 0x03, 0x00, 0x00];
        v.extend_from_slice(&(0x8000_0000 | dheader).to_le_bytes());
        v.extend_from_slice(members);
        v.extend_from_slice(&parent);
        v
    }

    /// The members of `OBJK_DataWriter_Binary` of Annex B.2.4: "Square", a
    /// QoS of reliable and keep all, a deadline of 120000 ms. Byte 35 of the
    /// example, which the table calls padding, is the presence octet of
    /// `deadline_msec` (1): the value after it starts on a multiple of four,
    /// and with it the XCDR2 encoding of the printed IDL is exactly the
    /// printed bytes.
    fn annex_b_2_4_members() -> Vec<u8> {
        let mut m = alloc::vec![0x07, 0x00, 0x00, 0x00];
        m.extend_from_slice(b"Square\0");
        m.extend_from_slice(&[
            0x01, // qos? (byte 31)
            0x03, 0x00, // qos_flags (32-33)
            0x00, // history_depth? (34)
            0x01, // deadline_msec? (35, called padding in the table)
            0xC0, 0xD4, 0x01, 0x00, // deadline 120000 (36-39)
            0x00, 0x00, 0x00, // lifespan?, user_data?, ownership_strength? (40-42)
        ]);
        m
    }

    /// Annex B.2.4. The DHEADER value is not printed. Two readings give the
    /// printed bytes, and both are read: 23 (the members end at byte 42) with
    /// the padding octet 43 before the `publisher_id`, and 24 (the padding
    /// octet counted in the DHEADER).
    #[test]
    fn annex_b_2_4_writer_with_qos_in_both_readings_of_the_padding() {
        let members = annex_b_2_4_members();
        assert_eq!(members.len(), 23);
        let mut padded_in = members.clone();
        padded_in.push(0x00);
        for (dheader, body) in [
            (23u32, {
                let mut b = members.clone();
                b.push(0x00); // padding to the id, outside the DHEADER
                b
            }),
            (24, padded_in),
        ] {
            let v = annex_b_variant(dheader, &body, 0x05, [0xBB, 0xB3]);
            // 38 octets of payload less the 4 of the BaseObjectRequest.
            assert_eq!(v.len(), 34, "bytes 12 to 45");
            let (kind, variant, extra, end) =
                ObjectVariant::decode_for_kind_at(&v, 0, Endianness::Little).unwrap();
            assert_eq!(kind, ObjectKind::DataWriter);
            assert_eq!(
                extra.parent,
                Some(ObjectId::from_raw(0xBBB3)),
                "DHEADER {dheader}"
            );
            assert_eq!(end, v.len());
            assert!(
                matches!(&variant, ObjectVariant::InBinary(b) if b[..4] == dheader.to_le_bytes()),
                "{variant:?}"
            );
        }
    }

    /// Annex B.2.6: the DataReader with default QoS. The text prints the
    /// DHEADER as `{0xB5, 0, 0, 0}`, a length of 181 without the bit 31 the
    /// other examples set, which the 26 octets of the message cannot hold; the
    /// figure and the text of B.2.3 give 12 (`0x8000000C`).
    #[test]
    fn annex_b_2_6_reader_with_default_qos() {
        let mut members = alloc::vec![0x07, 0x00, 0x00, 0x00];
        members.extend_from_slice(b"Square\0");
        members.push(0x00); // qos? = FALSE
        let v = annex_b_variant(12, &members, 0x06, [0xCC, 0xC4]);
        let (kind, _, extra, end) =
            ObjectVariant::decode_for_kind_at(&v, 0, Endianness::Little).unwrap();
        assert_eq!(kind, ObjectKind::DataReader);
        assert_eq!(extra.parent, Some(ObjectId::from_raw(0xCCC4)));
        assert_eq!(end, v.len());
        // The DHEADER as printed is no length of this message.
        let mut printed = v.clone();
        printed[4..8].copy_from_slice(&[0xB5, 0, 0, 0]);
        assert!(ObjectVariant::decode_for_kind_at(&printed, 0, Endianness::Little).is_err());
    }

    /// Annex B.2.7: the DataReader with a QoS and a content filter. Byte 35 is
    /// again the presence octet of `deadline_msec`; no padding comes before
    /// the id (it starts at byte 54, not on a multiple of four).
    #[test]
    fn annex_b_2_7_reader_with_qos_and_filter() {
        let mut members = alloc::vec![0x07, 0x00, 0x00, 0x00];
        members.extend_from_slice(b"Square\0");
        members.extend_from_slice(&[
            0x01, // qos? (31)
            0x02, 0x00, // qos_flags: is_history_keep_all (32-33)
            0x00, // history_depth? (34)
            0x01, // deadline_msec? (35)
            0x20, 0xBF, 0x02, 0x00, // 180000 (36-39)
            0x00, 0x00, // lifespan?, user_data? (40, 41)
            0x00, // timebasedfilter_msec? (42)
            0x01, // contentbased_filter? (43)
            0x06, 0x00, 0x00, 0x00, // length (44-47)
        ]);
        members.extend_from_slice(b"x>100\0");
        assert_eq!(members.len(), 34);
        let v = annex_b_variant(34, &members, 0x06, [0xCC, 0xC4]);
        assert_eq!(v.len(), 4 + 4 + 34 + 2, "bytes 12 to 55");
        let (kind, variant, extra, end) =
            ObjectVariant::decode_for_kind_at(&v, 0, Endianness::Little).unwrap();
        assert_eq!(kind, ObjectKind::DataReader);
        assert_eq!(extra.parent, Some(ObjectId::from_raw(0xCCC4)));
        assert_eq!(end, v.len());
        assert!(matches!(variant, ObjectVariant::InBinary(b) if b.len() == 38));
    }

    /// Annex B.2.2: the DomainParticipant, DHEADER `0x80000002`, two absent
    /// optionals, and the `domain_id` of `PARTICIPANT_Representation` after.
    #[test]
    fn annex_b_2_2_participant_variant() {
        let v = [
            0x01, 0x03, 0x00, 0x00, 0x02, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        ];
        let (kind, variant, extra, end) =
            ObjectVariant::decode_for_kind_at(&v, 0, Endianness::Little).unwrap();
        assert_eq!(kind, ObjectKind::Participant);
        assert_eq!(
            variant,
            ObjectVariant::InBinary(alloc::vec![2, 0, 0, 0, 0, 0])
        );
        assert_eq!(extra.domain_id, Some(0));
        assert_eq!(end, v.len());
    }

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

    /// §7.7.3.1: `string<REFERENCE_MAX_LEN>` with `REFERENCE_MAX_LEN = 128`.
    /// A reference of 128 characters is read; one of 129 is refused, on the
    /// wire and when written. An XML string is not bound by it.
    #[test]
    fn a_reference_longer_than_reference_max_len_is_refused() {
        let at_limit = ObjectVariant::ByReference("r".repeat(REFERENCE_MAX_LEN));
        let wire = at_limit.encode(Endianness::Little).unwrap();
        assert_eq!(
            ObjectVariant::decode(&wire, Endianness::Little).unwrap().0,
            at_limit
        );

        let too_long = ObjectVariant::ByReference("r".repeat(REFERENCE_MAX_LEN + 1));
        assert!(matches!(
            too_long.encode(Endianness::Little),
            Err(XrceError::PayloadTooLarge {
                limit: REFERENCE_MAX_LEN,
                actual: 129
            })
        ));
        // The same bytes as an XML string are fine; as a reference they are not.
        let mut wire = ObjectVariant::ByXmlString("r".repeat(REFERENCE_MAX_LEN + 1))
            .encode(Endianness::Little)
            .unwrap();
        assert!(ObjectVariant::decode(&wire, Endianness::Little).is_ok());
        wire[0] = repr_disc::BY_REFERENCE;
        assert!(matches!(
            ObjectVariant::decode(&wire, Endianness::Little),
            Err(XrceError::PayloadTooLarge {
                limit: REFERENCE_MAX_LEN,
                actual: 129
            })
        ));
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

    /// Spec §7.7.3: `ObjectVariant` is a union discriminated by
    /// `ObjectKind` — a fact a previous version of this module got wrong
    /// (modeling only the *inner* `RepresentationFormat` discriminator
    /// as if it were the whole wire form), caught only once checked
    /// against real eProsima bytes (see the module doc and
    /// `participant_representation_matches_the_captured_create_bytes`/
    /// `subscriber_representation_matches_the_captured_create_bytes`
    /// below). This test round-trips every kind through
    /// [`ObjectVariant::encode_for_kind`]/[`ObjectVariant::decode_for_kind`]
    /// with the trailing field that kind requires (or none, for the
    /// kinds that carry none).
    #[test]
    fn encode_for_kind_round_trips_every_objk_kind_with_its_own_trailing_field() {
        let participant_like = |kind: ObjectKind, extra: ObjectVariantExtra| {
            let v = ObjectVariant::ByXmlString("<dds/>".into());
            let bytes = v
                .encode_for_kind(kind, extra, Endianness::Little)
                .expect("encode");
            let (kind2, v2, extra2, consumed) =
                ObjectVariant::decode_for_kind(&bytes, Endianness::Little).expect("decode");
            assert_eq!(kind2, kind);
            assert_eq!(v2, v);
            assert_eq!(extra2, extra);
            assert_eq!(consumed, bytes.len());
        };
        participant_like(
            ObjectKind::Participant,
            ObjectVariantExtra {
                domain_id: Some(7),
                parent: None,
            },
        );
        for kind in [
            ObjectKind::Topic,
            ObjectKind::Publisher,
            ObjectKind::Subscriber,
            ObjectKind::DataWriter,
            ObjectKind::DataReader,
        ] {
            participant_like(
                kind,
                ObjectVariantExtra {
                    domain_id: None,
                    parent: Some(ObjectId::new(1, ObjectKind::Participant).unwrap()),
                },
            );
        }
        for kind in [
            ObjectKind::Type,
            ObjectKind::QosProfile,
            ObjectKind::Application,
        ] {
            participant_like(kind, ObjectVariantExtra::default());
        }
    }

    #[test]
    fn encode_for_kind_rejects_a_trailing_field_the_kind_does_not_carry() {
        let v = ObjectVariant::ByXmlString("<dds/>".into());
        let res = v.encode_for_kind(
            ObjectKind::Topic,
            ObjectVariantExtra {
                domain_id: Some(1),
                parent: None,
            },
            Endianness::Little,
        );
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    /// Byte-oracle: `client-to-agent.hex`'s first `CREATE(OBJK_TOPIC)`
    /// (line 2, `PublishHelloWorld`): the 86-byte XML string, then the
    /// participant's `ObjectId` `00 11` with no pad byte before it.
    #[test]
    fn topic_representation_matches_the_captured_create_bytes() {
        let xml = "<dds><topic><name>HelloWorldTopic</name><dataType>HelloWorld</dataType>\
                   </topic></dds>";
        let mut bytes = vec![0x02, 0x02, 0x00, 0x00, 0x56, 0x00, 0x00, 0x00];
        bytes.extend_from_slice(xml.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0x00, 0x11]);
        assert_eq!(bytes.len(), 96);
        let (kind, v, extra, consumed) =
            ObjectVariant::decode_for_kind(&bytes, Endianness::Little).expect("decode");
        assert_eq!(kind, ObjectKind::Topic);
        assert_eq!(v, ObjectVariant::ByXmlString(xml.into()));
        assert_eq!(
            extra.parent,
            Some(ObjectId::new(1, ObjectKind::Participant).unwrap())
        );
        assert_eq!(consumed, bytes.len());
        let again = v
            .encode_for_kind(kind, extra, Endianness::Little)
            .expect("encode");
        assert_eq!(again, bytes);
    }

    /// Byte-oracle: `client-to-agent.hex`'s first `CREATE(OBJK_PARTICIPANT)`
    /// — `request_id=[0,10]`, `ObjectVariant` bytes (everything in the
    /// `CreatePayload.representation` after the 4-byte
    /// `BaseObjectRequest`). The 89-byte (odd-length) XML string is why
    /// `domain_id` needs exactly one pad byte before it — see the module
    /// doc.
    #[test]
    fn participant_representation_matches_the_captured_create_bytes() {
        #[rustfmt::skip]
        let bytes: [u8; 100] = [
            0x01, 0x02, 0x00, 0x00, 0x59, 0x00, 0x00, 0x00, 0x3c, 0x64, 0x64, 0x73,
            0x3e, 0x3c, 0x70, 0x61, 0x72, 0x74, 0x69, 0x63, 0x69, 0x70, 0x61, 0x6e,
            0x74, 0x3e, 0x3c, 0x72, 0x74, 0x70, 0x73, 0x3e, 0x3c, 0x6e, 0x61, 0x6d,
            0x65, 0x3e, 0x64, 0x65, 0x66, 0x61, 0x75, 0x6c, 0x74, 0x5f, 0x78, 0x72,
            0x63, 0x65, 0x5f, 0x70, 0x61, 0x72, 0x74, 0x69, 0x63, 0x69, 0x70, 0x61,
            0x6e, 0x74, 0x3c, 0x2f, 0x6e, 0x61, 0x6d, 0x65, 0x3e, 0x3c, 0x2f, 0x72,
            0x74, 0x70, 0x73, 0x3e, 0x3c, 0x2f, 0x70, 0x61, 0x72, 0x74, 0x69, 0x63,
            0x69, 0x70, 0x61, 0x6e, 0x74, 0x3e, 0x3c, 0x2f, 0x64, 0x64, 0x73, 0x3e,
            0x00, 0x00, 0x00, 0x00,
        ];
        let (kind, variant, extra, consumed) =
            ObjectVariant::decode_for_kind(&bytes, Endianness::Little).expect("decode");
        assert_eq!(kind, ObjectKind::Participant);
        assert_eq!(
            variant,
            ObjectVariant::ByXmlString(
                "<dds><participant><rtps><name>default_xrce_participant</name></rtps>\
                 </participant></dds>"
                    .into()
            )
        );
        assert_eq!(extra.domain_id, Some(0));
        assert_eq!(extra.parent, None);
        assert_eq!(consumed, bytes.len());
    }

    /// Byte-oracle: `client-to-agent.hex`'s `CREATE(OBJK_SUBSCRIBER)` —
    /// `request_id=[0,12]`, empty-string XML representation (1-byte,
    /// odd-length payload after the NUL terminator), so the trailing
    /// `participant_id` (`ObjectId`, not aligned) sits immediately after
    /// it with no pad byte — see the module doc. This exact byte
    /// sequence is what made `zerodds-xrce-agentd`'s session engine
    /// answer `STATUS_ERR_INVALID_DATA` where the real agent answered
    /// `STATUS_OK`, before this module's two-level discriminator fix.
    #[test]
    fn subscriber_representation_matches_the_captured_create_bytes() {
        let bytes: [u8; 11] = [
            0x04, 0x02, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11,
        ];
        let (kind, variant, extra, consumed) =
            ObjectVariant::decode_for_kind(&bytes, Endianness::Little).expect("decode");
        assert_eq!(kind, ObjectKind::Subscriber);
        assert_eq!(variant, ObjectVariant::ByXmlString(String::new()));
        assert_eq!(extra.domain_id, None);
        assert_eq!(
            extra.parent,
            Some(ObjectId::new(1, ObjectKind::Participant).unwrap()),
            "participant_id 0x0011 = raw 1, kind 1 (Participant)"
        );
        assert_eq!(consumed, bytes.len());
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

    /// DDS-XML 1.0 `elementNameReference`: `((::)?[a-zA-Z0-9_.])+` after
    /// `whiteSpace collapse`.
    #[test]
    fn element_name_references_follow_the_xsd_pattern() {
        for ok in [
            "a",
            "MyQosLibrary::MyQosProfile",
            "::Lib::Profile",
            "default_xrce_participant",
            "Lib.v1::Name_2",
            "::a",
            "1",
            ".",
            "a::b::c",
        ] {
            assert_eq!(element_name_reference(ok).as_deref(), Some(ok), "{ok}");
        }
        // whiteSpace collapse comes first: the edges go, a run inside is one
        // space, which the pattern then refuses.
        assert_eq!(
            element_name_reference("  Lib::P \n").as_deref(),
            Some("Lib::P")
        );
        for bad in [
            "",
            "   ",
            "a b",
            "a  b",
            "a:b",
            ":",
            "::",
            "a::",
            "a:::b",
            "a::::b",
            "my-profile",
            "a/b",
            "Lib::",
            "\u{e4}",
        ] {
            assert_eq!(element_name_reference(bad), None, "{bad:?}");
        }
    }
}
