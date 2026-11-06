// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-proto` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/lib.rs`, `src/mapper.rs`,
//! `src/registry.rs`), all from `dynamic`:
//! - `DynamicTypeBuilder::{new, structure, primitive, string, sequence, map,
//!   union, add_case, enumeration, add_enumerator, add_member, finish,
//!   finish_as}` and `NewMember::new(..).with_id(..)`: a protobuf message is
//!   a structure whose member ids are the field numbers, `string` an
//!   unbounded `string`, `bytes` an unbounded `sequence<octet>`, `repeated` an
//!   unbounded `sequence`, a map entry a `map<K, V>`, a `oneof` a union over
//!   an `int32` discriminator whose case labels and member ids are the field
//!   numbers, an enum an enumeration with `@bit_bound(32)`, explicit values
//!   and the value-0 literal as default literal; the message's extensibility
//!   goes to the structure and to its unions.
//! - `TypeKind` (the primitive kinds of a scalar field), `ExtensibilityKind`,
//!   `DynamicType`, and `DynamicError`, whose `Display` text becomes
//!   `MapError::Build` (a duplicate member id among them).
//!
//! Oracle: DDS-XTypes 1.3 §7.5.2 (the DynamicType and DynamicTypeBuilder
//! interfaces: §7.5.2.1 member `id`, `label`; §7.5.2.8 `kind`, members),
//! §7.2.2.4.4.4.4 (member ids unique within a type), §7.2.2.2.1.2
//! (`BYTE_TYPE`), §7.3.1.2.1.10 (`@default_literal`) with Annex B
//! (`EnumeratedLiteralFlag` `IS_DEFAULT` at position 6), §7.3.1.2.1.8
//! (extensibility).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::dynamic::{
    DynamicType, DynamicTypeBuilder, ExtensibilityKind, NewMember, TypeKind,
};
use zerodds_types::type_object::CompleteTypeObject;
use zerodds_types::type_object::flags::EnumLiteralFlag;

fn ids(ty: &DynamicType) -> Vec<(String, u32)> {
    ty.members()
        .iter()
        .map(|m| (m.name().to_string(), m.id()))
        .collect()
}

fn member_kind(ty: &DynamicType, name: &str) -> TypeKind {
    ty.member_by_name(name)
        .unwrap()
        .dynamic_type()
        .unwrap()
        .kind()
}

#[test]
fn xtypes_7_5_2_1_a_message_is_a_structure_whose_member_ids_are_the_field_numbers() {
    for extensibility in [ExtensibilityKind::Appendable, ExtensibilityKind::Mutable] {
        let mut b = DynamicTypeBuilder::new();
        let message = b.structure("Sensor", extensibility).unwrap();
        let label = b.string(false, None).unwrap();
        let value = b.primitive(TypeKind::Int32).unwrap();
        let double = b.primitive(TypeKind::Float64).unwrap();
        let samples = b.sequence(double, None).unwrap();
        let octet = b.primitive(TypeKind::Byte).unwrap();
        let blob = b.sequence(octet, None).unwrap();
        for (name, type_, number) in [
            ("label", label, 1),
            ("value", value, 2),
            ("samples", samples, 5),
            ("blob", blob, 7),
        ] {
            b.add_member(message, &NewMember::new(name, type_).with_id(number))
                .unwrap();
        }
        let ty = b.finish_as(message).unwrap();
        assert_eq!(ty.kind(), TypeKind::Structure);
        assert_eq!(ty.extensibility(), Some(extensibility));
        assert_eq!(
            ids(&ty),
            [
                ("label".to_string(), 1),
                ("value".to_string(), 2),
                ("samples".to_string(), 5),
                ("blob".to_string(), 7),
            ]
        );
        assert_eq!(member_kind(&ty, "label"), TypeKind::String8);
        assert_eq!(member_kind(&ty, "value"), TypeKind::Int32);
        let samples = ty
            .member_by_name("samples")
            .unwrap()
            .dynamic_type()
            .unwrap();
        assert_eq!(samples.kind(), TypeKind::Sequence);
        assert_eq!(samples.element_type().unwrap().kind(), TypeKind::Float64);
        let blob = ty.member_by_name("blob").unwrap().dynamic_type().unwrap();
        assert_eq!(blob.kind(), TypeKind::Sequence);
        assert_eq!(blob.element_type().unwrap().kind(), TypeKind::Byte);
    }
}

#[test]
fn xtypes_7_2_2_4_4_4_4_a_second_member_with_the_same_id_is_refused() {
    let mut b = DynamicTypeBuilder::new();
    let message = b.structure("Twice", ExtensibilityKind::Appendable).unwrap();
    let int32 = b.primitive(TypeKind::Int32).unwrap();
    b.add_member(message, &NewMember::new("a", int32).with_id(3))
        .unwrap();
    let error = b
        .add_member(message, &NewMember::new("b", int32).with_id(3))
        .unwrap_err();
    // The crate carries the `Display` text; it is not empty.
    assert!(!format!("{error}").is_empty());
}

