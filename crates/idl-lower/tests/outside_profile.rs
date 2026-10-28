// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.3.1.1.1: a construct outside the building blocks is
//! ignored or warned about "rather than halting with an error".
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, Profile};
use zerodds_ast_core::ScopedName;
use zerodds_idl_lower::lower;
use zerodds_idl_parse::parse;

fn model(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    // The generators drop after the names are resolved; an interface used
    // as a type is an object reference only from that pass on.
    let mut ast = lower(&parsed.syntax(), "test.idl").ast;
    ast.resolve_all_by_name().expect("the resolve pass");
    ast
}

fn present(ast: &Ast, name: &str) -> bool {
    ast.find_by_scoped_name(&ScopedName::absolute(vec![name.to_string()]))
        .is_ok()
}

fn dropped_names(ast: &mut Ast, profile: Profile) -> Vec<String> {
    let mut names: Vec<String> = ast
        .drop_outside_profile(profile)
        .expect("the removal keeps the model sound")
        .into_iter()
        .map(|dropped| dropped.name)
        .collect();
    names.sort();
    names
}

/// What holds an `any` — directly, through an alias of a sequence of it,
/// or through a member that holds one — leaves the model, and what does
/// not stays, so the rest of the file can be generated.
#[test]
fn what_holds_an_any_is_dropped_and_the_rest_of_the_file_stays() {
    let mut ast = model(
        "struct Plain { long a; };\n\
         struct Holds { any a; };\n\
         typedef sequence<any> Many;\n\
         struct ViaAlias { Many m; };\n\
         struct Deep { Holds h; };\n\
         struct Beside { Plain p; string s; };\n",
    );
    let names = dropped_names(&mut ast, Profile::XTypes13);
    assert_eq!(names, ["::Deep", "::Holds", "::Many", "::ViaAlias"]);
    for gone in ["Deep", "Holds", "Many", "ViaAlias"] {
        assert!(!present(&ast, gone), "{gone} is still in the model");
    }
    for stays in ["Plain", "Beside"] {
        assert!(present(&ast, stays), "{stays} was dropped");
    }
}

/// The report names the construct and the paragraph, so a front end can
/// turn it into a warning that says what was ignored and why.
#[test]
fn the_report_names_the_construct_and_the_clause() {
    let mut ast = model("struct Holds { any a; };\nstruct Plain { long a; };\n");
    let dropped = ast
        .drop_outside_profile(Profile::XTypes13)
        .expect("the removal keeps the model sound");
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].name, "::Holds");
    assert_eq!(dropped[0].construct, "any");
    assert_eq!(dropped[0].spec_reference, "DDS-XTypes 1.3 §7.2.2");
    assert!(dropped[0].detail.contains("any"), "{}", dropped[0].detail);
}

/// `ValueBase`, `native` and an object reference are the other three
/// constructs with no data form.
#[test]
fn native_and_an_object_reference_are_dropped_too() {
    let mut ast = model(
        "native Opaque;\n\
         struct HoldsNative { Opaque o; };\n\
         interface Service { void call(); };\n\
         struct HoldsReference { Service s; };\n\
         struct Plain { long a; };\n",
    );
    let names = dropped_names(&mut ast, Profile::XTypes13);
    assert_eq!(names, ["::HoldsNative", "::HoldsReference"]);
    assert!(present(&ast, "Plain"));
}

/// A type an operation names stays: removing it would leave the
/// operation with a reference to nothing, and the caller would be told
/// about a broken reference instead of the construct.
#[test]
fn a_type_an_operation_names_is_kept() {
    let mut ast = model(
        "struct Holds { any a; };\n\
         interface Service { void call(in Holds h); };\n",
    );
    assert!(dropped_names(&mut ast, Profile::XTypes13).is_empty());
    assert!(present(&ast, "Holds"));
}

/// A profile that admits `any` removes nothing.
#[test]
fn a_profile_that_admits_any_keeps_it() {
    let mut ast = model("struct Holds { any a; };\n");
    assert!(dropped_names(&mut ast, Profile::Corba33).is_empty());
    assert!(present(&ast, "Holds"));
}
