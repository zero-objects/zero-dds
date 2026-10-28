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
//! `WRITE_DATA_Payload_Data` (§8.3.5.8, FORMAT_DATA) is `BaseObjectRequest
//! base; SampleData data;`, where `SampleData.serialized_data` runs
//! verbatim to the end of the submessage body (no length of its own — see
//! [`WriteDataPayload::encode_body`]). The other four formats
//! (`WRITE_DATA_Payload_Sample`/`DataSeq`/`SampleSeq`/`PackedSamples`) add
//! [`crate::sample_info::SampleInfo`] and, for the three sequence formats,
//! an element count; see [`AnyWriteData`] and the module doc of
//! `crate::sample_info` for why this crate cannot split a sequence's
//! elements itself (no application-type knowledge) and what it hands the
//! caller instead. FORMAT_DATA has no per-sample sequence number of its
//! own — the reliable stream sequence is carried by the
//! `MessageHeader.sequence_nr` (§8.3.2.3), not the sample body.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u32, write_u32};
use crate::error::XrceError;
use crate::object_info::BaseObjectRequest;
use crate::sample_info::SampleInfo;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// `DataFormat` (Annex A): `FORMAT_DATA` 0x00, `FORMAT_SAMPLE` 0x02,
/// `FORMAT_DATA_SEQ` 0x08, `FORMAT_SAMPLE_SEQ` 0x0A, `FORMAT_PACKED_SAMPLES`
/// 0x0E, selected with `FORMAT_MASK` 0x0E. These are the bits 1 to 3 of the
/// flags of a `WRITE_DATA` or `DATA` (§8.3.5.8, Table 11) and the value of the
/// `data_format` octet of a `ReadSpecification` (§7.7.14).
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
    /// Annex A `FORMAT_MASK`.
    pub const FORMAT_MASK: u8 = 0x0E;

    /// The Annex A value of the format: 0x00, 0x02, 0x08, 0x0A or 0x0E, as it
    /// stands in a `data_format` octet and, with the E flag in bit 0, in the
    /// flags of a submessage.
    #[must_use]
    pub fn raw(self) -> u8 {
        (self as u8) << 1
    }

    /// Reads a standalone `data_format` octet (Annex A values).
    ///
    /// # Errors
    /// `ValueOutOfRange` for a value outside `FORMAT_MASK` or one of the
    /// reserved combinations.
    pub fn from_raw(value: u8) -> Result<Self, XrceError> {
        if value & !Self::FORMAT_MASK != 0 {
            return Err(XrceError::ValueOutOfRange {
                message: "DataFormat value has bits outside FORMAT_MASK",
            });
        }
        match value >> 1 {
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
        self.raw()
    }

    /// Extracts from the flag byte (bits 1..3). Reserved values
    /// (e.g. `0b010`) are reported as `ValueOutOfRange`.
    ///
    /// # Errors
    /// `ValueOutOfRange` for reserved bit combinations.
    pub fn from_flag_bits(flags: u8) -> Result<Self, XrceError> {
        Self::from_raw(flags & Self::FORMAT_MASK)
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

    /// Encodes the submessage body: `BaseObjectRequest` (4 octets) then
    /// `SampleData.serialized_data` **verbatim**.
    ///
    /// `XCDRSerializedBuffer` carries no length of its own — §7.7.1 defines
    /// it as "the bytes resulting from serializing the application-specific
    /// data type", and Annex B.3.1 counts them: `submessageLength = 6` for a
    /// 4-octet `BaseObjectRequest` and a 2-octet `short`. A length would make
    /// that ten. The length only appears one level up, on the
    /// `sequence<SampleData>` of FORMAT_DATA_SEQ (Annex B.3.2), which is not
    /// this type.
    ///
    /// The `e` argument is kept because the other formats need it; FORMAT_DATA
    /// writes no field of its own whose byte order could differ.
    ///
    /// # Errors
    /// Never fails for FORMAT_DATA; the `Result` is kept for the formats that
    /// do write a length.
    pub fn encode_body(&self, _e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut body =
            Vec::with_capacity(BaseObjectRequest::WIRE_SIZE + self.serialized_data.len());
        body.extend_from_slice(&self.base.encode());
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
                message: "WriteDataPayload::try_from_submessage: not FORMAT_DATA \
                          (use AnyWriteData::try_from_submessage to dispatch on the format)",
            });
        }
        let (base, n) = BaseObjectRequest::decode(&sm.body)?;
        // Everything after the base class is `serialized_data`. There is no
        // length to read — see `encode_body`.
        let serialized_data = sm.body[n..].to_vec();
        Ok(Self {
            base,
            serialized_data,
        })
    }
}

