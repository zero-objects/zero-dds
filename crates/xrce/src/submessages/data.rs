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
//! **Phase 1 scope:** only `FORMAT_DATA` — `BaseObjectRequest` + a
//! `SampleData` `sequence<octet>` (u32 length in body endianness + bytes).
//! The `SampleInfo`-bearing formats (FORMAT_SAMPLE etc.) stay Phase 2.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::{Endianness, read_u32, write_u32};
use crate::error::XrceError;
use crate::object_info::BaseObjectRequest;
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

    /// Encodes the submessage body: `BaseObjectRequest` (4 octets) then the
    /// `SampleData` `sequence<octet>` (u32 length + bytes).
    ///
    /// # Errors
    /// `ValueOutOfRange` if `serialized_data` exceeds `u32`.
    pub fn encode_body(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let len =
            u32::try_from(self.serialized_data.len()).map_err(|_| XrceError::ValueOutOfRange {
                message: "DATA serialized_data exceeds u32",
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
                message: "DATA: only FORMAT_DATA is structured in phase 1",
            });
        }
        let e = sm.header.body_endianness();
        let (base, n) = BaseObjectRequest::decode(&sm.body)?;
        let rest = &sm.body[n..];
        let len = read_u32(rest, e)?;
        let len = usize::try_from(len).map_err(|_| XrceError::ValueOutOfRange {
            message: "DATA serialized_data length exceeds usize",
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
        assert_eq!(&sm.body[4..8], &[2, 0, 0, 0]);
        assert_eq!(&sm.body[8..10], &[0x01, 0x02]);
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
}
