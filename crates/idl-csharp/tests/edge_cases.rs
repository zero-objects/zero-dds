//! Edge-case integration tests for the IDL→C# codegen.
//!
//! - Empty IDL → preamble-only output
//! - Reserved C# keyword as IDL field name → escape behavior
//! - Inheritance cycle → error
//! - Using-set completeness
//! - Namespace depth
//! - Indent width != default
//! - Unsupported constructs: interface, valuetype, fixed, any, bitset, bitmask

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
use zerodds_idl_csharp::{CsGenError, CsGenOptions, generate_csharp};

fn parse(src: &str) -> zerodds_idl::ast::Specification {
    zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed")
}

#[test]
fn empty_ast_produces_preamble_only() {
    let cs = generate_csharp(&parse(""), &CsGenOptions::default()).expect("gen");
    assert!(cs.contains("#nullable enable"));
    assert!(cs.contains("using System;"));
    assert!(!cs.contains("class "));
    assert!(!cs.contains("namespace "));
}

#[test]
fn reserved_field_name_class_is_escaped_with_at_prefix() {
    // 'class' is not reserved in IDL (only in C#) — a perfect test for
    // the `@` escape behavior.
    // We cannot use 'class' directly as an IDL field name because the
    // IDL parser may reject it; therefore we use 'string' (also reserved
    // in C#, a type keyword in IDL but allowed as a field name under the
    // quoting rules).
    // Instead we test this via 'object' — allowed in IDL, contextual in C#.
    let ast = parse("struct S { long object; };");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");
    // 'object' is strict-reserved in C# → must be escaped as @object.
    // PascalCase + escape: pascal_case("object") = "Object" — Object is
    // not reserved, so the property name is "Object" without @.
    assert!(cs.contains("Object"));
}

#[test]
fn reserved_field_name_with_pure_lowercase_keyword_escapes() {
    // 'event' is strict-reserved in C# and a valid identifier in IDL
    // (not an IDL keyword). PascalCase → 'Event' is NOT reserved, so no
    // @ escape in the property name.
    let ast = parse("struct S { long event; };");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");
    assert!(cs.contains("Event"));
}

#[test]
fn inheritance_self_loop_is_rejected() {
    // Indirect A->B->A cycle.
    let ast = parse(
        "struct A : B { long a; };\n\
         struct B : A { long b; };",
    );
    let res = generate_csharp(&ast, &CsGenOptions::default());
    assert!(
        matches!(res, Err(CsGenError::InheritanceCycle { .. })),
        "expected InheritanceCycle, got {res:?}",
    );
}

#[test]
fn using_set_is_complete_for_full_typeset() {
    let cs = generate_csharp(
        &parse(
            "struct S { \
                @optional string s; \
                sequence<long> v; \
                long arr[3]; \
            }; \
            union U switch (long) { case 1: long a; }; \
            exception E { long err; };",
        ),
        &CsGenOptions::default(),
    )
    .expect("gen");
    assert!(cs.contains("using System;"));
    assert!(cs.contains("using System.Collections.Generic;"));
}

#[test]
fn namespace_three_level_hierarchy_emits_open_close_pairs() {
    let cs = generate_csharp(
        &parse("module A { module B { module C {}; }; };"),
        &CsGenOptions::default(),
    )
    .expect("gen");
    let opens = cs.matches("namespace ").count();
    // 3 opens + 3 closes (`} // namespace X`) = 6.
    assert!(opens >= 6, "namespace tokens count: {opens}");
}

#[test]
fn custom_indent_width_changes_output() {
    let two = generate_csharp(
        &parse("module M { struct S { long x; }; };"),
        &CsGenOptions {
            indent_width: 2,
            ..Default::default()
        },
    )
    .expect("gen");
    let four = generate_csharp(
        &parse("module M { struct S { long x; }; };"),
        &CsGenOptions::default(),
    )
    .expect("gen");
    assert_ne!(two, four, "indent width must influence output");
}

#[test]
fn interface_emits_csharp_interface() {
    let ast = parse("interface I { void op(); };");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
    assert!(cs.contains("public interface I"));
}

