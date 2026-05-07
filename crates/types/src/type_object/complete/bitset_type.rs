// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteBitsetType (XTypes §7.3.4.4).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_object::common::{
    CompleteMemberDetail, CompleteTypeDetail, decode_seq, encode_seq,
};
use crate::type_object::flags::{BitfieldFlag, BitsetTypeFlag};
use crate::type_object::minimal::CommonBitfield;

/// CompleteBitfield = common + CompleteMemberDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteBitfield {
    /// Common.
    pub common: CommonBitfield,
    /// Name + Annotationen.
    pub detail: CompleteMemberDetail,
}

/// CompleteBitsetType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteBitsetType {
    /// Flags.
    pub bitset_flags: BitsetTypeFlag,
    /// Detail.
    pub detail: CompleteTypeDetail,
    /// Felder.
    pub field_seq: Vec<CompleteBitfield>,
}

impl CompleteBitsetType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.bitset_flags.0)?;
        self.detail.encode_into(w)?;
        encode_seq(w, &self.field_seq, |w, f| {
            w.write_u16(f.common.position)?;
            w.write_u16(f.common.flags.0)?;
            w.write_u8(f.common.bitcount)?;
            w.write_u8(f.common.holder_type)?;
            f.detail.encode_into(w)
        })
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let bitset_flags = BitsetTypeFlag(r.read_u16()?);
        let detail = CompleteTypeDetail::decode_from(r)?;
        let field_seq = decode_seq(r, |r| {
            let position = r.read_u16()?;
            let flags = BitfieldFlag(r.read_u16()?);
            let bitcount = r.read_u8()?;
            let holder_type = r.read_u8()?;
            let detail = CompleteMemberDetail::decode_from(r)?;
            Ok(CompleteBitfield {
                common: CommonBitfield {
                    position,
                    flags,
                    bitcount,
                    holder_type,
                },
                detail,
            })
        })?;
        Ok(Self {
            bitset_flags,
            detail,
            field_seq,
        })
    }
}
