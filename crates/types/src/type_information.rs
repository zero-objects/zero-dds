// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeInformation (XTypes 1.3 §7.6.3.2.2) — Wrapper fuer die
//! Discovery von strongly-hashed TypeObjects inkl. transitiver
//! Abhaengigkeiten.
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
//! Das wird als Payload des `PID_TYPE_INFORMATION` (0x0075) in SEDP
//! Publikations-/Subscriptions-Announcements uebertragen (T8).
//!
//! Die Dependencies beschreiben transitiv benoetigte TypeObjects, die
//! der Empfaenger ueber den TypeLookup-Service (T11..T15) nachladen
//! kann, sofern ihm nur der TypeIdentifier bekannt ist.

use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter, EncodeError, Endianness};

use crate::error::TypeCodecError;
use crate::hash::{compute_complete_hash, compute_minimal_hash};
use crate::type_identifier::TypeIdentifier;
use crate::type_object::common::{decode_seq, encode_seq};
use crate::type_object::{CompleteTypeObject, MinimalTypeObject};

/// TypeIdentifier + Groesse des serialisierten TypeObjects (§7.6.3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentifierWithSize {
    /// Typ-Referenz (direkt oder strongly-hashed).
    pub type_id: TypeIdentifier,
    /// Groesse des serialisierten TypeObjects in Bytes.
    ///
    /// Null, wenn `type_id` ein direkt-identifiziertes Primitive ist
    /// (kein TypeObject braucht zu existieren) oder Groesse unbekannt.
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
    /// Haupt-Typ.
    pub typeid_with_size: TypeIdentifierWithSize,
    /// Anzahl transitiver Dependencies (kann `-1` sein = "unknown").
    ///
    /// Wenn `dependent_typeids.len() == dependent_typeid_count`, ist
    /// die Liste vollstaendig. Bei `-1` kann der Empfaenger via
    /// TypeLookup `getTypeDependencies` nachladen.
    pub dependent_typeid_count: i32,
    /// Transitive Dependencies (nur die, die der Sender kennt).
    pub dependent_typeids: Vec<TypeIdentifierWithSize>,
}

impl TypeIdentifierWithDependencies {
    /// Kurz-Konstruktor fuer Typen ohne bekannte Dependencies.
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
    /// Buffer-Underflow / Unknown-Kind-Fehler aus [`TypeIdentifier`].
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

/// TypeInformation (§7.6.3.2.2) — tupelt `minimal` und `complete`
/// TypeIdentifier-Referenzen. Wird als Payload von `PID_TYPE_INFORMATION`
/// (0x0075) in SEDP-Announcements uebertragen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeInformation {
    /// Minimal-TypeIdentifier mit Dependencies.
    pub minimal: TypeIdentifierWithDependencies,
    /// Complete-TypeIdentifier mit Dependencies.
    pub complete: TypeIdentifierWithDependencies,
}

impl TypeInformation {
    /// Baut TypeInformation aus einem Minimal/Complete-Paar, ohne
    /// transitive Dependencies. Nuetzlich wenn der Typ self-contained
    /// ist (alle Member sind Primitives / plain Collections).
    ///
    /// # Errors
    /// `EncodeError` beim Serialisieren fuer Size/Hash.
    pub fn from_minimal_and_complete(
        minimal: &MinimalTypeObject,
        complete: &CompleteTypeObject,
    ) -> Result<Self, EncodeError> {
        let min_hash = compute_minimal_hash(minimal)?;
        let com_hash = compute_complete_hash(complete)?;
        // Groesse = Bytes des wrapped TypeObjects (inkl. EquivalenceKind).
        // Bei >4GB (extrem unrealistisch) echten Fehler liefern statt
        // stummer Truncation — sonst bricht die Hash-Validation beim Peer
        // mit kryptischer Fehlermeldung.
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

    /// Fuegt eine transitive Dependency zu beiden Seiten hinzu.
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

    /// Encode als XCDR-CDR-Bytes (ohne Encapsulation-Header — der wird
    /// vom PID-Layer geliefert).
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

    /// Convenience: aus LE-Bytes.
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

        // Roundtrip auch mit Dependencies
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
