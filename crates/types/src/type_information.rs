// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeInformation (XTypes 1.3 §7.6.3.2.2) — wrapper for the
//! discovery of strongly-hashed TypeObjects incl. transitive
//! dependencies.
//!
//! Wire-Format:
//!
//! ```text
//! struct TypeIdentifierWithSize {
//!     TypeIdentifier type_id;
//!     uint32         typeobject_serialized_size;
//! };
//!
//! struct TypeIdentifierWithDependencies {
//!     TypeIdentifierWithSize       typeid_with_size;
//!     int32                        dependent_typeid_count; // -1 = unknown
//!     sequence<TypeIdentifierWithSize> dependent_typeids;
//! };
//!
//! struct TypeInformation {
//!     TypeIdentifierWithDependencies minimal;
//!     TypeIdentifierWithDependencies complete;
//! };
//! ```
//!
//! This is transmitted as the payload of `PID_TYPE_INFORMATION` (0x0075) in SEDP
//! publication/subscription announcements (T8).
//!
//! The dependencies describe transitively needed TypeObjects, which
//! the receiver can fetch via the TypeLookup service (T11..T15),
//! as long as it only knows the TypeIdentifier.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError, Endianness};

use crate::error::TypeCodecError;
use crate::hash::{compute_complete_hash, compute_minimal_hash};
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{decode_seq, encode_seq};
use crate::type_object::{CompleteTypeObject, MinimalTypeObject};

/// TypeIdentifier + size of the serialized TypeObject (§7.6.3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierWithSize {
    /// Type reference (direct or strongly-hashed).
    pub type_id: TypeIdentifier,
    /// Size of the serialized TypeObject in bytes.
    ///
    /// Zero if `type_id` is a directly-identified primitive
    /// (no TypeObject needs to exist) or the size is unknown.
    pub typeobject_serialized_size: u32,
}

impl TypeIdentifierWithSize {
    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.type_id.encode_into(w)?;
        w.write_u32(self.typeobject_serialized_size)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let type_id = TypeIdentifier::decode_from(r)?;
        let typeobject_serialized_size = r.read_u32()?;
        Ok(Self {
            type_id,
            typeobject_serialized_size,
        })
    }
}

/// TypeIdentifier + Abhaengigkeiten (§7.6.3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierWithDependencies {
    /// Main type.
    pub typeid_with_size: TypeIdentifierWithSize,
    /// Number of transitive dependencies (can be `-1` = "unknown").
    ///
    /// When `dependent_typeids.len() == dependent_typeid_count`, the
    /// list is complete. With `-1` the receiver can fetch more via
    /// TypeLookup `getTypeDependencies`.
    pub dependent_typeid_count: i32,
    /// Transitive dependencies (only those the sender knows).
    pub dependent_typeids: Vec<TypeIdentifierWithSize>,
}

impl TypeIdentifierWithDependencies {
    /// Short constructor for types without known dependencies.
    #[must_use]
    pub fn without_dependencies(typeid_with_size: TypeIdentifierWithSize) -> Self {
        Self {
            typeid_with_size,
            dependent_typeid_count: 0,
            dependent_typeids: Vec::new(),
        }
    }

    /// Encode.
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.typeid_with_size.encode_into(w)?;
        w.write_u32(self.dependent_typeid_count as u32)?;
        encode_seq(w, &self.dependent_typeids, |w, t| t.encode_into(w))
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer underflow / unknown-kind error from [`TypeIdentifier`].
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let typeid_with_size = TypeIdentifierWithSize::decode_from(r)?;
        let dependent_typeid_count = r.read_u32()? as i32;
        let dependent_typeids = decode_seq(r, |rr| {
            TypeIdentifierWithSize::decode_from(rr).map_err(|e| match e {
                TypeCodecError::Encode(_) => zerodds_cdr::DecodeError::UnexpectedEof {
                    needed: 0,
                    offset: 0,
                },
                TypeCodecError::Decode(d) => d,
                TypeCodecError::UnknownTypeKind { .. } => zerodds_cdr::DecodeError::UnexpectedEof {
                    needed: 0,
                    offset: 0,
                },
            })
        })?;
        Ok(Self {
            typeid_with_size,
            dependent_typeid_count,
            dependent_typeids,
        })
    }
}

