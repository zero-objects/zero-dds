// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalStructType (XTypes §7.3.4.4.1).

use alloc::vec::Vec;

use zerodds_wire::{Reader, WireError, Writer};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{
    CommonStructMember, NameHash, decode_seq_appendable, encode_seq_appendable,
};
use crate::type_object::flags::StructTypeFlag;

/// Header for MinimalStructType (only base_type, detail is empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStructHeader {
    /// Base type (inheritance). `TypeIdentifier::None` if no inheritance.
    pub base_type: TypeIdentifier,
}

impl MinimalStructHeader {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        // MinimalStructHeader is @appendable (XTypes §7.3.4.4.1) → DHEADER +
        // body. Body = base_type (the empty MinimalTypeDetail adds no bytes).
        crate::framing::encode_appendable(w, |w| self.base_type.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let base_type = crate::framing::decode_appendable(r, TypeIdentifier::decode_from)?;
        Ok(Self { base_type })
    }
}

/// MinimalStructMember = CommonStructMember + NameHash (in Minimal
/// the full name is replaced by a 4-byte hash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStructMember {
    /// Type/id/flags.
    pub common: CommonStructMember,
    /// Hash of the member name.
    pub detail: NameHash,
}

impl MinimalStructMember {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        // MinimalStructMember is @appendable (XTypes §7.3.4.4.1) → each element
        // in the member_seq carries its own DHEADER + body.
        crate::framing::encode_appendable(w, |w| {
            self.common.encode_into(w)?;
            self.detail.encode_into(w)?;
            // RTI Connext/CoreDX append one octet inside each minimal
            // aggregated member's DHEADER body; Cyclone/Fast DDS/HDDS do
            // not. XTypes 1.3 §7.3.4.5 leaves it open which camp's stream
            // the EquivalenceHash is taken over; `docs/omg-xtypes-issues.md`
            // §C2 records both. Off by default (Cyclone camp); the writer
            // carries the choice, set from the peer's dialect.
            if w.minimal_member_trailing_octet()
                && crate::framing::EncodingVersion::of_writer(w)
                    == crate::framing::EncodingVersion::Xcdr2
            {
                w.write_u8(0);
            }
            Ok(())
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        crate::framing::decode_appendable(r, |r| {
            let common = CommonStructMember::decode_from(r)?;
            let detail = NameHash::decode_from(r)?;
            Ok(Self { common, detail })
        })
    }
}

/// MinimalStructType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStructType {
    /// Flags (IS_FINAL/APPENDABLE/MUTABLE/NESTED/AUTOID_HASH).
    pub struct_flags: StructTypeFlag,
    /// Header (base_type).
    pub header: MinimalStructHeader,
    /// Member list.
    pub member_seq: Vec<MinimalStructMember>,
}

impl MinimalStructType {
    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u16(self.struct_flags.0);
        self.header.encode_into(w)?;
        encode_seq_appendable(w, &self.member_seq, |w, m| m.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// The stream ends inside this value.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let struct_flags = StructTypeFlag(r.read_u16()?);
        let header = MinimalStructHeader::decode_from(r)?;
        let member_seq = decode_seq_appendable(r, MinimalStructMember::decode_from)?;
        Ok(Self {
            struct_flags,
            header,
            member_seq,
        })
    }
}
