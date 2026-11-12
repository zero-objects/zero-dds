// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-idlc` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-idl-compose` (`src/main.rs`, `compose`;
//! the `*_support.rs` files): `compose_source_with` over a unit's text with a
//! `PathLoader` over `-I`, the dialect of `--strict` (`Dialect::STRICT`) or
//! the default (`ComposeOptions::default().dialect`, every vendor), the
//! `--default-extensibility` and the `--dialect` axes; the diagnostics,
//! which `check` prints and which fail every subcommand; `PathLoader::opened`,
//! which becomes the depfile; `compose_source` with `PathLoader::bare()` for
//! the type objects the support files embed (`Composed::registry`).
//!
//! Oracle: zerodds-idl-frontend-1.0 §3.5 (a vendor extension is admitted
//! only by its flag; the strict default refuses it), OpenDDS's
//! `#pragma keylist <type> <member>` (the vendor extension), IDL 4.2 §7.3
//! (an `#include` searched in the include directories, ISO C §6.10.2),
//! §7.5.2 (a declaration that carries the name of the scope it sits in) and
//! DDS-XTypes 1.3 §7.3.4.5 with Annex B (the COMPLETE hash of `to::Plain`
//! that CycloneDDS 11.0.1 and RTI Connext 7.7.0 report,
//! `crates/types/tests/typeobject_vendor_dheader.rs`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_idl_compose::{
    ComposeOptions, NameOfTheEnclosingScope, PathLoader, compose_source, compose_source_with,
};
use zerodds_idl_parse::{Dialect, Severity};
use zerodds_types::EquivalenceHash;

const KEYLIST: &str = "struct Sample { long id; long value; };\n#pragma keylist Sample id\n";

#[test]
fn a_vendor_pragma_is_an_error_under_strict_and_reaches_the_model_under_every_vendor() {
    let strict = compose_source_with(
        "k.idl",
        KEYLIST,
        &PathLoader::bare(),
        ComposeOptions {
            dialect: Dialect::STRICT,
            ..ComposeOptions::default()
        },
    );
    assert!(strict.failed(), "--strict refuses the OpenDDS extension");
    let vendors = compose_source_with(
        "k.idl",
        KEYLIST,
        &PathLoader::bare(),
        ComposeOptions::default(),
    );
    assert!(!vendors.failed(), "{:?}", vendors.diagnostics);
    let (sample, _) = vendors
        .ast
        .types()
        .find(|(_, n)| n.name == "Sample")
        .unwrap();
    let keyed: Vec<String> = vendors
        .ast
        .members_of(sample)
        .iter()
        .filter_map(|m| vendors.ast.member(*m).ok())
        .filter(|m| m.facets.is_key())
        .map(|m| m.name.clone())
        .collect();
    assert_eq!(keyed, ["id"], "the keylist names `id` the key");
}

#[test]
fn idl_7_3_an_include_is_found_in_an_include_directory_and_goes_into_the_depfile() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("inc.idl"), "struct Included { long a; };").unwrap();
    let loader = PathLoader::new(vec![dir.path().to_path_buf()]);
    let composed = compose_source_with(
        "/nowhere/main.idl",
        "#include \"inc.idl\"\nstruct Main { Included i; };",
        &loader,
        ComposeOptions::default(),
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    assert_eq!(loader.opened().len(), 1);
    assert!(loader.opened()[0].ends_with("inc.idl"));
}

#[test]
fn idl_7_3_an_error_in_an_include_names_the_include() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("bad.idl"), "struct Broken { long ; };").unwrap();
    let composed = compose_source_with(
        "/nowhere/main.idl",
        "#include \"bad.idl\"\nstruct Main { long a; };",
        &PathLoader::new(vec![dir.path().to_path_buf()]),
        ComposeOptions::default(),
    );
    let error = composed
        .diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
        .expect("the broken member is reported");
    assert!(
        error
            .file
            .as_deref()
            .is_some_and(|file| file.ends_with("bad.idl")),
        "{error:?}"
    );
}

#[test]
fn idl_7_5_2_a_declaration_named_as_its_scope_follows_the_axis() {
    let source = "module m { struct m { long a; }; };";
    let refused = compose_source_with(
        "s.idl",
        source,
        &PathLoader::bare(),
        ComposeOptions {
            name_of_the_enclosing_scope: NameOfTheEnclosingScope::Refused,
            ..ComposeOptions::default()
        },
    );
    assert!(refused.failed());
    let admitted = compose_source_with(
        "s.idl",
        source,
        &PathLoader::bare(),
        ComposeOptions {
            name_of_the_enclosing_scope: NameOfTheEnclosingScope::Admitted,
            ..ComposeOptions::default()
        },
    );
    assert!(!admitted.failed(), "{:?}", admitted.diagnostics);
}

