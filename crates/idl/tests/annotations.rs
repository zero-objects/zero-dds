//! Integration-Test: Annotation-Parsing + AST-Speicherung (C4.6 §1.7).
//!
//! Apply-Logik (XTypes) bleibt ausserhalb (siehe C4.3); hier nur
//! "wird der Annotation-AST-Knoten korrekt erfasst".

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

use zerodds_idl::ast::{Annotation, ConstrTypeDecl, Definition, StructDcl, TypeDecl};
use zerodds_idl::config::ParserConfig;
use zerodds_idl::parse;
use zerodds_idl::semantics::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_type_annotations,
};

fn first_struct(src: &str) -> zerodds_idl::ast::StructDef {
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    for d in &ast.definitions {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = d {
            return s.clone();
        }
    }
    panic!("no struct found");
}

fn first_member_annotations(src: &str) -> Vec<Annotation> {
    first_struct(src).members[0].annotations.clone()
}

fn struct_annotations(src: &str) -> Vec<Annotation> {
    first_struct(src).annotations
}

#[test]
fn key_member_annotation_lowers() {
    let anns = first_member_annotations("struct S { @key long id; };");
    let lowered = lower_annotations(&anns).expect("lower");
    assert!(lowered.has_key());
}

#[test]
fn id_with_int_param_lowers() {
    let anns = first_member_annotations("struct S { @id(42) long x; };");
    let lowered = lower_annotations(&anns).expect("lower");
    assert_eq!(lowered.explicit_id(), Some(42));
}

#[test]
fn extensibility_mutable_lowers() {
    let anns = struct_annotations("@extensibility(MUTABLE) struct S { long x; };");
    let lowered = lower_type_annotations(&anns).expect("lower");
    assert_eq!(lowered.extensibility(), Some(ExtensibilityKind::Mutable));
}

#[test]
fn extensibility_appendable_lowers() {
    let anns = struct_annotations("@extensibility(APPENDABLE) struct S { long x; };");
    let lowered = lower_type_annotations(&anns).expect("lower");
    assert_eq!(lowered.extensibility(), Some(ExtensibilityKind::Appendable));
}

#[test]
fn topic_marker_lowers() {
    let anns = struct_annotations("@topic struct S { long x; };");
    let lowered = lower_type_annotations(&anns).expect("lower");
    assert!(
        lowered
            .builtins
            .iter()
            .any(|b| matches!(b, BuiltinAnnotation::Topic))
    );
}

#[test]
fn nested_marker_lowers() {
    let anns = struct_annotations("@nested struct S { long x; };");
    let lowered = lower_type_annotations(&anns).expect("lower");
    assert!(
        lowered
            .builtins
            .iter()
            .any(|b| matches!(b, BuiltinAnnotation::Nested))
    );
}

#[test]
fn hashid_no_param_lowers() {
    let anns = first_member_annotations("struct S { @hashid long x; };");
    let lowered = lower_annotations(&anns).expect("lower");
    assert!(
        lowered
            .builtins
            .iter()
            .any(|b| matches!(b, BuiltinAnnotation::HashId(None)))
    );
}

#[test]
fn bit_bound_annotation_on_bitmask_lowers() {
    let src = "@bit_bound(8) bitmask Flags { F0, F1 };";
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    for d in &ast.definitions {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) = d {
            let lowered = lower_annotations(&b.annotations).expect("lower");
            assert!(
                lowered
                    .builtins
                    .iter()
                    .any(|x| matches!(x, BuiltinAnnotation::BitBound(8)))
            );
            return;
        }
    }
    panic!("no bitmask found");
}

#[test]
fn position_annotation_on_bitmask_value_lowers() {
    let src = "bitmask Flags { @position(3) F0, F1 };";
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    for d in &ast.definitions {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) = d {
            let lowered = lower_annotations(&b.values[0].annotations).expect("lower");
            assert!(
                lowered
                    .builtins
                    .iter()
                    .any(|x| matches!(x, BuiltinAnnotation::Position(3)))
            );
            return;
        }
    }
    panic!("no bitmask found");
}

#[test]
fn optional_member_annotation_lowers() {
    let anns = first_member_annotations("struct S { @optional long x; };");
    let lowered = lower_annotations(&anns).expect("lower");
    assert!(
        lowered
            .builtins
            .iter()
            .any(|b| matches!(b, BuiltinAnnotation::Optional))
    );
}

#[test]
fn must_understand_member_annotation_lowers() {
    let anns = first_member_annotations("struct S { @must_understand long x; };");
    let lowered = lower_annotations(&anns).expect("lower");
    assert!(
        lowered
            .builtins
            .iter()
            .any(|b| matches!(b, BuiltinAnnotation::MustUnderstand))
    );
}

#[test]
fn default_literal_annotation_on_enumerator_lowers() {
    // `@default_literal` ist die OMG-Form fuer Enum-Default-Werte.
    let src = "enum E { @default_literal A, B };";
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    for d in &ast.definitions {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) = d {
            let lowered = lower_annotations(&e.enumerators[0].annotations).expect("lower");
            assert!(
                lowered
                    .builtins
                    .iter()
                    .any(|b| matches!(b, BuiltinAnnotation::DefaultLiteral))
            );
            return;
        }
    }
    panic!("no enum found");
}

#[test]
fn multiple_annotations_combine_on_member() {
    let anns = first_member_annotations("struct S { @key @id(7) @optional long x; };");
    let lowered = lower_annotations(&anns).expect("lower");
    assert!(lowered.has_key());
    assert_eq!(lowered.explicit_id(), Some(7));
    assert!(
        lowered
            .builtins
            .iter()
            .any(|b| matches!(b, BuiltinAnnotation::Optional))
    );
}

#[test]
fn verbatim_marker_on_struct_parses() {
    // Phase-1: `@verbatim` ohne Multi-Param-Argumente.
    let src = "@verbatim struct S { long x; };";
    let ast = parse(src, &ParserConfig::default()).expect("parse");
    assert_eq!(ast.definitions.len(), 1);
}
