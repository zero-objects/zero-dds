// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! MinimalTypeObject (XTypes 1.3 §7.3.4.4) — hash-effiziente, namenlose
//! Repraesentation fuer Wire-Transport ueber TypeLookup.

pub mod alias_type;
pub mod annotation_type;
pub mod bitmask_type;
pub mod bitset_type;
pub mod collection_types;
pub mod enum_type;
pub mod struct_type;
pub mod union_type;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError};

use crate::error::TypeCodecError;
use crate::type_object::kinds::{
    TK_ALIAS, TK_ANNOTATION, TK_ARRAY, TK_BITMASK, TK_BITSET, TK_ENUM, TK_MAP, TK_SEQUENCE,
    TK_STRUCTURE, TK_UNION,
};

pub use alias_type::{CommonAliasBody, MinimalAliasBody, MinimalAliasType};
pub use annotation_type::{
    AnnotationParameterValue, CommonAnnotationParameter, MinimalAnnotationParameter,
    MinimalAnnotationType,
};
pub use bitmask_type::{CommonBitflag, MinimalBitflag, MinimalBitmaskType};
pub use bitset_type::{CommonBitfield, HoldingType, MinimalBitfield, MinimalBitsetType};
pub use collection_types::{
    CommonCollectionElement, MinimalArrayType, MinimalCollectionElement, MinimalMapType,
    MinimalSequenceType,
};
pub use enum_type::{
    CommonEnumeratedHeader, CommonEnumeratedLiteral, MinimalEnumeratedHeader,
    MinimalEnumeratedLiteral, MinimalEnumeratedType,
};
pub use struct_type::{MinimalStructHeader, MinimalStructMember, MinimalStructType};
pub use union_type::{
    CommonDiscriminatorMember, MinimalDiscriminatorMember, MinimalUnionMember, MinimalUnionType,
};

/// MinimalTypeObject (§7.3.4.4). Discriminated Union mit 1-byte
/// TypeKind-Discriminator ueber allen Variants.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MinimalTypeObject {
    /// `alias<T>`.
    Alias(MinimalAliasType),
    /// `annotation @X(...)`.
    Annotation(MinimalAnnotationType),
    /// `struct`.
    Struct(MinimalStructType),
    /// `union`.
    Union(MinimalUnionType),
    /// `bitset`.
    Bitset(MinimalBitsetType),
    /// `sequence<T>`.
    Sequence(MinimalSequenceType),
    /// `T[N]`.
    Array(MinimalArrayType),
    /// `map<K, V>`.
    Map(MinimalMapType),
    /// `enum`.
    Enumerated(MinimalEnumeratedType),
    /// `bitmask`.
    Bitmask(MinimalBitmaskType),
}

impl MinimalTypeObject {
    /// TypeKind-Discriminator-Byte.
    #[must_use]
    pub const fn discriminator(&self) -> u8 {
        match self {
            Self::Alias(_) => TK_ALIAS,
            Self::Annotation(_) => TK_ANNOTATION,
            Self::Struct(_) => TK_STRUCTURE,
            Self::Union(_) => TK_UNION,
            Self::Bitset(_) => TK_BITSET,
            Self::Sequence(_) => TK_SEQUENCE,
            Self::Array(_) => TK_ARRAY,
            Self::Map(_) => TK_MAP,
            Self::Enumerated(_) => TK_ENUM,
            Self::Bitmask(_) => TK_BITMASK,
        }
    }

    /// Encode als `{ octet _d; body }`.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        w.write_u8(self.discriminator())?;
        match self {
            Self::Alias(a) => a.encode_into(w),
            Self::Annotation(a) => a.encode_into(w),
            Self::Struct(s) => s.encode_into(w),
            Self::Union(u) => u.encode_into(w),
            Self::Bitset(b) => b.encode_into(w),
            Self::Sequence(s) => s.encode_into(w),
            Self::Array(a) => a.encode_into(w),
            Self::Map(m) => m.encode_into(w),
            Self::Enumerated(e) => e.encode_into(w),
            Self::Bitmask(b) => b.encode_into(w),
        }
    }

    /// Decode.
    ///
    /// # Errors
    /// `TypeCodecError::UnknownTypeKind` bei unbekanntem Discriminator,
    /// sonst CDR-Decoder-Fehler.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let d = r.read_u8()?;
        Ok(match d {
            TK_ALIAS => Self::Alias(MinimalAliasType::decode_from(r)?),
            TK_ANNOTATION => Self::Annotation(MinimalAnnotationType::decode_from(r)?),
            TK_STRUCTURE => Self::Struct(MinimalStructType::decode_from(r)?),
            TK_UNION => Self::Union(MinimalUnionType::decode_from(r)?),
            TK_BITSET => Self::Bitset(MinimalBitsetType::decode_from(r)?),
            TK_SEQUENCE => Self::Sequence(MinimalSequenceType::decode_from(r)?),
            TK_ARRAY => Self::Array(MinimalArrayType::decode_from(r)?),
            TK_MAP => Self::Map(MinimalMapType::decode_from(r)?),
            TK_ENUM => Self::Enumerated(MinimalEnumeratedType::decode_from(r)?),
            TK_BITMASK => Self::Bitmask(MinimalBitmaskType::decode_from(r)?),
            other => return Err(TypeCodecError::UnknownTypeKind { kind: other }),
        })
    }

    /// Convenience: in LE-Bytes serialisieren.
    ///
    /// # Errors
    /// Encode-Fehler.
    pub fn to_bytes_le(&self) -> Result<alloc::vec::Vec<u8>, EncodeError> {
        let mut w = BufferWriter::new(zerodds_cdr::Endianness::Little);
        self.encode_into(&mut w)?;
        Ok(w.into_bytes())
    }

    /// Convenience: aus LE-Bytes deserialisieren.
    ///
    /// # Errors
    /// Decode-Fehler.
    pub fn from_bytes_le(bytes: &[u8]) -> Result<Self, TypeCodecError> {
        let mut r = BufferReader::new(bytes, zerodds_cdr::Endianness::Little);
        Self::decode_from(&mut r)
    }
}
