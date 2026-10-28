// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The three KeyHash examples of DDS-XTypes 1.3 §7.6.8 (S. 238-241), with the
//! hash octets the spec prints. The input values are those of the examples;
//! the expected hashes are copied from the spec text, not computed by this
//! code.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast::{
    Ast, Bound, Extensibility, MemberIdAssignment, MemberIdOrigin, MemberNode, MemberSpec,
    PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{Encoding, Endianness, PlCdr1HeaderPolicy, PlCdr1Reading};
use zerodds_wire_codec::value::{Emit, ProbeRequest};
use zerodds_wire_codec::{decode, encode};

struct Model {
    ast: Ast,
}

impl Model {
    fn new() -> Self {
        Self { ast: Ast::new() }
    }

    fn primitive(&mut self, name: &str, kind: PrimitiveKind) -> Id<TypeNode> {
        let root = self.ast.root_module();
        self.ast
            .add_type(root, name, TypeKind::Primitive(kind))
            .expect("primitive")
    }

    fn string(&mut self, name: &str, bound: i128) -> Id<TypeNode> {
        let root = self.ast.root_module();
        let bound = Bound::Const(self.ast.add_int_expr(bound).expect("bound"));
        self.ast
            .add_type(
                root,
                name,
                TypeKind::String {
                    width: zerodds_ast::CharWidth::Char,
                    bound,
                },
            )
            .expect("string")
    }

    fn structure(&mut self, name: &str, extensibility: Extensibility) -> Id<TypeNode> {
        let root = self.ast.root_module();
        let id = self
            .ast
            .add_type(
                root,
                name,
                TypeKind::Structure {
                    base: None,
                    members: Vec::new(),
                },
            )
            .expect("struct");
        let any = self.ast.any_of_type(id).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .extensibility = Some(extensibility);
        id
    }

    fn member(
        &mut self,
        aggregate: Id<TypeNode>,
        name: &str,
        type_: Id<TypeNode>,
        key: bool,
        id: Option<u32>,
    ) -> Id<MemberNode> {
        let member = self
            .ast
            .add_member(aggregate, MemberSpec::new(name, Ref::Resolved(type_)))
            .expect("member");
        let any = self.ast.any_of_member(member).expect("any");
        let x = self.ast.facets_mut(any).expect("facets").xtypes_mut();
        x.is_key = key;
        if let Some(id) = id {
            x.member_id = Some(MemberIdAssignment {
                effective: id,
                origin: MemberIdOrigin::Declared,
            });
        }
        member
    }
}

fn key_hash(ast: &Ast, type_name: &str, value: &serde_json::Value) -> [u8; 16] {
    let mut request = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Big);
    request.emit = Emit::KeyHash;
    encode::encode(ast, &request, value)
        .expect("a KeyHash")
        .try_into()
        .expect("sixteen octets")
}

fn holder(ast: &Ast, type_name: &str, value: &serde_json::Value) -> Vec<u8> {
    let mut request = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Big);
    request.emit = Emit::KeyHash;
    encode::key_holder(ast, &request, value).expect("the key holder")
}

#[test]
fn example_1_a_four_octet_key_is_padded_with_zeros() {
    let mut m = Model::new();
    let long = m.primitive("Long", PrimitiveKind::Int32);
    let foo = m.structure("Foo", Extensibility::Final);
    m.member(foo, "id", long, true, None);
    m.member(foo, "x", long, false, None);
    m.member(foo, "y", long, false, None);

    let value = json!({"id": 0x1234_5678, "x": 10, "y": 20});
    assert_eq!(holder(&m.ast, "Foo", &value), [0x12, 0x34, 0x56, 0x78]);
    assert_eq!(
        key_hash(&m.ast, "Foo", &value),
        [
            0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00
        ]
    );
}

#[test]
fn example_2_a_key_of_at_most_28_octets_is_hashed_with_md5() {
    let mut m = Model::new();
    let long = m.primitive("Long", PrimitiveKind::Int32);
    let long_long = m.primitive("LongLong", PrimitiveKind::Int64);
    let label = m.string("Label12", 12);
    let foo = m.structure("Foo", Extensibility::Final);
    m.member(foo, "label", label, true, None);
    m.member(foo, "id", long_long, true, None);
    m.member(foo, "x", long, false, None);
    m.member(foo, "y", long, false, None);

    // 0x123456789abcdef0 as a string: a JSON number is not exact past 2^53.
    let value = json!({"label": "BLUE", "id": "1311768467463790320", "x": 10, "y": 20});
    assert_eq!(
        holder(&m.ast, "Foo", &value),
        [
            0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34,
            0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0
        ]
    );
    assert_eq!(
        key_hash(&m.ast, "Foo", &value),
        [
            0xf9, 0x1a, 0x59, 0xe3, 0x2e, 0x45, 0x35, 0xd9, 0xa6, 0x9c, 0xd5, 0xd9, 0xf5, 0xb6,
            0xe3, 0x6e
        ]
    );
}

