// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteBitsetType (XTypes §7.3.4.4).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::type_object::common::{
    CompleteMemberDetail, CompleteTypeDetail, decode_seq_appendable, encode_seq_appendable,
};
use crate::type_object::flags::{BitfieldFlag, BitsetTypeFlag};
use crate::type_object::minimal::CommonBitfield;

/// CompleteBitfield = common + CompleteMemberDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteBitfield {
    /// Common.
    pub common: CommonBitfield,
    /// Name + annotations.
    pub detail: CompleteMemberDetail,
}

/// CompleteBitsetType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteBitsetType {
    /// Flags.
    pub bitset_flags: BitsetTypeFlag,
    /// Detail.
    pub detail: CompleteTypeDetail,
    /// Fields.
    pub field_seq: Vec<CompleteBitfield>,
}

impl CompleteBitsetType {
    /// DDS-XTypes 1.3 Annex B (unchanged in 1.4 Beta 1): the type, its
    /// `CompleteBitsetHeader` (the type detail) and every
    /// `CompleteBitfield` are `@appendable` — a `DHEADER` in front of
    /// each under XCDR2, and one in front of the field sequence. Fast DDS
    /// 3.6.1 writes and hashes these octets.
    pub(super) fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.bitset_flags.0);
            crate::framing::encode_appendable(w, |w| self.detail.encode_into(w))?;
            encode_seq_appendable(w, &self.field_seq, |w, f| {
                crate::framing::encode_appendable(w, |w| {
                    w.write_u16(f.common.position);
                    w.write_u16(f.common.flags.0);
                    w.write_u8(f.common.bitcount);
                    w.write_u8(f.common.holder_type);
                    f.detail.encode_into(w)
                })
            })
        })
    }

    pub(super) fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let value = crate::framing::decode_appendable(r, |r| {
            let bitset_flags = BitsetTypeFlag(r.read_u16()?);
            let detail = crate::framing::decode_appendable(r, CompleteTypeDetail::decode_from)?;
            let field_seq = decode_seq_appendable(r, |r| {
                crate::framing::decode_appendable(r, |r| {
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
                })
            })?;
            Ok::<_, WireError>(Self {
                bitset_flags,
                detail,
                field_seq,
            })
        })?;
        Ok(value)
    }
}
