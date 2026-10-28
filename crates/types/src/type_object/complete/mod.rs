// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CompleteTypeObject (XTypes 1.3 §7.3.4.4) — full
//! representation with names + annotations.
//!
//! The structure is strictly parallel to [`super::minimal`]: for each
//! type kind there is a `Complete<Foo>Type` in its own file.

pub mod alias_type;
pub mod annotation_type;
pub mod bitmask_type;
pub mod bitset_type;
pub mod collection_types;
pub mod enum_type;
pub mod struct_type;
pub mod union_type;

use zerodds_wire::{Reader, WireError, Writer};

use crate::error::TypeCodecError;
use crate::type_object::kinds::{
    TK_ALIAS, TK_ANNOTATION, TK_ARRAY, TK_BITMASK, TK_BITSET, TK_ENUM, TK_MAP, TK_SEQUENCE,
    TK_STRUCTURE, TK_UNION,
};

pub use alias_type::{CompleteAliasBody, CompleteAliasHeader, CompleteAliasType};
pub use annotation_type::{
    AnnotationParameterOrder, CompleteAnnotationHeader, CompleteAnnotationParameter,
    CompleteAnnotationType, annotation_parameter_order, ordered_by_name,
    set_annotation_parameter_order,
};
pub use bitmask_type::{CompleteBitflag, CompleteBitmaskType};
pub use bitset_type::{CompleteBitfield, CompleteBitsetType};
pub use collection_types::{
    CompleteArrayType, CompleteCollectionElement, CompleteMapType, CompleteSequenceType,
};
pub use enum_type::{CompleteEnumeratedHeader, CompleteEnumeratedLiteral, CompleteEnumeratedType};
pub use struct_type::{CompleteStructHeader, CompleteStructMember, CompleteStructType};
pub use union_type::{
    CompleteDiscriminatorMember, CompleteUnionHeader, CompleteUnionMember, CompleteUnionType,
};

/// CompleteTypeObject (§7.3.4.4).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompleteTypeObject {
    /// `alias<T>`.
    Alias(CompleteAliasType),
    /// `annotation @X(...)`.
    Annotation(CompleteAnnotationType),
    /// `struct`.
    Struct(CompleteStructType),
    /// `union`.
    Union(CompleteUnionType),
    /// `bitset`.
    Bitset(CompleteBitsetType),
    /// `sequence<T>`.
    Sequence(CompleteSequenceType),
    /// `T[N]`.
    Array(CompleteArrayType),
    /// `map<K, V>`.
    Map(CompleteMapType),
    /// `enum`.
    Enumerated(CompleteEnumeratedType),
    /// `bitmask`.
    Bitmask(CompleteBitmaskType),
    /// The `default:` arm of Annex B's `union CompleteTypeObject`:
    /// `CompleteExtendedType extended_type`, under the comment
    /// `// =========== Future extensibility ============`.
    ///
    /// `CompleteExtendedType` is
    /// `@extensibility(MUTABLE) @nested struct { /* Empty. Available
    /// for future extension */ }`, so under encoding version 2 the
    /// arm is a `DHEADER` over a member list this revision has no
    /// member of. The member list is held verbatim — the `DHEADER`
    /// excluded, because that is a length the encoder writes and a
    /// byte order this value does not carry — so an object of a
    /// future TypeKind survives a decode and an encode unchanged
    /// instead of being refused.
    Extended {
        /// The TypeKind the stream carried.
        type_kind: u8,
        /// The `CompleteExtendedType` member list, verbatim.
        body: alloc::vec::Vec<u8>,
    },
}

impl CompleteTypeObject {
    /// Discriminator-Byte.
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
            Self::Extended { type_kind, .. } => *type_kind,
        }
    }

    /// Encode.
    ///
    /// # Errors
    /// A length that does not fit a `u32`.
    pub fn encode_into(&self, w: &mut Writer) -> Result<(), WireError> {
        w.write_u8(self.discriminator());
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
            // The member list goes back behind a `DHEADER` this
            // encoder writes, so the value carries no byte order of
            // its own.
            Self::Extended { body, .. } => crate::framing::encode_extended_members(w, body),
        }
    }

    /// Decode.
    ///
    /// # Errors
    /// `TypeCodecError::UnknownTypeKind` on an unknown discriminator.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, TypeCodecError> {
        let d = r.read_u8()?;
        Ok(match d {
            TK_ALIAS => Self::Alias(CompleteAliasType::decode_from(r)?),
            TK_ANNOTATION => Self::Annotation(CompleteAnnotationType::decode_from(r)?),
            TK_STRUCTURE => Self::Struct(CompleteStructType::decode_from(r)?),
            TK_UNION => Self::Union(CompleteUnionType::decode_from(r)?),
            TK_BITSET => Self::Bitset(CompleteBitsetType::decode_from(r)?),
            TK_SEQUENCE => Self::Sequence(CompleteSequenceType::decode_from(r)?),
            TK_ARRAY => Self::Array(CompleteArrayType::decode_from(r)?),
            TK_MAP => Self::Map(CompleteMapType::decode_from(r)?),
            TK_ENUM => Self::Enumerated(CompleteEnumeratedType::decode_from(r)?),
            TK_BITMASK => Self::Bitmask(CompleteBitmaskType::decode_from(r)?),
            // Annex B: `default: CompleteExtendedType extended_type;`.
            // Refusing here refused every stream carrying a TypeKind a
            // later revision defines, including one this reader could
            // have skipped past — the arm is `@extensibility(MUTABLE)`
            // and a `DHEADER` says how far.
            other => Self::Extended {
                type_kind: other,
                body: crate::framing::decode_extended_members(r)?,
            },
        })
    }
}
