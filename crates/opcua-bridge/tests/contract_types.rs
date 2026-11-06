// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-bridge` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`, only with its `daemon` feature:
//! - `src/idl.rs`: `dynamic::DynamicType::find` on a composed model, `ast`,
//!   `id`, `extensibility` (Final/Appendable/Mutable → 0/1/2), and
//!   `from_ast::complete_type_object` written with
//!   `TypeObject::Complete(..).to_bytes_le()` for the type support.
//! - `src/dynamic_type.rs`: the reflective walk of DDS-XTypes 1.3 §7.5.2 —
//!   `kind`, `qualified_name`, `member_count` / `member_by_index`,
//!   `DynamicTypeMember::{name, literal_value, dynamic_type, descriptor}`
//!   (`is_key`, `is_optional`, `is_default_label`), `bit_bound`, `base_type`,
//!   `discriminator_type`, `element_type`, `key_element_type` and
//!   `descriptor().bound`, of which it takes `bound.first()` as a sequence's
//!   or a map's bound.
//! - `src/dynamic_value.rs`, `ua_client.rs`, `pubsub_glue.rs`: the
//!   `DynamicData` shapes per kind (`Boolean`, `Integer`, `Float`,
//!   `Character`, `Text`, `Enumerator` by name, `Bits` of `Text`,
//!   `Collection`, a map entry as the two-element `Collection [key,
//!   value]`, `Aggregate`).
//! - `src/dds_bridge.rs`: `TypeIdentifier::EquivalenceHashComplete`,
//!   `resolve::DEFAULT_MAX_RESOLVE_NODES` and the registry's `get_complete`
//!   / `transitive_dependencies`.
//!
//! Oracles: DDS-XTypes 1.3 §7.5.2.4 and Table 50 (`bound`: an array's
//! dimensions, one bound otherwise, `LENGTH_UNLIMITED` for an unbounded
//! collection), §7.5.2.4.1 (an alias's `base_type`), §7.5.2.7 (the member
//! descriptor), §7.3.1.2.1.10 and the `@value` / `@position` rules of
//! §7.3.1.2.1 (literal values), §7.3.4 and Annex B (the complete
//! TypeObject). The complete TypeObject of `@final struct Robot { unsigned
//! long id; unsigned long label; }` below was written by hand from Annex B;
//! its MD5, computed with Python's `hashlib`, gives the complete hash Cyclone
//! DDS 11.0.1 announces (`crates/types/tests/typeobject_serialized_size.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use zerodds_ast::{Ast, PrimitiveKind};
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::dynamic::{
    DynamicData, DynamicType, DynamicTypeMember, ExtensibilityKind, LENGTH_UNLIMITED, TypeKind,
};
use zerodds_types::from_ast::complete_type_object;
use zerodds_types::resolve::{DEFAULT_MAX_RESOLVE_NODES, TypeRegistry};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

fn model(idl: &str) -> Arc<Ast> {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        idl,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(
        !composed.failed(),
        "the fixture does not compose: {:?}",
        composed.diagnostics
    );
    Arc::new(composed.ast)
}

const ROBOT_COMPLETE_HASH: [u8; 14] = [
    0xaa, 0xd3, 0x35, 0x12, 0xa5, 0xe4, 0x28, 0x68, 0xff, 0x3e, 0x9e, 0x01, 0x68, 0x9a,
];

#[rustfmt::skip]
const ROBOT_COMPLETE: [u8; 84] = [
    0x50, 0x00, 0x00, 0x00, 0xf2, 0x51, 0x01, 0x00, // DHEADER 80, EK_COMPLETE, TK_STRUCTURE, IS_FINAL
    0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // header: DHEADER 14, TK_NONE, no annotations
    0x06, 0x00, 0x00, 0x00, b'R', b'o', b'b', b'o', b't', 0x00, 0x00, 0x00,
    0x34, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, // member_seq: DHEADER 52, two
    0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x03, 0x00, 0x00, 0x00, b'i', b'd', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x06, 0x00, 0x00, 0x00, b'l', b'a', b'b', b'e', b'l', 0x00, 0x00, 0x00,
];

