//! Snapshot tests for the C++ codegen output.
//!
//! Instead of brittle `assert!(contains(...))` patterns we compare
//! the complete emitted header against a snapshot file.
//! Changes are accepted interactively via `cargo insta review`
//! or rejected. Catches unintended code drift in the codegen.

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
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header, generate_cpp_header_with_amqp};

fn gen_default(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

fn gen_with_amqp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header_with_amqp(&ast, &CppGenOptions::default()).expect("gen")
}

#[test]
fn snapshot_simple_struct() {
    let cpp = gen_default("struct Point { long x; long y; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_struct_with_string_and_sequence() {
    let cpp =
        gen_default("struct Bag { string name; sequence<long> ids; sequence<string, 16> tags; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_module_nesting() {
    let cpp = gen_default("module Outer { module Inner { struct S { long x; }; }; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_enum_and_typedef() {
    let cpp = gen_default("enum Color { RED, GREEN, BLUE }; typedef Color Hue;");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_union() {
    let cpp = gen_default(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
    );
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_keyed_struct() {
    let cpp = gen_default("struct Sensor { @key long id; double value; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_optional_member() {
    let cpp = gen_default("struct S { @optional long maybe; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_inheritance() {
    let cpp =
        gen_default("struct Base { long base_field; }; struct Child : Base { long child_field; };");
    insta::assert_snapshot!(cpp);
}

/// Issue #14: the FULL C++ backend must ESCAPE reserved-word identifier
/// collisions (trailing `_`), not reject them. The fixture uses C++ keywords
/// (that are not also IDL keywords) at every level: module `class`, struct
/// `template`, members `int`/`operator`, enum `mutable` with enumerators
/// `new`/`delete`, a union `namespace` with a keyword-labelled enum
/// discriminator, and a nested-struct member to exercise a reference site.
const RESERVED_WORD_IDL: &str = "module class { \
    enum mutable { new, delete }; \
    @final struct template { long int; long operator; }; \
    @appendable struct explicit { long friend; }; \
    struct volatile { template inline; explicit signed; }; \
    struct virtual : template { long throw; }; \
    union namespace switch (mutable) { \
        case new: long alignas; case delete: double noexcept; default: octet register; \
    }; \
};";

#[test]
fn snapshot_reserved_word_escaping() {
    let cpp = gen_default(RESERVED_WORD_IDL);
    insta::assert_snapshot!(cpp);
}

#[test]
fn reserved_words_are_escaped_not_rejected() {
    let cpp = gen_default(RESERVED_WORD_IDL);
    // Every reserved token is escaped with a trailing underscore.
    for tok in [
        "namespace class_ {",              // module
        "enum class mutable_ : int32_t {", // enum
        "new_ = 0,",                       // enumerator (explicit wire value)
        "delete_ = 1,",                    // enumerator (explicit wire value)
        "class template_ {",               // struct
        "int32_t& int_()",                 // member accessor (escaped)
        "int32_t& operator_()",            // member accessor (escaped)
        "int32_t int_field;",              // storage field: __-free (not int__)
        "class explicit_ {",               // @appendable struct
        "int32_t& friend_()",              // member of @appendable struct
        "class volatile_ {",               // container struct
        "class virtual_ ",                 // struct with base clause
        "int32_t& throw_()",               // member of derived struct
        "class namespace_ {",              // union
    ] {
        assert!(
            cpp.contains(tok),
            "expected escaped token missing: {tok:?}\n{cpp}"
        );
    }
    // Nested-struct member accessors (reference sites) are escaped:
    assert!(
        cpp.contains("inline_"),
        "final nested member accessor must escape"
    );
    assert!(
        cpp.contains("signed_"),
        "appendable nested member accessor must escape"
    );
    // Base-class reference is escaped and absolutely qualified:
    assert!(
        cpp.contains("class virtual_ : public ::class_::template_"),
        "base-class reference must be escaped:\n{cpp}"
    );
    // Union with an enum discriminator: keyword case labels are escaped
    // enumerators (render_case_label):
    assert!(
        cpp.contains("mutable_::new_"),
        "union label enumerator must escape"
    );
    assert!(
        cpp.contains("mutable_::delete_"),
        "union label enumerator must escape"
    );
    // Appendable nested struct is spliced through the escaped FQN:
    assert!(
        cpp.contains("topic_type_support<::class_::explicit_>"),
        "nested @appendable splice must reference escaped FQN:\n{cpp}"
    );

    // LOAD-BEARING: the DDS wire type name (type_name()) MUST stay the ORIGINAL
    // unescaped IDL FQN — escaping is name-local and must not move the wire.
    assert!(
        cpp.contains(r#"return "class::template""#),
        "type_name() must return the ORIGINAL IDL FQN, not the escaped one:\n{cpp}"
    );
    assert!(
        cpp.contains(r#"return "class::volatile""#),
        "type_name() must return the ORIGINAL IDL FQN:\n{cpp}"
    );
    assert!(
        cpp.contains(r#"return "class::namespace""#),
        "union type_name() must return the ORIGINAL IDL FQN:\n{cpp}"
    );
    // The escaped form must NEVER appear inside a type_name() string literal.
    assert!(
        !cpp.contains(r#"return "class_::template_""#),
        "escaped name must not leak into the wire type_name():\n{cpp}"
    );

    // Bug D invariant: escaping a keyword member must NOT introduce a reserved
    // double-underscore identifier (`operator_` storage is `operator_field`,
    // not `operator__`). Scan every identifier run.
    let mut dunder: Vec<String> = Vec::new();
    let b = cpp.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'_' || b[i].is_ascii_alphabetic() {
            let start = i;
            while i < b.len() && (b[i] == b'_' || b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let ident = &cpp[start..i];
            if ident != "__cplusplus" && ident.contains("__") {
                dunder.push(ident.to_string());
            }
        } else {
            i += 1;
        }
    }
    dunder.sort();
    dunder.dedup();
    assert!(
        dunder.is_empty(),
        "keyword-member escaping introduced reserved __ identifiers: {dunder:?}"
    );
}

#[test]
fn snapshot_amqp_helpers_struct() {
    let cpp = gen_with_amqp("struct Sensor { long id; double temp; };");
    insta::assert_snapshot!(cpp);
}

#[test]
fn snapshot_amqp_helpers_union() {
    let cpp = gen_with_amqp("union U switch (long) { case 1: long a; case 2: double b; };");
    insta::assert_snapshot!(cpp);
}