/// TypeInformation (§7.6.3.2.2) — tuples `minimal` and `complete`
/// TypeIdentifier references. Transmitted as the payload of `PID_TYPE_INFORMATION`
/// (0x0075) in SEDP announcements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInformation {
    /// Minimal TypeIdentifier with dependencies.
    pub minimal: TypeIdentifierWithDependencies,
    /// Complete TypeIdentifier with dependencies.
    pub complete: TypeIdentifierWithDependencies,
}

impl TypeInformation {
    /// Builds TypeInformation from a minimal/complete pair, without
    /// transitive dependencies. Useful when the type is self-contained
    /// (all members are primitives / plain collections).
    ///
    /// # Errors
    /// `EncodeError` when serializing for size/hash.
    pub fn from_minimal_and_complete(
        minimal: &MinimalTypeObject,
        complete: &CompleteTypeObject,
    ) -> Result<Self, EncodeError> {
        let min_hash = compute_minimal_hash(minimal)?;
        let com_hash = compute_complete_hash(complete)?;
        // Size = bytes of the wrapped TypeObject (incl. EquivalenceKind).
        // At >4GB (extremely unrealistic) return a real error instead of
        // silent truncation — otherwise hash validation at the peer breaks
        // with a cryptic error message.
        let min_size = u32::try_from(
            crate::type_object::TypeObject::Minimal(minimal.clone())
                .to_bytes_le()?
                .len(),
        )
        .map_err(|_| EncodeError::ValueOutOfRange {
            message: "minimal TypeObject serialized size exceeds u32::MAX",
        })?;
        let com_size = u32::try_from(
            crate::type_object::TypeObject::Complete(complete.clone())
                .to_bytes_le()?
                .len(),
        )
        .map_err(|_| EncodeError::ValueOutOfRange {
            message: "complete TypeObject serialized size exceeds u32::MAX",
        })?;
        Ok(Self {
            minimal: TypeIdentifierWithDependencies::without_dependencies(TypeIdentifierWithSize {
                type_id: TypeIdentifier::EquivalenceHashMinimal(min_hash),
                typeobject_serialized_size: min_size,
            }),
            complete: TypeIdentifierWithDependencies::without_dependencies(
                TypeIdentifierWithSize {
                    type_id: TypeIdentifier::EquivalenceHashComplete(com_hash),
                    typeobject_serialized_size: com_size,
                },
            ),
        })
    }

    /// Adds a transitive dependency to both sides.
    pub fn add_dependency(
        &mut self,
        minimal_dep: TypeIdentifierWithSize,
        complete_dep: TypeIdentifierWithSize,
    ) {
        self.minimal.dependent_typeids.push(minimal_dep);
        self.minimal.dependent_typeid_count =
            self.minimal.dependent_typeids.len().min(i32::MAX as usize) as i32;
        self.complete.dependent_typeids.push(complete_dep);
        self.complete.dependent_typeid_count =
            self.complete.dependent_typeids.len().min(i32::MAX as usize) as i32;
    }

    /// Encode as XCDR-CDR bytes (without the encapsulation header — that is
    /// provided by the PID layer).
    ///
    /// # Errors
    /// Buffer-Overflow.
    pub fn encode_into(&self, w: &mut BufferWriter) -> Result<(), EncodeError> {
        self.minimal.encode_into(w)?;
        self.complete.encode_into(w)
    }

    /// Decode.
    ///
    /// # Errors
    /// Buffer-Underflow / Unknown-TypeKind.
    pub fn decode_from(r: &mut BufferReader<'_>) -> Result<Self, TypeCodecError> {
        let minimal = TypeIdentifierWithDependencies::decode_from(r)?;
        let complete = TypeIdentifierWithDependencies::decode_from(r)?;
        Ok(Self { minimal, complete })
    }

    /// Convenience: LE-Bytes.
    ///
    /// # Errors
    /// Encode.
    pub fn to_bytes_le(&self) -> Result<Vec<u8>, EncodeError> {
        let mut w = BufferWriter::new(Endianness::Little);
        self.encode_into(&mut w)?;
        Ok(w.into_bytes())
    }

