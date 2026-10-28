// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `DATA` Submessage (id=9, DDS-XRCE 1.0 §8.3.5.10).
//!
//! Direction: Agent → Client. The reply an agent pushes to a client after a
//! `READ_DATA` request (or on a matched DataReader). Spec-conformant wire
//! form: `DATA_Payload_*` derives from `BaseObjectRequest` (§7.7.8) — the
//! `request_id` echoes the originating `READ_DATA` request and the `object_id`
//! identifies the DataReader the sample came from. This matches the reference
//! `DATA_Payload_Data { BaseObjectRequest base; SampleData data; }` layout
//! (OMG DDS-XRCE 1.0 §8.3.5.10; also eProsima Micro-XRCE-DDS
//! `DATA_Payload_Data`). We deliberately do **not** use the 6-byte
//! `BaseObjectReply`/`RelatedObjectRequest` shape here: the data submessages
//! carry a `BaseObjectRequest`, not a `ResultStatus`.
//!
//! Flag bits 1..3 = `DataFormat`, analogous to `WRITE_DATA`.
//!
//! `DATA_Payload_Data` (FORMAT_DATA) is `BaseObjectRequest` + a `SampleData`
//! that runs verbatim to the end of the submessage body (no length of its
//! own). The other four formats mirror `write_data.rs`'s
//! `WriteDataPayloadSample`/`DataSeq`/`SampleSeq`/`PackedSamples` exactly —
//! same body shape, same reasoning for why the three sequence formats hand
//! back a raw, undivided `elements` blob (see `crate::sample_info`'s module
//! doc and [`AnyData`]).

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u32, write_u32};
use crate::error::XrceError;
use crate::object_info::BaseObjectRequest;
use crate::sample_info::SampleInfo;
use crate::submessages::write_data::DataFormat;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// `DATA_Payload_Data` (§8.3.5.10, FORMAT_DATA): the related `BaseObjectRequest`
/// followed by `SampleData.serialized_data`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataPayload {
    /// `request_id` (echo of the `READ_DATA` request) + `object_id`
    /// (the DataReader). Spec §7.7.8 / §8.3.5.10.
    pub base: BaseObjectRequest,
    /// `SampleData.serialized_data` — the XCDR-encoded sample the agent pushes.
    pub serialized_data: Vec<u8>,
}

impl DataPayload {
    /// Computes the flag byte (E-flag LE + FORMAT_DATA).
    #[must_use]
    pub fn flags() -> u8 {
        FLAG_E_LITTLE_ENDIAN | DataFormat::Data.to_flag_bits()
    }