#[test]
fn xtypes_7_3_4_the_type_support_bytes_of_an_idl_type_are_the_vendors_type_object() {
    let ast = model("@final struct Robot { unsigned long id; unsigned long label; };");
    let ty = DynamicType::find(&ast, "Robot").unwrap();
    assert_eq!(ty.extensibility(), Some(ExtensibilityKind::Final));
    let complete = complete_type_object(ty.ast(), ty.id()).unwrap();
    assert_eq!(
        TypeObject::Complete(complete).to_bytes_le().unwrap(),
        ROBOT_COMPLETE
    );
    assert!(DynamicType::find(&ast, "Nobody").is_err());
}

const WALKED: &str = "module M {
    enum Colour { RED, @value(5) GREEN };
    @bit_bound(16) bitmask Flags { @position(0) A, @position(9) B };
    typedef long Count;
    @final union Pick switch (long) { case 1: long a; default: string b; };
    @final struct S {
        @key long id;
        @optional short o;
        sequence<long> q;
        sequence<long, 5> qb;
        long arr[2][3];
        map<long, string> m;
        Count al;
        Colour c;
        Flags f;
        Pick u;
    };
};";

fn member(ty: &DynamicType, index: u32) -> DynamicTypeMember {
    ty.member_by_index(index).expect("the member is there")
}

fn member_type(ty: &DynamicType, index: u32) -> DynamicType {
    member(ty, index).dynamic_type().unwrap()
}

#[test]
fn xtypes_7_5_2_7_members_in_declaration_order_with_key_and_optional() {
    let ast = model(WALKED);
    let s = DynamicType::find(&ast, "M::S").unwrap();
    assert_eq!(s.kind(), TypeKind::Structure);
    assert_eq!(s.qualified_name(), "M::S");
    assert_eq!(s.member_count(), 10);
    let names: Vec<String> = (0..10).map(|i| member(&s, i).name().to_string()).collect();
    assert_eq!(
        names,
        ["id", "o", "q", "qb", "arr", "m", "al", "c", "f", "u"]
    );
    assert!(s.member_by_index(10).is_none());
    let id = member(&s, 0).descriptor().unwrap();
    assert!(id.is_key && !id.is_optional);
    assert_eq!(member_type(&s, 0).kind(), TypeKind::Int32);
    let o = member(&s, 1).descriptor().unwrap();
    assert!(o.is_optional && !o.is_key);
    assert_eq!(member_type(&s, 1).kind(), TypeKind::Int16);
}

#[test]
fn xtypes_7_5_2_4_an_unbounded_collection_has_the_one_bound_length_unlimited() {
    let ast = model(WALKED);
    let s = DynamicType::find(&ast, "M::S").unwrap();

    let q = member_type(&s, 2);
    assert_eq!(q.kind(), TypeKind::Sequence);
    assert_eq!(q.element_type().unwrap().kind(), TypeKind::Int32);
    assert_eq!(q.descriptor().unwrap().bound, [LENGTH_UNLIMITED]);
    const { assert!(LENGTH_UNLIMITED == u32::MAX) };

    let qb = member_type(&s, 3);
    assert_eq!(qb.descriptor().unwrap().bound, [5]);

    let arr = member_type(&s, 4);
    assert_eq!(arr.kind(), TypeKind::Array);
    assert_eq!(arr.descriptor().unwrap().bound, [2, 3], "outermost first");
    assert_eq!(arr.element_type().unwrap().kind(), TypeKind::Int32);

    let m = member_type(&s, 5);
    assert_eq!(m.kind(), TypeKind::Map);
    assert_eq!(m.key_element_type().unwrap().kind(), TypeKind::Int32);
    assert_eq!(m.element_type().unwrap().kind(), TypeKind::String8);
    assert_eq!(m.descriptor().unwrap().bound, [LENGTH_UNLIMITED]);
}

