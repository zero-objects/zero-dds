// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeIdentifier hash computation (XTypes 1.3 §7.3.1.2).
//!
//! The `EquivalenceHash` (14 bytes) is formed from the **first 14 bytes** of the
//! **MD5** digest over the XCDR2-serialized TypeObject representation
//! (XTypes 1.3 §7.3.1.2.1):
//!
//! ```text
//! EquivalenceHash(T) := MD5(xcdr2_le_bytes(T))[0..14]
//! ```
//!
//! Cyclone DDS and Fast-DDS use MD5 — hence mandatory for live interop.
//!
//! - For `MinimalTypeObject T`: `EK_MINIMAL` hash
//! - For `CompleteTypeObject T`: `EK_COMPLETE` hash
//!
//! From the hash a TypeIdentifier of kind `EK_MINIMAL` resp.
//! `EK_COMPLETE` is built — this is the "strongly-hashed" form of the
//! TypeObject, as exchanged between peers via SEDP (`PID_TYPE_INFORMATION`) and
//! the TypeLookup service.

use zerodds_cdr::{BufferWriter, EncodeError, Endianness};
use zerodds_foundation::md5;

use crate::type_identifier::kinds::{EK_COMPLETE, EK_MINIMAL, EQUIVALENCE_HASH_LEN};
use crate::type_identifier::{EquivalenceHash, TypeIdentifier};
use crate::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};

/// Computes the 14-byte EquivalenceHash of a TypeObject.
///
/// Serializes the TypeObject to XCDR2-LE bytes (incl. the equivalence-
/// kind discriminator), hashes with MD5 and truncates to 14 bytes.
///
/// # Errors
/// `EncodeError` if serialization overflows.
pub fn compute_hash(to: &TypeObject) -> Result<EquivalenceHash, EncodeError> {
    let bytes = to.to_bytes_le()?;
    Ok(hash_bytes(&bytes))
}

/// Like [`compute_hash`], but directly over `MinimalTypeObject` (without
/// the EquivalenceKind discriminator wrapper clone). Writes the
/// EK_MINIMAL discriminator before the body itself.
///
/// # Errors
/// `EncodeError`.
pub fn compute_minimal_hash(t: &MinimalTypeObject) -> Result<EquivalenceHash, EncodeError> {
    let mut w = BufferWriter::new(Endianness::Little);
    w.write_u8(EK_MINIMAL)?;
    t.encode_into(&mut w)?;
    Ok(hash_bytes(&w.into_bytes()))
}

/// Like [`compute_minimal_hash`], only for `CompleteTypeObject`.
///
/// # Errors
/// `EncodeError`.
pub fn compute_complete_hash(t: &CompleteTypeObject) -> Result<EquivalenceHash, EncodeError> {
    let mut w = BufferWriter::new(Endianness::Little);
    w.write_u8(EK_COMPLETE)?;
    t.encode_into(&mut w)?;
    Ok(hash_bytes(&w.into_bytes()))
}

/// Raw hash function: MD5 + truncate to 14 bytes.
///
/// MD5 is **spec-conformant** here, not cryptographic. XTypes 1.3
/// §7.3.1.2.1 requires MD5 for wire compatibility with other DDS
/// implementations (Cyclone, Fast-DDS, RTI Connext).
#[must_use]
pub fn hash_bytes(data: &[u8]) -> EquivalenceHash {
    let digest = md5(data);
    let mut out = [0u8; EQUIVALENCE_HASH_LEN];
    out.copy_from_slice(&digest[..EQUIVALENCE_HASH_LEN]);
    EquivalenceHash(out)
}

