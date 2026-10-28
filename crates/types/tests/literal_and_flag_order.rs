// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The order of the sequences of a `TypeObject` (DDS-XTypes 1.3 §7.3.4.5):
//!
//! - "The elements in CompleteEnumeratedLiteralSeq shall be ordered in
//!   increasing values of their numeric value", and the same for
//!   MinimalEnumeratedLiteralSeq;
//! - "The elements in CompleteBitflagSeq shall be ordered in increasing
//!   values of their position", and the same for MinimalBitflagSeq.
//!
//! The hash of a type is taken over these octets, so a declaration order
//! that differs from the value order would give a type another identifier
//! than a peer that sorts.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::Ast;
use zerodds_types::from_ast::{complete_type_object, minimal_type_object};
use zerodds_types::type_object::flags::EnumLiteralFlag;
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject};

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "order.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn id(ast: &Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type {name}"))
}

#[test]
fn enumerated_literals_are_written_in_increasing_value_order() {
    let ast = model("enum E { @value(7) A, @value(0) B, @value(3) C };");
    let CompleteTypeObject::Enumerated(complete) =
        complete_type_object(&ast, id(&ast, "E")).unwrap()
    else {
        panic!("not an enumeration");
    };
    let complete_order: Vec<(&str, i32)> = complete
        .literal_seq
        .iter()
        .map(|l| (l.detail.name.as_str(), l.common.value))
        .collect();
    assert_eq!(complete_order, [("B", 0), ("C", 3), ("A", 7)]);

    let MinimalTypeObject::Enumerated(minimal) = minimal_type_object(&ast, id(&ast, "E")).unwrap()
    else {
        panic!("not an enumeration");
    };
    let minimal_values: Vec<i32> = minimal.literal_seq.iter().map(|l| l.common.value).collect();
    assert_eq!(minimal_values, [0, 3, 7]);

    // The default literal is the first one declared (§7.2.2.4.1.1), and
    // it keeps its flag where the sort puts it.
    let defaults: Vec<i32> = complete
        .literal_seq
        .iter()
        .filter(|l| l.common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0)
        .map(|l| l.common.value)
        .collect();
    assert_eq!(defaults, [7]);
}

#[test]
fn bitmask_flags_are_written_in_increasing_position_order() {
    let ast = model("bitmask M { @position(5) A, @position(1) B, C };");
    let CompleteTypeObject::Bitmask(complete) = complete_type_object(&ast, id(&ast, "M")).unwrap()
    else {
        panic!("not a bitmask");
    };
    let order: Vec<(&str, u16)> = complete
        .flag_seq
        .iter()
        .map(|f| (f.detail.name.as_str(), f.common.position))
        .collect();
    assert_eq!(order, [("B", 1), ("C", 2), ("A", 5)]);

    let MinimalTypeObject::Bitmask(minimal) = minimal_type_object(&ast, id(&ast, "M")).unwrap()
    else {
        panic!("not a bitmask");
    };
    let positions: Vec<u16> = minimal.flag_seq.iter().map(|f| f.common.position).collect();
    assert_eq!(positions, [1, 2, 5]);
}
