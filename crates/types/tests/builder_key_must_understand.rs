// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.2.2.4.4.4.8: "Key members shall never be optional, and
//! they shall always have their 'must understand' attribute set to true." The
//! builder and the lowering of IDL write the same flags for a key member.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::from_ast::complete_type_object;
use zerodds_types::type_identifier::{PrimitiveKind, TypeIdentifier};
use zerodds_types::type_object::CompleteTypeObject;
use zerodds_types::type_object::flags::StructMemberFlag;

#[test]
fn a_key_member_of_the_builder_must_be_understood() {
    let built = TypeObjectBuilder::struct_type("K")
        .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
            m.key()
        })
        .member("v", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
        .build_complete();
    let key = built.member_seq[0].common.member_flags;
    assert!(key.has(StructMemberFlag::IS_KEY));
    assert!(key.has(StructMemberFlag::IS_MUST_UNDERSTAND));
    assert!(
        !built.member_seq[1]
            .common
            .member_flags
            .has(StructMemberFlag::IS_MUST_UNDERSTAND)
    );
}

#[test]
fn the_builder_and_the_lowering_of_idl_write_one_object() {
    let parsed = zerodds_idl_parse::parse("struct K { @key long id; long v; };");
    assert!(!parsed.has_errors());
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "k.idl").ast;
    ast.resolve_all_by_name().expect("resolves");
    let node = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            "K".to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("K");
    let lowered = complete_type_object(&ast, node).expect("the object");
    let built = TypeObjectBuilder::struct_type("K")
        .member("id", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
            m.key()
        })
        .member("v", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
        .build_complete();
    assert_eq!(lowered, CompleteTypeObject::Struct(built));
}