#[test]
fn xtypes_7_5_2_the_scalar_kinds_are_primitives_and_a_non_primitive_kind_is_refused() {
    let mut b = DynamicTypeBuilder::new();
    for kind in [
        TypeKind::Float64,
        TypeKind::Float32,
        TypeKind::Int64,
        TypeKind::UInt64,
        TypeKind::Int32,
        TypeKind::UInt32,
        TypeKind::Boolean,
        TypeKind::Byte,
    ] {
        assert!(b.primitive(kind).is_ok(), "{kind:?}");
    }
    assert!(b.primitive(TypeKind::Structure).is_err());
    assert!(b.primitive(TypeKind::String8).is_err());
}

#[test]
fn xtypes_7_3_1_2_1_10_an_enum_keeps_its_values_and_its_zero_literal_is_the_default() {
    let mut b = DynamicTypeBuilder::new();
    let color = b.enumeration("Color", Some(32)).unwrap();
    for (name, value) in [("RED", 0), ("GREEN", 5), ("BLUE", 9)] {
        b.add_enumerator(color, name, Some(value), value == 0)
            .unwrap();
    }
    let ty = b.finish_as(color).unwrap();
    assert_eq!(ty.kind(), TypeKind::Enumeration);
    assert_eq!(ty.bit_bound(), Some(32));
    let values: Vec<(String, Option<u32>)> = ty
        .members()
        .iter()
        .map(|m| (m.name().to_string(), m.literal_value()))
        .collect();
    assert_eq!(
        values,
        [
            ("RED".to_string(), Some(0)),
            ("GREEN".to_string(), Some(5)),
            ("BLUE".to_string(), Some(9)),
        ]
    );
    // What reaches the wire: the default literal flag on RED only.
    let CompleteTypeObject::Enumerated(object) =
        zerodds_types::from_ast::complete_type_object(ty.ast(), ty.id()).unwrap()
    else {
        panic!("an enumeration");
    };
    let defaults: Vec<bool> = object
        .literal_seq
        .iter()
        .map(|l| l.common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0)
        .collect();
    assert_eq!(defaults, [true, false, false]);
}

#[test]
fn xtypes_7_5_2_1_a_oneof_is_a_union_labelled_and_numbered_by_its_field_numbers() {
    let mut b = DynamicTypeBuilder::new();
    let message = b.structure("Shape", ExtensibilityKind::Mutable).unwrap();
    let disc = b.primitive(TypeKind::Int32).unwrap();
    let union = b
        .union("Shape.kind", disc, ExtensibilityKind::Mutable)
        .unwrap();
    let double = b.primitive(TypeKind::Float64).unwrap();
    let text = b.string(false, None).unwrap();
    b.add_case(
        union,
        &[4],
        false,
        &NewMember::new("radius", double).with_id(4),
    )
    .unwrap();
    b.add_case(union, &[6], false, &NewMember::new("name", text).with_id(6))
        .unwrap();
    b.add_member(message, &NewMember::new("kind", union).with_id(4))
        .unwrap();
    let ty = b.finish_as(message).unwrap();
    let union = ty.member_by_name("kind").unwrap().dynamic_type().unwrap();
    assert_eq!(union.kind(), TypeKind::Union);
    assert_eq!(union.extensibility(), Some(ExtensibilityKind::Mutable));
    assert_eq!(union.discriminator_type().unwrap().kind(), TypeKind::Int32);
    assert_eq!(
        ids(&union),
        [("radius".to_string(), 4), ("name".to_string(), 6)]
    );
    let labels: Vec<Vec<i64>> = union
        .members()
        .iter()
        .map(|m| m.descriptor().unwrap().label)
        .collect();
    assert_eq!(labels, [vec![4], vec![6]]);
}

#[test]
fn xtypes_7_5_2_a_map_entry_is_an_unbounded_map() {
    let mut b = DynamicTypeBuilder::new();
    let key = b.string(false, None).unwrap();
    let value = b.primitive(TypeKind::Int32).unwrap();
    let map = b.map(key, value, None).unwrap();
    let message = b.structure("Tally", ExtensibilityKind::Appendable).unwrap();
    b.add_member(message, &NewMember::new("counts", map).with_id(1))
        .unwrap();
    let ty = b.finish_as(message).unwrap();
    let map = ty.member_by_name("counts").unwrap().dynamic_type().unwrap();
    assert_eq!(map.kind(), TypeKind::Map);
    assert_eq!(map.key_element_type().unwrap().kind(), TypeKind::String8);
    assert_eq!(map.element_type().unwrap().kind(), TypeKind::Int32);
}