/// `WRITE_DATA_Payload_Sample` (§8.3.5.8, FORMAT_SAMPLE): `BaseObjectRequest
/// base; Sample sample;` where `Sample = SampleInfo info; SampleData data;`
/// and `data` runs verbatim to the end of the submessage body (it is the
/// last field, same reasoning as [`WriteDataPayload::encode_body`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteDataPayloadSample {
    /// `request_id` + target `object_id` (the DataWriter). Spec §7.7.8.
    pub base: BaseObjectRequest,
    /// Per-sample metadata (§7.7.1).
    pub info: SampleInfo,
    /// `SampleData.serialized_data` — the XCDR-encoded user sample.
    pub serialized_data: Vec<u8>,
}

impl WriteDataPayloadSample {
    /// Computes the flag byte (E-flag LE + FORMAT_SAMPLE).
    #[must_use]
    pub fn flags() -> u8 {
        FLAG_E_LITTLE_ENDIAN | DataFormat::Sample.to_flag_bits()
    }

    /// Encodes the submessage body: `BaseObjectRequest`(4) +
    /// `SampleInfo`(12) + `serialized_data` verbatim.
    ///
    /// # Errors
    /// None expected; kept `Result` for symmetry with the rest of the crate.
    pub fn encode_body(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut body = Vec::with_capacity(
            BaseObjectRequest::WIRE_SIZE + SampleInfo::WIRE_SIZE + self.serialized_data.len(),
        );
        body.extend_from_slice(&self.base.encode());
        self.info.encode_into(e, &mut body)?;
        body.extend_from_slice(&self.serialized_data);
        Ok(body)
    }

    /// Packs into a `Submessage` (FORMAT_SAMPLE, little-endian body).
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
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_SAMPLE; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::WriteData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not WRITE_DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::Sample {
            return Err(XrceError::ValueOutOfRange {
                message: "WriteDataPayloadSample::try_from_submessage: not FORMAT_SAMPLE",
            });
        }
        let e = sm.header.body_endianness();
        let (base, n1) = BaseObjectRequest::decode(&sm.body)?;
        let (info, n2) = SampleInfo::decode(&sm.body[n1..], e)?;
        let serialized_data = sm.body[n1 + n2..].to_vec();
        Ok(Self {
            base,
            info,
            serialized_data,
        })
    }
}

/// `WRITE_DATA_Payload_DataSeq` (§8.3.5.8, FORMAT_DATA_SEQ):
/// `BaseObjectRequest base; SampleDataSeq data_seq;` where `SampleDataSeq =
/// sequence<SampleData>`. Annex B.5.2's worked example lays this out as a
/// 4-byte element count followed by the elements' `serialized_data`
/// concatenated **with no per-element length** — this crate cannot split
/// that concatenation into individual samples (see the module doc and
/// `crate::sample_info`'s), so [`Self::elements`] is the raw, undivided
/// remainder for a type-aware caller to walk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteDataPayloadDataSeq {
    /// `request_id` + target `object_id` (the DataWriter). Spec §7.7.8.
    pub base: BaseObjectRequest,
    /// `data_seq.length` — the number of `SampleData` elements packed into
    /// [`Self::elements`].
    pub sample_count: u32,
    /// The `sample_count` elements' `serialized_data`, concatenated with no
    /// per-element delimiter (Annex B.5.2).
    pub elements: Vec<u8>,
}

impl WriteDataPayloadDataSeq {
    /// Computes the flag byte (E-flag LE + FORMAT_DATA_SEQ).
    #[must_use]
    pub fn flags() -> u8 {
        FLAG_E_LITTLE_ENDIAN | DataFormat::DataSeq.to_flag_bits()
    }

