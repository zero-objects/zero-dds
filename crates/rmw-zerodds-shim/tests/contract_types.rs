// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `rmw-zerodds-shim` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (dependency features `alloc`,
//! `dynamic`):
//! - `src/dynamic.rs`, the ROS 2 dynamic type support: a ROS message type
//!   is built with `dynamic::DynamicTypeBuilder` (`new`, `module`,
//!   `structure_in` with `ExtensibilityKind::Final`, `structure`,
//!   `primitive`, `string`, `sequence`, `array`, `annotate`, `add_member`,
//!   `model`, `finish_as`). `graft` copies a member by filling every field
//!   of a `NewMember` struct literal from the source member's facet
//!   (`TryConstructKind::from`, `default_expr_text` for the default) and
//!   maps primitives with `TypeKind::of_primitive`. The finished
//!   `DynamicType` is read with `qualified_name`, `ast`, `id`, `kind`,
//!   `members`, `member_count`, `member_by_name`/`_index`/`_id`,
//!   `element_type`, `resolve_alias`, `equals`; a member with `id` and
//!   `dynamic_type`; `MEMBER_ID_INVALID` marks an absent id. Values are
//!   `DynamicData` built with `integer`, `float`, `character`, `text`,
//!   `boolean` and read with `members`, `items`, `as_char` and the
//!   variants; errors are `DynamicError::{Unresolved, Model,
//!   Unrepresentable}`.
//! - `src/services.rs`: `qos::ExtensibilityForRepr::Final` for the wire
//!   extensibility of a service writer.
//!
//! Oracle: DDS-XTypes 1.3 §7.5.2.5 (`MEMBER_ID_INVALID` = 0x0FFFFFFF),
//! §7.3.1.2.1.1 (without `@id` the members are numbered from 0 in
//! declaration order, `@id` sets one explicitly), §7.2.2.4.4.4.8 (a key
//! member is must-understand), §7.2.2.7 (`@try_construct`), §7.3.4.2
//! (`TK_BYTE` and `TK_UINT8` are distinct kinds), §7.5.2.4 (the element
//! type of a collection).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast::PrimitiveKind;
use zerodds_types::dynamic::{
    DynamicData, DynamicError, DynamicTypeBuilder, ExtensibilityKind, MEMBER_ID_INVALID, NewMember,
    TryConstructKind, TypeKind, default_expr_text,
};
use zerodds_types::qos::ExtensibilityForRepr;

#[test]
fn xtypes_7_5_2_5_member_id_invalid_is_0x0fffffff() {
    assert_eq!(MEMBER_ID_INVALID, 0x0FFF_FFFF);
}

#[test]
fn xtypes_7_3_1_2_1_1_a_ros_message_built_like_the_shim_numbers_its_members_from_zero() {
    let mut model = DynamicTypeBuilder::new();
    let module = model.module(&["pkg", "msg"]).unwrap();
    let root = model
        .structure_in(module, "Sample", ExtensibilityKind::Final)
        .unwrap();
    let int32 = model.primitive(TypeKind::Int32).unwrap();
    let float64 = model.primitive(TypeKind::Float64).unwrap();
    let string = model.string(false, None).unwrap();
    let array = model.array(int32, &[2]).unwrap();
    let int16 = model.primitive(TypeKind::Int16).unwrap();
    let sequence = model.sequence(int16, None).unwrap();
    for (name, type_) in [
        ("a", int32),
        ("b", float64),
        ("s", string),
        ("arr", array),
        ("seq", sequence),
    ] {
        model
            .add_member(root, &NewMember::new(name, type_))
            .unwrap();
    }
    assert!(model.model().type_(root).is_ok());
    let ty = model.finish_as(root).unwrap();

    assert_eq!(ty.qualified_name(), "pkg::msg::Sample");
    assert_eq!(ty.kind(), TypeKind::Structure);
    assert_eq!(ty.extensibility(), Some(ExtensibilityKind::Final));
    assert_eq!(ty.member_count(), 5);
    let numbered: Vec<(String, u32)> = ty
        .members()
        .iter()
        .map(|m| (m.name().to_string(), m.id()))
        .collect();
    assert_eq!(
        numbered,
        [
            ("a".to_string(), 0),
            ("b".to_string(), 1),
            ("s".to_string(), 2),
            ("arr".to_string(), 3),
            ("seq".to_string(), 4),
        ]
    );
    assert_eq!(ty.member_by_name("s").unwrap().id(), 2);
    assert_eq!(ty.member_by_index(1).unwrap().name(), "b");
    assert!(ty.member_by_id(MEMBER_ID_INVALID).is_none());

    let seq = ty.member_by_id(4).unwrap().dynamic_type().unwrap();
    assert_eq!(seq.kind(), TypeKind::Sequence);
    assert_eq!(seq.element_type().unwrap().kind(), TypeKind::Int16);
    let arr = ty.member_by_name("arr").unwrap().dynamic_type().unwrap();
    assert_eq!(arr.kind(), TypeKind::Array);
    assert_eq!(arr.element_type().unwrap().kind(), TypeKind::Int32);
    assert_eq!(
        ty.resolve_alias().unwrap().kind(),
        TypeKind::Structure,
        "a struct resolves to itself"
    );
    assert!(ty.equals(&ty.clone()));
}