#[test]
fn xtypes_annex_b_the_registry_holds_the_complete_object_under_the_vendors_hash() {
    let composed = compose_source(
        "plain.idl",
        "module to { @final struct Plain { long a; short b; }; };",
        &PathLoader::bare(),
    );
    let hash = EquivalenceHash([
        0x7c, 0x0c, 0x6f, 0xab, 0x7d, 0x06, 0x6a, 0x0c, 0x56, 0xb8, 0xe9, 0x3c, 0x0e, 0xa7,
    ]);
    assert!(
        composed.registry.get_complete(&hash).is_some(),
        "CycloneDDS and RTI hash the COMPLETE object of to::Plain to 7c0c6fab7d066a0c56b8e93c0ea7"
    );
}

/// The dialect of `zerodds-idlc --case-sensitive` (GitHub #66).
fn case_sensitive() -> ComposeOptions {
    let mut options = ComposeOptions::default();
    options.dialect.case_sensitive = true;
    options
}

/// OMG IDL 4.2 §7.2.3 has `module shape` and `struct Shape` collide in one
/// scope. Under the case-sensitive dialect both stand, and `shape::Details`
/// is the type of the module, as Cyclone DDS reads the file with
/// `idlc -f case-sensitive`.
#[test]
fn idl_7_2_3_two_names_that_differ_in_case_stand_only_under_the_case_sensitive_dialect() {
    let source = "module shapes { module shape { struct Details { long value; }; }; \
                  struct Shape { shape::Details details; }; };";
    let refused = compose_source("s.idl", source, &PathLoader::bare());
    assert!(refused.failed(), "the default is OMG IDL 4.2 §7.2.3");

    let admitted = compose_source_with("s.idl", source, &PathLoader::bare(), case_sensitive());
    assert!(!admitted.failed(), "{:?}", admitted.diagnostics);
    let (_, shape) = admitted
        .ast
        .types()
        .find(|(_, node)| node.name == "Shape")
        .expect("the struct Shape");
    let zerodds_ast::TypeKind::Structure { members, .. } = &shape.kind else {
        panic!("Shape is a structure: {:?}", shape.kind);
    };
    let member = admitted.ast.member(members[0]).expect("the member");
    let details = member.type_.resolved().expect("a resolved member type");
    assert_eq!(
        admitted.ast.type_(details).expect("the type").name,
        "Details"
    );
}

/// The other places §7.2.3 and §7.2.4 compare without regard to case: an
/// aggregate's name against its members, two members, a member against a
/// type used in its scope, two declarations of a module, a parameter
/// against its type, and an identifier against a keyword.
#[test]
fn idl_7_2_3_every_comparison_is_exact_under_the_case_sensitive_dialect() {
    for source in [
        "struct A { long a; };",
        "struct S { long x; long X; };",
        "struct Inner { long n; }; struct Outer { Inner inner; long other; };",
        "module m { struct Foo { long n; }; typedef long foo; };",
        "module m { typedef long M; };",
        "union U switch (long) { case 1: long v; case 2: short V; };",
        "typedef long Foo; interface I { void doit(in Foo foo); void Doit(); };",
        "struct Base { long id; }; struct Derived : Base { long ID; };",
        "struct Struct { long Long; };",
    ] {
        let refused = compose_source("s.idl", source, &PathLoader::bare());
        assert!(refused.failed(), "refused by default: {source}");
        let admitted = compose_source_with("s.idl", source, &PathLoader::bare(), case_sensitive());
        assert!(!admitted.failed(), "{source}: {:?}", admitted.diagnostics);
    }
}

/// What stays an error under the case-sensitive dialect: one spelling
/// declared twice, and a keyword in its own spelling.
#[test]
fn idl_7_2_3_one_spelling_twice_is_refused_under_the_case_sensitive_dialect() {
    for source in [
        "struct S { long n; }; struct S { long m; };",
        "struct S { long x; long x; };",
        "module m { typedef long m; };",
        "interface I { attribute long level; void level(); };",
        "struct struct { long n; };",
    ] {
        let refused = compose_source_with("s.idl", source, &PathLoader::bare(), case_sensitive());
        assert!(refused.failed(), "{source}");
    }
}
