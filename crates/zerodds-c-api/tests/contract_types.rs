// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-c-api` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`:
//! - `TypeIdentifier::None` and `TypeIdentifier::default()` for an
//!   untyped writer or reader, `EquivalenceHashComplete`/`Minimal` built
//!   from a registered type object's hash (`src/lib.rs`,
//!   `src/publisher_ffi.rs`, `src/subscriber_ffi.rs`).
//! - `type_object::TypeObject::from_bytes_le` on the serialized type
//!   object a binding hands over, `TypeObject::Complete`/`Minimal`,
//!   `CompleteTypeObject::Struct`, `MinimalTypeObject::Struct`,
//!   `complete::CompleteStructType` (`member_seq[..].common.member_flags`,
//!   `header.base_type`) and `flags::StructMemberFlag::IS_KEY` to decide
//!   whether a type has a key (`src/lib.rs`).
//! - `from_ast::complete_type_object`, `from_ast::register_all`,
//!   `TypeObject::to_bytes_le`, `hash::compute_hash`,
//!   `resolve::{TypeRegistry, collect_referenced_hashes,
//!   DEFAULT_MAX_RESOLVE_DEPTH, DEFAULT_MAX_RESOLVE_NODES}` to turn IDL
//!   text into a type object and its dependency closure
//!   (`src/dyntype_ffi.rs`).
//! - `qos::{TypeConsistencyEnforcement, TypeConsistencyKind::from_u32,
//!   to_u32}` (`src/qos_ffi.rs`) and `qos::ExtensibilityForRepr`
//!   (`src/lib.rs`, `src/xcdr2.rs`, `src/publisher_ffi.rs`).
//! - `dynamic::DynamicSample` as the sample type of a multitopic reader
//!   (`src/multitopic_ffi.rs`).
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.2 Table 29 (`TK_NONE` 0x00, `EK_MINIMAL`
//! 0xF1, `EK_COMPLETE` 0xF2), §7.3.4.6.2 (a hash identifier is the first 14
//! octets of the MD5 of the XCDR2 little-endian serialized type object),
//! Annex B (`IS_KEY` is bit 5 of a `MemberFlag`), §7.6.3.2.1 (the
//! dependencies of a type are the types its members name), §7.6.3.4.1 and
//! Annex D (`TypeConsistencyKind` 0 and 1, the policy's defaults); and
//! the type objects CycloneDDS 11.0.1 `idlc` serializes for `to::Plain`
//! and `to::Inner`, recorded in `crates/types/tests/typeobject_vendor_dheader.rs`
//! (row set `xtypes-typeobject`). The hashes below are the MD5 prefixes of
//! those octets, computed with Python's `hashlib`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_types::from_ast::{complete_type_object, register_all};
use zerodds_types::hash::compute_hash;
use zerodds_types::qos::{ExtensibilityForRepr, TypeConsistencyEnforcement, TypeConsistencyKind};
use zerodds_types::resolve::{
    DEFAULT_MAX_RESOLVE_DEPTH, DEFAULT_MAX_RESOLVE_NODES, TypeRegistry, collect_referenced_hashes,
};
use zerodds_types::type_object::flags::StructMemberFlag;
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// Cyclone's COMPLETE `TypeObject` of `@final struct to::Plain { long a;
/// short b; }`.
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
/// Cyclone's MINIMAL `TypeObject` of the same type.
const PLAIN_MINIMAL: &str = "33000000f1510100010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe";
/// Cyclone's COMPLETE `TypeObject` of `@final struct to::Inner { long x;
/// long y; }`.
const INNER_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";

const PLAIN_COMPLETE_HASH: [u8; 14] = [
    0x7c, 0x0c, 0x6f, 0xab, 0x7d, 0x06, 0x6a, 0x0c, 0x56, 0xb8, 0xe9, 0x3c, 0x0e, 0xa7,
];
const PLAIN_MINIMAL_HASH: [u8; 14] = [
    0xde, 0xb9, 0x22, 0xf0, 0x40, 0xf7, 0xc2, 0x28, 0xb2, 0x57, 0x28, 0xbe, 0x51, 0xf6,
];
const INNER_COMPLETE_HASH: [u8; 14] = [
    0xf0, 0x03, 0x88, 0xe0, 0xe0, 0xc6, 0x61, 0x9e, 0x41, 0x75, 0xc6, 0x8e, 0x8a, 0xf8,
];

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// IDL text to a resolved model, the steps `src/dyntype_ffi.rs` takes.
fn model(idl: &str) -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "the IDL parses");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "contract.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().unwrap();
    ast
}

fn type_named(ast: &zerodds_ast::Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type `{name}`"))
}

#[test]
fn xtypes_7_3_4_2_the_default_type_identifier_is_tk_none() {
    assert_eq!(TypeIdentifier::default(), TypeIdentifier::None);
    assert_eq!(TypeIdentifier::None.to_bytes_le().unwrap(), [0x00]);
}

