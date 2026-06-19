// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalEnumeratedType (XTypes §7.3.4.4.5).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_object::common::{NameHash, decode_seq, encode_seq};
use crate::type_object::flags::{EnumLiteralFlag, EnumTypeFlag};

/// Common header for enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommonEnumeratedHeader {
    /// Bit width (8/16/32). Default 32.
    pub bit_bound: u16,
}

/// MinimalEnumeratedHeader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MinimalEnumeratedHeader {
    /// Common.
    pub common: CommonEnumeratedHeader,
}

/// CommonEnumeratedLiteral.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonEnumeratedLiteral {
    /// Ordinalwert.
    pub value: i32,
    /// Flags (IS_DEFAULT_LITERAL).
    pub flags: EnumLiteralFlag,
}

/// MinimalEnumeratedLiteral.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimalEnumeratedLiteral {
    /// Common.
    pub common: CommonEnumeratedLiteral,
    /// Hash of the label.
    pub detail: NameHash,
}

impl MinimalEnumeratedLiteral {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u32(self.common.value as u32)?;
        w.write_u16(self.common.flags.0)?;
        self.detail.encode_into(w)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let value = r.read_u32()? as i32;
        let flags = EnumLiteralFlag(r.read_u16()?);
        let detail = NameHash::decode_from(r)?;
        Ok(Self {
            common: CommonEnumeratedLiteral { value, flags },
            detail,
        })
    }
}

/// MinimalEnumeratedType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalEnumeratedType {
    /// Flags.
    pub enum_flags: EnumTypeFlag,
    /// Header (bit_bound).
    pub header: MinimalEnumeratedHeader,
    /// Literale.
    pub literal_seq: Vec<MinimalEnumeratedLiteral>,
}

impl MinimalEnumeratedType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.enum_flags.0)?;
        w.write_u16(self.header.common.bit_bound)?;
        encode_seq(w, &self.literal_seq, |w, l| l.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let enum_flags = EnumTypeFlag(r.read_u16()?);
        let bit_bound = r.read_u16()?;
        let literal_seq = decode_seq(r, MinimalEnumeratedLiteral::decode_from)?;
        Ok(Self {
            enum_flags,
            header: MinimalEnumeratedHeader {
                common: CommonEnumeratedHeader { bit_bound },
            },
            literal_seq,
        })
    }
}
