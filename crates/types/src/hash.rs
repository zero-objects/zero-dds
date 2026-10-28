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

use zerodds_wire::WireError;
use zerodds_wire::md5;

use crate::type_identifier::kinds::EQUIVALENCE_HASH_LEN;
use crate::type_identifier::{EquivalenceHash, TypeIdentifier};
use crate::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};

/// Computes the 14-byte EquivalenceHash of a TypeObject.
///
/// Serializes the TypeObject to XCDR2-LE bytes (incl. the equivalence-
/// kind discriminator), hashes with MD5 and truncates to 14 bytes.
///
/// # Errors
/// `WireError` if serialization overflows.
pub fn compute_hash(to: &TypeObject) -> Result<EquivalenceHash, WireError> {
    // Route through the per-variant functions so EVERY hash path uses the same
    // XCDR2 framing (DHEADER + EK + body). Both variants now obtain that
    // stream from TypeObject::to_bytes_le; no additional DHEADER is needed.
    match to {
        TypeObject::Minimal(m) => compute_minimal_hash(m),
        TypeObject::Complete(c) => compute_complete_hash(c),
    }
}

/// Like [`compute_hash`], but accepting a `MinimalTypeObject`. Wraps a clone
/// as `TypeObject::Minimal` so the shared encoder writes DHEADER, EK_MINIMAL
/// and the body exactly once.
///
/// # Errors
/// `WireError`.
pub fn compute_minimal_hash(t: &MinimalTypeObject) -> Result<EquivalenceHash, WireError> {
    compute_minimal_hash_with(t, false)
}

/// Like [`compute_minimal_hash`], choosing the minimal-`TypeObject`
/// vendor camp. `minimal_member_trailing_octet` selects the RTI/CoreDX
/// reading (`docs/omg-xtypes-issues.md` §C2); `false` is the default
/// Cyclone/Fast DDS camp.
///
/// # Errors
/// `WireError`.
pub fn compute_minimal_hash_with(
    t: &MinimalTypeObject,
    minimal_member_trailing_octet: bool,
) -> Result<EquivalenceHash, WireError> {
    Ok(hash_bytes(&minimal_hash_input_with(
        t,
        minimal_member_trailing_octet,
    )?))
}

/// Like [`compute_minimal_hash`], only for `CompleteTypeObject`.
///
/// # Errors
/// `WireError`.
pub fn compute_complete_hash(t: &CompleteTypeObject) -> Result<EquivalenceHash, WireError> {
    Ok(hash_bytes(&complete_hash_input(t)?))
}

/// The exact byte stream fed to MD5 for a `MinimalTypeObject` hash (XCDR2
/// DHEADER + EK + body). Exposed so conformance harnesses can byte-diff it
/// against vendor TypeObject goldens.
///
/// # Errors
/// `WireError`.
pub fn minimal_hash_input(t: &MinimalTypeObject) -> Result<alloc::vec::Vec<u8>, WireError> {
    minimal_hash_input_with(t, false)
}

/// Like [`minimal_hash_input`], choosing the minimal-`TypeObject`
/// vendor camp (`docs/omg-xtypes-issues.md` §C2).
///
/// # Errors
/// `WireError`.
pub fn minimal_hash_input_with(
    t: &MinimalTypeObject,
    minimal_member_trailing_octet: bool,
) -> Result<alloc::vec::Vec<u8>, WireError> {
    TypeObject::Minimal(t.clone()).to_bytes_le_with(minimal_member_trailing_octet)
}

/// The exact byte stream fed to MD5 for a `CompleteTypeObject` hash.
///
/// # Errors
/// `WireError`.
pub fn complete_hash_input(t: &CompleteTypeObject) -> Result<alloc::vec::Vec<u8>, WireError> {
    TypeObject::Complete(t.clone()).to_bytes_le()
}

