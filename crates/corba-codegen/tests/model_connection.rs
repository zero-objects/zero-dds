// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The crate against a model the real front end built.
//!
//! Every unit test in `src/` starts from a value constructed by hand,
//! which proves the tables and not the attachment. These start from IDL
//! text, run it through `zerodds-idl-parse` and `zerodds-idl-lower`, and
//! ask the questions a generator asks. A row that is right in the table
//! and unreachable from a lowered model is a row nothing can use.

// Tests assert on Results directly; a failed expect IS the test failure.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;

use zerodds_ast::{Ast, InterfaceNode};
use zerodds_ast_core::Id;
use zerodds_corba_codegen::{
    SpecialType, TargetLanguage, TypeShape, repository_id_of, shape_of, skeleton_ops_of,
    special_type_of, stub_op_of,
};

/// The thirteen, as far as IDL can write them, in one file.
const IDL: &str = r#"
module TimeBase {
    typedef unsigned long long TimeT;
};

module demo {
    native Handle;

    abstract interface Opaque {
    };

    valuetype Value {
        public long x;
    };

    struct TwoLongs {
        long a;
        long b;
    };

    struct WithString {
        string s;
    };

    struct Nested {
        TwoLongs inner;
    };

    struct NestedVariable {
        WithString inner;
    };

    typedef fixed<10,2> Money;
    typedef sequence<any> AnySeq;
    typedef string Name;
    typedef long Grid[3][4];
    typedef WithString VariableGrid[2];

    enum Colour { RED, GREEN };

    exception Bad {
        long code;
    };

    interface Echo {
        readonly attribute long counter;
        attribute string label;

        string echo(in string msg) raises (Bad);
        oneway void announce(in long n);
        void swap(inout long a, out long b);
        void pass(in any a, in AnySeq s, in wstring w, in Money m,
                  in unsigned long long u, in long double d,
                  in Object o, in Handle h, in ValueBase v,
                  in Opaque ab, in TimeBase::TimeT t);
        WithString make();
        TwoLongs pair();
    };
};
"#;

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the corpus IDL must parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "special-types.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the corpus IDL must lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    // Lowering leaves a name it could not bind on the spot pending; the
    // resolve pass is the caller's, which is the contract every other
    // consumer of `idl-lower` follows. Without it a `TimeBase::TimeT`
    // parameter and an interface used as a type both arrive unresolved.
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "every name in the corpus must resolve: {:?}",
        report.broken
    );
    ast
}

/// Every named type in the model, by its scoped name.
fn types_by_name(ast: &Ast) -> BTreeMap<String, Id<zerodds_ast::TypeNode>> {
    ast.types()
        .filter(|(_, node)| !node.name.is_empty())
        .filter_map(|(id, node)| {
            let scoped = ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))?;
            Some((scoped.segments().join("::"), id))
        })
        .collect()
}

fn interface(ast: &Ast, name: &str) -> Id<InterfaceNode> {
    ast.interfaces()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no interface `{name}`"))
}

/// The `pass` operation's parameters, in order, as stub types.
fn pass_params(ast: &Ast) -> Vec<zerodds_corba_codegen::StubParam> {
    let echo = interface(ast, "Echo");
    let node = ast.interface(echo).expect("Echo");
    for id in &node.operations {
        let op = ast.operation(*id).expect("operation");
        if op.name == "pass" {
            return stub_op_of(ast, *id).expect("stub op").params;
        }
    }
    panic!("no operation `pass`");
}

#[test]
fn every_special_type_idl_can_write_is_reached_from_a_lowered_model() {
    let ast = model();
    let params = pass_params(&ast);
    let seen: Vec<Option<SpecialType>> = params
        .iter()
        .map(|param| match param.type_.resolved() {
            zerodds_corba_codegen::StubType::Special(t) => Some(*t),
            _ => None,
        })
        .collect();

    // The order the IDL writes them in. `resolved()` is what a language
    // without a typedef sees, which is where the row is visible: the
    // signature itself writes `probe::AnySeq` and `probe::Money`.
    assert_eq!(
        seen,
        vec![
            Some(SpecialType::Any),
            Some(SpecialType::SequenceOfAny),
            Some(SpecialType::WString),
            Some(SpecialType::Fixed),
            Some(SpecialType::ULongLong),
            Some(SpecialType::LongDouble),
            Some(SpecialType::Object),
            // `native Handle` is a declaration, not an alias: every ORB
            // writes `probe::Handle` and none declares it, so nothing
            // resolves through to the `void*` of the table's row.
            None,
            Some(SpecialType::ValueBase),
            // An `abstract interface` used as a type is written by its
            // own name in all three ORBs, so it is not the
            // `AbstractBase` row.
            None,
            // `TimeBase::TimeT` is a `typedef unsigned long long`; a
            // lowered model cannot tell it from any other one, which is
            // why `special_type_of_scoped` exists and takes the name.
            Some(SpecialType::ULongLong),
        ],
        "the `pass` parameters, in order"
    );
}