#[test]
fn xtypes_7_5_2_4_1_alias_enumeration_bitmask_and_union_as_the_walk_reads_them() {
    let ast = model(WALKED);
    let s = DynamicType::find(&ast, "M::S").unwrap();

    let al = member_type(&s, 6);
    assert_eq!(al.kind(), TypeKind::Alias);
    assert_eq!(al.base_type().unwrap().kind(), TypeKind::Int32);

    let colour = member_type(&s, 7);
    assert_eq!(colour.kind(), TypeKind::Enumeration);
    assert_eq!(colour.member_count(), 2);
    assert_eq!(
        (
            member(&colour, 0).name(),
            member(&colour, 0).literal_value()
        ),
        ("RED", Some(0))
    );
    assert_eq!(
        (
            member(&colour, 1).name(),
            member(&colour, 1).literal_value()
        ),
        ("GREEN", Some(5))
    );

    let flags = member_type(&s, 8);
    assert_eq!(flags.kind(), TypeKind::Bitmask);
    assert_eq!(flags.bit_bound(), Some(16));
    assert_eq!(member(&flags, 0).literal_value(), Some(0));
    assert_eq!(member(&flags, 1).literal_value(), Some(9));

    let u = member_type(&s, 9);
    assert_eq!(u.kind(), TypeKind::Union);
    assert_eq!(u.discriminator_type().unwrap().kind(), TypeKind::Int32);
    assert_eq!(u.member_count(), 2);
    assert!(!member(&u, 0).descriptor().unwrap().is_default_label);
    assert!(member(&u, 1).descriptor().unwrap().is_default_label);
    assert_eq!(member_type(&u, 1).kind(), TypeKind::String8);
}

#[test]
fn a_map_entry_and_a_bitmask_have_the_shapes_the_value_conversion_builds() {
    use zerodds_wire_codec::Sink;
    let entry = <DynamicData as Sink>::map_entry(
        DynamicData::Text(String::from("k")),
        DynamicData::integer(PrimitiveKind::Int32, 1),
    );
    assert_eq!(
        entry,
        DynamicData::Collection(vec![
            DynamicData::Text(String::from("k")),
            DynamicData::integer(PrimitiveKind::Int32, 1),
        ])
    );
    assert_eq!(
        <DynamicData as Sink>::bit_names(vec![String::from("A")]),
        DynamicData::Bits(vec![DynamicData::Text(String::from("A"))])
    );
    let c = DynamicData::character(PrimitiveKind::Char, 'x');
    assert_eq!(
        c,
        DynamicData::Character {
            kind: PrimitiveKind::Char,
            value: String::from("x"),
        }
    );
}

#[test]
fn a_discovered_complete_identifier_resolves_with_its_dependencies() {
    let mut registry = TypeRegistry::new();
    let robot = registry
        .insert_object(TypeObject::from_bytes_le(&ROBOT_COMPLETE).unwrap())
        .unwrap();
    assert_eq!(robot, EquivalenceHash(ROBOT_COMPLETE_HASH));
    let identifier = TypeIdentifier::EquivalenceHashComplete(robot);
    assert!(matches!(
        identifier,
        TypeIdentifier::EquivalenceHashComplete(hash) if hash == robot
    ));
    assert!(registry.get_complete(&robot).is_some());
    assert!(
        registry
            .transitive_dependencies(&robot, DEFAULT_MAX_RESOLVE_NODES)
            .is_empty()
    );

    // An aggregate naming Robot by its complete hash depends on it.
    let outer = TypeObjectBuilder::struct_type("Outer")
        .extensibility(Extensibility::Final)
        .member("r", identifier.clone(), |m| m)
        .build_complete();
    let outer = registry
        .insert_object(TypeObject::Complete(CompleteTypeObject::Struct(outer)))
        .unwrap();
    assert_eq!(
        registry.transitive_dependencies(&outer, DEFAULT_MAX_RESOLVE_NODES),
        vec![robot]
    );
    const { assert!(DEFAULT_MAX_RESOLVE_NODES > 0) };
}
