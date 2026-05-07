// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalStructType (XTypes §7.3.4.4.1).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{CommonStructMember, NameHash, decode_seq, encode_seq};
use crate::type_object::flags::StructTypeFlag;

/// Header fuer MinimalStructType (nur base_type, detail ist leer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStructHeader {
    /// Base-Type (Inheritance). `TypeIdentifier::None` wenn keine Vererbung.
    pub base_type: TypeIdentifier,
}

impl MinimalStructHeader {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.base_type.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            base_type: TypeIdentifier::decode_from(r)?,
        })
    }
}

/// MinimalStructMember = CommonStructMember + NameHash (im Minimal
/// wird der volle Name durch 4-byte-Hash ersetzt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStructMember {
    /// Typ/Id/Flags.
    pub common: CommonStructMember,
    /// Hash des Member-Namens.
    pub detail: NameHash,
}

impl MinimalStructMember {
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
        let common = CommonStructMember::decode_from(r)?;
        let detail = NameHash::decode_from(r)?;
        Ok(Self { common, detail })
    }
}

/// MinimalStructType.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStructType {
    /// Flags (IS_FINAL/APPENDABLE/MUTABLE/NESTED/AUTOID_HASH).
    pub struct_flags: StructTypeFlag,
    /// Header (base_type).
    pub header: MinimalStructHeader,
    /// Member-Liste.
    pub member_seq: Vec<MinimalStructMember>,
}

impl MinimalStructType {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.struct_flags.0)?;
        self.header.encode_into(w)?;
        encode_seq(w, &self.member_seq, |w, m| m.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let struct_flags = StructTypeFlag(r.read_u16()?);
        let header = MinimalStructHeader::decode_from(r)?;
        let member_seq = decode_seq(r, MinimalStructMember::decode_from)?;
        Ok(Self {
            struct_flags,
            header,
            member_seq,
        })
    }
}
