// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalBitmaskType (XTypes §7.3.4.4.9).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_object::common::{NameHash, decode_seq, encode_seq};
use crate::type_object::flags::{BitflagFlag, BitmaskTypeFlag};

/// Common-Bitflag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonBitflag {
    /// Position (0..bit_bound-1).
    pub position: u16,
    /// Flags.
    pub flags: BitflagFlag,
}

/// MinimalBitflag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimalBitflag {
    /// Common.
    pub common: CommonBitflag,
    /// Hash des Flag-Namens.
    pub detail: NameHash,
}

impl MinimalBitflag {
    fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.common.position)?;
        w.write_u16(self.common.flags.0)?;
        self.detail.encode_into(w)
    }

    fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let position = r.read_u16()?;
        let flags = BitflagFlag(r.read_u16()?);
        let detail = NameHash::decode_from(r)?;
        Ok(Self {
            common: CommonBitflag { position, flags },
            detail,
        })
    }
}

/// MinimalBitmaskType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalBitmaskType {
    /// Flags.
    pub bitmask_flags: BitmaskTypeFlag,
    /// Bit-Breite (Header).
    pub bit_bound: u16,
    /// Flag list.
    pub flag_seq: Vec<MinimalBitflag>,
}

impl MinimalBitmaskType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.bitmask_flags.0)?;
        w.write_u16(self.bit_bound)?;
        encode_seq(w, &self.flag_seq, |w, f| f.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let bitmask_flags = BitmaskTypeFlag(r.read_u16()?);
        let bit_bound = r.read_u16()?;
        let flag_seq = decode_seq(r, MinimalBitflag::decode_from)?;
        Ok(Self {
            bitmask_flags,
            bit_bound,
            flag_seq,
        })
    }
}
