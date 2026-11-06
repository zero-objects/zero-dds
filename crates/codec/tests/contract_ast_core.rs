// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-ast-core`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-ast-core` (`src/model.rs`): `Id<T>` as
//! the key of the map from a struct to its plan's index, so a type that
//! reaches itself is planned once — `Copy`, `Eq` and `Hash`, one id per
//! node and the same id for every reference to it.
//!
//! Oracle: IDL 4.2 §7.4.1.4.4.4.4, a struct that reaches itself through a
//! sequence is one type: every reference resolves to its one declaration.

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::HashMap;

use zerodds_ast::{TypeKind, TypeNode};
use zerodds_ast_core::Id;

#[test]
fn idl_4_2_7_4_1_4_every_reference_to_a_type_is_its_one_id() {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        "module m { struct Node; struct Node { long v; sequence<m::Node> kids; }; \
         struct Pair { m::Node a; m::Node b; }; };",
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed());
    let ast = composed.ast;
    let ids: Vec<Id<TypeNode>> = ast
        .types()
        .filter(|(_, node)| node.name == "Node" && matches!(node.kind, TypeKind::Structure { .. }))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids.len(), 1, "one declaration");
    let node = ids[0];
    let (pair, _) = ast.types().find(|(_, node)| node.name == "Pair").unwrap();
    let mut seen: HashMap<Id<TypeNode>, usize> = HashMap::new();
    for member in ast.members_of(pair) {
        let target = ast.member(*member).unwrap().type_.resolved().unwrap();
        *seen.entry(target).or_default() += 1;
    }
    assert_eq!(seen.get(&node), Some(&2), "both members name the one id");
    let copied = node;
    assert_eq!(copied, node);
}
