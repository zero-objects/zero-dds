// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-webdds-bridge` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`:
//! - `src/types_registry.rs`: from IDL, `dynamic::DynamicType::find`,
//!   `DynamicType::id` and `from_ast::complete_type_object` for the root and
//!   for every other type of the model; from a peer,
//!   `TypeIdentifier::EquivalenceHashComplete`, `resolve::TypeRegistry::
//!   {get_complete, transitive_dependencies, generation}` with
//!   `DEFAULT_MAX_RESOLVE_NODES`; then for both
//!   `TypeObject::Complete(..).to_bytes_le`, `TypeRegistry::{new,
//!   insert_object}`, `dynamic::ModelFromTypeObjects::{new,
//!   add_type_object, finish}`, `DynamicType::new`, `DynamicType::
//!   extensibility` (`ExtensibilityKind::{Final, Appendable, Mutable}`, `None`
//!   refused) and `DynamicType::name`. The wait for a TypeLookup reply polls
//!   `TypeRegistry::generation` for a change.
//! - `src/rpc_dispatch.rs`: `builder::TypeObjectBuilder::struct_type` and
//!   `union_type` with `Extensibility::Final`, `member`, `case`,
//!   `default_case`, `build_complete`; `from_ast::{type_identifier(..,
//!   Form::Complete), complete_type_object, dependency_type_objects}`, a
//!   `FromAstError::Unrepresentable` from `complete_type_object` meaning a
//!   fully descriptive type with no object.
//! - `src/dynxml.rs`, `src/gateway.rs`, `src/dynamic_rpc.rs`:
//!   `DynamicType` and `DynamicData` as the reflective view and the value
//!   (covered with the codec in `contract_wire_codec.rs`).
//!
//! This crate declares `zerodds-types` with `default-features = false,
//! features = ["alloc"]` and uses `dynamic`, which is behind the `dynamic`
//! feature; the feature reaches the build through the other dependencies.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B, the COMPLETE objects and
//! hashes CycloneDDS 11.0.1 and RTI Connext 7.7.0 give for `to::Inner`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`), and the COMPLETE
//! objects of `@final struct to::Outer { to::Inner i; }` and `@final struct
//! Calc_add_In { long a; long b; }` written out by hand from Annex B (MD5
//! computed outside this tree); §7.3.4.1 and §7.3.4.6.1 (a plain type has a
//! fully descriptive identifier and no TypeObject); §7.2.2.4.4.4.4 and the
//! sequential `@autoid` (member ids from 0); DDS-RPC 1.0 §7.5.1.1.6 (`Call`
//! is a union over a `long`, its default case an `octet`).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::dynamic::{DynamicType, ExtensibilityKind, ModelFromTypeObjects};
use zerodds_types::from_ast::{
    Form, FromAstError, complete_type_object, dependency_type_objects, type_identifier,
};
use zerodds_types::resolve::{DEFAULT_MAX_RESOLVE_NODES, TypeRegistry};
use zerodds_types::type_object::flags::{StructTypeFlag, UnionMemberFlag};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, PrimitiveKind, TypeIdentifier};

const INNER: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";
const INNER_HASH: [u8; 14] = [
    0xf0, 0x03, 0x88, 0xe0, 0xe0, 0xc6, 0x61, 0x9e, 0x41, 0x75, 0xc6, 0x8e, 0x8a, 0xf8,
];
const OUTER: &str = "48000000f251010012000000000000000a000000746f3a3a4f75746572000000280000000100000020000000000000000100f2f00388e0e0c6619e4175c68e8af80000000200000069000000";
const OUTER_HASH: [u8; 14] = [
    0x91, 0x23, 0x71, 0x8c, 0xb7, 0x2c, 0x3e, 0x15, 0xbd, 0xec, 0x18, 0x80, 0x34, 0xf7,
];
/// `@final struct Calc_add_In { long a; long b; }`, COMPLETE: DHEADER 76,
/// `EK_COMPLETE`, `TK_STRUCTURE`, `IS_FINAL`, the name, members 0 and 1
/// with `TRY_CONSTRUCT1` and `TK_INT32`.
const CALC_ADD_IN: &str = "4c000000f251010014000000000000000c00000043616c635f6164645f496e002c0000000200000010000000000000000100040002000000610000001000000001000000010004000200000062000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn complete(hex: &str) -> CompleteTypeObject {
    let TypeObject::Complete(object) = TypeObject::from_bytes_le(&unhex(hex)).unwrap() else {
        panic!("a COMPLETE object");
    };
    object
}

