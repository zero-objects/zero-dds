// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Cross-impl test vectors for TypeMatcher (WP 1.6 T5).
//!
//! Shaped by Cyclone DDS + eProsima Fast-DDS as a reference:
//! which publication↔subscription combinations match in their
//! implementation, and which do not? We check each case in both
//! directions (writer struct/reader struct swapped) to find
//! asymmetries.

#![allow(clippy::unwrap_used, clippy::panic)]

use crate::TypeIdentifier;
use crate::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use crate::resolve::TypeRegistry;
use crate::type_identifier::PrimitiveKind;
use crate::type_matcher::TypeMatcher;

fn reg() -> TypeRegistry {
    TypeRegistry::new()
}

fn default_tce() -> TypeConsistencyEnforcement {
    TypeConsistencyEnforcement::default()
}

// ---------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------

/// Identical primitives always match.
#[test]
fn identical_int32_matches() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    assert!(m.match_types(&w, &w, &reg()).is_match());
}

/// int16 → int32 with default TCE (AllowTypeCoercion, no
/// PreventWidening): Cyclone/Fast-DDS match here, so do we.
#[test]
fn widening_int16_to_int32_matches_default_tce() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    assert!(m.match_types(&w, &r, &reg()).is_match());
}

/// Narrowing int32 → int16: all impls reject.
#[test]
fn narrowing_int32_to_int16_mismatch() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Int16);
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

/// Float32 → Float64 widening, accepted with default TCE.
#[test]
fn widening_float_matches() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Float32);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Float64);
    assert!(m.match_types(&w, &r, &reg()).is_match());
}

/// Cross-type float ↔ int is not coerced.
#[test]
fn float_int_cross_type_mismatch() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Float32);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

// ---------------------------------------------------------------------
// Strings with bounds
// ---------------------------------------------------------------------

/// String8 bounds are IGNORED by default (TCE default).
/// Writer-bound 256 > reader-bound 64 → match anyway.
#[test]
fn string_bounds_ignored_by_default_tce() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::String8Small { bound: 255 };
    let r = TypeIdentifier::String8Small { bound: 64 };
    assert!(m.match_types(&w, &r, &reg()).is_match());
}

