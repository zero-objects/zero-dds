// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteBitmaskType (XTypes §7.3.4.4).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_object::common::{
    CompleteMemberDetail, CompleteTypeDetail, decode_seq, encode_seq,
};
use crate::type_object::flags::{BitflagFlag, BitmaskTypeFlag};
use crate::type_object::minimal::CommonBitflag;

/// CompleteBitflag = common + CompleteMemberDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteBitflag {
    /// Position + flags.
    pub common: CommonBitflag,
    /// Name + annotations.
    pub detail: CompleteMemberDetail,
}

/// CompleteBitmaskType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteBitmaskType {
    /// Flags.
    pub bitmask_flags: BitmaskTypeFlag,
    /// Bit width.
    pub bit_bound: u16,
    /// Header-Detail.
    pub detail: CompleteTypeDetail,
    /// Flags.
    pub flag_seq: Vec<CompleteBitflag>,
}

impl CompleteBitmaskType {
    pub(super) fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.bitmask_flags.0)?;
        w.write_u16(self.bit_bound)?;
        self.detail.encode_into(w)?;
        encode_seq(w, &self.flag_seq, |w, f| {
            w.write_u16(f.common.position)?;
            w.write_u16(f.common.flags.0)?;
            f.detail.encode_into(w)
        })
    }

    pub(super) fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let bitmask_flags = BitmaskTypeFlag(r.read_u16()?);
        let bit_bound = r.read_u16()?;
        let detail = CompleteTypeDetail::decode_from(r)?;
        let flag_seq = decode_seq(r, |r| {
            let position = r.read_u16()?;
            let flags = BitflagFlag(r.read_u16()?);
            let detail = CompleteMemberDetail::decode_from(r)?;
            Ok(CompleteBitflag {
                common: CommonBitflag { position, flags },
                detail,
            })
        })?;
        Ok(Self {
            bitmask_flags,
            bit_bound,
            detail,
            flag_seq,
        })
    }
}
