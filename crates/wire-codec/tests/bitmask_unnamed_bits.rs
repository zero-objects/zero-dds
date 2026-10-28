// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A bit of a bitmask the reader's type does not name.
//!
//! DDS-XTypes 1.3 §7.4.1.1.3.2: "the states of the remaining serialized
//! bits are not specified, and those bits are not considered to be part
//! of the bitmask". Table 18: "Preserve bit values by index for all bits
//! identified in both T1 and T2." The probe's reader returns the names of
//! the declared bits that are set and does not refuse a stream for the
//! others.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Value, json};
use zerodds_ast::{Ast, Extensibility, MemberSpec, TypeKind};
use zerodds_ast_core::Ref;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::decode;
use zerodds_wire_codec::value::ProbeRequest;

/// `@bit_bound(8) bitmask Flags { @position(0) A, @position(1) B };
/// @final struct S { Flags f; };`
fn model() -> Ast {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let flags = ast
        .add_type(root, "Flags", TypeKind::Bitmask { bits: Vec::new() })
        .expect("Flags");
    ast.add_bit_value(flags, "A", Some(0)).expect("A");
    ast.add_bit_value(flags, "B", Some(1)).expect("B");
    let any = ast.any_of_type(flags).expect("any");
    ast.facets_mut(any).expect("facets").xtypes_mut().bit_bound = Some(8);

    let s = ast
        .add_type(
            root,
            "S",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("S");
    let any = ast.any_of_type(s).expect("any");
    ast.facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .extensibility = Some(Extensibility::Final);
    ast.add_member(s, MemberSpec::new("f", Ref::Resolved(flags)))
        .expect("f");
    ast
}

fn read(byte: u8) -> Value {
    let ast = model();
    let request = ProbeRequest::new("S", Encoding::PlainCdr2, Endianness::Little);
    decode(&ast, &request, &[byte]).expect("an unnamed bit is not a reason to refuse")
}

#[test]
fn a_bit_inside_the_bound_that_the_type_does_not_name_is_not_read() {
    // Bits 0 and 1 are `A` and `B`; bit 2 has no name.
    assert_eq!(read(0b0000_0101), json!({"f": ["A"]}));
    assert_eq!(read(0b0000_0100), json!({"f": []}));
}

#[test]
fn the_declared_bits_beside_an_unnamed_one_are_kept() {
    assert_eq!(read(0b1000_0011), json!({"f": ["A", "B"]}));
}
