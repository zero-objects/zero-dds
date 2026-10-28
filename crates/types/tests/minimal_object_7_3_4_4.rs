// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.3.4.4: "This representation does not include
//! information on the type that would not impact type assignability. For
//! example user-defined annotations or the order of members for types with
//! extensibility kind MUTABLE."
//!
//! The example sentence about the order of the members of a MUTABLE type
//! disagrees with §7.3.4.5 (S. 127): "The elements in StructMemberSeq and
//! UnionMemberSeq shall be ordered in increasing values of the member_index."
//! The ordering rules of §7.3.4.5 are the normative serialization rules, so
//! the members of a MUTABLE type are in declaration order in the minimal
//! object as well; the user-defined annotations and the names are not in it.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_types::from_ast::{Form, minimal_type_object, type_identifier};
use zerodds_types::type_object::MinimalTypeObject;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "minimal.idl").ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_id(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
        name.to_string(),
    ]))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .unwrap_or_else(|| panic!("`{name}` is not in the model"))
}

/// A user-defined annotation and a `@verbatim` change the COMPLETE object and
/// not the MINIMAL one: the same MINIMAL object, the same MINIMAL identifier.
#[test]
fn user_defined_annotations_are_not_in_the_minimal_object() {
    let annotated = model(
        "@annotation Tag { long v default 1; };\n\
         @verbatim(language=\"c++\", placement=BEFORE_DECLARATION, text=\"//\")\n\
         @Tag(3) struct S { @Tag(4) long a; long b; };",
    );
    let plain = model("struct S { long a; long b; };");
    let (a, p) = (type_id(&annotated, "S"), type_id(&plain, "S"));
    assert_eq!(
        minimal_type_object(&annotated, a).expect("minimal"),
        minimal_type_object(&plain, p).expect("minimal")
    );
    assert_eq!(
        type_identifier(&annotated, a, Form::Minimal).expect("identifier"),
        type_identifier(&plain, p, Form::Minimal).expect("identifier")
    );
    assert_ne!(
        type_identifier(&annotated, a, Form::Complete).expect("identifier"),
        type_identifier(&plain, p, Form::Complete).expect("identifier"),
        "the COMPLETE object does carry them"
    );
}

/// The members of a MUTABLE structure are in declaration order in the
/// MINIMAL object (§7.3.4.5), whatever their ids.
#[test]
fn the_members_of_a_mutable_type_are_in_declaration_order() {
    let ast = model("@mutable struct M { @id(20) long b; @id(10) long a; @id(30) long c; };");
    let MinimalTypeObject::Struct(s) = minimal_type_object(&ast, type_id(&ast, "M")).expect("m")
    else {
        panic!()
    };
    let ids: Vec<u32> = s.member_seq.iter().map(|m| m.common.member_id).collect();
    assert_eq!(ids, [20, 10, 30]);
}

/// A MINIMAL member has the hash of its name and nothing of the name.
#[test]
fn a_minimal_member_has_the_hash_of_its_name() {
    use zerodds_types::type_object::common::NameHash;
    let ast = model("struct N { long color; };");
    let MinimalTypeObject::Struct(s) = minimal_type_object(&ast, type_id(&ast, "N")).expect("m")
    else {
        panic!()
    };
    // Annex B: "the member name "color" has NameHash {0x70, 0xDD, 0xA5, 0xDF}".
    assert_eq!(s.member_seq[0].detail, NameHash([0x70, 0xDD, 0xA5, 0xDF]));
}