    /// Encodes the submessage body: `BaseObjectRequest`(4) + `length`(4) +
    /// `elements` verbatim.
    ///
    /// # Errors
    /// `ValueOutOfRange` if `sample_count` does not match a caller invariant
    /// beyond `u32` range — never for a value already typed `u32`; kept for
    /// symmetry with the rest of the crate.
    pub fn encode_body(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut body = Vec::with_capacity(BaseObjectRequest::WIRE_SIZE + 4 + self.elements.len());
        body.extend_from_slice(&self.base.encode());
        let mut len_buf = [0u8; 4];
        write_u32(&mut len_buf, self.sample_count, e)?;
        body.extend_from_slice(&len_buf);
        body.extend_from_slice(&self.elements);
        Ok(body)
    }

    /// Packs into a `Submessage` (FORMAT_DATA_SEQ, little-endian body).
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
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_DATA_SEQ; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::WriteData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not WRITE_DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::DataSeq {
            return Err(XrceError::ValueOutOfRange {
                message: "WriteDataPayloadDataSeq::try_from_submessage: not FORMAT_DATA_SEQ",
            });
        }
        let e = sm.header.body_endianness();
        let (base, n1) = BaseObjectRequest::decode(&sm.body)?;
        if sm.body.len() < n1 + 4 {
            return Err(XrceError::UnexpectedEof {
                needed: n1 + 4,
                offset: sm.body.len(),
            });
        }
        let sample_count = read_u32(&sm.body[n1..n1 + 4], e)?;
        let elements = sm.body[n1 + 4..].to_vec();
        Ok(Self {
            base,
            sample_count,
            elements,
        })
    }
}

/// `WRITE_DATA_Payload_SampleSeq` (§8.3.5.8, FORMAT_SAMPLE_SEQ):
/// `BaseObjectRequest base; SampleSeq sample_seq;` where `SampleSeq =
/// sequence<Sample>`. Annex B.5.4's worked example: 4-byte element count,
/// then each element's fixed 12-byte `SampleInfo` immediately followed by
/// its `SampleData` with no length between them or before the next
/// element's `SampleInfo` — so, as with [`WriteDataPayloadDataSeq`], this
/// crate hands back the raw, undivided remainder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteDataPayloadSampleSeq {
    /// `request_id` + target `object_id` (the DataWriter). Spec §7.7.8.
    pub base: BaseObjectRequest,
    /// `sample_seq.length` — the number of `Sample` elements packed into
    /// [`Self::elements`].
    pub sample_count: u32,
    /// The `sample_count` elements (`SampleInfo` + `SampleData` each),
    /// concatenated with no per-element delimiter (Annex B.5.4). Walk with
    /// [`SampleInfo::decode`] for the fixed 12-byte prefix of each element,
    /// then the topic's type decoder for the `SampleData` that follows it.
    pub elements: Vec<u8>,
}

impl WriteDataPayloadSampleSeq {
    /// Computes the flag byte (E-flag LE + FORMAT_SAMPLE_SEQ).
    #[must_use]
    pub fn flags() -> u8 {
        FLAG_E_LITTLE_ENDIAN | DataFormat::SampleSeq.to_flag_bits()
    }

    /// Encodes the submessage body: `BaseObjectRequest`(4) + `length`(4) +
    /// `elements` verbatim.
    ///
    /// # Errors
    /// None expected; kept `Result` for symmetry with the rest of the crate.
    pub fn encode_body(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut body = Vec::with_capacity(BaseObjectRequest::WIRE_SIZE + 4 + self.elements.len());
        body.extend_from_slice(&self.base.encode());
        let mut len_buf = [0u8; 4];
        write_u32(&mut len_buf, self.sample_count, e)?;
        body.extend_from_slice(&len_buf);
        body.extend_from_slice(&self.elements);
        Ok(body)
    }

    /// Packs into a `Submessage` (FORMAT_SAMPLE_SEQ, little-endian body).
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
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_SAMPLE_SEQ; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::WriteData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not WRITE_DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::SampleSeq {
            return Err(XrceError::ValueOutOfRange {
                message: "WriteDataPayloadSampleSeq::try_from_submessage: not FORMAT_SAMPLE_SEQ",
            });
        }
        let e = sm.header.body_endianness();
        let (base, n1) = BaseObjectRequest::decode(&sm.body)?;
        if sm.body.len() < n1 + 4 {
            return Err(XrceError::UnexpectedEof {
                needed: n1 + 4,
                offset: sm.body.len(),
            });
        }
        let sample_count = read_u32(&sm.body[n1..n1 + 4], e)?;
        let elements = sm.body[n1 + 4..].to_vec();
        Ok(Self {
            base,
            sample_count,
            elements,
        })
    }
}