#[test]
fn xtypes_7_3_4_3_a_peers_objects_become_one_model_and_a_type_view_on_it() {
    let mut registry = TypeRegistry::new();
    let generation = registry.generation();
    let inner = registry
        .insert_object(TypeObject::Complete(complete(INNER)))
        .unwrap();
    // The wait for a TypeLookup reply polls for exactly this change.
    assert_ne!(registry.generation(), generation);
    let outer = registry
        .insert_object(TypeObject::Complete(complete(OUTER)))
        .unwrap();
    assert_eq!(inner, EquivalenceHash(INNER_HASH));
    assert_eq!(outer, EquivalenceHash(OUTER_HASH));

    // `resolve_discovered`: the root and its closure out of the registry.
    let root = registry.get_complete(&outer).cloned().unwrap();
    let closure = registry.transitive_dependencies(&outer, DEFAULT_MAX_RESOLVE_NODES);
    assert_eq!(closure, [inner]);
    assert!(registry.get_complete(&closure[0]).is_some());

    // `resolve_from_complete`.
    let mut model = ModelFromTypeObjects::new(&registry);
    let id = model
        .add_type_object(&TypeObject::Complete(root.clone()))
        .unwrap();
    let ty = DynamicType::new(model.finish(), id);
    assert_eq!(ty.name(), "Outer");
    assert_eq!(ty.qualified_name(), "to::Outer");
    assert_eq!(ty.extensibility(), Some(ExtensibilityKind::Final));
    // The octets handed to the serialized type support are the object's
    // Annex B octets.
    assert_eq!(
        TypeObject::Complete(root).to_bytes_le().unwrap(),
        unhex(OUTER)
    );

    // A MINIMAL object carries no names and builds no view.
    let mut model = ModelFromTypeObjects::new(&registry);
    let minimal = TypeObject::from_bytes_le(&unhex(
        "33000000f1510100010000000000000023000000020000000b000000000000000100049dd4e461000b0000000100000001000441529076",
    ))
    .unwrap();
    assert!(model.add_type_object(&minimal).is_err());
}

#[test]
fn xtypes_7_3_4_6_1_the_rpc_parameter_types_have_identifiers_and_the_aggregates_objects() {
    let ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl(
            "module to {
               @final struct Inner { long x; long y; };
               @final struct Outer { Inner i; };
             };",
            "contract.idl",
        )
        .unwrap(),
    );
    let outer = DynamicType::find(&ast, "to::Outer").unwrap();
    let inner = DynamicType::find(&ast, "to::Inner").unwrap();
    let long = inner
        .member_by_name("x")
        .unwrap()
        .dynamic_type()
        .unwrap()
        .id();

    assert_eq!(
        type_identifier(&ast, inner.id(), Form::Complete).unwrap(),
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(INNER_HASH))
    );
    assert_eq!(
        type_identifier(&ast, outer.id(), Form::Complete).unwrap(),
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(OUTER_HASH))
    );
    assert_eq!(
        complete_type_object(&ast, outer.id()).unwrap(),
        complete(OUTER)
    );
    assert_eq!(
        dependency_type_objects(&ast, outer.id()).unwrap(),
        [complete(INNER)]
    );

    // A `long` parameter: an identifier that says what it is, no object.
    assert_eq!(
        type_identifier(&ast, long, Form::Complete).unwrap(),
        TypeIdentifier::Primitive(PrimitiveKind::Int32)
    );
    assert!(matches!(
        complete_type_object(&ast, long),
        Err(FromAstError::Unrepresentable { .. })
    ));
    assert!(dependency_type_objects(&ast, long).unwrap().is_empty());
}

#[test]
fn xtypes_7_3_4_5_the_builder_writes_a_final_in_struct_with_sequential_ids() {
    let built = TypeObjectBuilder::struct_type("Calc_add_In")
        .extensibility(Extensibility::Final)
        .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |mb| {
            mb
        })
        .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int32), |mb| {
            mb
        })
        .build_complete();
    assert_eq!(
        TypeObject::Complete(CompleteTypeObject::Struct(built))
            .to_bytes_le()
            .unwrap(),
        unhex(CALC_ADD_IN)
    );
}

#[test]
fn rpc_7_5_1_1_6_the_builder_writes_a_final_call_union_over_a_long() {
    let add = TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(INNER_HASH));
    let union =
        TypeObjectBuilder::union_type("Calc_Call", TypeIdentifier::Primitive(PrimitiveKind::Int32))
            .extensibility(Extensibility::Final)
            .case("add", add.clone(), vec![1])
            .default_case("unknownOp", TypeIdentifier::Primitive(PrimitiveKind::Byte))
            .build_complete();
    assert_eq!(union.header.detail.type_name, "Calc_Call");
    assert_eq!(
        union.union_flags.0,
        StructTypeFlag::IS_FINAL,
        "IS_FINAL only"
    );
    assert_eq!(
        union.discriminator.common.type_id,
        TypeIdentifier::Primitive(PrimitiveKind::Int32)
    );
    let cases: Vec<(u32, &str, Vec<i32>, bool)> = union
        .member_seq
        .iter()
        .map(|m| {
            (
                m.common.member_id,
                m.detail.name.as_str(),
                m.common.label_seq.clone(),
                m.common.member_flags.0 & UnionMemberFlag::IS_DEFAULT != 0,
            )
        })
        .collect();
    assert_eq!(
        cases,
        [(0, "add", vec![1], false), (1, "unknownOp", vec![], true),]
    );
    assert_eq!(union.member_seq[0].common.type_id, add);
    assert_eq!(
        union.member_seq[1].common.type_id,
        TypeIdentifier::Primitive(PrimitiveKind::Byte)
    );
    // It is an object the registry can file.
    let mut registry = TypeRegistry::new();
    registry
        .insert_object(TypeObject::Complete(CompleteTypeObject::Union(union)))
        .unwrap();
}
