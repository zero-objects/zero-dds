// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! `decode_prefix` reads one value from the start of a buffer and says where
//! it stopped, so that serialized samples packed back to back with no length
//! (DDS-XRCE 1.0 Annex A `sequence<SampleData>`) can be told apart by their
//! type alone.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Value, json};
use zerodds_ast::{Ast, Extensibility, MemberSpec, PrimitiveKind, TypeKind};
use zerodds_ast_core::Ref;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_codec::{decode, decode_prefix, encode};

/// `@final struct Point { long id; string label; }` — a type whose size
/// depends on the value.
fn model() -> Ast {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "L", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let string = ast
        .add_type(
            root,
            "S",
            TypeKind::String {
                width: zerodds_ast::CharWidth::Char,
                bound: zerodds_ast::Bound::Unbounded,
            },
        )
        .expect("string");
    let point = ast
        .add_type(
            root,
            "Point",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Point");
    let any = ast.any_of_type(point).expect("any");
    ast.facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .extensibility = Some(Extensibility::Final);
    ast.add_member(point, MemberSpec::new("id", Ref::Resolved(long)))
        .expect("id");
    ast.add_member(point, MemberSpec::new("label", Ref::Resolved(string)))
        .expect("label");
    ast
}

fn request() -> ProbeRequest {
    ProbeRequest::new("Point", Encoding::PlainCdr2, Endianness::Little)
}

#[test]
fn the_first_of_several_packed_samples_is_read_and_its_length_returned() {
    let ast = model();
    let a = encode(&ast, &request(), &json!({"id": 1, "label": "ab"})).unwrap();
    let b = encode(&ast, &request(), &json!({"id": 2, "label": "cdefg"})).unwrap();
    let mut packed = a.clone();
    packed.extend_from_slice(&b);

    let (first, n) = decode_prefix::<Value>(&ast, &request(), &packed).unwrap();
    assert_eq!(first, json!({"id": 1, "label": "ab"}));
    assert_eq!(n, a.len());
    let (second, m) = decode_prefix::<Value>(&ast, &request(), &packed[n..]).unwrap();
    assert_eq!(second, json!({"id": 2, "label": "cdefg"}));
    assert_eq!(m, b.len());
    // `decode` of the whole buffer is a mismatch: it refuses the second sample
    // as left-over octets, which is why the prefix form exists.
    assert!(decode::<Value>(&ast, &request(), &packed).is_err());
}

#[test]
fn a_truncated_sample_is_an_error_not_a_short_read() {
    let ast = model();
    let a = encode(&ast, &request(), &json!({"id": 1, "label": "abcdef"})).unwrap();
    assert!(decode_prefix::<Value>(&ast, &request(), &a[..a.len() - 2]).is_err());
}
