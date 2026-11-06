// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalBitsetType (XTypes §7.3.4.4.11).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::type_object::common::{NameHash, decode_seq_appendable, encode_seq_appendable};
use crate::type_object::flags::{BitfieldFlag, BitsetTypeFlag};

/// Bitfield holding-type — reduziert (spec §7.3.4.4.11: TypeKind u8).
pub type HoldingType = u8;

/// CommonBitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonBitfield {
    /// Start position.
    pub position: u16,
    /// Flags.
    pub flags: BitfieldFlag,
    /// Width in bits (1..64).
    pub bitcount: u8,
    /// Holding type (e.g. TK_UINT32).
    pub holder_type: HoldingType,
}

/// MinimalBitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimalBitfield {
    /// Common.
    pub common: CommonBitfield,
    /// Hash of the name.
    pub name_hash: NameHash,
}

impl MinimalBitfield {
    /// `MinimalBitfield` is `@appendable` (Annex B): a `DHEADER` in front
    /// of each field under XCDR2; the `@final` `CommonBitfield` inside
    /// has none.
    fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.common.position);
            w.write_u16(self.common.flags.0);
            w.write_u8(self.common.bitcount);
            w.write_u8(self.common.holder_type);
            self.name_hash.encode_into(w)
        })
    }

    fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let position = r.read_u16()?;
            let flags = BitfieldFlag(r.read_u16()?);
            let bitcount = r.read_u8()?;
            let holder_type = r.read_u8()?;
            let name_hash = NameHash::decode_from(r)?;
            Ok(Self {
                common: CommonBitfield {
                    position,
                    flags,
                    bitcount,
                    holder_type,
                },
                name_hash,
            })
        })
    }
}

/// MinimalBitsetType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalBitsetType {
    /// Flags.
    pub bitset_flags: BitsetTypeFlag,
    /// Fields.
    pub field_seq: Vec<MinimalBitfield>,
}

impl MinimalBitsetType {
    /// Encode.
    ///
    /// DDS-XTypes 1.3 Annex B (unchanged in 1.4 Beta 1): the type, its
    /// empty `MinimalBitsetHeader` and every `MinimalBitfield` are
    /// `@appendable` — a `DHEADER` in front of each under XCDR2, and one
    /// in front of the field sequence. Fast DDS 3.6.1 writes and hashes
    /// these octets (`tests/typeobject_bitmask_bitset_alias_vendors.rs`).
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        crate::framing::encode_appendable(w, |w| {
            w.write_u16(self.bitset_flags.0);
            // MinimalBitsetHeader: `@appendable` and empty, a DHEADER of 0.
            crate::framing::encode_appendable(w, |_| Ok::<_, WireError>(()))?;
            encode_seq_appendable(w, &self.field_seq, |w, f| f.encode_into(w))
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let bitset_flags = BitsetTypeFlag(r.read_u16()?);
            // MinimalBitsetHeader has no member yet; what a later revision
            // appends to it is skipped with its DHEADER.
            crate::framing::decode_appendable(r, |_| Ok(()))?;
            let field_seq = decode_seq_appendable(r, MinimalBitfield::decode_from)?;
            Ok(Self {
                bitset_flags,
                field_seq,
            })
        })
    }
}
