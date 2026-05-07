// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteAnnotationType (XTypes §7.3.4.4).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{CompleteTypeDetail, MemberId, decode_seq, encode_seq};
use crate::type_object::flags::{AnnotationParameterFlag, AnnotationTypeFlag};

/// CompleteAnnotationParameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAnnotationParameter {
    /// Member-ID.
    pub member_id: MemberId,
    /// Flags.
    pub member_flags: AnnotationParameterFlag,
    /// Parameter-Typ.
    pub member_type_id: TypeIdentifier,
    /// Parameter-Name.
    pub name: String,
    /// Default-Value als opaque bytes.
    pub default_value: Vec<u8>,
}

/// CompleteAnnotationType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteAnnotationType {
    /// Flags.
    pub annotation_flag: AnnotationTypeFlag,
    /// Header-Detail.
    pub detail: CompleteTypeDetail,
    /// Parameter.
    pub member_seq: Vec<CompleteAnnotationParameter>,
}

impl CompleteAnnotationType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.annotation_flag.0)?;
        self.detail.encode_into(w)?;
        encode_seq(w, &self.member_seq, |w, p| {
            w.write_u32(p.member_id)?;
            w.write_u16(p.member_flags.0)?;
            p.member_type_id.encode_into(w)?;
            w.write_string(&p.name)?;
            let len =
                u32::try_from(p.default_value.len()).map_err(|_| EncodeError::ValueOutOfRange {
                    message: "annotation default value exceeds u32::MAX",
                })?;
            w.write_u32(len)?;
            w.write_bytes(&p.default_value)
        })
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let annotation_flag = AnnotationTypeFlag(r.read_u16()?);
        let detail = CompleteTypeDetail::decode_from(r)?;
        let member_seq = decode_seq(r, |r| {
            let member_id = r.read_u32()?;
            let member_flags = AnnotationParameterFlag(r.read_u16()?);
            let member_type_id = TypeIdentifier::decode_from(r)?;
            let name = r.read_string()?;
            let len = r.read_u32()? as usize;
            let default_value = r.read_bytes(len)?.to_vec();
            Ok(CompleteAnnotationParameter {
                member_id,
                member_flags,
                member_type_id,
                name,
                default_value,
            })
        })?;
        Ok(Self {
            annotation_flag,
            detail,
            member_seq,
        })
    }
}
