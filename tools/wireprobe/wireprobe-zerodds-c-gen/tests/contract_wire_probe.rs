// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-c-gen` with `zerodds-wire-probe`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-probe`:
//! - `src/dump.rs`: `recursion::recursive_aggregates` to refuse a recursive
//!   type by name before a dump function is emitted for it;
//! - `src/main.rs` and the matrix driver: `encode::model_from_idl` for the
//!   model and `canonical::{canonicalise, text}` for the canonical form the
//!   C column's read direction is compared in.
//!
//! So the crate relies on:
//! - the aggregates on a cycle are exactly the recursive ones, the types
//!   only holding one are not, and a recursion the IDL forbids never
//!   reaches it;
//! - the canonical text is the one form of a value: keys sorted, a 64-bit
//!   integer as a string, a value outside the type refused with its path.
//!
//! Oracles:
//! OMG IDL 4.2 §7.4.1.4.4.4.4 (a recursive type needs a sequence, a map,
//! an `@optional` or an `@external` member); the value format of
//! `tests/interop-corpus/README.md` (enum by name, sequence as array, a
//! 64-bit integer as a string).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_wire_probe::canonical::{canonicalise, text};
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

#[test]
fn the_canonical_text_sorts_the_keys_and_writes_a_64_bit_integer_as_a_string() {
    let idl = "enum Colour { RED, GREEN };\n\
               @final struct V { long long w; Colour c; sequence<short> q; boolean f; };";
    let ast = model_from_idl(idl, "canonical.idl").unwrap();
    let value = json!({"w": -3, "c": "GREEN", "q": [1, -1], "f": true});
    let canonical = canonicalise(&ast, type_in(&ast, &["V"]), &value, "sample").unwrap();
    assert_eq!(
        text(&canonical),
        r#"{"c":"GREEN","f":true,"q":[1,-1],"w":"-3"}"#
    );
    // A value that is not one of the type's is refused with its path.
    let error = canonicalise(
        &ast,
        type_in(&ast, &["V"]),
        &json!({"w": 1, "c": "BLUE", "q": [], "f": true}),
        "sample",
    )
    .err()
    .unwrap();
    assert!(format!("{error}").contains("sample"), "{error}");
}
