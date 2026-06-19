//! Edge-case integration tests for the IDL→C++ codegen.
//!
//! - Empty IDL → preamble-only header
//! - Reserved C++ keyword as IDL field name → error
//! - Inheritance cycle (self-reference) → error
//! - Include-set completeness
//! - Namespace depth
//! - Indent width ≠ default
//! - C++ construct negatives: interface, valuetype, fixed, any, map, bitset, bitmask

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
    // IDL does not allow 'class' as an identifier (it is not a keyword
    // in IDL either, but the C++ generator must reject it).
    // 'register' is reserved in C++, not in IDL — a perfect test.
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
    // Self-loop via forward-decl + def. IDL does not allow `struct X : X;`
    // directly, but we can construct it manually.
    // We build an AST with a self-reference via parse('forward + def'),
    // where IDL does not allow a self-loop in the grammar — so we test
    // this via two structs that inherit from each other.
    // The direct self-loop is constructed in a dedicated builder path.
    // Here: indirect A->B->A cycle.
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
    // open + close visible for all three modules
    let opens = cpp.matches("namespace ").count();
    // 'namespace ' appears both in `namespace X {` and in
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
    // Previously rejected; now fully covered — see
    // `spec_conformance::fixed_member_emits_dds_core_fixed_template`.
    let ast = parse("typedef fixed<5, 2> Money;");
    let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
    assert!(cpp.contains("dds::core::Fixed<5, 2>"));
    assert!(cpp.contains("using Money"));
}

#[test]
fn bitset_emits_struct_with_bitfields() {
    // Previously an unsupported-reject; now fully covered — see
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
    // The ZeroDDS open must come BEFORE the Inner open.
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
    // First appearance is a forward class decl:
    assert!(cpp.contains("class Forward;"));
    // The full definition follows later:
    assert!(cpp.contains("class Forward {"));
}

#[test]
fn single_module_with_constant_does_not_emit_unwanted_includes() {
    // Ensure that <vector> does not appear when no sequence is present.
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