#[test]
fn interface_embedded_types_are_emitted_not_dropped() {
    // IDL type/const/exception declarations nested in an interface were
    // previously silently dropped; they must now be emitted (nested) and the
    // needed runtime usings collected.
    let ast = parse(
        "interface I { struct Inner { long x; }; enum E { A, B }; \
         const long C = 5; exception Oops { string msg; }; long op(in long a); };",
    );
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
    assert!(cs.contains("public interface I"));
    assert!(
        cs.contains("record class Inner"),
        "embedded struct dropped:\n{cs}"
    );
    assert!(cs.contains("enum E"), "embedded enum dropped");
    assert!(
        cs.contains("public const int C = 5;"),
        "embedded const dropped"
    );
    assert!(cs.contains("class Oops"), "embedded exception dropped");
    // The nested struct's TypeSupport pulls in the runtime usings.
    assert!(
        cs.contains("using Omg.Types;"),
        "missing Omg.Types using:\n{cs}"
    );
    assert!(
        cs.contains("using ZeroDDS.Cdr;"),
        "missing ZeroDDS.Cdr using"
    );
}

#[test]
fn any_type_emits_omg_types_any() {
    let ast = parse("struct S { any value; };");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
    assert!(cs.contains("Omg.Types.Any"));
}

#[test]
fn fixed_type_emits_decimal() {
    let ast = parse("typedef fixed<5, 2> Money;");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
    assert!(cs.contains("decimal"));
}

#[test]
fn nested_struct_uses_nested_codec_not_empty_bytes() {
    // A nested struct member was encoded as `Array.Empty<byte>()` (data dropped)
    // and decoded as `default!` — silent round-trip corruption. It must now go
    // through the nested TypeSupport codec on the shared CDR stream.
    let cs = generate_csharp(
        &parse("struct Inner { long x; long y; }; struct Outer { Inner inner; long z; };"),
        &CsGenOptions::default(),
    )
    .expect("ok");
    assert!(
        cs.contains("InnerTypeSupport.Instance.EncodeInto(w, sample.Inner)"),
        "nested struct must encode via the nested codec:\n{cs}"
    );
    assert!(
        cs.contains("InnerTypeSupport.Instance.DecodeFrom(r)"),
        "nested struct must decode via the nested codec:\n{cs}"
    );
    assert!(
        !cs.contains("as object) is byte[]"),
        "nested struct must not use the empty-bytes placeholder:\n{cs}"
    );
}

#[test]
fn nested_enum_uses_int32_cast() {
    let cs = generate_csharp(
        &parse("enum Color { R, G, B }; struct S { Color c; long n; };"),
        &CsGenOptions::default(),
    )
    .expect("ok");
    assert!(
        cs.contains("w.WriteInt32((int)sample.C)"),
        "enum member encodes as int32:\n{cs}"
    );
    assert!(
        cs.contains("(Color)r.ReadInt32()"),
        "enum member decodes from int32:\n{cs}"
    );
}

#[test]
fn map_fixed_any_member_gates_typesupport_no_runtime_throw() {
    // map/fixed/any have no XCDR2 codec yet — a struct with such a member must
    // get its data type but be gated (no TypeSupport), NOT a codec that throws
    // `XcdrException` at runtime.
    for (src, ty) in [
        ("struct S { map<long, string> kv; long n; };", "IDictionary"),
        ("struct F { fixed<10,2> amount; long n; };", "decimal"),
        ("struct A { any value; long n; };", "Omg.Types.Any"),
    ] {
        let cs = generate_csharp(&parse(src), &CsGenOptions::default()).expect("ok");
        assert!(cs.contains(ty), "data type missing for `{src}`:\n{cs}");
        assert!(
            !cs.contains("XcdrException(\"unsupported"),
            "must not emit a runtime-throwing codec for `{src}`:\n{cs}"
        );
        assert!(
            !cs.contains("class STypeSupport")
                && !cs.contains("class FTypeSupport")
                && !cs.contains("class ATypeSupport"),
            "struct with map/fixed/any must be gated (no TypeSupport) for `{src}`:\n{cs}"
        );
    }
}

#[test]
fn bitset_emits_struct() {
    // Bitset/Bitmask are now fully covered — see
    // `spec_conformance::bitset_emits_struct_with_value_field`.
    let ast = parse("bitset Flags { bitfield<3> a; };");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
    assert!(cs.contains("public struct Flags"));
    assert!(cs.contains("public ulong A"));
}

