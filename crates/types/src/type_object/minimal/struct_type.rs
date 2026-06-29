// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalStructType (XTypes §7.3.4.4.1).

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, DecodeError, EncodeError};

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
    /// Buffer overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        // MinimalStructHeader is @appendable (XTypes §7.3.4.4.1) → DHEADER +
        // body. Body = base_type (the empty MinimalTypeDetail adds no bytes).
        zerodds_cdr::struct_enc::encode_appendable(w, |w| self.base_type.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        let base_type = zerodds_cdr::struct_enc::decode_appendable(r, TypeIdentifier::decode_from)?;
        Ok(Self { base_type })
    }
}

/// MinimalStructMember = CommonStructMember + NameHash (in Minimal
/// the full name is replaced by a 4-byte hash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimalStructMember {
    /// Type/id/flags.
    pub common: CommonStructMember,
    /// Hash des Member-Namens.
    pub detail: NameHash,
}

impl MinimalStructMember {
    /// Encode.
    ///
    /// # Errors
    /// Buffer overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        // MinimalStructMember is @appendable (XTypes §7.3.4.4.1) → each element
        // in the member_seq carries its own DHEADER + body.
        zerodds_cdr::struct_enc::encode_appendable(w, |w| {
            self.common.encode_into(w)?;
            self.detail.encode_into(w)
        })
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
        zerodds_cdr::struct_enc::decode_appendable(r, |r| {
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
    /// Buffer overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u16(self.struct_flags.0)?;
        self.header.encode_into(w)?;
        encode_seq_appendable(w, &self.member_seq, |w, m| m.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, DecodeError> {
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