#[test]
fn a_typedef_is_written_by_its_own_name_and_not_by_what_it_aliases() {
    // The rule all three ORBs agree on and an earlier version of this
    // crate got wrong: `typedef fixed<10,2> Money` is `probe::Money` in
    // a C++ signature, `java.math.BigDecimal` in a Java one, and its
    // Java holder is `probe.MoneyHolder`.
    let ast = model();
    let params = pass_params(&ast);
    let by_name = |name: &str| {
        params
            .iter()
            .find(|param| param.name == name)
            .map(|param| param.type_.clone())
            .unwrap_or_else(|| panic!("no parameter `{name}`"))
    };

    let money = by_name("m");
    assert_eq!(
        zerodds_lang_render::cpp::type_(&zerodds_corba_codegen::cpp_param_type(
            &money,
            zerodds_ast::ParamDirection::In
        )),
        "const demo::Money&"
    );
    assert_eq!(
        zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_param_type(
            &money,
            zerodds_ast::ParamDirection::In
        )),
        "java.math.BigDecimal"
    );
    assert_eq!(
        zerodds_lang_render::java::type_(&zerodds_corba_codegen::java_holder_type(&money)),
        "demo.MoneyHolder"
    );

    // A `typedef string Name` is the one C++ resolves through, because
    // `string` maps to a pointer and there is nothing to alias.
    let types = types_by_name(&ast);
    let name = types.get("demo::Name").expect("demo::Name");
    let name = zerodds_corba_codegen::stub_type_of(&ast, *name);
    assert_eq!(
        zerodds_lang_render::cpp::type_(&zerodds_corba_codegen::cpp_param_type(
            &name,
            zerodds_ast::ParamDirection::In
        )),
        "const char*"
    );
}

#[test]
fn special_type_of_follows_a_typedef_even_though_the_signature_does_not() {
    // The two questions are different: what a `Name` *is* and what a
    // signature *writes*. `special_type_of` answers the first.
    let ast = model();
    let types = types_by_name(&ast);
    let name = types.get("demo::Name").expect("demo::Name");
    assert_eq!(special_type_of(&ast, *name), Some(SpecialType::String));
    let money = types.get("demo::Money").expect("demo::Money");
    assert_eq!(special_type_of(&ast, *money), Some(SpecialType::Fixed));
}

#[test]
fn an_ordinary_type_is_not_a_special_type() {
    let ast = model();
    let types = types_by_name(&ast);
    for ordinary in ["demo::TwoLongs", "demo::WithString", "demo::Colour"] {
        let id = types.get(ordinary).unwrap_or_else(|| panic!("{ordinary}"));
        assert_eq!(special_type_of(&ast, *id), None, "{ordinary}");
    }
}

#[test]
fn fixed_and_variable_length_are_computed_from_the_members_and_not_the_name() {
    let ast = model();
    let types = types_by_name(&ast);
    let shape = |name: &str| {
        let id = types.get(name).unwrap_or_else(|| panic!("{name}"));
        shape_of(&ast, *id)
    };
    assert_eq!(shape("demo::TwoLongs"), TypeShape::FixedAggregate);
    assert_eq!(shape("demo::WithString"), TypeShape::VariableAggregate);
    // One level down: a struct holding a fixed struct is fixed, and one
    // holding a variable struct is variable. This is the case a rule
    // that looked only at the immediate members would get wrong.
    assert_eq!(shape("demo::Nested"), TypeShape::FixedAggregate);
    assert_eq!(shape("demo::NestedVariable"), TypeShape::VariableAggregate);
    assert_eq!(shape("demo::Colour"), TypeShape::Scalar);
    assert_eq!(shape("demo::Grid"), TypeShape::Array { variable: false });
    assert_eq!(
        shape("demo::VariableGrid"),
        TypeShape::Array { variable: true }
    );
}

