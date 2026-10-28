// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! `from_ast::dependency_type_objects`: the closure a type's object
//! names by hash, so that an endpoint can register what its own
//! identity resolves through.
//!
//! Measured before this existed, on the OMG `dds-xtypes` suite (25
//! compiled-in types): every pair with an inherited `Shape5*` on one
//! side communicated regardless of extensibility — 55 of 625 cells —
//! because the generated type registered its own object and not its
//! base's, the local registry could not resolve the type's own closure,
//! and the match site treated an unresolvable pair as matched.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::Ast;
use zerodds_types::from_ast::{
    Form, complete_type_object, dependency_type_objects, type_identifier,
};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::{CompleteTypeObject, TypeIdentifier, TypeObject, compute_hash};

const IDL: &str = r#"
module dep {
    enum Color { RED, GREEN };
    typedef long Meters;
    struct Inner {
        Color c;
        string<8> s;
        sequence<long> q;
    };
    struct Base {
        long a;
        Meters m;
    };
    struct Derived : Base {
        float angle;
        Inner nested;
    };
    @final struct Plain {
        long x;
        string<4> y;
    };
};
"#;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "dep.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_of(ast: &Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    let segments: Vec<String> = name.split("::").map(ToString::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{name} is not in the model"))
}

fn name_of(object: &CompleteTypeObject) -> String {
    match object {
        CompleteTypeObject::Struct(s) => s.header.detail.type_name.clone(),
        CompleteTypeObject::Enumerated(e) => e.header.detail.type_name.clone(),
        CompleteTypeObject::Alias(a) => a.header.detail.type_name.clone(),
        other => format!("{other:?}"),
    }
}

/// The closure of `Derived` is its base, the member aggregate, that
/// aggregate's enumeration and the base's alias — the hashed types and
/// nothing fully descriptive (§7.3.4.6) — and not `Derived` itself.
#[test]
fn the_closure_is_every_hashed_type_the_object_reaches() {
    let ast = model(IDL);
    let derived = type_of(&ast, "dep::Derived");
    let objects = dependency_type_objects(&ast, derived).expect("closure");
    let mut names: Vec<String> = objects.iter().map(name_of).collect();
    names.sort();
    assert_eq!(
        names,
        ["dep::Base", "dep::Color", "dep::Inner", "dep::Meters"],
        "the closure of Derived"
    );
}

/// A type that names nothing by hash has an empty closure.
#[test]
fn a_type_of_fully_descriptive_members_has_no_dependencies() {
    let ast = model(IDL);
    let plain = type_of(&ast, "dep::Plain");
    assert!(
        dependency_type_objects(&ast, plain)
            .expect("closure")
            .is_empty()
    );
}

/// Why the closure has to be registered: the type's own object, alone
/// in a registry, does not resolve; with the closure it does.
#[test]
fn the_closure_is_what_makes_the_types_own_identity_resolvable() {
    let ast = model(IDL);
    let derived = type_of(&ast, "dep::Derived");
    let own = complete_type_object(&ast, derived).expect("object");
    let own_hash = compute_hash(&TypeObject::Complete(own.clone())).expect("hash");
    let id = type_identifier(&ast, derived, Form::Complete).expect("identifier");
    assert_eq!(id, TypeIdentifier::EquivalenceHashComplete(own_hash));

    let mut registry = TypeRegistry::new();
    registry.insert_complete(own_hash, own);
    registry.project_completes_to_minimal();
    assert!(
        registry.first_unresolved_dependency(&id, 1024).is_some(),
        "the object alone must leave its closure unresolved"
    );

    for object in dependency_type_objects(&ast, derived).expect("closure") {
        let hash = compute_hash(&TypeObject::Complete(object.clone())).expect("hash");
        registry.insert_complete(hash, object);
    }
    registry.project_completes_to_minimal();
    assert!(
        registry.first_unresolved_dependency(&id, 1024).is_none(),
        "with the closure registered nothing is left unresolved"
    );
}
