// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalBitmaskType (XTypes §7.3.4.4.9).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::type_object::common::{NameHash, decode_seq_appendable, encode_seq_appendable};
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
    /// Hash of the flag name.
    pub detail: NameHash,
}

impl MinimalBitflag {
    /// `MinimalBitflag` is `@appendable` (Annex B), so under XCDR2 each
    /// flag carries its own `DHEADER`; `CommonBitflag` inside it is
    /// `@final` and has none.
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.common.position);
            w.write_u16(self.common.flags.0);
            self.detail.encode_into(w)
        })
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let position = r.read_u16()?;
            let flags = BitflagFlag(r.read_u16()?);
            let detail = NameHash::decode_from(r)?;
            Ok(Self {
                common: CommonBitflag { position, flags },
                detail,
            })
        })
    }
}

/// MinimalBitmaskType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalBitmaskType {
    /// Flags.
    pub bitmask_flags: BitmaskTypeFlag,
    /// Bit width (header).
    pub bit_bound: u16,
    /// Flag list.
    pub flag_seq: Vec<MinimalBitflag>,
}

impl MinimalBitmaskType {
    /// Encode.
    ///
    /// DDS-XTypes 1.3 Annex B (unchanged in 1.4 Beta 1,
    /// `dds-xtypes_typeobject.idl`): `MinimalBitmaskType` is
    /// `@appendable`, its `MinimalBitmaskHeader` is the `@appendable`
    /// `MinimalEnumeratedHeader`, and `MinimalBitflagSeq` holds
    /// `@appendable` elements. Under XCDR2 that is a `DHEADER` around the
    /// type, one around the header, one around the sequence and one per
    /// flag — the octets CycloneDDS 11.0.1 and Fast DDS 3.6.1 write and
    /// hash (`tests/typeobject_bitmask_bitset_alias_vendors.rs`).
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.bitmask_flags.0);
            crate::framing::encode_appendable(w, |w| {
                w.write_u16(self.bit_bound);
                Ok::<_, WireError>(())
            })?;
            encode_seq_appendable(w, &self.flag_seq, |w, f| f.encode_into(w))
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let bitmask_flags = BitmaskTypeFlag(r.read_u16()?);
            let bit_bound = crate::framing::decode_appendable(r, |r| r.read_u16())?;
            let flag_seq = decode_seq_appendable(r, MinimalBitflag::decode_from)?;
            Ok(Self {
                bitmask_flags,
                bit_bound,
                flag_seq,
            })
        })
    }
}
