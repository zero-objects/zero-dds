// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! An interface named where a type is expected. IDL 4.2 §7.4.6.
//!
//! `Ast::resolve_all_by_name` has carried the rule from the start: a
//! pending reference whose waiter wants a type and whose target is an
//! interface is pointed at the derived object-reference type. What it
//! never received was a key it could find the declaration by.
//!
//! `Lowering::type_ref_from_scoped_name` looked the single written
//! segment up in the scope stack, kept the answer only if it was already
//! a type, and otherwise recorded the bare segment as an **absolute**
//! name. A segment written inside `module m` is relative to `m`, so the
//! resolve pass asked the index for `::I` where the declaration is
//! `m::I` and broke the reference with `NotFound`.
//!
//! The shape of the failure is what made it survive: it needs an
//! interface *and* a module *and* the unqualified spelling. At global
//! scope it resolved, written `m::I` it resolved, and any other kind of
//! declaration resolved because the scope lookup already answered a
//! type. Everything the front end's own corpus exercised was on the
//! working side of that line.

use zerodds_ast::{Ast, TypeKind};

fn resolved(source: &str) -> (Ast, zerodds_ast::ResolveReport) {
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "{:?}",
        parsed.diagnostics()
    );
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "interface-as-type.idl").ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    (ast, report)
}

/// Every spelling that reaches an interface where a type is expected.
const CASES: &[(&str, &str)] = &[
    ("global scope", "interface I {}; struct S { I r; };"),
    (
        "inside a module, unqualified",
        "module m { interface I {}; struct S { I r; }; };",
    ),
    (
        "inside a module, qualified",
        "module m { interface I {}; struct S { m::I r; }; };",
    ),
    (
        "an operation parameter",
        "module m { interface I {}; interface E { void f(in I a); }; };",
    ),
    (
        "an abstract interface",
        "module m { abstract interface I {}; interface E { void f(in I a); }; };",
    ),
    (
        "a nested module",
        "module a { module b { interface I {}; interface E { void f(in I x); }; }; };",
    ),
    (
        "a forward declaration, with the body after the use",
        "module m { interface I; struct S { I r; }; interface I {}; };",
    ),
    (
        "a sequence element",
        "module m { interface I {}; typedef sequence<I> Refs; };",
    ),
    (
        "an operation's return type",
        "module m { interface I {}; interface E { I make(); }; };",
    ),
];

#[test]
fn an_interface_used_as_a_type_resolves_in_every_spelling() {
    for (label, source) in CASES {
        let (_, report) = resolved(source);
        assert!(report.broken.is_empty(), "{label}: {:?}", report.broken);
    }
}

#[test]
fn what_it_resolves_to_is_an_object_reference_and_not_the_interface() {
    // The distinction the `AnyId::Interface` arm of `resolve_all_by_name`
    // exists for: `I r;` is a member whose type is a reference to `I`,
    // not the interface itself.
    let (ast, _) = resolved("module m { interface I {}; struct S { I r; }; };");
    let structure = ast
        .types()
        .find(|(_, node)| node.name == "S")
        .map(|(id, _)| id)
        .expect("struct S");
    let TypeKind::Structure { members, .. } = &ast.type_(structure).expect("S").kind else {
        panic!("S is a struct");
    };
    let member = ast.member(members[0]).expect("member r");
    let target = member.type_.resolved().expect("a resolved member type");
    let kind = &ast.type_(target).expect("the member's type").kind;
    let TypeKind::ObjectRef { interface } = kind else {
        panic!("expected an object reference, got {kind:?}");
    };
    let interface = interface
        .as_ref()
        .and_then(zerodds_ast_core::Ref::resolved)
        .expect("the reference names the interface");
    assert_eq!(ast.interface(interface).expect("I").name, "I");
}

#[test]
fn the_key_is_the_declaration_s_own_scoped_name_and_not_the_written_segment() {
    // The mechanism, asserted rather than described: the index holds the
    // interface under `m::I`, so recording the written segment as the
    // absolute name `::I` asks for a key that is not there. This is what
    // made the failure need a module — at global scope the two spellings
    // coincide.
    let (ast, _) = resolved("module m { interface I {}; struct S { I r; }; };");
    let interface = ast
        .all_nodes()
        .find(|any| {
            any.as_interface()
                .and_then(|id| ast.interface(id).ok())
                .is_some_and(|node| node.name == "I")
        })
        .expect("interface I");
    let scoped = ast.scoped_name_of(interface).expect("a scoped name");
    assert_eq!(
        scoped.to_string(),
        "::m::I",
        "the declaration is indexed under its own scope, not under the segment written at the use"
    );
}

#[test]
fn a_name_that_is_not_declared_still_breaks() {
    // The fix must not turn a missing name into a resolved one: the
    // scope lookup has to find something before its scoped name is used
    // as the key.
    let (_, report) = resolved("module m { struct S { Absent r; }; };");
    assert_eq!(report.broken.len(), 1, "{:?}", report.broken);
}
