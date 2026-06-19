// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteStructType (XTypes §7.3.4.4).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    CommonStructMember, CompleteMemberDetail, CompleteTypeDetail, decode_seq, encode_seq,
};
use crate::type_object::flags::StructTypeFlag;

/// CompleteStructHeader = base_type + CompleteTypeDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStructHeader {
    /// Base type (inheritance).
    pub base_type: TypeIdentifier,
    /// Type name + annotations.
    pub detail: CompleteTypeDetail,
}

/// CompleteStructMember = common + CompleteMemberDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStructMember {
    /// Type/id/flags.
    pub common: CommonStructMember,
    /// Name + annotations.
    pub detail: CompleteMemberDetail,
}

/// CompleteStructType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStructType {
    /// Flags.
    pub struct_flags: StructTypeFlag,
    /// Header.
    pub header: CompleteStructHeader,
    /// Members.
    pub member_seq: Vec<CompleteStructMember>,
}

impl CompleteStructType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.struct_flags.0)?;
        self.header.base_type.encode_into(w)?;
        self.header.detail.encode_into(w)?;
        encode_seq(w, &self.member_seq, |w, m| {
            m.common.encode_into(w)?;
            m.detail.encode_into(w)
        })
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let struct_flags = StructTypeFlag(r.read_u16()?);
        let base_type = TypeIdentifier::decode_from(r)?;
        let detail = CompleteTypeDetail::decode_from(r)?;
        let member_seq = decode_seq(r, |r| {
            let common = CommonStructMember::decode_from(r)?;
            let detail = CompleteMemberDetail::decode_from(r)?;
            Ok(CompleteStructMember { common, detail })
        })?;
        Ok(Self {
            struct_flags,
            header: CompleteStructHeader { base_type, detail },
            member_seq,
        })
    }
}
