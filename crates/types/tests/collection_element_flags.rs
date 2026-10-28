// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `CollectionElementFlag` of a plain collection's `TypeIdentifier`
//! (DDS-XTypes 1.3 Annex B: "T1, T2, X").
//!
//! - bits 0 and 1 are the try-construct behaviour of the element:
//!   `01` DISCARD, `10` USE_DEFAULT, `11` TRIM (§7.3.1.2.1.1 and
//!   Annex B's `MemberFlag`);
//! - bit 2, `IS_EXTERNAL`, is the `@external` the collection's element
//!   carries (§7.3.1.2.1.4: "To declare the elements of a collection type
//!   external, apply the annotation to the collection declaration").

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::Ast;
use zerodds_types::TypeIdentifier;
use zerodds_types::from_ast::{Form, type_identifier};

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "flags.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

/// The identifier of the type of member `name` of `S`.
fn member_identifier(ast: &Ast, name: &str) -> TypeIdentifier {
    let s = ast
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .expect("S");
    let member = ast
        .members_of(s)
        .iter()
        .map(|id| ast.member(*id).expect("member"))
        .find(|member| member.name == name)
        .unwrap_or_else(|| panic!("no member {name}"));
    let target = member.type_.resolved().expect("resolved");
    type_identifier(ast, target, Form::Minimal).expect("an identifier")
}

fn element_flags(ti: &TypeIdentifier) -> u16 {
    match ti {
        TypeIdentifier::PlainSequenceSmall { header, .. }
        | TypeIdentifier::PlainSequenceLarge { header, .. }
        | TypeIdentifier::PlainArraySmall { header, .. }
        | TypeIdentifier::PlainArrayLarge { header, .. }
        | TypeIdentifier::PlainMapSmall { header, .. }
        | TypeIdentifier::PlainMapLarge { header, .. } => header.element_flags.0,
        other => panic!("not a plain collection: {other:?}"),
    }
}

fn key_flags(ti: &TypeIdentifier) -> u16 {
    match ti {
        TypeIdentifier::PlainMapSmall { key_flags, .. }
        | TypeIdentifier::PlainMapLarge { key_flags, .. } => key_flags.0,
        other => panic!("not a map: {other:?}"),
    }
}

#[test]
fn the_element_of_a_collection_carries_its_external_and_its_try_construct() {
    let ast = model(
        "typedef string<5> String5;
        @final struct S {
            sequence<long, 4> plain;
            sequence<@external long, 4> external;
            sequence<@try_construct(TRIM) String5, 4> trim;
            sequence<@external @try_construct(USE_DEFAULT) long, 4> both;
            map<@external long, @external long, 4> map_external;
            map<long, @try_construct(TRIM) String5, 4> map_trim;
        };",
    );
    // DISCARD, the behaviour of an element that says nothing.
    assert_eq!(element_flags(&member_identifier(&ast, "plain")), 0b001);
    // IS_EXTERNAL is bit 2.
    assert_eq!(element_flags(&member_identifier(&ast, "external")), 0b101);
    // TRIM is `11`: the behaviour was dropped from the identifier before.
    assert_eq!(element_flags(&member_identifier(&ast, "trim")), 0b011);
    assert_eq!(element_flags(&member_identifier(&ast, "both")), 0b110);
    let map = member_identifier(&ast, "map_external");
    assert_eq!(element_flags(&map), 0b101, "the value");
    assert_eq!(key_flags(&map), 0b101, "the key");
    let map = member_identifier(&ast, "map_trim");
    assert_eq!(element_flags(&map), 0b011);
    assert_eq!(key_flags(&map), 0b001, "a map key is DISCARD");
}
