// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `WRITE_DATA` Submessage (id=7, DDS-XRCE 1.0 §8.3.5.8).
//!
//! Direction: Client → Agent. Spec-conformant wire form: the payload is a
//! `WRITE_DATA_Payload_*` that **derives from `BaseObjectRequest`** (§7.7.8),
//! so `request_id` (2 octets) + `object_id` (2 octets) come *before* the
//! user data. Without those four leading octets a foreign XRCE agent cannot
//! map the frame to the target DataWriter — which is exactly the conformance
//! gap this module closes (previously the whole body was an opaque sample).
//!
//! Flags bits 1..3 encode the `DataFormat`:
//! - `FORMAT_DATA = 0b000`
//! - `FORMAT_SAMPLE = 0b001`
//! - `FORMAT_DATA_SEQ = 0b100`
//! - `FORMAT_SAMPLE_SEQ = 0b101`
//! - `FORMAT_PACKED_SAMPLES = 0b111`
//!
//! Bit 0 is the E-flag (LE). Bits 4..7 are reserved.
//!
//! **Phase 1 scope:** only `FORMAT_DATA` — `WRITE_DATA_Payload_Data`
//! (§8.3.5.8), i.e. `BaseObjectRequest base; SampleData data;` where
//! `SampleData.serialized_data` is an XCDR `sequence<octet>` (4-byte length in
//! body endianness + raw bytes). The other four formats add `SampleInfo`
//! (state + `sequence_number` + `session_time_offset`) and sequence framing;
//! they stay Phase 2. Note the per-sample sequence number lives in
//! `SampleInfo` (FORMAT_SAMPLE, Phase 2); FORMAT_DATA has no per-sample
//! sequence number — the reliable stream sequence is carried by the
//! `MessageHeader.sequence_nr` (§8.3.2.3), not the sample body.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u32, write_u32};
use crate::error::XrceError;
use crate::object_info::BaseObjectRequest;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// `DataFormat` values (3-bit field in bits 1..3 of the submessage flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum DataFormat {
    #[default]
    Data = 0b000,
    Sample = 0b001,
    DataSeq = 0b100,
    SampleSeq = 0b101,
    PackedSamples = 0b111,
}

impl DataFormat {
    /// Raw 3-bit value (as it appears in a standalone `DataFormat` octet,
    /// e.g. inside `ReadSpecification`, §8.3.5.9).
    #[must_use]
    pub fn raw(self) -> u8 {
        self as u8
    }

    /// Reads from a standalone `DataFormat` octet (0..=7).
    ///
    /// # Errors
    /// `ValueOutOfRange` for reserved bit combinations.
    pub fn from_raw(value: u8) -> Result<Self, XrceError> {
        match value & 0b111 {
            0b000 => Ok(Self::Data),
            0b001 => Ok(Self::Sample),
            0b100 => Ok(Self::DataSeq),
            0b101 => Ok(Self::SampleSeq),
            0b111 => Ok(Self::PackedSamples),
            _ => Err(XrceError::ValueOutOfRange {
                message: "reserved DataFormat value",
            }),
        }
    }

    /// Encodes into the flag byte (bits 1..3).
    #[must_use]
    pub fn to_flag_bits(self) -> u8 {
        (self as u8) << 1
    }

    /// Extracts from the flag byte (bits 1..3). Reserved values
    /// (e.g. `0b010`) are reported as `ValueOutOfRange`.
    ///
    /// # Errors
    /// `ValueOutOfRange` for reserved bit combinations.
    pub fn from_flag_bits(flags: u8) -> Result<Self, XrceError> {
        Self::from_raw((flags >> 1) & 0b111)
    }
}

/// `WRITE_DATA_Payload_Data` (§8.3.5.8, FORMAT_DATA): a `BaseObjectRequest`
/// followed by `SampleData.serialized_data`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteDataPayload {
    /// `request_id` + target `object_id` (the DataWriter). Spec §7.7.8.
    pub base: BaseObjectRequest,
    /// `SampleData.serialized_data` — the XCDR-encoded user sample.
    pub serialized_data: Vec<u8>,
}

impl WriteDataPayload {
    /// Computes the flag byte (E-flag LE + FORMAT_DATA).
    #[must_use]
    pub fn flags() -> u8 {
        FLAG_E_LITTLE_ENDIAN | DataFormat::Data.to_flag_bits()
    }

    /// Encodes the submessage body: `BaseObjectRequest` (4 octets) then the
    /// `SampleData` `sequence<octet>` (u32 length + bytes). The length sits at
    /// offset 4, already 4-byte aligned, so no XCDR padding is inserted.
    ///
    /// # Errors
    /// `ValueOutOfRange` if `serialized_data` exceeds `u32`.
    pub fn encode_body(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let len =
            u32::try_from(self.serialized_data.len()).map_err(|_| XrceError::ValueOutOfRange {
                message: "WRITE_DATA serialized_data exceeds u32",
            })?;
        let mut body =
            Vec::with_capacity(BaseObjectRequest::WIRE_SIZE + 4 + self.serialized_data.len());
        body.extend_from_slice(&self.base.encode());
        let mut len_buf = [0u8; 4];
        write_u32(&mut len_buf, len, e)?;
        body.extend_from_slice(&len_buf);
        body.extend_from_slice(&self.serialized_data);
        Ok(body)
    }