/// `WRITE_DATA_Payload_PackedSamples` (§8.3.5.8, FORMAT_PACKED_SAMPLES):
/// `BaseObjectRequest base; PackedSamples packed_samples;` where
/// `PackedSamples = SampleInfo info_base; sequence<SampleDelta>
/// sample_delta_seq;`. Annex B.5.5's worked example: the fixed 12-byte
/// `info_base`, a 4-byte element count, then each element's fixed 4-byte
/// `SampleInfoDelta` immediately followed by its `SampleData` with no
/// length between them or before the next element — so, as with
/// [`WriteDataPayloadSampleSeq`], this crate hands back the raw, undivided
/// remainder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteDataPayloadPackedSamples {
    /// `request_id` + target `object_id` (the DataWriter). Spec §7.7.8.
    pub base: BaseObjectRequest,
    /// The common `SampleInfo` every element's `SampleInfoDelta` is
    /// relative to (§7.7.1's `resulting_info` formula, see
    /// `SampleInfoDelta::resolve`).
    pub info_base: SampleInfo,
    /// `sample_delta_seq.length` — the number of `SampleDelta` elements
    /// packed into [`Self::elements`].
    pub sample_count: u32,
    /// The `sample_count` elements (`SampleInfoDelta` + `SampleData` each),
    /// concatenated with no per-element delimiter (Annex B.5.5). Walk with
    /// `SampleInfoDelta::decode` for the fixed 4-byte prefix of each
    /// element, then the topic's type decoder for the `SampleData` that
    /// follows it.
    pub elements: Vec<u8>,
}

impl WriteDataPayloadPackedSamples {
    /// Computes the flag byte (E-flag LE + FORMAT_PACKED_SAMPLES).
    #[must_use]
    pub fn flags() -> u8 {
        FLAG_E_LITTLE_ENDIAN | DataFormat::PackedSamples.to_flag_bits()
    }

    /// Encodes the submessage body: `BaseObjectRequest`(4) +
    /// `info_base`(12) + `length`(4) + `elements` verbatim.
    ///
    /// # Errors
    /// None expected; kept `Result` for symmetry with the rest of the crate.
    pub fn encode_body(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut body = Vec::with_capacity(
            BaseObjectRequest::WIRE_SIZE + SampleInfo::WIRE_SIZE + 4 + self.elements.len(),
        );
        body.extend_from_slice(&self.base.encode());
        self.info_base.encode_into(e, &mut body)?;
        let mut len_buf = [0u8; 4];
        write_u32(&mut len_buf, self.sample_count, e)?;
        body.extend_from_slice(&len_buf);
        body.extend_from_slice(&self.elements);
        Ok(body)
    }

    /// Packs into a `Submessage` (FORMAT_PACKED_SAMPLES, little-endian
    /// body).
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
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_PACKED_SAMPLES; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::WriteData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not WRITE_DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::PackedSamples {
            return Err(XrceError::ValueOutOfRange {
                message: "WriteDataPayloadPackedSamples::try_from_submessage: not FORMAT_PACKED_SAMPLES",
            });
        }
        let e = sm.header.body_endianness();
        let (base, n1) = BaseObjectRequest::decode(&sm.body)?;
        let (info_base, n2) = SampleInfo::decode(&sm.body[n1..], e)?;
        let base_len = n1 + n2;
        if sm.body.len() < base_len + 4 {
            return Err(XrceError::UnexpectedEof {
                needed: base_len + 4,
                offset: sm.body.len(),
            });
        }
        let sample_count = read_u32(&sm.body[base_len..base_len + 4], e)?;
        let elements = sm.body[base_len + 4..].to_vec();
        Ok(Self {
            base,
            info_base,
            sample_count,
            elements,
        })
    }
}

/// Any of the five `WRITE_DATA_Payload_*` shapes (§8.3.5.8), dispatched on
/// the submessage flags' `DataFormat` bits. A session engine that does not
/// care which format a client used calls
/// [`AnyWriteData::try_from_submessage`] once and matches on the result,
/// instead of trying each of the five typed decoders in turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyWriteData {
    /// FORMAT_DATA.
    Data(WriteDataPayload),
    /// FORMAT_SAMPLE.
    Sample(WriteDataPayloadSample),
    /// FORMAT_DATA_SEQ.
    DataSeq(WriteDataPayloadDataSeq),
    /// FORMAT_SAMPLE_SEQ.
    SampleSeq(WriteDataPayloadSampleSeq),
    /// FORMAT_PACKED_SAMPLES.
    PackedSamples(WriteDataPayloadPackedSamples),
}