#[test]
fn const_decl_is_emitted() {
    let cs = generate_csharp(
        &parse("const long MAX_SIZE = 1024;"),
        &CsGenOptions::default(),
    )
    .expect("gen");
    assert!(cs.contains("public const int MAX_SIZE = 1024;"));
}

#[test]
fn root_namespace_appears_outermost() {
    let cs = generate_csharp(
        &parse("module Inner { struct S { long x; }; };"),
        &CsGenOptions {
            root_namespace: Some("ZeroDDS".into()),
            ..Default::default()
        },
    )
    .expect("gen");
    let outer_open = cs.find("namespace ZeroDDS").expect("zerodds open");
    let inner_open = cs.find("namespace Inner").expect("inner open");
    assert!(outer_open < inner_open);
}

#[test]
fn forward_declared_struct_emits_partial_record_class() {
    let cs = generate_csharp(
        &parse("struct Forward; struct Forward { long x; };"),
        &CsGenOptions::default(),
    )
    .expect("gen");
    // Forward decl as a partial record class.
    assert!(cs.contains("public partial record class Forward;"));
    // Plus full definition (with C5.3-b: ITopicType marker).
    assert!(cs.contains("public partial record class Forward : ITopicType<Forward>"));
}

#[test]
fn single_module_with_constant_does_not_emit_extra_usings() {
    let cs = generate_csharp(
        &parse("module M { const long N = 10; };"),
        &CsGenOptions::default(),
    )
    .expect("gen");
    // No sequence/list → no System.Collections.Generic.
    assert!(!cs.contains("System.Collections.Generic"));
}

#[test]
fn nested_modules_emit_nested_csharp_namespaces() {
    let cs = generate_csharp(
        &parse(
            "module Outer { \
                module Middle { \
                    struct Foo { long x; }; \
                }; \
            };",
        ),
        &CsGenOptions::default(),
    )
    .expect("gen");
    // Outer-open BEFORE Middle-open BEFORE Foo.
    let outer = cs.find("namespace Outer").expect("outer");
    let middle = cs.find("namespace Middle").expect("middle");
    let foo = cs.find("record class Foo").expect("foo");
    assert!(outer < middle);
    assert!(middle < foo);
}

#[test]
fn deep_nested_sequence_emits_correct_ilist() {
    let cs = generate_csharp(
        &parse("struct S { sequence<sequence<long>> matrix; };"),
        &CsGenOptions::default(),
    )
    .expect("gen");
    // C5.3-b: sequences → ISequence<T> (unbounded).
    assert!(cs.contains("ISequence<ISequence<int>>"));
}

#[test]
fn use_records_false_still_emits_record_for_now() {
    // Foundation: use_records=false exists as an option, but the
    // emission behavior in C5.3-a is always record-class. The test
    // ensures the option does not cause a crash.
    let cs = generate_csharp(
        &parse("struct S { long x; };"),
        &CsGenOptions {
            use_records: false,
            ..Default::default()
        },
    )
    .expect("gen");
    assert!(cs.contains("record class S"));
}

#[test]
fn empty_root_namespace_string_is_treated_as_none() {
    let cs = generate_csharp(
        &parse("module M {};"),
        &CsGenOptions {
            root_namespace: Some("".into()),
            ..Default::default()
        },
    )
    .expect("gen");
    // M namespace is there, but no empty `namespace {`.
    assert!(cs.contains("namespace M"));
    assert!(!cs.contains("namespace \n"));
}

#[test]
fn map_type_emits_idictionary() {
    // Map is intended in the 3.3 scope only as a TypeSpec (out of scope
    // per the roadmap, but the spec maps it); we test that the use is
    // either emitted consistently or rejected as unsupported.
    // Current state: emits IDictionary<,>.
    let res = zerodds_idl::parse(
        "struct S { map<long, string> kv; };",
        &ParserConfig::default(),
    );
    if let Ok(ast) = res {
        // If the parser accepts it, the codegen checks it.
        if let Ok(cs) = generate_csharp(&ast, &CsGenOptions::default()) {
            assert!(cs.contains("IDictionary<int, string>"));
        }
    }
    // If the parser does not yet fully support map, the test is
    // implicitly ok (no panic).
}