#[test]
fn xtypes_7_2_2_4_4_4_8_every_field_of_a_new_member_reaches_the_model() {
    // `graft` builds the member with a struct literal over all nine fields.
    let mut model = DynamicTypeBuilder::new();
    // Not `K`: a member named `k` would collide with it (IDL 4.2 §7.2.3,
    // identifiers differ in more than case).
    let root = model.structure("Holder", ExtensibilityKind::Final).unwrap();
    let int32 = model.primitive(TypeKind::Int32).unwrap();
    let member = NewMember {
        name: "k".to_string(),
        type_: int32,
        id: Some(7),
        is_key: true,
        is_optional: false,
        is_must_understand: false,
        is_external: false,
        try_construct: Some(TryConstructKind::from(zerodds_ast::TryConstruct::Trim)),
        default: Some("5".to_string()),
    };
    model.add_member(root, &member).unwrap();
    let ty = model.finish_as(root).unwrap();

    let descriptor = ty.member_by_name("k").unwrap().descriptor().unwrap();
    assert_eq!(descriptor.id, 7);
    assert!(descriptor.is_key);
    assert!(
        descriptor.is_must_understand,
        "a key member is must-understand"
    );
    assert!(!descriptor.is_optional);
    assert_eq!(descriptor.try_construct_kind, TryConstructKind::Trim);
    assert!(descriptor.has_default_value);

    // The facet `graft` reads back, and the text of its default.
    let ast = ty.ast();
    let zerodds_ast::TypeKind::Structure { members, .. } = &ast.type_(ty.id()).unwrap().kind else {
        panic!("a structure");
    };
    let node = ast.member(members[0]).unwrap();
    let facet = node.facets.xtypes.as_ref().unwrap();
    assert_eq!(facet.effective_member_id(), Some(7));
    let expr = facet.default_value.unwrap();
    assert_eq!(default_expr_text(ast, expr, "k").unwrap(), "5");
}

#[test]
fn xtypes_7_3_4_2_octet_and_uint8_are_two_type_kinds() {
    assert_eq!(TypeKind::of_primitive(PrimitiveKind::Octet), TypeKind::Byte);
    assert_eq!(
        TypeKind::of_primitive(PrimitiveKind::UInt8),
        TypeKind::UInt8
    );
    assert_eq!(TypeKind::of_primitive(PrimitiveKind::Char), TypeKind::Char8);
    assert_eq!(
        TypeKind::of_primitive(PrimitiveKind::WChar),
        TypeKind::Char16
    );
    for primitive in PrimitiveKind::ALL {
        assert_eq!(
            TypeKind::of_primitive(primitive).as_primitive(),
            Some(primitive)
        );
    }
}

#[test]
fn dynamic_data_values_carry_the_kind_they_were_built_with() {
    assert_eq!(
        DynamicData::integer(PrimitiveKind::Int16, -3),
        DynamicData::Integer {
            kind: Some(PrimitiveKind::Int16),
            value: -3
        }
    );
    assert_eq!(
        DynamicData::float(PrimitiveKind::Float32, 0.5),
        DynamicData::Float {
            kind: Some(PrimitiveKind::Float32),
            value: 0.5
        }
    );
    assert_eq!(DynamicData::boolean(true), DynamicData::Boolean(true));
    assert_eq!(
        DynamicData::text("x".to_string()),
        DynamicData::Text("x".to_string())
    );
    let omega = DynamicData::character(PrimitiveKind::WChar, 'Ω');
    assert_eq!(omega.as_char(), Some('Ω'));

    let aggregate = DynamicData::Aggregate(vec![(
        "a".to_string(),
        DynamicData::Collection(vec![DynamicData::boolean(false)]),
    )]);
    assert_eq!(aggregate.members().unwrap().len(), 1);
    assert_eq!(
        aggregate.member("a").unwrap().items().unwrap(),
        [DynamicData::Boolean(false)]
    );
    assert!(aggregate.items().is_none());
}

#[test]
fn the_error_and_extensibility_values_the_shim_builds() {
    let unresolved = DynamicError::Unresolved {
        at: "pkg::msg::Missing".to_string(),
    };
    let model = DynamicError::Model {
        detail: "a duplicate member".to_string(),
    };
    assert_ne!(format!("{unresolved:?}"), format!("{model:?}"));
    assert_ne!(
        ExtensibilityForRepr::Final,
        ExtensibilityForRepr::Appendable
    );
}
