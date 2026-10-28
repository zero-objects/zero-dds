// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Five things the Kotlin and Node columns left out of the generated
//! file and reported success for.
//!
//! # Why these are tests
//!
//! `tools/silent-drop-guard.sh` found all five. Four carried a
//! substitute that a reader of the generated file cannot tell from a
//! value the source declared:
//!
//! - `adapter/kotlin.rs::zero_of` folded an array dimension that does
//!   not evaluate to `0` and built `arrayOf()` for it. The property's
//!   declared type stays `Array<Int>`, so the file compiles; an encoder
//!   reading the sample a bare `S()` produces then writes zero elements
//!   where the declaration states `long a[N]`.
//! - `default_kotlin.rs::point_literal` answered `Char('\0')` for a
//!   `char` member whose `@default` is not a Unicode scalar value, and
//!   `0` for one outside `u32` — the same fold
//!   `surrogate_default_is_not_folded.rs` measured in the C, Java and C#
//!   columns, in a column that test does not reach.
//! - `default_node.rs::explicit` answered `'\0'` for a `@default`
//!   `wchar` literal that is a lone surrogate, under a comment claiming
//!   the surrogate was kept. `Literal::Char` holds a Rust `char`, which
//!   a surrogate is not, so the comment described the opposite of the
//!   code.
//! - `adapter/node.rs::node_constant` did the same for a `const wchar`
//!   declaration, three lines above the `WString` arm that refuses the
//!   plural case by name.
//!
//! The fifth is a walk that asked the model a narrower question than the
//! declaration it was writing: `adapter/kotlin.rs` built an interface's
//! class from `Ast::operations_of`, which answers what an interface
//! *declares*, while writing no supertype — so `interface B : A` came
//! out as `class B(val g: () -> Unit)` with `A`'s `f` nowhere in the
//! file. The structure arm three arms up already takes
//! `Ast::all_members_of`, the inherited members included, for exactly
//! that reason.
//!
//! `1/0` is the shortest expression the frontend accepts and the
//! evaluator rejects — `unevaluated.rs` uses it for the same reason.
//! `0xD800` is the boundary a Rust `char` and a UTF-16 code unit
//! disagree on.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_ast::{Ast, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Ident, Path};

/// IDL to a resolved model, refusing to measure anything the frontend
/// already rejected.
fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower, so the frontend refused it after all \
         and this test measures the wrong thing: {:?}",
        lowered.diagnostics
    );
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the model resolves");
    ast
}

/// The member `name` of the structure `S`, with the type it is declared
/// with.
fn member_of(ast: &Ast, name: &str) -> (MemberNode, Id<TypeNode>) {
    let (root, _) = ast
        .types()
        .find(|(_, node)| node.name.as_str() == "S")
        .expect("the structure S");
    let node = ast.type_(root).expect("the structure node");
    let TypeKind::Structure { members, .. } = &node.kind else {
        panic!("S is a structure");
    };
    for member in members {
        let member_node = ast.member(*member).expect("a member");
        if member_node.name.as_str() == name {
            let type_ = member_node.type_.resolved().expect("the member's type");
            return (member_node.clone(), type_);
        }
    }
    panic!("S declares `{name}`");
}

/// An array dimension that does not evaluate is refused, not built as an
/// empty array.
///
/// The path is the *materialiser*, not the codec: the Kotlin data class
/// states a resting value for every primary-constructor property, and
/// that value is what an encoder reads out of a sample nobody assigned
/// to. `arrayOf()` is a legal `Array<Int>`, so the substitution reached
/// the wire with exit 0 and no diagnostic.
#[test]
fn a_kotlin_array_dimension_that_does_not_evaluate_is_refused() {
    let ast = model("struct S { long a[1/0]; };");
    let graph = zerodds_lang_rules::ModelGraph::of(&ast);
    let applied = zerodds_lang_rules::apply(&graph, &zerodds_lang_rules::rule::kotlin::catalogue())
        .expect("the Kotlin rules apply");
    let error = zerodds_lang_rules::adapter::kotlin::kotlin_file(&ast, &applied)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_else(|| {
            panic!(
                "the Kotlin column rendered `long a[1/0]` and said nothing; \
                 the resting value it built for the property holds no elements"
            )
        });
    assert!(
        error.contains("array length"),
        "unexpected refusal: {error}"
    );
}

/// A `@default` a Kotlin `Char` cannot hold is refused, not written as
/// NUL.
///
/// An IDL `char` is one octet, so the only route to a point outside the
/// Unicode scalar values is an out-of-range `@default` on one — which
/// the frontend accepts, measured here by the assertion in [`model`].
#[test]
fn a_kotlin_char_default_that_is_not_a_scalar_value_is_refused() {
    let ast = model("struct S { @default(0xD800) char c; };");
    let (member, type_) = member_of(&ast, "c");
    let answer = zerodds_lang_rules::default_kotlin::member_default(
        &ast,
        &member,
        type_,
        zerodds_lang_cst::kotlin::Expr::Path(Path::new(&["v"])),
    );
    let error = match answer {
        Ok(statements) => panic!(
            "the Kotlin column took `@default(0xD800)` on a `char` and emitted \
             {statements:?}; U+D800 is not a Unicode scalar value and the \
             substitute is indistinguishable from a declared `'\\0'`"
        ),
        Err(error) => format!("{error:?}"),
    };
    assert!(
        error.contains("not a Unicode scalar value"),
        "unexpected refusal: {error}"
    );
}