    /// Packs into a `Submessage` (FORMAT_DATA, little-endian body).
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::WriteData, Self::flags(), body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong or the format is not FORMAT_DATA;
    /// `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::WriteData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not WRITE_DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "WRITE_DATA: only FORMAT_DATA is structured in phase 1",
            });
        }
        let e = sm.header.body_endianness();
        let (base, n) = BaseObjectRequest::decode(&sm.body)?;
        let rest = &sm.body[n..];
        let len = read_u32(rest, e)?;
        let len = usize::try_from(len).map_err(|_| XrceError::ValueOutOfRange {
            message: "WRITE_DATA serialized_data length exceeds usize",
        })?;
        if rest.len() < 4 + len {
            return Err(XrceError::UnexpectedEof {
                needed: 4 + len,
                offset: n,
            });
        }
        let serialized_data = rest[4..4 + len].to_vec();
        Ok(Self {
            base,
            serialized_data,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::object_id::ObjectId;
    use crate::object_kind::ObjectKind;

    fn writer_base() -> BaseObjectRequest {
        BaseObjectRequest {
            request_id: [0xAB, 0xCD],
            object_id: ObjectId::new(0x123, ObjectKind::DataWriter).unwrap(),
        }
    }

    #[test]
    fn write_data_roundtrip_format_data() {
        let p = WriteDataPayload {
            base: writer_base(),
            serialized_data: alloc::vec![1, 2, 3, 4],
        };
        let sm = p.clone().into_submessage().unwrap();
        let p2 = WriteDataPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn write_data_flags_are_e_le_and_format_data() {
        // FORMAT_DATA = 0b000 → flag byte is just the E-flag (0x01).
        assert_eq!(WriteDataPayload::flags(), 0x01);
    }

    #[test]
    fn write_data_base_object_request_precedes_the_sample() {
        // The four leading octets MUST be request_id[2] + object_id[2],
        // then the sequence<octet> length, then the sample (§7.7.8/§8.3.5.8).
        let p = WriteDataPayload {
            base: writer_base(),
            serialized_data: alloc::vec![0xDE, 0xAD, 0xBE, 0xEF],
        };
        let sm = p.into_submessage().unwrap();
        let oid = writer_base().object_id.to_bytes();
        assert_eq!(sm.body[0], 0xAB, "request_id[0]");
        assert_eq!(sm.body[1], 0xCD, "request_id[1]");
        assert_eq!(sm.body[2], oid[0], "object_id[0]");
        assert_eq!(sm.body[3], oid[1], "object_id[1]");
        // sequence<octet> length = 4, little-endian.
        assert_eq!(&sm.body[4..8], &[4, 0, 0, 0]);
        assert_eq!(&sm.body[8..12], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn write_data_empty_sample_roundtrip() {
        let p = WriteDataPayload {
            base: writer_base(),
            serialized_data: alloc::vec![],
        };
        let sm = p.clone().into_submessage().unwrap();
        // 4 (base) + 4 (length) + 0.
        assert_eq!(sm.body.len(), 8);
        assert_eq!(WriteDataPayload::try_from_submessage(&sm).unwrap(), p);
    }

    #[test]
    fn write_data_rejects_wrong_id() {
        let sm = Submessage::new(SubmessageId::Data, 0x01, alloc::vec![0; 8]).unwrap();
        assert!(WriteDataPayload::try_from_submessage(&sm).is_err());
    }

    #[test]
    fn write_data_rejects_non_data_format() {
        // FORMAT_SAMPLE flag (0b001<<1 | E) = 0x03: not structured in phase 1.
        let flags = FLAG_E_LITTLE_ENDIAN | DataFormat::Sample.to_flag_bits();
        let sm = Submessage::new(SubmessageId::WriteData, flags, alloc::vec![0; 8]).unwrap();
        assert!(matches!(
            WriteDataPayload::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }

    #[test]
    fn write_data_rejects_reserved_format() {
        // 0b010 is reserved.
        let bad_flags = FLAG_E_LITTLE_ENDIAN | (0b010 << 1);
        let sm = Submessage::new(SubmessageId::WriteData, bad_flags, alloc::vec![0; 8]).unwrap();
        assert!(matches!(
            WriteDataPayload::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }

    #[test]
    fn write_data_rejects_truncated_length() {
        // Body claims a 100-byte sample but carries none.
        let mut body = writer_base().encode().to_vec();
        body.extend_from_slice(&100u32.to_le_bytes());
        let sm = Submessage::new(SubmessageId::WriteData, WriteDataPayload::flags(), body).unwrap();
        assert!(matches!(
            WriteDataPayload::try_from_submessage(&sm),
            Err(XrceError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn data_format_raw_roundtrip() {
        for fmt in [
            DataFormat::Data,
            DataFormat::Sample,
            DataFormat::DataSeq,
            DataFormat::SampleSeq,
            DataFormat::PackedSamples,
        ] {
            assert_eq!(DataFormat::from_raw(fmt.raw()).unwrap(), fmt);
        }
        assert!(DataFormat::from_raw(0b010).is_err());
    }
}