    /// Convenience: from LE bytes.
    ///
    /// # Errors
    /// Decode / Unknown-TypeKind.
    pub fn from_bytes_le(bytes: &[u8]) -> Result<Self, TypeCodecError> {
        let mut r = BufferReader::new(bytes, Endianness::Little);
        Self::decode_from(&mut r)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::builder::{Extensibility, TypeObjectBuilder};
    use crate::type_identifier::{EquivalenceHash, PrimitiveKind};

    fn sample_type_info() -> TypeInformation {
        let b = TypeObjectBuilder::struct_type("::chat::Chatter")
            .extensibility(Extensibility::Appendable)
            .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| {
                m.key()
            })
            .member("text", TypeIdentifier::String8Small { bound: 255 }, |m| m);
        let minimal = MinimalTypeObject::Struct(b.build_minimal());
        let complete = CompleteTypeObject::Struct(b.build_complete());
        TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap()
    }

    #[test]
    fn typeinformation_roundtrips() {
        let ti = sample_type_info();
        let bytes = ti.to_bytes_le().unwrap();
        let decoded = TypeInformation::from_bytes_le(&bytes).unwrap();
        assert_eq!(ti, decoded);
    }

    #[test]
    fn typeinformation_minimal_has_minimal_discriminator() {
        let ti = sample_type_info();
        assert!(matches!(
            ti.minimal.typeid_with_size.type_id,
            TypeIdentifier::EquivalenceHashMinimal(_)
        ));
        assert!(matches!(
            ti.complete.typeid_with_size.type_id,
            TypeIdentifier::EquivalenceHashComplete(_)
        ));
    }

    #[test]
    fn typeinformation_size_matches_actual_typeobject_bytes() {
        let b = TypeObjectBuilder::struct_type("::X").member(
            "a",
            TypeIdentifier::Primitive(PrimitiveKind::Int32),
            |m| m,
        );
        let minimal = MinimalTypeObject::Struct(b.build_minimal());
        let complete = CompleteTypeObject::Struct(b.build_complete());
        let actual_min_size = crate::type_object::TypeObject::Minimal(minimal.clone())
            .to_bytes_le()
            .unwrap()
            .len();
        let actual_com_size = crate::type_object::TypeObject::Complete(complete.clone())
            .to_bytes_le()
            .unwrap()
            .len();
        let ti = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
        assert_eq!(
            ti.minimal.typeid_with_size.typeobject_serialized_size,
            actual_min_size as u32
        );
        assert_eq!(
            ti.complete.typeid_with_size.typeobject_serialized_size,
            actual_com_size as u32
        );
    }

    #[test]
    fn add_dependency_updates_count_and_list() {
        let mut ti = sample_type_info();
        assert_eq!(ti.minimal.dependent_typeid_count, 0);
        let dep_min = TypeIdentifierWithSize {
            type_id: TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0xAA; 14])),
            typeobject_serialized_size: 42,
        };
        let dep_com = TypeIdentifierWithSize {
            type_id: TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([0xBB; 14])),
            typeobject_serialized_size: 84,
        };
        ti.add_dependency(dep_min.clone(), dep_com.clone());
        assert_eq!(ti.minimal.dependent_typeid_count, 1);
        assert_eq!(ti.minimal.dependent_typeids[0], dep_min);
        assert_eq!(ti.complete.dependent_typeid_count, 1);
        assert_eq!(ti.complete.dependent_typeids[0], dep_com);

        // Roundtrip with dependencies too
        let bytes = ti.to_bytes_le().unwrap();
        assert_eq!(TypeInformation::from_bytes_le(&bytes).unwrap(), ti);
    }

    #[test]
    fn negative_dependent_typeid_count_roundtrips() {
        // -1 = "unknown/incomplete" (spec-compliant).
        let mut ti = sample_type_info();
        ti.minimal.dependent_typeid_count = -1;
        ti.complete.dependent_typeid_count = -1;
        let bytes = ti.to_bytes_le().unwrap();
        let decoded = TypeInformation::from_bytes_le(&bytes).unwrap();
        assert_eq!(decoded.minimal.dependent_typeid_count, -1);
        assert_eq!(decoded.complete.dependent_typeid_count, -1);
    }
}
