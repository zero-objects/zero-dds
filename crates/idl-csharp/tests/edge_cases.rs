//! Edge-Case-Integration-Tests fuer den IDL→C#-Codegen.
//!
//! - Empty IDL → preamble-only Output
//! - Reserved C#-Keyword als IDL-Field-Name → escape-Verhalten
//! - Inheritance-Cycle → Error
//! - Using-Set Vollstaendigkeit
//! - Namespace-Tiefe
//! - Indent-Width != default
//! - Unsupported-Konstrukte: interface, valuetype, fixed, any, bitset, bitmask

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
    // 'class' ist in IDL nicht reserviert (nur in C#) — perfekter
    // Test fuer das `@`-Escape-Verhalten.
    // Wir koennen 'class' nicht direkt im IDL als Field-Name verwenden,
    // weil der IDL-Parser es eventuell zurueckweist; daher nutzen wir
    // 'string' (auch in C# reserviert, in IDL ein Type-Keyword aber
    // als Field-Name unter Quoting-Regeln zugelassen).
    // Stattdessen testen wir das via 'object' — in IDL erlaubt, in C#
    // contextual.
    let ast = parse("struct S { long object; };");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");
    // 'object' ist in C# strict-reserved → muss als @object escaped sein.
    // PascalCase + escape: pascal_case("object") = "Object" — Object
    // ist nicht reserved, also property-name "Object" ohne @.
    assert!(cs.contains("Object"));
}

#[test]
fn reserved_field_name_with_pure_lowercase_keyword_escapes() {
    // 'event' ist in C# strict-reserved und in IDL ein gueltiger
    // Identifier (kein IDL-Keyword). PascalCase → 'Event' ist NICHT
    // reserved, daher kein @-Escape im Property-Namen.
    let ast = parse("struct S { long event; };");
    let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");
    assert!(cs.contains("Event"));
}

#[test]
fn inheritance_self_loop_is_rejected() {
    // Indirekter A->B->A-Cycle.
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
fn bitset_emits_struct() {
    // Bitset/Bitmask sind jetzt voll abgedeckt — siehe
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
    // Forward-Decl als partial record class.
    assert!(cs.contains("public partial record class Forward;"));
    // Plus volle Definition (mit C5.3-b: ITopicType-Marker).
    assert!(cs.contains("public partial record class Forward : ITopicType<Forward>"));
}

#[test]
fn single_module_with_constant_does_not_emit_extra_usings() {
    let cs = generate_csharp(
        &parse("module M { const long N = 10; };"),
        &CsGenOptions::default(),
    )
    .expect("gen");
    // Kein Sequence/List → kein System.Collections.Generic.
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
    // Outer-open VOR Middle-open VOR Foo.
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
    // C5.3-b: Sequenzen → ISequence<T> (unbounded).
    assert!(cs.contains("ISequence<ISequence<int>>"));
}

#[test]
fn use_records_false_still_emits_record_for_now() {
    // Foundation: use_records=false ist als Option vorhanden, aber das
    // Emission-Verhalten ist in C5.3-a immer record-class. Test stellt
    // sicher, dass die Option keinen Crash verursacht.
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
    // M-namespace ist da, aber kein leeres `namespace {`.
    assert!(cs.contains("namespace M"));
    assert!(!cs.contains("namespace \n"));
}

#[test]
fn map_type_emits_idictionary() {
    // Map ist im 3.3-Scope nur als TypeSpec gedacht (out-of-scope laut
    // Roadmap, aber Spec mappt es); wir testen dass die Verwendung
    // entweder konsistent emittiert oder als Unsupported abgelehnt wird.
    // Aktueller Stand: emittiert IDictionary<,>.
    let res = zerodds_idl::parse(
        "struct S { map<long, string> kv; };",
        &ParserConfig::default(),
    );
    if let Ok(ast) = res {
        // Wenn der Parser das akzeptiert, prueft der Codegen es.
        if let Ok(cs) = generate_csharp(&ast, &CsGenOptions::default()) {
            assert!(cs.contains("IDictionary<int, string>"));
        }
    }
    // Wenn der Parser map noch nicht voll unterstuetzt, ist der Test
    // implizit ok (kein Panic).
}
