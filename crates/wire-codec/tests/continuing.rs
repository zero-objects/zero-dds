// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A value that continues a sample counts its alignment from the sample.
//!
//! DDS-RPC 1.0 §7.5.1.1.6 puts the `Call` union behind a header of
//! variable length; under XCDR1 (XTypes 1.3 §7.4.1.1: alignment up to 8) an
//! 8-octet member of it aligns to the sample, not to the union.
#![cfg(feature = "serde-json")]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Value, json};
use zerodds_ast::{Ast, Extensibility, MemberSpec, PrimitiveKind, TypeKind};
use zerodds_ast_core::Ref;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_codec::{decode, decode_continuing, encode, encode_continuing};

/// `struct W { long a; unsigned long long b; };`
fn model() -> Ast {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let wide = ast
        .add_type(root, "U", TypeKind::Primitive(PrimitiveKind::UInt64))
        .expect("unsigned long long");
    let w = ast
        .add_type(
            root,
            "W",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("W");
    // DDS-RPC 1.0 §7.5.1.1: the topic types are plain, `@final`. The default
    // extensibility is `@appendable`, which XCDR2 frames with a DHEADER.
    let any = ast.any_of_type(w).expect("any");
    ast.facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .extensibility = Some(Extensibility::Final);
    ast.add_member(w, MemberSpec::new("a", Ref::Resolved(long)))
        .expect("a");
    ast.add_member(w, MemberSpec::new("b", Ref::Resolved(wide)))
        .expect("b");
    ast
}

fn value() -> Value {
    json!({"a": 1, "b": 2})
}

#[test]
fn xcdr1_pads_the_eight_octet_member_to_the_sample() {
    let ast = model();
    let request = ProbeRequest::new("W", Encoding::PlainCdr1, Endianness::Little);

    // On its own: a at 0, padding 4..8, b at 8.
    let alone = encode(&ast, &request, &value()).unwrap();
    assert_eq!(alone.len(), 16);

    // Behind 4 octets of someone else's: a at 4, b at 8 — no padding.
    let behind_four = encode_continuing(&ast, &request, &value(), 4).unwrap();
    assert_eq!(behind_four.len(), 12, "{behind_four:02x?}");
    assert_eq!(behind_four[..4], 1i32.to_le_bytes());
    assert_eq!(behind_four[4..], 2u64.to_le_bytes());

    // Behind 8 octets the value is what it is on its own.
    assert_eq!(
        encode_continuing(&ast, &request, &value(), 8).unwrap(),
        alone
    );

    // And it reads back where it stands.
    let mut sample = vec![0xAA; 4];
    sample.extend_from_slice(&behind_four);
    let back: Value = decode_continuing(&ast, &request, &sample, 4).unwrap();
    // A 64-bit integer reads back as a string, which keeps it exact in JSON.
    assert_eq!(back, json!({"a": 1, "b": "2"}));
    let back_alone: Value = decode(&ast, &request, &alone).unwrap();
    assert_eq!(back_alone, back);
}

#[test]
fn xcdr2_aligns_to_four_at_most() {
    let ast = model();
    let request = ProbeRequest::new("W", Encoding::PlainCdr2, Endianness::Little);
    let behind_four = encode_continuing(&ast, &request, &value(), 4).unwrap();
    assert_eq!(behind_four.len(), 12);
    let alone = encode(&ast, &request, &value()).unwrap();
    assert_eq!(
        alone.len(),
        12,
        "XCDR2 has no padding before the 8-octet member"
    );
}
