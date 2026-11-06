// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-typescript-gen` with `zerodds-wire-probe`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-probe`:
//! - `src/dump.rs`: `recursion::recursive_aggregates` to refuse a recursive
//!   type by name before a dump function is emitted for it.
//!
//! So the crate relies on:
//! - the aggregates on a cycle are exactly the recursive ones, the types
//!   only holding one are not, and a recursion the IDL forbids never
//!   reaches it.
//!
//! Oracles:
//! OMG IDL 4.2 §7.4.1.4.4.4.4 (a recursive type needs a sequence, a map,
//! an `@optional` or an `@external` member).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_wire_probe::encode::model_from_idl;
use zerodds_wire_probe::recursion::recursive_aggregates;

fn type_in(ast: &Ast, name: &[&str]) -> Id<TypeNode> {
    ast.find_by_scoped_name(&ScopedName::absolute(
        name.iter().map(|segment| (*segment).to_string()).collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the type is in the model")
}

#[test]
fn idl_7_4_1_4_4_4_4_the_aggregates_on_a_cycle_are_the_recursive_ones() {
    // A type that reaches itself through a sequence, two that reach each
    // other, and a holder of both that is on no cycle itself.
    let idl = "module m {\n\
               @appendable struct Node { long value; sequence<Node> children; };\n\
               struct B;\n\
               @appendable struct A { long value; sequence<B> bs; };\n\
               @appendable struct B { long weight; sequence<A> as; };\n\
               @final struct Leaf { long v; };\n\
               @final struct Holder { Node n; Leaf l; };\n\
               };";
    let ast = model_from_idl(idl, "recursion.idl").unwrap();
    let node = type_in(&ast, &["m", "Node"]);
    let a = type_in(&ast, &["m", "A"]);
    let b = type_in(&ast, &["m", "B"]);
    let leaf = type_in(&ast, &["m", "Leaf"]);
    let holder = type_in(&ast, &["m", "Holder"]);
    assert_eq!(recursive_aggregates(&ast, node), vec![node]);
    let pair = recursive_aggregates(&ast, a);
    assert!(
        pair.len() == 2 && pair.contains(&a) && pair.contains(&b),
        "both ends of a mutual recursion"
    );
    assert_eq!(
        recursive_aggregates(&ast, holder),
        vec![node],
        "the holder is on no cycle"
    );
    assert!(recursive_aggregates(&ast, leaf).is_empty());
}

#[test]
fn idl_7_4_1_4_4_4_4_a_recursion_without_a_collection_is_no_model() {
    // An array is no indirection: the front end refuses the file, so a
    // generator never sees such a type.
    let idl = "module m { struct Node; @appendable struct Node { long value; Node pair[2]; }; };";
    let error = format!("{}", model_from_idl(idl, "array.idl").err().unwrap());
    assert!(error.contains("7.4.1.4.4.4.4"), "{error}");
}
