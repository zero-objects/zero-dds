// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalUnionType (XTypes §7.3.4.4.2).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{CommonUnionMember, NameHash, decode_seq, encode_seq};
use crate::type_object::flags::{UnionDiscriminatorFlag, UnionTypeFlag};

// MinimalUnionHeader ist in der Spec ein leerer Typ (§7.3.4.4.2). Wir
// stellen ihn nicht mehr explizit als Zero-Size-Struct dar — das hatte
// nur no-op encode/decode-Methoden und 20 Zeilen Overhead fuer null
// Wire-Information (Finding #25). Im `MinimalUnionType` bleibt das
// header-Feld implizit weg.

/// Diskriminator einer Union (§7.3.4.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonDiscriminatorMember {
    /// Flags auf dem Discriminator (z.B. @key).
    pub member_flags: UnionDiscriminatorFlag,
    /// Discriminator-Typ (in praxi Enum oder Integer).
    pub type_id: TypeIdentifier,
}

impl CommonDiscriminatorMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.member_flags.0)?;
        self.type_id.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let member_flags = UnionDiscriminatorFlag(r.read_u16()?);
        let type_id = TypeIdentifier::decode_from(r)?;
        Ok(Self {
            member_flags,
            type_id,
        })
    }
}

/// Diskriminator im Minimal (= common + kein detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalDiscriminatorMember {
    /// Common.
    pub common: CommonDiscriminatorMember,
}

impl MinimalDiscriminatorMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.common.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            common: CommonDiscriminatorMember::decode_from(r)?,
        })
    }
}

/// MinimalUnionMember.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalUnionMember {
    /// Common-Fields (IDs, Flags, Typ, Labels).
    pub common: CommonUnionMember,
    /// Hash des Member-Namens.
    pub detail: NameHash,
}

impl MinimalUnionMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.common.encode_into(w)?;
        self.detail.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
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
    /// Case-Members.
    pub member_seq: Vec<MinimalUnionMember>,
}

impl MinimalUnionType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.union_flags.0)?;
        self.discriminator.encode_into(w)?;
        encode_seq(w, &self.member_seq, |w, m| m.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
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