impl AnyWriteData {
    /// The `BaseObjectRequest` every variant starts with — the
    /// `request_id`/target `object_id` a session engine needs before it
    /// even looks at which format was used.
    #[must_use]
    pub fn base(&self) -> BaseObjectRequest {
        match self {
            Self::Data(p) => p.base,
            Self::Sample(p) => p.base,
            Self::DataSeq(p) => p.base,
            Self::SampleSeq(p) => p.base,
            Self::PackedSamples(p) => p.base,
        }
    }

    /// Decodes whichever of the five formats the submessage's flags select.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the submessage is not `WRITE_DATA` or the
    /// format bits are a reserved value; the per-format `UnexpectedEof`
    /// otherwise.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::WriteData {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not WRITE_DATA",
            });
        }
        match DataFormat::from_flag_bits(sm.header.flags)? {
            DataFormat::Data => Ok(Self::Data(WriteDataPayload::try_from_submessage(sm)?)),
            DataFormat::Sample => Ok(Self::Sample(WriteDataPayloadSample::try_from_submessage(
                sm,
            )?)),
            DataFormat::DataSeq => Ok(Self::DataSeq(WriteDataPayloadDataSeq::try_from_submessage(
                sm,
            )?)),
            DataFormat::SampleSeq => Ok(Self::SampleSeq(
                WriteDataPayloadSampleSeq::try_from_submessage(sm)?,
            )),
            DataFormat::PackedSamples => Ok(Self::PackedSamples(
                WriteDataPayloadPackedSamples::try_from_submessage(sm)?,
            )),
        }
    }

    /// Packs whichever variant this is into its `Submessage`.
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        match self {
            Self::Data(p) => p.into_submessage(),
            Self::Sample(p) => p.into_submessage(),
            Self::DataSeq(p) => p.into_submessage(),
            Self::SampleSeq(p) => p.into_submessage(),
            Self::PackedSamples(p) => p.into_submessage(),
        }
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
        // and the sample follows them with no length of its own
        // (§7.7.8 / §8.3.5.8, Annex B.3.1).
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
        assert_eq!(&sm.body[4..], &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(sm.body.len(), 8);
    }

    /// DDS-XRCE 1.0 Annex B.3.1, octet for octet.
    ///
    /// "Writing a single data sample with no sample information": a
    /// `Temperature { short value; }` of 25 to the DataWriter
    /// `{0x44, 0x05}`, request `{0xAA, 0x01}`. Table 25 counts the body
    /// at six octets — four for the `BaseObjectRequest` and two for the
    /// `short`. A `sequence<octet>` length would make it ten, which is
    /// what this crate wrote before the capture contradicted it.
    #[test]
    fn write_data_matches_the_annex_b_example() {
        let p = WriteDataPayload {
            base: BaseObjectRequest {
                request_id: [0xAA, 0x01],
                object_id: ObjectId::from_bytes(&[0x44, 0x05]).unwrap(),
            },
            serialized_data: alloc::vec![0x19, 0x00],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::WriteData);
        assert_eq!(sm.header.flags, 0x01);
        assert_eq!(sm.header.submessage_length, 6);
        assert_eq!(sm.body, alloc::vec![0xAA, 0x01, 0x44, 0x05, 0x19, 0x00]);
        assert_eq!(WriteDataPayload::try_from_submessage(&sm).unwrap(), p);
    }

    #[test]
    fn write_data_empty_sample_roundtrip() {
        let p = WriteDataPayload {
            base: writer_base(),
            serialized_data: alloc::vec![],
        };
        let sm = p.clone().into_submessage().unwrap();
        // 4 (base) and nothing else.
        assert_eq!(sm.body.len(), 4);
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
    fn write_data_rejects_a_body_shorter_than_the_base_class() {
        // Three octets cannot hold request_id + object_id.
        let sm = Submessage::new(
            SubmessageId::WriteData,
            WriteDataPayload::flags(),
            alloc::vec![0; 3],
        )
        .unwrap();
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
        // Annex A: the values of the constants, FORMAT_MASK = 0x0E.
        assert_eq!(DataFormat::Data.raw(), 0x00);
        assert_eq!(DataFormat::Sample.raw(), 0x02);
        assert_eq!(DataFormat::DataSeq.raw(), 0x08);
        assert_eq!(DataFormat::SampleSeq.raw(), 0x0A);
        assert_eq!(DataFormat::PackedSamples.raw(), 0x0E);
        // The old numbering is not a format: 0b001 sets a bit outside the
        // mask, 0b100 is a reserved combination once shifted.
        assert!(DataFormat::from_raw(0b001).is_err());
        assert!(DataFormat::from_raw(0x04).is_err());
        assert!(DataFormat::from_raw(0x10).is_err());
    }

    use crate::sample_info::{SampleInfo, SampleInfoDelta, SampleInfoFlags};

    fn sample_info(seq: u32, offset: u32) -> SampleInfo {
        SampleInfo {
            state: SampleInfoFlags(SampleInfoFlags::VIEW_STATE_NEW),
            sequence_number: seq,
            session_time_offset: offset,
        }
    }

    // ---- WriteDataPayloadSample (FORMAT_SAMPLE) --------------------------

    #[test]
    fn write_data_sample_roundtrip() {
        let p = WriteDataPayloadSample {
            base: writer_base(),
            info: sample_info(3, 1500),
            serialized_data: alloc::vec![0xAA, 0xBB, 0xCC],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(WriteDataPayloadSample::try_from_submessage(&sm).unwrap(), p);
    }

    /// DDS-XRCE 1.0 Annex B.5.3 lays out `Sample` (used identically by
    /// `WRITE_DATA_Payload_Sample` and `DATA_Payload_Sample` — both derive
    /// from a 4-octet base and then `Sample info; data;`, per Annex A) as
    /// `BaseObjectRequest`(4) + `info.state`(1)+pad(3) +
    /// `info.sequence_number`(4) + `info.session_time_offset`(4) +
    /// `XCDR_Serialization(foo)`. This checks that exact byte layout.
    #[test]
    fn write_data_sample_matches_annex_b_5_3_layout() {
        let p = WriteDataPayloadSample {
            base: writer_base(),
            info: sample_info(1, 0),
            serialized_data: alloc::vec![0x00, 0x00, 0x00, 0x13], // foo.count = 19
        };
        let sm =
            crate::encoding::with_sample_info_form(crate::encoding::SampleInfoForm::Flat, || {
                p.clone().into_submessage().unwrap()
            });
        assert_eq!(sm.header.submessage_length, 4 + 12 + 4);
        assert_eq!(&sm.body[4..5], &[SampleInfoFlags::VIEW_STATE_NEW]);
        assert_eq!(&sm.body[5..8], &[0, 0, 0], "padding to the 4-aligned u32");
        assert_eq!(&sm.body[8..12], &1u32.to_le_bytes());
        assert_eq!(&sm.body[12..16], &0u32.to_le_bytes());
        assert_eq!(&sm.body[16..20], &[0x00, 0x00, 0x00, 0x13]);
        // Annex A form (default): the SampleInfoDetail discriminator
        // FORMAT_SEQN_TIMS sits in the first padding octet.
        let sm =
            crate::encoding::with_sample_info_form(crate::encoding::SampleInfoForm::AnnexA, || {
                p.into_submessage().unwrap()
            });
        assert_eq!(sm.body[5], crate::sample_info::info_format::SEQN_TIMS);
        assert_eq!(&sm.body[6..8], &[0, 0]);
    }

    #[test]
    fn write_data_sample_flags_carry_format_sample() {
        assert_eq!(
            WriteDataPayloadSample::flags(),
            FLAG_E_LITTLE_ENDIAN | 0b0000_0010
        );
    }

    #[test]
    fn write_data_sample_rejects_non_sample_format() {
        let sm = Submessage::new(
            SubmessageId::WriteData,
            WriteDataPayload::flags(),
            alloc::vec![0; 16],
        )
        .unwrap();
        assert!(matches!(
            WriteDataPayloadSample::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }

    // ---- WriteDataPayloadDataSeq (FORMAT_DATA_SEQ) ------------------------

    #[test]
    fn write_data_data_seq_roundtrip() {
        let p = WriteDataPayloadDataSeq {
            base: writer_base(),
            sample_count: 2,
            elements: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(
            WriteDataPayloadDataSeq::try_from_submessage(&sm).unwrap(),
            p
        );
    }

    /// DDS-XRCE 1.0 Annex B.5.2: `BaseObjectRequest`(4) +
    /// `data_seq.length`(4) + `data_seq[0].data`(4) + `data_seq[1].data`(4)
    /// — no per-element length, elements run straight into each other.
    #[test]
    fn write_data_data_seq_matches_annex_b_5_2_layout() {
        let p = WriteDataPayloadDataSeq {
            base: BaseObjectRequest {
                request_id: [0xAA, 0x01],
                object_id: ObjectId::from_bytes(&[0x44, 0x06]).unwrap(),
            },
            sample_count: 2,
            // foo1.count=1, foo2.count=2, each a `long` (4 bytes).
            elements: alloc::vec![0, 0, 0, 1, 0, 0, 0, 2],
        };
        let sm = p.into_submessage().unwrap();
        assert_eq!(sm.header.submessage_length, 4 + 4 + 8);
        assert_eq!(&sm.body[4..8], &2u32.to_le_bytes(), "data_seq.length");
        assert_eq!(&sm.body[8..12], &[0, 0, 0, 1], "data_seq[0].data");
        assert_eq!(&sm.body[12..16], &[0, 0, 0, 2], "data_seq[1].data");
    }

    #[test]
    fn write_data_data_seq_empty_roundtrip() {
        let p = WriteDataPayloadDataSeq {
            base: writer_base(),
            sample_count: 0,
            elements: Vec::new(),
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.body.len(), 8);
        assert_eq!(
            WriteDataPayloadDataSeq::try_from_submessage(&sm).unwrap(),
            p
        );
    }

    // ---- WriteDataPayloadSampleSeq (FORMAT_SAMPLE_SEQ) --------------------

    #[test]
    fn write_data_sample_seq_roundtrip() {
        let p = WriteDataPayloadSampleSeq {
            base: writer_base(),
            sample_count: 2,
            elements: alloc::vec![0u8; 32],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(
            WriteDataPayloadSampleSeq::try_from_submessage(&sm).unwrap(),
            p
        );
    }

    /// DDS-XRCE 1.0 Annex B.5.4: `BaseObjectRequest`(4) +
    /// `sample_seq.length`(4) + `sample_seq[0].info`(12) +
    /// `sample_seq[0].data`(4) + `sample_seq[1].info`(12) +
    /// `sample_seq[1].data`(4) — submessageLength = 40, no length between
    /// an element's `info`/`data` or before the next element.
    #[test]
    fn write_data_sample_seq_matches_annex_b_5_4_layout() {
        let mut elements = Vec::new();
        sample_info(0, 0)
            .encode_into(Endianness::Little, &mut elements)
            .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 1]); // foo1.count = 1
        sample_info(0, 0)
            .encode_into(Endianness::Little, &mut elements)
            .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 2]); // foo2.count = 2
        let p = WriteDataPayloadSampleSeq {
            base: BaseObjectRequest {
                request_id: [0xAA, 0x01],
                object_id: ObjectId::from_bytes(&[0x44, 0x06]).unwrap(),
            },
            sample_count: 2,
            elements,
        };
        let sm = p.into_submessage().unwrap();
        assert_eq!(
            sm.header.submessage_length, 40,
            "Table 33: submessageLength=40"
        );
        assert_eq!(&sm.body[4..8], &2u32.to_le_bytes(), "sample_seq.length");
        assert_eq!(sm.body[8..20].len(), 12, "sample_seq[0].info");
        assert_eq!(&sm.body[20..24], &[0, 0, 0, 1], "sample_seq[0].data");
        assert_eq!(sm.body[24..36].len(), 12, "sample_seq[1].info");
        assert_eq!(&sm.body[36..40], &[0, 0, 0, 2], "sample_seq[1].data");
    }

    // ---- WriteDataPayloadPackedSamples (FORMAT_PACKED_SAMPLES) -----------

    #[test]
    fn write_data_packed_samples_roundtrip() {
        let p = WriteDataPayloadPackedSamples {
            base: writer_base(),
            info_base: sample_info(1000, 0),
            sample_count: 2,
            elements: alloc::vec![0u8; 16],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(
            WriteDataPayloadPackedSamples::try_from_submessage(&sm).unwrap(),
            p
        );
    }

    /// DDS-XRCE 1.0 Annex B.5.5: `BaseObjectRequest`(4) + `info_base`(12) +
    /// `sample_delta_seq.length`(4) +
    /// (`sample_delta_seq[i].info_delta`(4) +
    /// `sample_delta_seq[i].data`(4)) × 2 — submessageLength = 36.
    #[test]
    fn write_data_packed_samples_matches_annex_b_5_5_layout() {
        let mut elements = Vec::new();
        SampleInfoDelta {
            state: SampleInfoFlags(0),
            seq_number_delta: 0,
            timestamp_delta: 0,
        }
        .encode_into(Endianness::Little, &mut elements)
        .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 1]); // foo1.count = 1
        SampleInfoDelta {
            state: SampleInfoFlags(0),
            seq_number_delta: 0,
            timestamp_delta: 0,
        }
        .encode_into(Endianness::Little, &mut elements)
        .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 2]); // foo2.count = 2
        let p = WriteDataPayloadPackedSamples {
            base: BaseObjectRequest {
                request_id: [0xAA, 0x03],
                object_id: ObjectId::from_bytes(&[0x44, 0x06]).unwrap(),
            },
            info_base: sample_info(0, 0),
            sample_count: 2,
            elements,
        };
        let sm = p.into_submessage().unwrap();
        assert_eq!(
            sm.header.submessage_length, 36,
            "Table 34: submessageLength=36"
        );
        assert_eq!(sm.body[4..16].len(), 12, "info_base");
        assert_eq!(
            &sm.body[16..20],
            &2u32.to_le_bytes(),
            "sample_delta_seq.length"
        );
        assert_eq!(sm.body[20..24].len(), 4, "sample_delta_seq[0].info_delta");
        assert_eq!(&sm.body[24..28], &[0, 0, 0, 1], "sample_delta_seq[0].data");
        assert_eq!(sm.body[28..32].len(), 4, "sample_delta_seq[1].info_delta");
        assert_eq!(&sm.body[32..36], &[0, 0, 0, 2], "sample_delta_seq[1].data");
    }

    // ---- AnyWriteData dispatch ---------------------------------------

    #[test]
    fn any_write_data_dispatches_every_format() {
        let base = writer_base();
        let cases: alloc::vec::Vec<(DataFormat, Submessage)> = alloc::vec![
            (
                DataFormat::Data,
                WriteDataPayload {
                    base,
                    serialized_data: alloc::vec![1],
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::Sample,
                WriteDataPayloadSample {
                    base,
                    info: sample_info(0, 0),
                    serialized_data: alloc::vec![1],
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::DataSeq,
                WriteDataPayloadDataSeq {
                    base,
                    sample_count: 0,
                    elements: Vec::new(),
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::SampleSeq,
                WriteDataPayloadSampleSeq {
                    base,
                    sample_count: 0,
                    elements: Vec::new(),
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::PackedSamples,
                WriteDataPayloadPackedSamples {
                    base,
                    info_base: sample_info(0, 0),
                    sample_count: 0,
                    elements: Vec::new(),
                }
                .into_submessage()
                .unwrap(),
            ),
        ];
        for (fmt, sm) in cases {
            let decoded = AnyWriteData::try_from_submessage(&sm).unwrap();
            assert_eq!(decoded.base(), base);
            let matches_format = matches!(
                (&decoded, fmt),
                (AnyWriteData::Data(_), DataFormat::Data)
                    | (AnyWriteData::Sample(_), DataFormat::Sample)
                    | (AnyWriteData::DataSeq(_), DataFormat::DataSeq)
                    | (AnyWriteData::SampleSeq(_), DataFormat::SampleSeq)
                    | (AnyWriteData::PackedSamples(_), DataFormat::PackedSamples)
            );
            assert!(matches_format, "AnyWriteData variant did not match {fmt:?}");
            // Round-trips back to the same bytes.
            let re = decoded.into_submessage().unwrap();
            assert_eq!(re, sm);
        }
    }

    #[test]
    fn any_write_data_rejects_wrong_submessage_id() {
        let sm = Submessage::new(SubmessageId::Data, 0x01, alloc::vec![0; 8]).unwrap();
        assert!(matches!(
            AnyWriteData::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }
}