#[test]
fn xtypes_7_3_4_2_hash_identifiers_are_ek_complete_or_ek_minimal_and_fourteen_octets() {
    let hash = EquivalenceHash([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    let mut complete = vec![0xf2];
    complete.extend_from_slice(&hash.0);
    assert_eq!(
        TypeIdentifier::EquivalenceHashComplete(hash)
            .to_bytes_le()
            .unwrap(),
        complete
    );
    let mut minimal = vec![0xf1];
    minimal.extend_from_slice(&hash.0);
    assert_eq!(
        TypeIdentifier::EquivalenceHashMinimal(hash)
            .to_bytes_le()
            .unwrap(),
        minimal
    );
}

#[test]
fn xtypes_7_3_4_6_2_cyclones_type_objects_decode_and_hash_to_their_md5_prefix() {
    let octets = unhex(PLAIN_COMPLETE);
    let object = TypeObject::from_bytes_le(&octets).unwrap();
    let TypeObject::Complete(CompleteTypeObject::Struct(st)) = &object else {
        panic!("a complete struct: {object:?}");
    };
    assert_eq!(st.header.base_type, TypeIdentifier::None);
    assert_eq!(st.header.detail.type_name, "to::Plain");
    let names: Vec<&str> = st
        .member_seq
        .iter()
        .map(|m| m.detail.name.as_str())
        .collect();
    assert_eq!(names, ["a", "b"]);
    assert!(
        st.member_seq
            .iter()
            .all(|m| m.common.member_flags.0 & StructMemberFlag::IS_KEY == 0)
    );
    assert_eq!(compute_hash(&object).unwrap().0, PLAIN_COMPLETE_HASH);
    assert_eq!(object.to_bytes_le().unwrap(), octets, "re-encoded as read");

    let octets = unhex(PLAIN_MINIMAL);
    let object = TypeObject::from_bytes_le(&octets).unwrap();
    let TypeObject::Minimal(MinimalTypeObject::Struct(st)) = &object else {
        panic!("a minimal struct: {object:?}");
    };
    assert_eq!(st.member_seq.len(), 2);
    assert_eq!(compute_hash(&object).unwrap().0, PLAIN_MINIMAL_HASH);
}

#[test]
fn xtypes_annex_b_is_key_is_bit_five_and_marks_the_key_member() {
    assert_eq!(StructMemberFlag::IS_KEY, 1 << 5);
    let ast = model("@final struct K { @key long id; long v; };");
    let complete = complete_type_object(&ast, type_named(&ast, "K")).unwrap();
    let CompleteTypeObject::Struct(st) = complete else {
        panic!("a struct");
    };
    let key_bits: Vec<(&str, u16)> = st
        .member_seq
        .iter()
        .map(|m| {
            (
                m.detail.name.as_str(),
                m.common.member_flags.0 & StructMemberFlag::IS_KEY,
            )
        })
        .collect();
    assert_eq!(key_bits, [("id", StructMemberFlag::IS_KEY), ("v", 0)]);
}

#[test]
fn xtypes_7_6_3_2_1_the_dependency_of_a_nested_struct_is_its_member_types_hash() {
    let ast = model(
        "module to { @final struct Inner { long x; long y; }; @final struct Outer { Inner i; }; };",
    );
    let outer = complete_type_object(&ast, type_named(&ast, "Outer")).unwrap();
    let CompleteTypeObject::Struct(st) = &outer else {
        panic!("a struct");
    };
    assert_eq!(
        st.member_seq[0].common.member_type_id,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(INNER_COMPLETE_HASH))
    );

    let root = TypeObject::Complete(outer.clone());
    let hashes = collect_referenced_hashes(&root, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
    assert_eq!(hashes, [EquivalenceHash(INNER_COMPLETE_HASH)]);
    assert!(hashes.len() <= DEFAULT_MAX_RESOLVE_NODES);

    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).unwrap();
    let inner = registry
        .get_complete(&EquivalenceHash(INNER_COMPLETE_HASH))
        .expect("the dependency is registered under its hash");
    assert_eq!(
        TypeObject::Complete(inner.clone()).to_bytes_le().unwrap(),
        unhex(INNER_COMPLETE),
        "Cyclone's octets for to::Inner"
    );
}

#[test]
fn xtypes_7_6_3_4_1_type_consistency_kinds_and_the_policy_default() {
    assert_eq!(
        TypeConsistencyKind::from_u32(0),
        Ok(TypeConsistencyKind::DisallowTypeCoercion)
    );
    assert_eq!(
        TypeConsistencyKind::from_u32(1),
        Ok(TypeConsistencyKind::AllowTypeCoercion)
    );
    assert!(TypeConsistencyKind::from_u32(2).is_err());
    assert_eq!(TypeConsistencyKind::AllowTypeCoercion.to_u32(), 1);

    assert_eq!(
        TypeConsistencyEnforcement::default(),
        TypeConsistencyEnforcement {
            kind: TypeConsistencyKind::AllowTypeCoercion,
            ignore_sequence_bounds: true,
            ignore_string_bounds: true,
            ignore_member_names: false,
            prevent_type_widening: false,
            force_type_validation: false,
        }
    );
}

#[test]
fn the_three_extensibility_kinds_the_c_api_maps_are_distinct_values() {
    fn plain<T: Copy + Eq + core::fmt::Debug>() {}
    plain::<ExtensibilityForRepr>();
    let kinds = [
        ExtensibilityForRepr::Final,
        ExtensibilityForRepr::Appendable,
        ExtensibilityForRepr::Mutable,
    ];
    assert_ne!(kinds[0], kinds[1]);
    assert_ne!(kinds[1], kinds[2]);
    assert_ne!(kinds[0], kinds[2]);
}

#[test]
fn a_dynamic_sample_is_a_dds_sample_type() {
    fn sample_type<T: zerodds_dcps::DdsType>() {}
    sample_type::<zerodds_types::dynamic::DynamicSample>();
}