#[test]
fn the_return_position_differs_between_a_fixed_and_a_variable_result() {
    let ast = model();
    let echo = interface(&ast, "Echo");
    let node = ast.interface(echo).expect("Echo");
    let mut rendered = BTreeMap::new();
    for id in &node.operations {
        let op = stub_op_of(&ast, *id).expect("stub op");
        rendered.insert(
            op.name.clone(),
            zerodds_lang_render::cpp::type_(&zerodds_corba_codegen::cpp_return_type(
                &op.return_type,
            )),
        );
    }
    assert_eq!(
        rendered.get("make").map(String::as_str),
        Some("demo::WithString*")
    );
    assert_eq!(
        rendered.get("pair").map(String::as_str),
        Some("demo::TwoLongs")
    );
    assert_eq!(rendered.get("echo").map(String::as_str), Some("char*"));
    assert_eq!(rendered.get("announce").map(String::as_str), Some("void"));
}

#[test]
fn a_raises_clause_reaches_the_stub() {
    let ast = model();
    let echo = interface(&ast, "Echo");
    let node = ast.interface(echo).expect("Echo");
    let raises: Vec<Vec<String>> = node
        .operations
        .iter()
        .filter_map(|id| stub_op_of(&ast, *id))
        .filter(|op| op.name == "echo")
        .map(|op| op.raises)
        .collect();
    assert_eq!(raises, vec![vec!["Bad".to_string()]]);
}

#[test]
fn oneway_survives_lowering() {
    let ast = model();
    let echo = interface(&ast, "Echo");
    let node = ast.interface(echo).expect("Echo");
    let oneway: Vec<String> = node
        .operations
        .iter()
        .filter_map(|id| stub_op_of(&ast, *id))
        .filter(|op| op.oneway)
        .map(|op| op.name)
        .collect();
    assert_eq!(oneway, vec!["announce".to_string()]);
}

#[test]
fn the_skeleton_dispatches_attributes_as_the_underscore_operations() {
    let ast = model();
    let echo = interface(&ast, "Echo");
    let ops = skeleton_ops_of(&ast, echo);
    let names: Vec<&str> = ops.iter().map(|op| op.name.as_str()).collect();

    // Every declared operation.
    for expected in ["echo", "announce", "swap", "pass", "make", "pair"] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }
    // A `readonly` attribute contributes a getter and no setter; a
    // read-write one contributes both. §7.4.4.
    assert!(names.contains(&"_get_counter"), "{names:?}");
    assert!(!names.contains(&"_set_counter"), "{names:?}");
    assert!(names.contains(&"_get_label"), "{names:?}");
    assert!(names.contains(&"_set_label"), "{names:?}");
}

#[test]
fn a_dispatch_built_from_the_model_names_every_wire_operation() {
    let ast = model();
    let echo = interface(&ast, "Echo");
    let ops = skeleton_ops_of(&ast, echo);
    for language in TargetLanguage::RENDERABLE {
        let text = zerodds_corba_codegen::render_skeleton_dispatch("EchoImpl", &ops, language)
            .expect("renderable");
        for op in &ops {
            assert!(
                text.contains(&op.name),
                "{language:?} dropped `{}`:\n{text}",
                op.name
            );
        }
    }
}

#[test]
fn a_types_repository_id_is_the_models_and_not_a_second_derivation() {
    let ast = model();
    let types = types_by_name(&ast);
    let id = types.get("demo::WithString").expect("demo::WithString");
    let node = ast.type_(*id).expect("type");
    let resolved = repository_id_of(&ast, zerodds_ast::AnyId::Type(*id, node.node_id))
        .expect("a repository id");
    assert_eq!(resolved.id, "IDL:demo/WithString:1.0");
    assert_eq!(
        resolved.origin,
        Some(zerodds_ast::RepositoryIdOrigin::Derived),
        "the identifier must come from the model, not from this crate"
    );
}