/// The same for a `@default` `wchar` literal in the Node column.
#[test]
fn a_node_wchar_default_that_is_a_lone_surrogate_is_refused() {
    let ast = model("struct S { @default(L'\\uD800') wchar c; };");
    let (member, type_) = member_of(&ast, "c");
    let answer = zerodds_lang_rules::default_node::member_default(
        &ast,
        &member,
        type_,
        zerodds_lang_cst::node::Expr::Name(Ident::new("v")),
    );
    let error = match answer {
        Ok(statements) => panic!(
            "the Node column took `@default(L'\\uD800')` and emitted \
             {statements:?}; `Literal::Char` holds a Rust `char`, which a lone \
             surrogate is not, so the value written is not the one declared"
        ),
        Err(error) => format!("{error:?}"),
    };
    assert!(
        error.contains("not a Unicode scalar value"),
        "unexpected refusal: {error}"
    );
}

/// A type handle the model does not hold costs no type its defaulter.
///
/// `node_defaulters_over` takes the handles from its caller, so a stale
/// one is reachable through the public entry point. `value_of` answered
/// `Ok(None)` for it, which the loop reads as "this type has no Node
/// data form" and skips — and the generated codec then calls
/// `zd_def_unresolved`, a name nothing in the file binds. Measured
/// against the parent revision: the same call answered `Ok([])`, an
/// empty statement list and exit 0.
#[test]
fn a_type_the_model_does_not_hold_refuses_rather_than_going_missing() {
    let ast = model("struct S { long a; };");
    // Past the end of the type arena: the slot cannot be occupied, so
    // the handle is stale by construction rather than by a removal the
    // public API does not offer.
    let stale: Id<TypeNode> = Id::from_parts(9999, 1).expect("a handle with a live generation");
    let answer = zerodds_lang_rules::default_node::node_defaulters_over(&ast, &[stale]);
    let error = match answer {
        Ok(statements) => panic!(
            "the Node column answered {statements:?} for a type handle the \
             model does not hold; a defaulter that is never emitted is one \
             the generated codec calls by a name nothing binds"
        ),
        Err(error) => format!("{error:?}"),
    };
    assert!(
        error.contains("a type the model does not hold"),
        "unexpected refusal: {error}"
    );
}

/// An inherited operation reaches the class the Kotlin column writes.
///
/// The class carries one property per operation and names no supertype,
/// so an operation the walk does not collect is one the generated file
/// does not have — and `class B(val g: () -> Unit)` compiles, which is
/// what made the omission silent.
#[test]
fn a_kotlin_interface_carries_the_operations_it_inherits() {
    let ast = model("interface A { void f(); }; interface B : A { void g(); };");
    let graph = zerodds_lang_rules::ModelGraph::of(&ast);
    let applied = zerodds_lang_rules::apply(&graph, &zerodds_lang_rules::rule::kotlin::catalogue())
        .expect("the Kotlin rules apply");
    let file = zerodds_lang_rules::adapter::kotlin::kotlin_file(&ast, &applied)
        .expect("the file materialises");
    let rendered = zerodds_lang_render::kotlin::file(&file);
    let class_b = rendered
        .split("class B(")
        .nth(1)
        .unwrap_or_else(|| panic!("the file declares `class B`:\n{rendered}"));
    // `\n)` and not `)`: a property's type is `() -> Unit`, whose own
    // parentheses come first.
    let body = class_b
        .split_once("\n)")
        .map(|(body, _)| body)
        .unwrap_or_else(|| panic!("`class B(` is never closed:\n{rendered}"));
    assert!(
        body.contains("val f:"),
        "`class B` is {body:?}; `B` inherits `f` from `A` and this column \
         writes no supertype, so an operation missing from the property \
         list is an operation the generated file does not have"
    );
    assert!(
        body.contains("val g:"),
        "`class B` is {body:?} and lost its own operation"
    );
}

/// And for a `const wchar` declaration, which the Node column writes as
/// a one-character string.
#[test]
fn a_node_wchar_constant_that_is_a_lone_surrogate_is_refused() {
    let ast = model("const wchar C = L'\\uD800';");
    let answer = zerodds_lang_rules::adapter::node::node_items(&ast);
    let error = match answer {
        Ok(items) => panic!(
            "the Node column declared `const wchar C = L'\\uD800'` as {items:?}; \
             the `WString` arm beside it refuses a lone surrogate by name and \
             this one substituted NUL"
        ),
        Err(error) => format!("{error:?}"),
    };
    assert!(
        error.contains("not a Unicode scalar value"),
        "unexpected refusal: {error}"
    );
}
