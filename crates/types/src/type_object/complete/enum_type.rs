// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteEnumeratedType (XTypes §7.3.4.4).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::type_object::common::{
    CompleteMemberDetail, CompleteTypeDetail, decode_seq_appendable, encode_seq_appendable,
};
use crate::type_object::flags::{EnumLiteralFlag, EnumTypeFlag};
use crate::type_object::minimal::{CommonEnumeratedHeader, CommonEnumeratedLiteral};

/// CompleteEnumeratedHeader = common + detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteEnumeratedHeader {
    /// Common (bit_bound).
    pub common: CommonEnumeratedHeader,
    /// Detail (type_name + annotations).
    pub detail: CompleteTypeDetail,
}

/// CompleteEnumeratedLiteral = common + CompleteMemberDetail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteEnumeratedLiteral {
    /// Ordinal + flags.
    pub common: CommonEnumeratedLiteral,
    /// Name + annotations.
    pub detail: CompleteMemberDetail,
}

/// CompleteEnumeratedType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteEnumeratedType {
    /// Flags.
    pub enum_flags: EnumTypeFlag,
    /// Header.
    pub header: CompleteEnumeratedHeader,
    /// Literale.
    pub literal_seq: Vec<CompleteEnumeratedLiteral>,
}

impl CompleteEnumeratedType {
    pub(super) fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.enum_flags.0);
        // CompleteEnumeratedHeader is @appendable → DHEADER around bit_bound +
        // CompleteTypeDetail. Each literal: per-literal @appendable DHEADER, the
        // CommonEnumeratedLiteral itself nested @appendable (value+flags), then
        // the detail — mirrors the MINIMAL framing (byte-verified vs FastDDS).
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.header.common.bit_bound);
            self.header.detail.encode_into(w)
        })?;
        encode_seq_appendable(w, &self.literal_seq, |w, l| {
            crate::framing::encode_appendable(w, |w| {
                crate::framing::encode_appendable(w, |w| {
                    w.write_u32(l.common.value as u32);
                    {
                        w.write_u16(l.common.flags.0);
                        Ok(())
                    }
                })?;
                l.detail.encode_into(w)
            })
        })
    }

    pub(super) fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let enum_flags = EnumTypeFlag(r.read_u16()?);
        let (bit_bound, detail) = crate::framing::decode_appendable(r, |r| {
            let bit_bound = r.read_u16()?;
            let detail = CompleteTypeDetail::decode_from(r)?;
            Ok::<_, zerodds_wire::WireError>((bit_bound, detail))
        })?;
        let literal_seq = decode_seq_appendable(r, |r| {
            crate::framing::decode_appendable(r, |r| {
                let (value, flags) = crate::framing::decode_appendable(r, |r| {
                    let value = r.read_u32()? as i32;
                    let flags = EnumLiteralFlag(r.read_u16()?);
                    Ok::<_, zerodds_wire::WireError>((value, flags))
                })?;
                let detail = CompleteMemberDetail::decode_from(r)?;
                Ok(CompleteEnumeratedLiteral {
                    common: CommonEnumeratedLiteral { value, flags },
                    detail,
                })
            })
        })?;
        Ok(Self {
            enum_flags,
            header: CompleteEnumeratedHeader {
                common: CommonEnumeratedHeader { bit_bound },
                detail,
            },
            literal_seq,
        })
    }
}
