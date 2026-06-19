// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalUnionType (XTypes §7.3.4.4.2).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{CommonUnionMember, NameHash, decode_seq, encode_seq};
use crate::type_object::flags::{UnionDiscriminatorFlag, UnionTypeFlag};

// MinimalUnionHeader is an empty type in the spec (§7.3.4.4.2). We
// no longer represent it explicitly as a zero-size struct — it had
// only no-op encode/decode methods and 20 lines of overhead for zero
// wire information (Finding #25). In `MinimalUnionType` the
// header field is implicitly omitted.

/// Discriminator of a union (§7.3.4.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonDiscriminatorMember {
    /// Flags on the discriminator (e.g. @key).
    pub member_flags: UnionDiscriminatorFlag,
    /// Discriminator type (in practice an enum or integer).
    pub type_id: TypeIdentifier,
}

impl CommonDiscriminatorMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.member_flags.0)?;
        self.type_id.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let member_flags = UnionDiscriminatorFlag(r.read_u16()?);
        let type_id = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            member_flags,
            type_id,
        })
    }
}

/// Discriminator in Minimal (= common + no detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalDiscriminatorMember {
    /// Common.
    pub common: CommonDiscriminatorMember,
}

impl MinimalDiscriminatorMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.common.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            common: CommonDiscriminatorMember::decode_from(r)?,
        })
    }
}

/// MinimalUnionMember.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalUnionMember {
    /// Common fields (IDs, flags, type, labels).
    pub common: CommonUnionMember,
    /// Hash of the member name.
    pub detail: NameHash,
}

impl MinimalUnionMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.common.encode_into(w)?;
        self.detail.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let common = CommonUnionMember::decode_from(r)?;
        let detail = NameHash::decode_from(r)?;
        Ok(Self { common, detail })
    }
}

/// MinimalUnionType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalUnionType {
    /// Flags.
    pub union_flags: UnionTypeFlag,
    /// Discriminator.
    pub discriminator: MinimalDiscriminatorMember,
    /// Case members.
    pub member_seq: Vec<MinimalUnionMember>,
}

impl MinimalUnionType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.union_flags.0)?;
        self.discriminator.encode_into(w)?;
        encode_seq(w, &self.member_seq, |w, m| m.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let union_flags = UnionTypeFlag(r.read_u16()?);
        let discriminator = MinimalDiscriminatorMember::decode_from(r)?;
        let member_seq = decode_seq(r, MinimalUnionMember::decode_from)?;
        Ok(Self {
            union_flags,
            discriminator,
            member_seq,
        })
    }
}