    /// Encodes the submessage body: `BaseObjectRequest` (4 octets) then
    /// `SampleData.serialized_data` **verbatim** — see
    /// [`crate::submessages::write_data::WriteDataPayload::encode_body`] for
    /// why there is no length.
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
        Submessage::new(SubmessageId::Data, Self::flags(), body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong or the format is not FORMAT_DATA;
    /// `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "DataPayload::try_from_submessage: not FORMAT_DATA \
                          (use AnyData::try_from_submessage to dispatch on the format)",
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

/// `DATA_Payload_Sample` (§8.3.5.10, FORMAT_SAMPLE): `BaseObjectRequest
/// base; Sample sample;` — see
/// [`crate::submessages::write_data::WriteDataPayloadSample`], the
/// identical body shape in the other direction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataPayloadSample {
    /// `request_id` (echo of the `READ_DATA` request) + `object_id` (the
    /// DataReader). Spec §7.7.8 / §8.3.5.10.
    pub base: BaseObjectRequest,
    /// Per-sample metadata (§7.7.1).
    pub info: SampleInfo,
    /// `SampleData.serialized_data` — the XCDR-encoded sample the agent
    /// pushes.
    pub serialized_data: Vec<u8>,
}

impl DataPayloadSample {
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
        Submessage::new(SubmessageId::Data, Self::flags(), body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_SAMPLE; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::Sample {
            return Err(XrceError::ValueOutOfRange {
                message: "DataPayloadSample::try_from_submessage: not FORMAT_SAMPLE",
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

/// `DATA_Payload_DataSeq` (§8.3.5.10, FORMAT_DATA_SEQ) — see
/// [`crate::submessages::write_data::WriteDataPayloadDataSeq`], the
/// identical body shape in the other direction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataPayloadDataSeq {
    /// `request_id` (echo of the `READ_DATA` request) + `object_id` (the
    /// DataReader). Spec §7.7.8 / §8.3.5.10.
    pub base: BaseObjectRequest,
    /// `data_seq.length` — the number of `SampleData` elements packed into
    /// [`Self::elements`].
    pub sample_count: u32,
    /// The `sample_count` elements' `serialized_data`, concatenated with no
    /// per-element delimiter (Annex B.5.2).
    pub elements: Vec<u8>,
}

impl DataPayloadDataSeq {
    /// Computes the flag byte (E-flag LE + FORMAT_DATA_SEQ).
    #[must_use]
    pub fn flags() -> u8 {
        FLAG_E_LITTLE_ENDIAN | DataFormat::DataSeq.to_flag_bits()
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

    /// Packs into a `Submessage` (FORMAT_DATA_SEQ, little-endian body).
    ///
    /// # Errors
    /// `PayloadTooLarge`, `ValueOutOfRange`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        let body = self.encode_body(Endianness::Little)?;
        Submessage::new(SubmessageId::Data, Self::flags(), body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_DATA_SEQ; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::DataSeq {
            return Err(XrceError::ValueOutOfRange {
                message: "DataPayloadDataSeq::try_from_submessage: not FORMAT_DATA_SEQ",
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

/// `DATA_Payload_SampleSeq` (§8.3.5.10, FORMAT_SAMPLE_SEQ) — see
/// [`crate::submessages::write_data::WriteDataPayloadSampleSeq`], the
/// identical body shape in the other direction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataPayloadSampleSeq {
    /// `request_id` (echo of the `READ_DATA` request) + `object_id` (the
    /// DataReader). Spec §7.7.8 / §8.3.5.10.
    pub base: BaseObjectRequest,
    /// `sample_seq.length` — the number of `Sample` elements packed into
    /// [`Self::elements`].
    pub sample_count: u32,
    /// The `sample_count` elements (`SampleInfo` + `SampleData` each),
    /// concatenated with no per-element delimiter (Annex B.5.4).
    pub elements: Vec<u8>,
}

impl DataPayloadSampleSeq {
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
        Submessage::new(SubmessageId::Data, Self::flags(), body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_SAMPLE_SEQ; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::SampleSeq {
            return Err(XrceError::ValueOutOfRange {
                message: "DataPayloadSampleSeq::try_from_submessage: not FORMAT_SAMPLE_SEQ",
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

/// `DATA_Payload_PackedSamples` (§8.3.5.10, FORMAT_PACKED_SAMPLES) — see
/// [`crate::submessages::write_data::WriteDataPayloadPackedSamples`], the
/// identical body shape in the other direction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataPayloadPackedSamples {
    /// `request_id` (echo of the `READ_DATA` request) + `object_id` (the
    /// DataReader). Spec §7.7.8 / §8.3.5.10.
    pub base: BaseObjectRequest,
    /// The common `SampleInfo` every element's `SampleInfoDelta` is
    /// relative to (§7.7.1's `resulting_info` formula, see
    /// [`crate::sample_info::SampleInfoDelta::resolve`]).
    pub info_base: SampleInfo,
    /// `sample_delta_seq.length` — the number of `SampleDelta` elements
    /// packed into [`Self::elements`].
    pub sample_count: u32,
    /// The `sample_count` elements (`SampleInfoDelta` + `SampleData` each),
    /// concatenated with no per-element delimiter (Annex B.5.5).
    pub elements: Vec<u8>,
}

impl DataPayloadPackedSamples {
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
        Submessage::new(SubmessageId::Data, Self::flags(), body)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the ID is wrong or the format is not
    /// FORMAT_PACKED_SAMPLES; `UnexpectedEof` on a truncated body.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DATA",
            });
        }
        if DataFormat::from_flag_bits(sm.header.flags)? != DataFormat::PackedSamples {
            return Err(XrceError::ValueOutOfRange {
                message: "DataPayloadPackedSamples::try_from_submessage: not FORMAT_PACKED_SAMPLES",
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

/// Any of the five `DATA_Payload_*` shapes (§8.3.5.10), dispatched on the
/// submessage flags' `DataFormat` bits — the agent→client counterpart of
/// [`crate::submessages::write_data::AnyWriteData`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyData {
    /// FORMAT_DATA.
    Data(DataPayload),
    /// FORMAT_SAMPLE.
    Sample(DataPayloadSample),
    /// FORMAT_DATA_SEQ.
    DataSeq(DataPayloadDataSeq),
    /// FORMAT_SAMPLE_SEQ.
    SampleSeq(DataPayloadSampleSeq),
    /// FORMAT_PACKED_SAMPLES.
    PackedSamples(DataPayloadPackedSamples),
}

impl AnyData {
    /// The `BaseObjectRequest` every variant starts with (`request_id`
    /// echoes the originating `READ_DATA`, `object_id` names the
    /// DataReader).
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
    /// `ValueOutOfRange` if the submessage is not `DATA` or the format bits
    /// are a reserved value; the per-format `UnexpectedEof` otherwise.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Data {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not DATA",
            });
        }
        match DataFormat::from_flag_bits(sm.header.flags)? {
            DataFormat::Data => Ok(Self::Data(DataPayload::try_from_submessage(sm)?)),
            DataFormat::Sample => Ok(Self::Sample(DataPayloadSample::try_from_submessage(sm)?)),
            DataFormat::DataSeq => Ok(Self::DataSeq(DataPayloadDataSeq::try_from_submessage(sm)?)),
            DataFormat::SampleSeq => Ok(Self::SampleSeq(
                DataPayloadSampleSeq::try_from_submessage(sm)?,
            )),
            DataFormat::PackedSamples => Ok(Self::PackedSamples(
                DataPayloadPackedSamples::try_from_submessage(sm)?,
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

    fn reader_base() -> BaseObjectRequest {
        BaseObjectRequest {
            request_id: [0x11, 0x22],
            object_id: ObjectId::new(0x0AB, ObjectKind::DataReader).unwrap(),
        }
    }

    #[test]
    fn data_roundtrip_format_data() {
        let p = DataPayload {
            base: reader_base(),
            serialized_data: alloc::vec![0xCC; 16],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(DataPayload::try_from_submessage(&sm).unwrap(), p);
    }

    #[test]
    fn data_base_object_request_precedes_the_sample() {
        let p = DataPayload {
            base: reader_base(),
            serialized_data: alloc::vec![0x01, 0x02],
        };
        let sm = p.into_submessage().unwrap();
        let oid = reader_base().object_id.to_bytes();
        assert_eq!(sm.body[0], 0x11, "request_id[0] echoes the read request");
        assert_eq!(sm.body[1], 0x22, "request_id[1] echoes the read request");
        assert_eq!(sm.body[2], oid[0], "object_id[0] = DataReader");
        assert_eq!(sm.body[3], oid[1], "object_id[1] = DataReader");
        // `serialized_data` follows the base class with no length of its
        // own — §7.7.1 and Annex B.3.1, the same rule as WRITE_DATA.
        assert_eq!(&sm.body[4..], &[0x01, 0x02]);
        assert_eq!(sm.body.len(), 6);
    }

    #[test]
    fn data_uses_id_9() {
        let sm = DataPayload {
            base: reader_base(),
            serialized_data: alloc::vec![],
        }
        .into_submessage()
        .unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::Data);
    }

    #[test]
    fn data_rejects_wrong_id() {
        let sm = Submessage::new(SubmessageId::WriteData, 0x01, alloc::vec![0; 8]).unwrap();
        assert!(DataPayload::try_from_submessage(&sm).is_err());
    }

    use crate::sample_info::{SampleInfo, SampleInfoDelta, SampleInfoFlags};

    fn sample_info(seq: u32, offset: u32) -> SampleInfo {
        SampleInfo {
            state: SampleInfoFlags(SampleInfoFlags::VIEW_STATE_NEW),
            sequence_number: seq,
            session_time_offset: offset,
        }
    }

    // ---- DataPayloadSample (FORMAT_SAMPLE) --------------------------------

    #[test]
    fn data_sample_roundtrip() {
        let p = DataPayloadSample {
            base: reader_base(),
            info: sample_info(3, 1500),
            serialized_data: alloc::vec![0xAA, 0xBB, 0xCC],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(DataPayloadSample::try_from_submessage(&sm).unwrap(), p);
    }

    /// DDS-XRCE 1.0 Annex B.5.3 ("Receiving a single sample that includes
    /// SampleInfo"): `BaseObjectRequest`(4) + `info.state`(1)+pad(3) +
    /// `info.sequence_number`(4) + `info.session_time_offset`(4) +
    /// `XCDR_Serialization(foo)`.
    #[test]
    fn data_sample_matches_annex_b_5_3_layout() {
        let p = DataPayloadSample {
            base: BaseObjectRequest {
                request_id: [0xAA, 0x02],
                object_id: ObjectId::new(0x0AB, ObjectKind::DataReader).unwrap(),
            },
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
    fn data_sample_rejects_non_sample_format() {
        let sm =
            Submessage::new(SubmessageId::Data, DataPayload::flags(), alloc::vec![0; 16]).unwrap();
        assert!(matches!(
            DataPayloadSample::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }

    // ---- DataPayloadDataSeq (FORMAT_DATA_SEQ) -----------------------------

    #[test]
    fn data_data_seq_roundtrip() {
        let p = DataPayloadDataSeq {
            base: reader_base(),
            sample_count: 2,
            elements: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(DataPayloadDataSeq::try_from_submessage(&sm).unwrap(), p);
    }

    /// DDS-XRCE 1.0 Annex B.5.2: `BaseObjectRequest`(4) +
    /// `data_seq.length`(4) + `data_seq[0].data`(4) + `data_seq[1].data`(4)
    /// — no per-element length.
    #[test]
    fn data_data_seq_matches_annex_b_5_2_layout() {
        let p = DataPayloadDataSeq {
            base: BaseObjectRequest {
                request_id: [0xAA, 0x01],
                object_id: ObjectId::from_bytes(&[0x44, 0x06]).unwrap(),
            },
            sample_count: 2,
            elements: alloc::vec![0, 0, 0, 1, 0, 0, 0, 2],
        };
        let sm = p.into_submessage().unwrap();
        assert_eq!(
            sm.header.submessage_length, 16,
            "Table 32: submessageLength=16"
        );
        assert_eq!(&sm.body[4..8], &2u32.to_le_bytes(), "data_seq.length");
        assert_eq!(&sm.body[8..12], &[0, 0, 0, 1], "data_seq[0].data");
        assert_eq!(&sm.body[12..16], &[0, 0, 0, 2], "data_seq[1].data");
    }

    // ---- DataPayloadSampleSeq (FORMAT_SAMPLE_SEQ) -------------------------

    #[test]
    fn data_sample_seq_roundtrip() {
        let p = DataPayloadSampleSeq {
            base: reader_base(),
            sample_count: 2,
            elements: alloc::vec![0u8; 32],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(DataPayloadSampleSeq::try_from_submessage(&sm).unwrap(), p);
    }

    /// DDS-XRCE 1.0 Annex B.5.4: `BaseObjectRequest`(4) +
    /// `sample_seq.length`(4) + (`info`(12) + `data`(4)) × 2 —
    /// submessageLength = 40.
    #[test]
    fn data_sample_seq_matches_annex_b_5_4_layout() {
        let mut elements = Vec::new();
        sample_info(0, 0)
            .encode_into(Endianness::Little, &mut elements)
            .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 1]);
        sample_info(0, 0)
            .encode_into(Endianness::Little, &mut elements)
            .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 2]);
        let p = DataPayloadSampleSeq {
            base: BaseObjectRequest {
                request_id: [0xAA, 0x02],
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
    }

    // ---- DataPayloadPackedSamples (FORMAT_PACKED_SAMPLES) -----------------

    #[test]
    fn data_packed_samples_roundtrip() {
        let p = DataPayloadPackedSamples {
            base: reader_base(),
            info_base: sample_info(1000, 0),
            sample_count: 2,
            elements: alloc::vec![0u8; 16],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(
            DataPayloadPackedSamples::try_from_submessage(&sm).unwrap(),
            p
        );
    }

    /// DDS-XRCE 1.0 Annex B.5.5: `BaseObjectRequest`(4) + `info_base`(12) +
    /// `sample_delta_seq.length`(4) + (`info_delta`(4) + `data`(4)) × 2 —
    /// submessageLength = 36.
    #[test]
    fn data_packed_samples_matches_annex_b_5_5_layout() {
        let mut elements = Vec::new();
        SampleInfoDelta {
            state: SampleInfoFlags(0),
            seq_number_delta: 0,
            timestamp_delta: 0,
        }
        .encode_into(Endianness::Little, &mut elements)
        .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 1]);
        SampleInfoDelta {
            state: SampleInfoFlags(0),
            seq_number_delta: 0,
            timestamp_delta: 0,
        }
        .encode_into(Endianness::Little, &mut elements)
        .unwrap();
        elements.extend_from_slice(&[0, 0, 0, 2]);
        let p = DataPayloadPackedSamples {
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
    }

    // ---- AnyData dispatch --------------------------------------------

    #[test]
    fn any_data_dispatches_every_format() {
        let base = reader_base();
        let cases: alloc::vec::Vec<(DataFormat, Submessage)> = alloc::vec![
            (
                DataFormat::Data,
                DataPayload {
                    base,
                    serialized_data: alloc::vec![1],
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::Sample,
                DataPayloadSample {
                    base,
                    info: sample_info(0, 0),
                    serialized_data: alloc::vec![1],
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::DataSeq,
                DataPayloadDataSeq {
                    base,
                    sample_count: 0,
                    elements: Vec::new(),
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::SampleSeq,
                DataPayloadSampleSeq {
                    base,
                    sample_count: 0,
                    elements: Vec::new(),
                }
                .into_submessage()
                .unwrap(),
            ),
            (
                DataFormat::PackedSamples,
                DataPayloadPackedSamples {
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
            let decoded = AnyData::try_from_submessage(&sm).unwrap();
            assert_eq!(decoded.base(), base);
            let matches_format = matches!(
                (&decoded, fmt),
                (AnyData::Data(_), DataFormat::Data)
                    | (AnyData::Sample(_), DataFormat::Sample)
                    | (AnyData::DataSeq(_), DataFormat::DataSeq)
                    | (AnyData::SampleSeq(_), DataFormat::SampleSeq)
                    | (AnyData::PackedSamples(_), DataFormat::PackedSamples)
            );
            assert!(matches_format, "AnyData variant did not match {fmt:?}");
            let re = decoded.into_submessage().unwrap();
            assert_eq!(re, sm);
        }
    }

    #[test]
    fn any_data_rejects_wrong_submessage_id() {
        let sm = Submessage::new(SubmessageId::WriteData, 0x01, alloc::vec![0; 8]).unwrap();
        assert!(matches!(
            AnyData::try_from_submessage(&sm),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }
}