/// Shortcut: builds a strongly-hashed TypeIdentifier from a
/// TypeObject. Wraps [`compute_hash`] in the matching `EquivalenceHash*`
/// TypeIdentifier variant depending on Minimal/Complete.
///
/// # Errors
/// `EncodeError`.
pub fn to_hashed_type_identifier(to: &TypeObject) -> Result<TypeIdentifier, EncodeError> {
    let h = compute_hash(to)?;
    Ok(match to {
        TypeObject::Minimal(_) => TypeIdentifier::EquivalenceHashMinimal(h),
        TypeObject::Complete(_) => TypeIdentifier::EquivalenceHashComplete(h),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    use crate::type_identifier::PrimitiveKind;
    use crate::type_object::common::{CommonStructMember, NameHash};
    use crate::type_object::flags::{StructMemberFlag, StructTypeFlag};
    use crate::type_object::minimal::{
        MinimalStructHeader, MinimalStructMember, MinimalStructType,
    };

    fn sample_minimal_struct(field_count: u32) -> MinimalTypeObject {
        MinimalTypeObject::Struct(MinimalStructType {
            struct_flags: StructTypeFlag(StructTypeFlag::IS_APPENDABLE),
            header: MinimalStructHeader {
                base_type: TypeIdentifier::None,
            },
            member_seq: (0..field_count)
                .map(|i| MinimalStructMember {
                    common: CommonStructMember {
                        member_id: i + 1,
                        member_flags: StructMemberFlag::default(),
                        member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                    },
                    detail: NameHash([i as u8; 4]),
                })
                .collect(),
        })
    }

    #[test]
    fn hash_is_14_bytes_and_deterministic() {
        let t = sample_minimal_struct(3);
        let h1 = compute_minimal_hash(&t).unwrap();
        let h2 = compute_minimal_hash(&t).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.0.len(), 14);
    }

    #[test]
    fn different_type_objects_have_different_hashes() {
        let t1 = sample_minimal_struct(3);
        let t2 = sample_minimal_struct(4);
        let h1 = compute_minimal_hash(&t1).unwrap();
        let h2 = compute_minimal_hash(&t2).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn minimal_and_complete_same_semantic_differ_in_hash() {
        // Even if Minimal and Complete represent the same "type",
        // the equivalence-kind discriminator at the start of the
        // serialized bytes distinguishes them → different hashes.
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteMemberDetail,
            CompleteTypeDetail, OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{
            CompleteStructHeader, CompleteStructMember, CompleteStructType,
        };

        let minimal = sample_minimal_struct(1);
        let complete = CompleteStructType {
            struct_flags: StructTypeFlag(StructTypeFlag::IS_APPENDABLE),
            header: CompleteStructHeader {
                base_type: TypeIdentifier::None,
                detail: CompleteTypeDetail {
                    ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                    type_name: alloc::string::String::from("::Sample"),
                },
            },
            member_seq: alloc::vec![CompleteStructMember {
                common: CommonStructMember {
                    member_id: 1,
                    member_flags: StructMemberFlag::default(),
                    member_type_id: TypeIdentifier::Primitive(PrimitiveKind::Int64),
                },
                detail: CompleteMemberDetail {
                    name: alloc::string::String::from("x"),
                    ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                    ann_custom: OptionalAppliedAnnotationSeq::default(),
                },
            }],
        };

        let hm = compute_minimal_hash(&minimal).unwrap();
        let complete_wrapped = CompleteTypeObject::Struct(complete);
        let hc = compute_complete_hash(&complete_wrapped).unwrap();
        assert_ne!(hm, hc, "minimal and complete must hash to different values");
    }

    #[test]
    fn to_hashed_type_identifier_picks_correct_kind() {
        let minimal = sample_minimal_struct(2);
        let ti = to_hashed_type_identifier(&TypeObject::Minimal(minimal.clone())).unwrap();
        assert!(matches!(ti, TypeIdentifier::EquivalenceHashMinimal(_)));

        // Complete variant with a minimal shape (we only simulate the
        // dispatch) — here we use an array fixture so that no
        // name needs to be verified.
        use crate::type_object::common::{
            AppliedBuiltinMemberAnnotations, AppliedBuiltinTypeAnnotations, CompleteTypeDetail,
            OptionalAppliedAnnotationSeq,
        };
        use crate::type_object::complete::{CompleteCollectionElement, CompleteSequenceType};
        use crate::type_object::flags::{CollectionElementFlag, CollectionTypeFlag};
        use crate::type_object::minimal::CommonCollectionElement;

        let complete_seq = CompleteSequenceType {
            collection_flag: CollectionTypeFlag::default(),
            bound: 10,
            detail: CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::Seq"),
            },
            element: CompleteCollectionElement {
                common: CommonCollectionElement {
                    element_flags: CollectionElementFlag::default(),
                    type_id: TypeIdentifier::Primitive(PrimitiveKind::Int32),
                },
                ann_builtin: AppliedBuiltinMemberAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
            },
        };
        let ti2 = to_hashed_type_identifier(&TypeObject::Complete(CompleteTypeObject::Sequence(
            complete_seq,
        )))
        .unwrap();
        assert!(matches!(ti2, TypeIdentifier::EquivalenceHashComplete(_)));
    }

    #[test]
    fn hash_bytes_matches_md5_truncated_reference() {
        // MD5("") = d41d8cd98f00b204e9800998ecf8427e
        // Erste 14 bytes: d4 1d 8c d9 8f 00 b2 04 e9 80 09 98 ec f8
        let h = hash_bytes(b"");
        let expected: [u8; 14] = [
            0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
        ];
        assert_eq!(h.0, expected);
    }

    #[test]
    fn hash_bytes_deterministic_for_known_input() {
        let h1 = hash_bytes(b"ZeroDDS");
        let h2 = hash_bytes(b"ZeroDDS");
        assert_eq!(h1, h2);
        let h3 = hash_bytes(b"ZeroDDs"); // case-sensitiv
        assert_ne!(h1, h3);
    }
}
