//! Integration-Test: Anonymous-Types in Member-Position (C4.6 §1.6).
//!
//! Spec §7.4.13.4.1.7: `sequence<long, 100>` direkt als Member-Type
//! muss als anonymer Type-AST-Knoten erfasst werden (nicht als String).

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

use zerodds_idl::ast::{ConstrTypeDecl, Definition, StructDcl, TypeDecl, TypeSpec};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::parse;

fn first_struct(src: &str) -> zerodds_idl::ast::StructDef {
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    for d in &ast.definitions {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = d {
            return s.clone();
        }
    }
    panic!("no struct found");
}

#[test]
fn sequence_as_member_is_inline_typespec() {
    let s = first_struct("struct S { sequence<long, 100> ids; };");
    assert_eq!(s.members.len(), 1);
    let m = &s.members[0];
    assert!(matches!(m.type_spec, TypeSpec::Sequence(_)));
}

#[test]
fn unbounded_sequence_as_member_is_inline_typespec() {
    let s = first_struct("struct S { sequence<long> ids; };");
    let m = &s.members[0];
    assert!(matches!(m.type_spec, TypeSpec::Sequence(_)));
    if let TypeSpec::Sequence(seq) = &m.type_spec {
        assert!(seq.bound.is_none());
    }
}

#[test]
fn nested_sequence_as_member_is_inline_typespec() {
    let s = first_struct("struct S { sequence<sequence<long> > nested; };");
    let m = &s.members[0];
    if let TypeSpec::Sequence(outer) = &m.type_spec {
        assert!(matches!(*outer.elem, TypeSpec::Sequence(_)));
    } else {
        panic!("outer not a Sequence: {:?}", m.type_spec);
    }
}

#[test]
fn bounded_string_as_member() {
    let s = first_struct("struct S { string<32> name; };");
    let m = &s.members[0];
    assert!(matches!(m.type_spec, TypeSpec::String(_)));
}

#[test]
fn array_declarator_carries_dimensions() {
    let s = first_struct("struct S { long matrix[3][4]; };");
    let m = &s.members[0];
    if let zerodds_idl::ast::Declarator::Array(a) = &m.declarators[0] {
        assert_eq!(a.sizes.len(), 2);
    } else {
        panic!("not Array Declarator");
    }
}
