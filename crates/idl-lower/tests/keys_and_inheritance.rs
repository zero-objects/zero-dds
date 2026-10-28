// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Keys under inheritance, and the key directives OpenSplice, Cyclone DDS,
//! RTI and OpenDDS write.
//!
//! DDS-XTypes 1.3 §7.2.2.4.5 lets a structure derive from another when its
//! extensibility is unspecified or the base's, when no member repeats a name
//! or member id of the base chain, and when it defines no key fields. None of
//! the three was checked. The vendor rows below were measured on 2026-09-29
//! with Cyclone DDS 11.0.1 `idlc` and OpenSplice 6.9 `idlpp`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{AnyId, Ast, Extensibility};
use zerodds_ast_core::ScopedName;
use zerodds_idl_lower::{Lowered, lower};
use zerodds_idl_parse::parse;

fn lowered(source: &str) -> Lowered {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    lower(&parsed.syntax(), "test.idl")
}

/// The errors a source lowers with, as `code message`.
fn errors(source: &str) -> Vec<String> {
    lowered(source)
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{} {}", d.code.as_str(), d.message))
        .collect()
}

fn scoped(path: &str) -> ScopedName {
    ScopedName::absolute(path.split("::").map(ToString::to_string).collect())
}

/// The key members of a structure, declared and inherited, by name.
fn keys_of(ast: &Ast, path: &str) -> Vec<String> {
    let Ok(AnyId::Type(id, _)) = ast.find_by_scoped_name(&scoped(path)) else {
        panic!("{path} is not a type");
    };
    ast.all_members_of(id)
        .iter()
        .filter_map(|m| {
            let node = ast.member(*m).ok()?;
            let is_key = ast
                .facets(AnyId::Member(*m, node.node_id))
                .and_then(|f| f.xtypes.as_ref())
                .is_some_and(|x| x.is_key);
            is_key.then(|| node.name.clone())
        })
        .collect()
}

fn keys_after(source: &str, path: &str) -> Vec<String> {
    let out = lowered(source);
    let errors: Vec<_> = out.diagnostics.iter().filter(|d| d.is_error()).collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    keys_of(&out.ast, path)
}

fn has_error(errors: &[String], code: &str, fragment: &str) -> bool {
    errors
        .iter()
        .any(|e| e.starts_with(code) && e.contains(fragment))
}

// -- §7.2.2.4.5: extensibility, names, ids ---------------------------------

#[test]
fn a_derived_structure_with_another_extensibility_than_its_base_is_refused() {
    let found =
        errors("@final struct Base { long a; }; @appendable struct Derived : Base { long b; };");
    assert!(
        has_error(&found, "IDL0021", "`Derived` is `@appendable`"),
        "{found:?}"
    );
}

#[test]
fn a_derived_structure_that_repeats_its_bases_extensibility_is_accepted() {
    let found =
        errors("@mutable struct Base { long a; }; @mutable struct Derived : Base { long b; };");
    assert!(found.is_empty(), "{found:?}");
    let out = lowered("@mutable struct Base { long a; }; struct Derived : Base { long b; };");
    let Ok(AnyId::Type(id, _)) = out.ast.find_by_scoped_name(&scoped("Derived")) else {
        panic!("Derived");
    };
    assert_eq!(out.ast.effective_extensibility(id), Extensibility::Mutable);
}

#[test]
fn a_member_name_of_the_base_chain_cannot_be_declared_again() {
    let found = errors(
        "struct Root { long a; }; struct Mid : Root { long b; }; struct Leaf : Mid { long A; };",
    );
    assert!(
        has_error(
            &found,
            "IDL0008",
            "`A` is declared in `Leaf` and inherited from `Mid`"
        ),
        "{found:?}"
    );
}

#[test]
fn a_member_id_of_the_base_chain_cannot_be_given_again() {
    let found = errors(
        "@mutable struct Base { @id(5) long a; }; @mutable struct Derived : Base { @id(5) long b; };",
    );
    assert!(has_error(&found, "IDL0016", "member id 5"), "{found:?}");
}