#[test]
fn an_interfaces_repository_id_comes_from_the_model() {
    // This used to assert `origin: None` — the model carried no
    // identifier for an interface, because
    // `zerodds-idl-lower::complete::derive_repository_ids` walked the
    // type arena and nothing else, and `repository_id_of` fell back to
    // the §10.7 default form built from the scoped name.
    //
    // The fallback produced the right string here and the wrong one
    // wherever a prefix applied: `typeprefix`, `#pragma prefix` and
    // `#pragma version` are the front end's, and a form built from a
    // scoped name sees none of them. The front end derives an
    // interface's and an exception's identifier now, so the answer
    // comes from the model and `origin` says so.
    let ast = model();
    let echo = interface(&ast, "Echo");
    let node = ast.interface(echo).expect("Echo");
    let resolved = repository_id_of(&ast, zerodds_ast::AnyId::Interface(echo, node.node_id))
        .expect("a repository id");
    assert_eq!(resolved.id, "IDL:demo/Echo:1.0");
    assert_eq!(
        resolved.origin,
        Some(zerodds_ast::RepositoryIdOrigin::Derived),
        "the identifier must come from the model, not from this crate"
    );
}

#[test]
fn a_prefix_reaches_an_interface_and_an_exception() {
    // The case the fallback got wrong, and the reason the front end
    // derives these two now. omniORB, TAO and JacORB all write
    // `IDL:omg.org/CosNaming/NotFound:1.0` for this file; a repository
    // id built from the scoped name alone writes
    // `IDL:CosNaming/NotFound:1.0`, and an exception whose identifier
    // does not match is an exception the peer's `raises` arm does not
    // recognise.
    let source = "\
typeprefix CosNaming \"omg.org\";
module CosNaming {
  exception NotFound { long why; };
  interface NamingContext { long resolve(in long n) raises (NotFound); };
};
";
    let parsed = zerodds_idl_parse::parse(source);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "prefixed-service.idl");
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("a resolve pass");

    let context = interface(&ast, "NamingContext");
    let node = ast.interface(context).expect("NamingContext");
    let resolved = repository_id_of(&ast, zerodds_ast::AnyId::Interface(context, node.node_id))
        .expect("a repository id");
    assert_eq!(resolved.id, "IDL:omg.org/CosNaming/NamingContext:1.0");

    let (id, node) = ast
        .exceptions()
        .find(|(_, node)| node.name == "NotFound")
        .expect("NotFound");
    let resolved = repository_id_of(&ast, zerodds_ast::AnyId::Exception(id, node.node_id))
        .expect("a repository id");
    assert_eq!(resolved.id, "IDL:omg.org/CosNaming/NotFound:1.0");
}

#[test]
fn a_pragma_prefix_reaches_the_identifier_through_the_model() {
    // The case a second derivation in this crate would get wrong: the
    // prefix is a preprocessor directive the front end applies, and a
    // builder that took only the scoped name would answer
    // `IDL:demo/Prefixed:1.0`.
    let source = "\
#pragma prefix \"omg.org\"
module demo {
    struct Prefixed { long a; };
};
";
    let parsed = zerodds_idl_parse::parse(source);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "prefixed.idl");
    let ast = lowered.ast;
    let types = types_by_name(&ast);
    let id = types.get("demo::Prefixed").expect("demo::Prefixed");
    let node = ast.type_(*id).expect("type");
    let resolved = repository_id_of(&ast, zerodds_ast::AnyId::Type(*id, node.node_id))
        .expect("a repository id");
    assert_eq!(resolved.id, "IDL:omg.org/demo/Prefixed:1.0");
    assert_eq!(
        resolved.origin,
        Some(zerodds_ast::RepositoryIdOrigin::PragmaPrefix)
    );
    // And the crate's own builder, given the same name and no model,
    // answers the unprefixed form — which is why it is the fallback.
    assert_eq!(
        zerodds_corba_codegen::build_repository_id(&["demo"], "Prefixed", 1, 0),
        "IDL:demo/Prefixed:1.0"
    );
}

#[test]
fn the_readme_table_is_the_crates_table() {
    // A table written twice is a table that will disagree with itself.
    // `mapping_table_markdown` produces the README's, so the check is a
    // containment and not a second transcription.
    let readme = include_str!("../README.md");
    let generated = zerodds_corba_codegen::mapping_table_markdown();
    for line in generated.lines() {
        assert!(
            readme.contains(line),
            "README.md is missing the row `{line}`; regenerate it from mapping_table_markdown()"
        );
    }
}
