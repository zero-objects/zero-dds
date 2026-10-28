// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Three OMG IDL 4.2 grammar forms the front end refused, each with the
//! model it now produces.
//!
//! `docs/spec-coverage/idl-4.2.gaps.md` category A recorded them as
//! "not recognised — the front end refuses": adjacent string literals
//! (rule 17, §7.2.6.3), a type defined inside a `typedef` (rule 64), and
//! `typeid`/`typeprefix`/`import` inside an interface body (rule 112).
//! The probes in `tools/idl-4.2-gap-scan/cases.tsv` measure the six
//! `zerodds-idlc` subcommands; these tests hold the model.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{AnyId, Ast, ConstValue, RepositoryIdOrigin, TypeKind};
use zerodds_ast_core::ScopedName;
use zerodds_idl_lower::lower;
use zerodds_idl_parse::parse;

fn model(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the fixture does not lower: {:?}",
        lowered.diagnostics
    );
    lowered.ast
}

fn scoped(segments: &[&str]) -> ScopedName {
    ScopedName::absolute(segments.iter().map(|s| (*s).to_string()).collect())
}

/// The value of the constant `name`.
fn constant(ast: &Ast, name: &str) -> ConstValue {
    let (_, node) = ast
        .constants()
        .find(|(_, node)| node.name == name)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"));
    ast.eval_const(node.expr)
        .unwrap_or_else(|error| panic!("`{name}` does not evaluate: {error:?}"))
}

// -- rule (17): adjacent string literals ----------------------------------

#[test]
fn adjacent_string_literals_are_one_literal() {
    let ast = model("const string K = \"ab\" \"cd\";");
    assert_eq!(constant(&ast, "K"), ConstValue::String("abcd".to_string()));
}

#[test]
fn the_characters_of_concatenated_strings_are_kept_distinct() {
    // §7.2.6.3: "Characters in concatenated strings are kept
    // distinct": `"\x4" "1"` is the character 0x04 and then `1`, not
    // the escape `\x41`. Each piece is decoded on its own.
    let ast = model("const string K = \"\\x4\" \"1\";");
    assert_eq!(
        constant(&ast, "K"),
        ConstValue::String("\u{4}1".to_string())
    );
}

#[test]
fn adjacent_wide_string_literals_are_one_literal() {
    let ast = model("const wstring W = L\"ab\" L\"cd\";");
    assert_eq!(
        constant(&ast, "W"),
        ConstValue::WString("abcd".chars().map(u32::from).collect())
    );
}

#[test]
fn a_narrow_and_a_wide_literal_are_not_concatenated() {
    let parsed = parse("const string K = \"ab\" L\"cd\";");
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("not concatenated")),
        "{:?}",
        parsed.diagnostics()
    );
}

#[test]
fn an_annotation_argument_takes_the_whole_concatenated_literal() {
    // The first piece alone used to reach the model: `text = "a" "b"`
    // lowered to `a`, with no diagnostic.
    let ast =
        model("@verbatim(language = \"c\", text = \"// one \" \"two\") struct S { long a; };");
    let id = ast.find_by_scoped_name(&scoped(&["S"])).expect("S");
    let directives = &ast
        .facets(id)
        .expect("facets")
        .xtypes
        .as_ref()
        .expect("an xtypes facet")
        .verbatim;
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0].text, "// one two");
}

#[test]
fn a_typeid_takes_the_whole_concatenated_literal() {
    let ast = model(
        "module Robot { struct Pose { long x; }; };
         typeid Robot::Pose \"IDL:custom/\" \"Pose:2.0\";",
    );
    let any = ast
        .find_by_scoped_name(&scoped(&["Robot", "Pose"]))
        .expect("Pose");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:custom/Pose:2.0")
    );
}

// -- rule (64): a constructed type defined inside a typedef ---------------

#[test]
fn a_struct_defined_inside_a_typedef_is_declared_and_aliased() {
    let ast = model("typedef struct Inline { long m; } Ali;");
    let inline = ast
        .find_by_scoped_name(&scoped(&["Inline"]))
        .ok()
        .and_then(AnyId::as_type)
        .expect("the struct the typedef defines");
    assert!(
        matches!(
            ast.type_(inline).expect("Inline").kind,
            TypeKind::Structure { .. }
        ),
        "Inline is not a struct"
    );
    let alias = ast
        .find_by_scoped_name(&scoped(&["Ali"]))
        .ok()
        .and_then(AnyId::as_type)
        .expect("the alias");
    match &ast.type_(alias).expect("Ali").kind {
        TypeKind::Alias { aliased } => assert_eq!(aliased.resolved(), Some(inline)),
        other => panic!("Ali is {other:?}, not an alias of Inline"),
    }
}

#[test]
fn every_constructed_form_can_be_defined_inside_a_typedef() {
    let ast = model(
        "typedef union U switch (long) { case 1: long a; } UA, UB[2];
         typedef enum E { RED, GREEN } EA;
         typedef bitmask M { F0, F1 } MA;
         typedef bitset B { bitfield<3> low; } BA;",
    );
    for name in ["U", "UA", "UB", "E", "EA", "M", "MA", "B", "BA"] {
        assert!(
            ast.find_by_scoped_name(&scoped(&[name])).is_ok(),
            "`{name}` is not in the model"
        );
    }
}

// -- rule (112): typeid, typeprefix and import in an interface body --------

#[test]
fn a_typeid_inside_an_interface_body_sets_the_id() {
    let ast = model("interface Ia { typeid Ia \"IDL:Ia:2.0\"; void op(); };");
    let any = ast.find_by_scoped_name(&scoped(&["Ia"])).expect("Ia");
    let corba = ast
        .facets(any)
        .expect("facets")
        .corba
        .as_ref()
        .expect("a corba facet");
    assert_eq!(
        corba.repository_id.as_ref().map(|id| id.0.as_str()),
        Some("IDL:Ia:2.0")
    );
    assert_eq!(
        corba.repository_id_origin,
        Some(RepositoryIdOrigin::PragmaId)
    );
}

#[test]
fn typeprefix_and_import_inside_an_interface_body_parse() {
    // Both are accepted where §7.4.6.3 rule (112) puts them; what each
    // does is the file-scope directive's, already measured in
    // `dialects.rs`.
    let _ = model(
        "module Mb { struct T { long a; }; };
         interface Ia { typeprefix Ia \"ex.org\"; import Mb; void op(); };",
    );
}
