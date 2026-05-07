//! Edge-Case-Integration-Tests fuer den IDL→C++-Codegen.
//!
//! - Empty IDL → preamble-only Header
//! - Reserved C++-Keyword als IDL-Field-Name → Error
//! - Inheritance-Cycle (Self-Reference) → Error
//! - Include-Set vollstaendigkeit
//! - Namespace-Tiefe
//! - Indent-Width ≠ default
//! - C++-Konstrukt-Negative: interface, valuetype, fixed, any, map, bitset, bitmask

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

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenError, CppGenOptions, generate_cpp_header};

fn parse(src: &str) -> zerodds_idl::ast::Specification {
    zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed")
}

#[test]
fn empty_ast_produces_preamble_only() {
    let cpp = generate_cpp_header(&parse(""), &CppGenOptions::default()).expect("gen");
    assert!(cpp.contains("#pragma once"));
    assert!(cpp.contains("#include <cstdint>"));
    // No type definitions, no namespaces.
    assert!(!cpp.contains("class "));
    assert!(!cpp.contains("namespace "));
}

#[test]
fn struct_with_reserved_field_name_is_rejected() {
    // IDL erlaubt 'class' nicht als Identifier (es ist auch in IDL kein
    // Keyword, aber der C++-Generator muss es ablehnen).
    // 'register' ist in C++ reserviert, in IDL nicht — perfekter Test.
    let ast = parse("struct Foo { long register; };");
    let res = generate_cpp_header(&ast, &CppGenOptions::default());
    match res {
        Err(CppGenError::InvalidName { name, .. }) => {
            assert_eq!(name, "register");
        }
        other => panic!("expected InvalidName, got {other:?}"),
    }
}

#[test]
fn inheritance_self_loop_is_rejected() {
    // Self-loop ueber forward-decl + def. IDL erlaubt zwar nicht
    // direkt `struct X : X;`, aber wir koennen es manuell konstruieren.
    // Wir bauen einen AST mit Self-Reference per parse('forward + def'),
    // wobei IDL einen Self-Loop in der Grammar nicht erlaubt — daher
    // testen wir das ueber zwei Structs, die sich gegenseitig erben.
    // Der direkte Self-Loop wird in einem dedizierten Builder-Path
    // konstruiert. Hier: indirekter A->B->A-Cycle.
    let ast = parse(
        "struct A : B { long a; };\n\
         struct B : A { long b; };",
    );
    let res = generate_cpp_header(&ast, &CppGenOptions::default());
    assert!(
        matches!(res, Err(CppGenError::InheritanceCycle { .. })),
        "expected InheritanceCycle, got {res:?}",
    );
}

#[test]
fn include_set_is_complete_for_full_typeset() {
    let cpp = generate_cpp_header(
        &parse(
            "struct S { \
                @optional string s; \
                sequence<long> v; \
                long arr[3]; \
            }; \
            union U switch (long) { case 1: long a; }; \
            exception E { long err; };",
        ),
        &CppGenOptions::default(),
    )
    .expect("gen");
    for h in [
        "<cstdint>",
        "<string>",
        "<vector>",
        "<array>",
        "<optional>",
        "<variant>",
        "<exception>",
    ] {
        assert!(cpp.contains(h), "missing include {h}");
    }
}

#[test]
fn namespace_three_level_hierarchy_emits_open_close_pairs() {
    let cpp = generate_cpp_header(
        &parse("module A { module B { module C {}; }; };"),
        &CppGenOptions::default(),
    )
    .expect("gen");
    // open + close fuer alle drei Module sichtbar
    let opens = cpp.matches("namespace ").count();
    // 'namespace ' erscheint sowohl in `namespace X {` als auch in
    // `} // namespace X` — 3 opens + 3 closes = 6.
    assert!(opens >= 6, "namespace tokens count: {opens}");
}

#[test]
fn custom_indent_width_changes_output() {
    let two = generate_cpp_header(
        &parse("module M { struct S { long x; }; };"),
        &CppGenOptions {
            indent_width: 2,
            ..Default::default()
        },
    )
    .expect("gen");
    let four = generate_cpp_header(
        &parse("module M { struct S { long x; }; };"),
        &CppGenOptions::default(),
    )
    .expect("gen");
    assert_ne!(two, four, "indent width must influence output");
}

#[test]
fn interface_emits_pure_virtual_class() {
    let ast = parse("interface I { void op(); };");
    let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
    assert!(cpp.contains("class I"));
}

#[test]
fn any_type_emits_dds_core_any() {
    let ast = parse("struct S { any value; };");
    let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
    assert!(cpp.contains("dds::core::Any"));
}

#[test]
fn fixed_type_emits_dds_core_template() {
    // Frueher Reject; jetzt voll abgedeckt — siehe
    // `spec_conformance::fixed_member_emits_dds_core_fixed_template`.
    let ast = parse("typedef fixed<5, 2> Money;");
    let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
    assert!(cpp.contains("dds::core::Fixed<5, 2>"));
    assert!(cpp.contains("using Money"));
}

#[test]
fn bitset_emits_struct_with_bitfields() {
    // Frueher Unsupported-Reject; jetzt voll abgedeckt — siehe
    // `spec_conformance::bitset_emits_struct_with_value_field`.
    let ast = parse("bitset Flags { bitfield<3> a; };");
    let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
    assert!(cpp.contains("struct Flags"));
    assert!(cpp.contains("uint64_t a()"));
}

#[test]
fn const_decl_is_emitted() {
    let cpp = generate_cpp_header(
        &parse("const long MAX_SIZE = 1024;"),
        &CppGenOptions::default(),
    )
    .expect("gen");
    assert!(cpp.contains("constexpr int32_t MAX_SIZE = 1024;"));
}

#[test]
fn double_quoted_namespace_prefix_appears_outermost() {
    let cpp = generate_cpp_header(
        &parse("module Inner { struct S { long x; }; };"),
        &CppGenOptions {
            namespace_prefix: Some("ZeroDDS".into()),
            ..Default::default()
        },
    )
    .expect("gen");
    // ZeroDDS-Open muss VOR Inner-Open kommen.
    let zerodds_open = cpp.find("namespace ZeroDDS {").expect("zerodds open");
    let inner_open = cpp.find("namespace Inner {").expect("inner open");
    assert!(zerodds_open < inner_open);
}

#[test]
fn forward_declared_struct_is_emitted_as_class_decl() {
    let cpp = generate_cpp_header(
        &parse("struct Forward; struct Forward { long x; };"),
        &CppGenOptions::default(),
    )
    .expect("gen");
    // Erste Erscheinung ist eine Forward-Class-Decl:
    assert!(cpp.contains("class Forward;"));
    // Spaeter folgt die volle Definition:
    assert!(cpp.contains("class Forward {"));
}

#[test]
fn single_module_with_constant_does_not_emit_unwanted_includes() {
    // Sicherstellen, dass <vector> nicht erscheint, wenn keine Sequence vorkommt.
    let cpp = generate_cpp_header(
        &parse("module M { const long N = 10; };"),
        &CppGenOptions::default(),
    )
    .expect("gen");
    assert!(!cpp.contains("<vector>"));
    assert!(!cpp.contains("<variant>"));
    assert!(!cpp.contains("<exception>"));
}

#[test]
fn include_guard_prefix_emits_comment() {
    let cpp = generate_cpp_header(
        &parse(""),
        &CppGenOptions {
            include_guard_prefix: Some("ZD_C5_1_A".into()),
            ..Default::default()
        },
    )
    .expect("gen");
    assert!(cpp.contains("guard-prefix: ZD_C5_1_A"));
}