#[test]
fn example_3_the_key_members_are_reordered_by_member_id_and_nested_keys_reduced() {
    let mut m = Model::new();
    let long = m.primitive("Long", PrimitiveKind::Int32);
    let label = m.string("Label16", 16);
    let nested = m.structure("Nested", Extensibility::Mutable);
    m.member(nested, "m_long", long, true, None);
    m.member(nested, "u", long, false, None);
    m.member(nested, "w", long, false, None);
    let foo = m.structure("Foo", Extensibility::Mutable);
    m.member(foo, "label", label, true, Some(40));
    m.member(foo, "m_nested", nested, true, Some(30));
    m.member(foo, "x", long, false, Some(20));
    m.member(foo, "y", long, false, Some(10));

    let value = json!({
        "label": "BLUE",
        "m_nested": {"m_long": 0x1234_5678, "u": 10, "w": 20},
        "x": 100,
        "y": 200
    });
    assert_eq!(
        holder(&m.ast, "Foo", &value),
        [
            0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x05, 0x42, 0x4c, 0x55, 0x45, 0x00
        ]
    );
    assert_eq!(
        key_hash(&m.ast, "Foo", &value),
        [
            0x37, 0x4b, 0x96, 0xe2, 0xe7, 0x27, 0x23, 0x7f, 0x01, 0x6c, 0xc4, 0xce, 0xbb, 0x6e,
            0xb7, 0x1e
        ]
    );
}

/// `wire.pl_cdr1_headers` and `wire.pl_cdr1_reading` say how PL_CDR1
/// parameters are written and read. The holder is PLAIN_CDR2 (§7.6.8) and
/// holds none, so under every value of the two the holder of `type_name`
/// validates, encodes to the octets of the specification's request and
/// decodes back to what that request decodes. A tolerant reading once took
/// the type out of the inverse holder altogether: a dispose then went out
/// without its `serializedKey`, and a reader could not name the instance of
/// a `serializedKey` that came without a KeyHash.
fn assert_holder_left_by_pl_cdr1_settings(ast: &Ast, type_name: &str, value: &serde_json::Value) {
    let specification = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Big);
    let octets = encode::key_holder(ast, &specification, value).expect("the key holder");
    let decoded: serde_json::Value =
        decode::key_holder(ast, &specification, &octets).expect("the holder decodes");
    for reading in [PlCdr1Reading::Strict, PlCdr1Reading::Tolerant] {
        for headers in PlCdr1HeaderPolicy::ALL {
            let mut request = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Big);
            request.pl_cdr1_reading = reading;
            request.pl_cdr1_headers = headers;
            let setting = format!("{type_name}, {reading:?}, {headers:?}");
            decode::validate_key_holder(ast, &request)
                .unwrap_or_else(|error| panic!("{setting}: {error}"));
            assert_eq!(
                encode::key_holder(ast, &request, value).expect("the key holder"),
                octets,
                "{setting}"
            );
            let again: serde_json::Value =
                decode::key_holder(ast, &request, &octets).expect("the holder decodes");
            assert_eq!(again, decoded, "{setting}");
        }
    }
}

#[test]
fn the_pl_cdr1_settings_leave_the_holders_of_examples_2_and_3() {
    let mut m = Model::new();
    let long = m.primitive("Long", PrimitiveKind::Int32);
    let long_long = m.primitive("LongLong", PrimitiveKind::Int64);
    let label12 = m.string("Label12", 12);
    let label16 = m.string("Label16", 16);
    let example_2 = m.structure("Example2", Extensibility::Final);
    m.member(example_2, "label", label12, true, None);
    m.member(example_2, "id", long_long, true, None);
    m.member(example_2, "x", long, false, None);
    m.member(example_2, "y", long, false, None);
    let nested = m.structure("Nested", Extensibility::Mutable);
    m.member(nested, "m_long", long, true, None);
    m.member(nested, "u", long, false, None);
    m.member(nested, "w", long, false, None);
    let example_3 = m.structure("Example3", Extensibility::Mutable);
    m.member(example_3, "label", label16, true, Some(40));
    m.member(example_3, "m_nested", nested, true, Some(30));
    m.member(example_3, "x", long, false, Some(20));
    m.member(example_3, "y", long, false, Some(10));

    assert_holder_left_by_pl_cdr1_settings(
        &m.ast,
        "Example2",
        &json!({"label": "BLUE", "id": "1311768467463790320", "x": 10, "y": 20}),
    );
    assert_holder_left_by_pl_cdr1_settings(
        &m.ast,
        "Example3",
        &json!({
            "label": "BLUE",
            "m_nested": {"m_long": 0x1234_5678, "u": 10, "w": 20},
            "x": 100,
            "y": 200
        }),
    );
}