/// The `typeobject_serialized_size` of a `MinimalTypeObject`.
///
/// XTypes 1.3 §7.6.3.2.1 says `minimal.typeid_with_size` carries "the
/// MINIMAL Hash TypeIdentifier of the TopicType and the serialized size of
/// the associated TypeObject", and the Annex-B comment on `EquivalenceHash`
/// says the identifier is the "First 14 bytes of MD5 of the serialized
/// TypeObject using XCDR version 2 with Little Endian encoding". One object,
/// one serialization: the size is the length of the very stream the hash is
/// taken over. Annex B declares `union TypeObject` `@extensibility(APPENDABLE)`,
/// so that stream opens with a DHEADER and the DHEADER counts.
///
/// The length is taken from [`minimal_hash_input`], which now delegates to
/// `TypeObject::to_bytes_le` including its DHEADER. The historical unframed
/// form was four octets short. Announcing that shorter number is what made
/// RTI Connext 7.7.0 refuse a ZeroDDS endpoint
/// at discovery — it had the same TypeObject under the same COMPLETE hash,
/// counted 84 octets for it and was told 80
/// (`PRESTypeObjectV2Table_assertTypeObjectRecord: TypeObjectV2
/// serializedSize mismatch`).
///
/// # Errors
/// `WireError`, and a TypeObject that does not fit a `u32` serialized.
pub fn minimal_serialized_size(t: &MinimalTypeObject) -> Result<u32, WireError> {
    serialized_size_of(minimal_hash_input(t)?.len())
}

/// The `typeobject_serialized_size` of a `CompleteTypeObject`. See
/// [`minimal_serialized_size`] for the clause and for why the DHEADER counts.
///
/// # Errors
/// `WireError`, and a TypeObject that does not fit a `u32` serialized.
pub fn complete_serialized_size(t: &CompleteTypeObject) -> Result<u32, WireError> {
    serialized_size_of(complete_hash_input(t)?.len())
}

/// Narrows a serialized length to the `u32` the field holds.
///
/// A TypeObject at or beyond 4 GiB is not reachable in practice, and the
/// truncation that `as` would do is the silent shape: the peer would receive
/// a size that matches nothing, look up the hash, find a different length
/// and reject the endpoint with a message about the type rather than about
/// the number.
fn serialized_size_of(len: usize) -> Result<u32, WireError> {
    u32::try_from(len).map_err(|_| WireError::OutOfRange {
        at: 0,
        expected: alloc::string::String::from("a TypeObject shorter than u32::MAX serialized"),
    })
}

/// XTypes 1.3 §7.3.4.5: the serialized TypeObject that the EquivalenceHash is
/// computed over is an **XCDR2** delimited stream — a 4-byte DHEADER (the byte
/// length of the body) followed by the EquivalenceKind discriminator + the
/// TypeObject body, all under the XCDR2 alignment cap (4). Byte-verified
/// against CycloneDDS 11.0.1, Fast DDS 3.6.1 and RTI Connext 7.7 — the
/// goldens and the per-vendor extractors are
/// `idl-conformance/proofs/typeobject/` in the `zerodds-examples`
/// repository, and `crates/types/tests/typeobject_vendor_dheader.rs`
/// carries the `Plain` case's 55 octets in this one. The old code used a
/// plain (XCDR1) writer with no DHEADER, which matched no vendor.
///
/// **That stream is now what [`TypeObject::encode_into`] itself writes.**
/// This module used to add the DHEADER on top of an `encode_into` that
/// wrote only the discriminator and the body, which made
/// `TypeObject::to_bytes_le` four octets shorter than the stream the hash
/// and the `typeobject_serialized_size` were taken over. Announcing that
/// shorter number is what made RTI Connext 7.7.0 refuse a ZeroDDS
/// endpoint at discovery — it had the same TypeObject under the same
/// COMPLETE hash, counted 84 octets for it and was told 80
/// (`PRESTypeObjectV2Table_assertTypeObjectRecord: TypeObjectV2
/// serializedSize mismatch`). Both paths now draw on the one encoder, so
/// the two numbers cannot drift apart again — and the header is written
/// exactly once. A second one added here would move every octet of every
/// hash this project computes, and a peer that disagrees about a hash
/// looks like a type mismatch rather than like a framing defect.
///
/// `typeobject_hash_input_is_the_serialized_object` in this module holds
/// the two together.
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
/// `WireError`.
pub fn to_hashed_type_identifier(to: &TypeObject) -> Result<TypeIdentifier, WireError> {
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
            detail: Some(CompleteTypeDetail {
                ann_builtin: AppliedBuiltinTypeAnnotations::default(),
                ann_custom: OptionalAppliedAnnotationSeq::default(),
                type_name: alloc::string::String::from("::Seq"),
            }),
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
        // First 14 bytes: d4 1d 8c d9 8f 00 b2 04 e9 80 09 98 ec f8
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