/// If `ignore_string_bounds=false`, writer bound must be ≤ reader bound.
#[test]
fn string_bound_overflow_rejected_when_strict() {
    let mut tce = default_tce();
    tce.ignore_string_bounds = false;
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::String8Small { bound: 255 };
    let r = TypeIdentifier::String8Small { bound: 64 };
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

/// String8 ↔ String16 (UTF-8 vs UTF-16) are spec-foreign, no match.
#[test]
fn string8_vs_string16_mismatch() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::String8Small { bound: 32 };
    let r = TypeIdentifier::String16Small { bound: 32 };
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

// ---------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------

/// sequence<int32, 100> ↔ sequence<int32, 10>: bounds ignored by
/// default, accepted.
#[test]
fn sequence_bounds_ignored_by_default_tce() {
    use crate::type_identifier::PlainCollectionHeader;
    use alloc::boxed::Box;
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::PlainSequenceSmall {
        header: PlainCollectionHeader::default(),
        bound: 100,
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    let r = TypeIdentifier::PlainSequenceSmall {
        header: PlainCollectionHeader::default(),
        bound: 10,
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    assert!(m.match_types(&w, &r, &reg()).is_match());
}

/// Array bounds are structural, not policy-filtered.
#[test]
fn array_different_bounds_mismatch() {
    use crate::type_identifier::PlainCollectionHeader;
    use alloc::boxed::Box;
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::PlainArraySmall {
        header: PlainCollectionHeader::default(),
        array_bounds: alloc::vec![10],
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    let r = TypeIdentifier::PlainArraySmall {
        header: PlainCollectionHeader::default(),
        array_bounds: alloc::vec![20],
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

// ---------------------------------------------------------------------
// TCE-Kombinationen
// ---------------------------------------------------------------------

/// DisallowTypeCoercion + default flags: widening blocked.
#[test]
fn disallow_coercion_blocks_widening() {
    let mut tce = default_tce();
    tce.kind = TypeConsistencyKind::DisallowTypeCoercion;
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

/// AllowTypeCoercion + PreventTypeWidening: blockiert.
#[test]
fn prevent_widening_overrides_allow_coercion() {
    let mut tce = default_tce();
    tce.prevent_type_widening = true;
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

/// ForceTypeValidation + AllowTypeCoercion: ForceTypeValidation
/// uebertrumpft (§7.6.3.7.1).
#[test]
fn force_validation_beats_allow_coercion() {
    let mut tce = default_tce();
    tce.force_type_validation = true;
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::Int16);
    let r = TypeIdentifier::Primitive(PrimitiveKind::Int32);
    assert!(!m.match_types(&w, &r, &reg()).is_match());
}

// ---------------------------------------------------------------------
// Cross-Encoding Small ↔ Large (Round-2 Review #2)
// ---------------------------------------------------------------------

/// SequenceSmall with bound=50 ↔ SequenceLarge with bound=50 are semantically
/// equal — only the wire encoding differs.
#[test]
fn sequence_small_large_cross_encoding_matches() {
    use crate::type_identifier::PlainCollectionHeader;
    use alloc::boxed::Box;
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::PlainSequenceSmall {
        header: PlainCollectionHeader::default(),
        bound: 50,
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    let r = TypeIdentifier::PlainSequenceLarge {
        header: PlainCollectionHeader::default(),
        bound: 50,
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    assert!(m.match_types(&w, &r, &reg()).is_match());
    assert!(m.match_types(&r, &w, &reg()).is_match());
}

/// ArraySmall[3] ↔ ArrayLarge[3] match over the normalized bounds.
#[test]
fn array_small_large_cross_encoding_matches() {
    use crate::type_identifier::PlainCollectionHeader;
    use alloc::boxed::Box;
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::PlainArraySmall {
        header: PlainCollectionHeader::default(),
        array_bounds: alloc::vec![3u8, 4u8],
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    let r = TypeIdentifier::PlainArrayLarge {
        header: PlainCollectionHeader::default(),
        array_bounds: alloc::vec![3u32, 4u32],
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
    };
    assert!(m.match_types(&w, &r, &reg()).is_match());
}

/// MapSmall(bound=8) ↔ MapLarge(bound=8) with the same key/value.
#[test]
fn map_small_large_cross_encoding_matches() {
    use crate::type_identifier::{CollectionElementFlag, PlainCollectionHeader};
    use alloc::boxed::Box;
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let hdr = PlainCollectionHeader::default();
    let w = TypeIdentifier::PlainMapSmall {
        header: hdr,
        bound: 8,
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        key_flags: CollectionElementFlag(0),
        key: Box::new(TypeIdentifier::String8Small { bound: 32 }),
    };
    let r = TypeIdentifier::PlainMapLarge {
        header: hdr,
        bound: 8,
        element: Box::new(TypeIdentifier::Primitive(PrimitiveKind::Int32)),
        key_flags: CollectionElementFlag(0),
        key: Box::new(TypeIdentifier::String8Small { bound: 32 }),
    };
    assert!(m.match_types(&w, &r, &reg()).is_match());
}

// ---------------------------------------------------------------------
// Unsigned widening (Round-2 Review #4)
// ---------------------------------------------------------------------

/// UInt8 → UInt32 accepted with default TCE (lossless).
#[test]
fn uint8_to_uint32_widening_matches() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::UInt8);
    let r = TypeIdentifier::Primitive(PrimitiveKind::UInt32);
    assert!(m.match_types(&w, &r, &reg()).is_match());
}

/// UInt16 → UInt64 widening.
#[test]
fn uint16_to_uint64_widening_matches() {
    let tce = default_tce();
    let m = TypeMatcher::new(&tce);
    let w = TypeIdentifier::Primitive(PrimitiveKind::UInt16);
    let r = TypeIdentifier::Primitive(PrimitiveKind::UInt64);
    assert!(m.match_types(&w, &r, &reg()).is_match());
}
