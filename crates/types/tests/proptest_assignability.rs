//! Property-Tests fuer XTypes-Assignability-Invarianten.
//!
//! Spec OMG XTypes 1.3 §7.2.4 — semantische Invarianten:
//! 1. **Reflexivität:** `is_assignable(T, T) == Yes` für alle T.
//! 2. **Anti-Symmetrie für Primitive:** zwei verschiedene primitive
//!    Types sind **nicht** wechselseitig assignable (außer
//!    `is_assignable(T, T)`).
//! 3. **Konsistenz mit Roundtrip:** `T → bytes → T'` erhält
//!    `is_assignable(T, T') == Yes` (encode/decode-Roundtrip).

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

fn arb_type_identifier() -> impl Strategy<Value = TypeIdentifier> {
    prop_oneof![
        primitive_kinds().prop_map(TypeIdentifier::Primitive),
        any::<u8>().prop_map(|b| TypeIdentifier::String8Small { bound: b }),
        any::<u32>().prop_map(|b| TypeIdentifier::String8Large {
            bound: b.min(1_000_000),
        }),
        any::<u8>().prop_map(|b| TypeIdentifier::String16Small { bound: b }),
        Just(TypeIdentifier::None),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Spec §7.2.4 — Reflexivität: `is_assignable(T, T)` MUSS `Yes`
    /// für jede TypeIdentifier-Variante sein.
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

    /// Reflexivität für String-Types mit gleichem bound.
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

    /// Spec §7.2.4 — distinct primitives sind paarweise nicht
    /// assignable. (Boolean und Byte z.B. sind unterschiedliche
    /// Wire-Formate und MUESSEN reject werden.)
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