// -- §7.2.2.4.5: keys are the root base's ----------------------------------

#[test]
fn a_key_written_in_a_derived_structure_is_refused() {
    // Cyclone 11.0.1: "Derived struct is not allowed to expand on keys".
    let found = errors("struct Base { @key long id; }; struct Derived : Base { @key long w; };");
    assert!(
        has_error(&found, "IDL0021", "`w` is a key of `Derived`"),
        "{found:?}"
    );
}

#[test]
fn a_keylist_that_gives_a_derived_structure_its_own_key_is_refused() {
    // Cyclone 11.0.1 accepts this one through `#pragma keylist` (keys
    // `parent.id, w`) while it refuses the same key written as `@key`.
    let found = errors(
        "struct Base { long id; };\n#pragma keylist Base id\n\
         struct Derived : Base { long w; };\n#pragma keylist Derived w\n",
    );
    assert!(
        has_error(&found, "IDL0021", "`w` is a key of `Derived`"),
        "{found:?}"
    );
}

#[test]
fn a_keylist_on_a_derived_structure_may_restate_its_bases_keys() {
    let source = "struct Base { long id; long v; };\n#pragma keylist Base id\n\
                  struct Derived : Base { long w; };\n#pragma keylist Derived id\n";
    assert_eq!(keys_after(source, "Base"), ["id"]);
    assert_eq!(keys_after(source, "Derived"), ["id"]);
}

#[test]
fn a_keylist_on_a_derived_structure_cannot_make_a_base_member_a_key() {
    // Cyclone 11.0.1 accepts it and keys `Derived` by `parent.id` while
    // `Base` stays keyless: the derived structure defining a key.
    let found = errors(
        "struct Base { long id; long v; };\nstruct Derived : Base { long w; };\n\
         #pragma keylist Derived id\n",
    );
    assert!(
        has_error(&found, "IDL0021", "the key belongs on `Base`"),
        "{found:?}"
    );
}

// -- the directive's own syntax ---------------------------------------------

#[test]
fn a_keylist_separates_its_members_by_commas_as_well() {
    // OpenSplice 6.9 keeps `"a,b"`, Cyclone 11.0.1 keys `a, b`.
    let source = "struct S { long a; long b; long c; };\n#pragma keylist S a, b\n";
    assert_eq!(keys_after(source, "S"), ["a", "b"]);
    let source = "struct S { long a; long b; long c; };\n#pragma keylist S a,b\n";
    assert_eq!(keys_after(source, "S"), ["a", "b"]);
}

#[test]
fn a_keylist_inside_a_module_names_the_modules_type() {
    // The placement OpenSplice's `idlpp` requires; Cyclone accepts it too.
    let source = "module M {\n  struct T { long k; long v; };\n  #pragma keylist T k\n};\n";
    assert_eq!(keys_after(source, "M::T"), ["k"]);
}

#[test]
fn a_keylist_that_names_nothing_says_where_it_stands() {
    let out = lowered("struct S { long a; };\n#pragma keylist S missing\n");
    let diagnostic = out
        .diagnostics
        .iter()
        .find(|d| d.code.as_str() == "IDL0010")
        .expect("an unresolved-name diagnostic");
    assert!(
        diagnostic.primary.start > 0,
        "the diagnostic points at the start of the file: {diagnostic:?}"
    );
}

// -- paths ------------------------------------------------------------------

#[test]
fn a_keylist_path_keys_the_member_of_the_nested_structure() {
    // OpenSplice 6.9 keeps `"nested.b"` as the key list.
    let source = "struct Inner { long a; long b; };\n\
                  struct Outer { Inner nested; long x; };\n#pragma keylist Outer nested.b\n";
    assert_eq!(keys_after(source, "Outer"), ["nested"]);
    assert_eq!(keys_after(source, "Inner"), ["b"]);
}

