//! Property tests for XTypes assignability invariants.
//!
//! Spec OMG XTypes 1.3 §7.2.4 — semantic invariants:
//! 1. **Reflexivity:** `is_assignable(T, T) == Yes` for all T.
//! 2. **Anti-symmetry for primitives:** two distinct primitive types
//!    are **not** mutually assignable (except
//!    `is_assignable(T, T)`).
//! 3. **Consistency with roundtrip:** `T → bytes → T'` preserves
//!    `is_assignable(T, T') == Yes` (encode/decode roundtrip).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use proptest::prelude::*;
use zerodds_types::TypeIdentifier;
use zerodds_types::assignability::{AssignabilityConfig, Assignable, is_assignable};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::PrimitiveKind;

fn primitive_kinds() -> impl Strategy<Value = PrimitiveKind> {
    prop_oneof![
        Just(PrimitiveKind::Boolean),
        Just(PrimitiveKind::Byte),
        Just(PrimitiveKind::Char8),
        Just(PrimitiveKind::Char16),
        Just(PrimitiveKind::Int8),
        Just(PrimitiveKind::Int16),
        Just(PrimitiveKind::Int32),
        Just(PrimitiveKind::Int64),
        Just(PrimitiveKind::UInt8),
        Just(PrimitiveKind::UInt16),
        Just(PrimitiveKind::UInt32),
        Just(PrimitiveKind::UInt64),
        Just(PrimitiveKind::Float32),
        Just(PrimitiveKind::Float64),
        Just(PrimitiveKind::Float128),
    ]
}

/// The identifiers reflexivity is a property of.
///
/// `TypeIdentifier::None` is **not** among them. It is `TK_NONE`, which
/// DDS-XTypes 1.3 Table 29 calls an invalid type identifier rather than
/// a type: it is the absence of one, so "is it assignable to itself"
/// has no type to ask about. `is_assignable` says so by name, and
/// [`tk_none_is_not_a_type_identifier`] holds it to that — a variant
/// dropped from a generator with nothing put in its place would be a
/// case nobody measures.
fn arb_type_identifier() -> impl Strategy<Value = TypeIdentifier> {
    prop_oneof![
        primitive_kinds().prop_map(TypeIdentifier::Primitive),
        any::<u8>().prop_map(|b| TypeIdentifier::String8Small { bound: b }),
        any::<u32>().prop_map(|b| TypeIdentifier::String8Large {
            bound: b.min(1_000_000),
        }),
        any::<u8>().prop_map(|b| TypeIdentifier::String16Small { bound: b }),
    ]
}

/// `TK_NONE` is refused, and refused by name.
#[test]
fn tk_none_is_not_a_type_identifier() {
    let t = TypeIdentifier::None;
    let reg = TypeRegistry::new();
    let result = is_assignable(&t, &t, &reg, &AssignabilityConfig::default());
    match result {
        Assignable::No(why) => assert!(
            why.contains("TK_NONE") && why.contains("Table 29"),
            "the refusal names neither the identifier nor the clause: {why}"
        ),
        other => panic!("TK_NONE is not a type identifier; got {other:?}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Spec §7.2.4 — reflexivity: `is_assignable(T, T)` MUST be `Yes`
    /// for every TypeIdentifier variant.
    #[test]
    fn assignability_is_reflexive_for_primitives(p in primitive_kinds()) {
        let t = TypeIdentifier::Primitive(p);
        let reg = TypeRegistry::new();
        let result = is_assignable(&t, &t, &reg, &AssignabilityConfig::default());
        prop_assert!(
            matches!(result, Assignable::Yes),
            "primitive {p:?} must be assignable to itself, got {result:?}"
        );
    }

    /// Reflexivity for string types with the same bound.
    #[test]
    fn assignability_is_reflexive_for_string8_small(bound in any::<u8>()) {
        let t = TypeIdentifier::String8Small { bound };
        let reg = TypeRegistry::new();
        let result = is_assignable(&t, &t, &reg, &AssignabilityConfig::default());
        prop_assert!(
            matches!(result, Assignable::Yes),
            "String8Small({bound}) must be assignable to itself, got {result:?}"
        );
    }

    #[test]
    fn assignability_is_reflexive_for_arbitrary(t in arb_type_identifier()) {
        let reg = TypeRegistry::new();
        let result = is_assignable(&t, &t, &reg, &AssignabilityConfig::default());
        prop_assert!(
            matches!(result, Assignable::Yes),
            "{t:?} must be assignable to itself, got {result:?}"
        );
    }

    /// Spec §7.2.4 — distinct primitives are pairwise not assignable.
    /// (Boolean and Byte, for example, are different wire formats and
    /// MUST be rejected.)
    #[test]
    fn distinct_primitives_not_assignable(
        a in primitive_kinds(),
        b in primitive_kinds(),
    ) {
        if a == b {
            return Ok(());
        }
        let ta = TypeIdentifier::Primitive(a);
        let tb = TypeIdentifier::Primitive(b);
        let reg = TypeRegistry::new();
        let result = is_assignable(&ta, &tb, &reg, &AssignabilityConfig::default());
        prop_assert!(
            !matches!(result, Assignable::Yes),
            "distinct primitives {a:?} → {b:?} must not be assignable, got {result:?}"
        );
    }

    /// Wire-Roundtrip-Konsistenz: `T → bytes → T' → is_assignable(T, T')` MUSS Yes.
    #[test]
    fn roundtrip_preserves_assignability_for_primitive(p in primitive_kinds()) {
        let t = TypeIdentifier::Primitive(p);
        let bytes = t.to_bytes_le().unwrap();
        let decoded = TypeIdentifier::from_bytes_le(&bytes).unwrap();
        let reg = TypeRegistry::new();
        let result = is_assignable(&t, &decoded, &reg, &AssignabilityConfig::default());
        prop_assert!(
            matches!(result, Assignable::Yes),
            "T → bytes → T' must preserve assignability, got {result:?}"
        );
    }
}