#[test]
fn paths_of_one_directive_into_one_member_key_the_nested_structure_together() {
    let source = "struct Inner { long a; long b; long c; };\n\
                  struct Outer { Inner nested; long x; };\n#pragma keylist Outer nested.a nested.c\n";
    assert_eq!(keys_after(source, "Inner"), ["a", "c"]);
}

#[test]
fn a_path_cannot_key_a_structure_that_keys_itself_differently() {
    let found = errors(
        "struct Inner { long a; long b; };\n#pragma keylist Inner a\n\
         struct Outer { Inner nested; long x; };\n#pragma keylist Outer nested.b\n",
    );
    assert!(
        has_error(&found, "IDL0013", "`Inner` keys itself by a"),
        "{found:?}"
    );
}

#[test]
fn two_paths_cannot_key_one_structure_by_different_members() {
    let found = errors(
        "struct Inner { long a; long b; };\n\
         struct One { Inner n; };\n#pragma keylist One n.a\n\
         struct Two { Inner n; };\n#pragma keylist Two n.b\n",
    );
    assert!(
        has_error(&found, "IDL0013", "another path keys `Inner`"),
        "{found:?}"
    );
}

#[test]
fn a_path_cannot_shrink_the_key_of_a_member_that_keys_by_the_whole_structure() {
    let found = errors(
        "struct Inner { long a; long b; };\n\
         struct Whole { @key Inner all; };\n\
         struct Outer { Inner nested; };\n#pragma keylist Outer nested.b\n",
    );
    assert!(
        has_error(&found, "IDL0013", "keys by the whole `Inner`"),
        "{found:?}"
    );
}

#[test]
fn a_path_through_a_member_that_is_no_structure_is_refused() {
    let found = errors("struct S { long a; };\n#pragma keylist S a.b\n");
    assert!(
        has_error(&found, "IDL0006", "is not a structure"),
        "{found:?}"
    );
}

#[test]
fn an_opendds_key_path_keys_the_nested_member_and_an_element_key_is_refused() {
    let source = "struct Inner { long a; long b; };\n\
                  struct Outer { Inner nested; long arr[2]; };\n\
                  #pragma DCPS_DATA_KEY Outer nested.b\n";
    assert_eq!(keys_after(source, "Inner"), ["b"]);
    let found = errors("struct Outer { long arr[2]; };\n#pragma DCPS_DATA_KEY Outer arr[0]\n");
    assert!(
        has_error(&found, "IDL0006", "names an element of a collection"),
        "{found:?}"
    );
}

// -- the module a pragma stands in ------------------------------------------

#[test]
fn a_pragma_prefix_inside_a_module_prefixes_that_module_only() {
    let out = lowered(
        "module A {\n#pragma prefix \"a.example\"\nstruct S { long x; };\n};\n\
         module B { struct S { long x; }; };\n",
    );
    let id = |path: &str| {
        let any = out.ast.find_by_scoped_name(&scoped(path)).expect(path);
        out.ast
            .facets(any)
            .and_then(|f| f.corba.as_ref())
            .and_then(|c| c.repository_id.as_ref())
            .map(|id| id.0.clone())
    };
    assert_eq!(id("A::S").as_deref(), Some("IDL:a.example/A/S:1.0"));
    assert_eq!(id("B::S").as_deref(), Some("IDL:B/S:1.0"));
}

#[test]
fn a_pragma_id_inside_a_module_names_the_modules_declaration() {
    let out = lowered("module M {\nstruct S { long x; };\n#pragma ID S \"IDL:custom/S:2.0\"\n};\n");
    let any = out.ast.find_by_scoped_name(&scoped("M::S")).expect("M::S");
    let id = out
        .ast
        .facets(any)
        .and_then(|f| f.corba.as_ref())
        .and_then(|c| c.repository_id.as_ref())
        .map(|id| id.0.clone());
    assert_eq!(id.as_deref(), Some("IDL:custom/S:2.0"));
}
