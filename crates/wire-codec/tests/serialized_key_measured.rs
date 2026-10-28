// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `decode::serialized_key` against `serializedKey` octets measured
//! against Eclipse Cyclone DDS 11.0.1 with
//! `ddsi_serdata_from_sample(sertype, SDK_KEY, &sample)` +
//! `ddsi_serdata_to_ser`, writer QoS `data_representation` XCDR1 resp.
//! XCDR2, x86 native byte order (little-endian). Each vector's first
//! four bytes are its RTPS encapsulation header. The IDL these types
//! were measured against is `tests/data/serialized_key_measured.idl`,
//! and `tests/data/serialized_key_measured_probe.c` is the program that
//! printed the vectors; its header says how to build and run it.
//!
//! What the vectors show, and what this file checks: the wire
//! `serializedKey` is not a (possibly truncated) full sample — it is
//! only the `@key` members, in **declaration** order (not ascending
//! member id — see `r`/`q`, `@id(5)` on `a` before `@id(2)` on `b`),
//! framed by the type's OWN extensibility (a DHEADER over the key
//! members alone for `@appendable`/`@mutable` under XCDR2, nothing for
//! `@final` or under XCDR1), with a nested keyed member reduced
//! recursively to its own `@key` members and its own DHEADER. Reading
//! these bytes as a truncated full sample — this crate's first attempt
//! at `SerializedTypeSupport::read_key_only` — decoded `a`'s bytes as
//! its own declared name whenever a `@key` member is not first, which is
//! the worst bug class there is: silently the wrong KeyHash.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Value, json};
use zerodds_ast::{
    Ast, Extensibility, MemberIdAssignment, MemberIdOrigin, MemberNode, MemberSpec, PrimitiveKind,
    TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{Encoding, Endianness, KeyHolderExtensibility, KeyHolderOrder};
use zerodds_wire_codec::decode::serialized_key;
use zerodds_wire_codec::encode;
use zerodds_wire_codec::value::{Emit, ProbeRequest};

/// The `Ast` model builder, adapted from
/// `crates/ast-layout/tests/wire_vectors.rs`'s `Model` — that one is
/// private to its own crate, and this crate's own tests
/// (`appendable_tail_xcdr1.rs`) build a model the same way rather than
/// share one.
struct Model {
    ast: Ast,
    root: Id<zerodds_ast::ModuleNode>,
}

impl Model {
    fn new() -> Self {
        let ast = Ast::new();
        let root = ast.root_module();
        Self { ast, root }
    }

    fn primitive(&mut self, name: &str, kind: PrimitiveKind) -> Id<TypeNode> {
        self.ast
            .add_type(self.root, name, TypeKind::Primitive(kind))
            .expect("primitive")
    }

    fn string(&mut self, name: &str) -> Id<TypeNode> {
        self.ast
            .add_type(
                self.root,
                name,
                TypeKind::String {
                    width: zerodds_ast::CharWidth::Char,
                    bound: zerodds_ast::Bound::Unbounded,
                },
            )
            .expect("string")
    }

    fn structure(&mut self, name: &str, extensibility: Extensibility) -> Id<TypeNode> {
        let id = self
            .ast
            .add_type(
                self.root,
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
    ) -> Id<MemberNode> {
        self.ast
            .add_member(aggregate, MemberSpec::new(name, Ref::Resolved(type_)))
            .expect("member")
    }

    fn set_member_id(&mut self, member: Id<MemberNode>, id: u32) {
        let any = self.ast.any_of_member(member).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .member_id = Some(MemberIdAssignment {
            effective: id,
            origin: MemberIdOrigin::Declared,
        });
    }

    fn mark_key(&mut self, member: Id<MemberNode>) {
        let any = self.ast.any_of_member(member).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .is_key = true;
    }
}

/// IDL module K + O from the review, all in one root module — the type
/// names are already distinct, and this builder does not model IDL
/// module scoping at all. Every type is looked up by name afterward
/// (`ProbeRequest::new`), so only the finished `Model` is returned.
fn types() -> Model {
    let mut m = Model::new();
    let long = m.primitive("Long", PrimitiveKind::Int32);
    let short = m.primitive("Short", PrimitiveKind::Int16);
    let string = m.string("Str");

    // @appendable struct A { long x; @key long id; long y; };
    let a = m.structure("A", Extensibility::Appendable);
    m.member(a, "x", long);
    let a_id = m.member(a, "id", long);
    m.mark_key(a_id);
    m.member(a, "y", long);

    // @final struct F { long x; @key long id; };
    let f = m.structure("F", Extensibility::Final);
    m.member(f, "x", long);
    let f_id = m.member(f, "id", long);
    m.mark_key(f_id);

    // @mutable struct M { long x; @key long id; };
    let mutable = m.structure("M", Extensibility::Mutable);
    m.member(mutable, "x", long);
    let m_id = m.member(mutable, "id", long);
    m.mark_key(m_id);

    // @appendable struct Inner { long a; @key long b; };
    let inner = m.structure("Inner", Extensibility::Appendable);
    m.member(inner, "a", long);
    let inner_b = m.member(inner, "b", long);
    m.mark_key(inner_b);

    // @appendable struct N { long x; @key Inner k; };
    let n = m.structure("N", Extensibility::Appendable);
    m.member(n, "x", long);
    let n_k = m.member(n, "k", inner);
    m.mark_key(n_k);

    // @final struct S { @key string name; long x; @key short id; };
    let s = m.structure("S", Extensibility::Final);
    let s_name = m.member(s, "name", string);
    m.mark_key(s_name);
    m.member(s, "x", long);
    let s_id = m.member(s, "id", short);
    m.mark_key(s_id);

    // @final struct R { @id(5) @key long a; long x; @id(2) @key long b; };
    let r = m.structure("R", Extensibility::Final);
    let r_a = m.member(r, "a", long);
    m.mark_key(r_a);
    m.set_member_id(r_a, 5);
    m.member(r, "x", long);
    let r_b = m.member(r, "b", long);
    m.mark_key(r_b);
    m.set_member_id(r_b, 2);

    // @appendable struct Q { @id(5) @key long a; long x; @id(2) @key long b; };
    let q = m.structure("Q", Extensibility::Appendable);
    let q_a = m.member(q, "a", long);
    m.mark_key(q_a);
    m.set_member_id(q_a, 5);
    m.member(q, "x", long);
    let q_b = m.member(q, "b", long);
    m.mark_key(q_b);
    m.set_member_id(q_b, 2);

    m
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex byte"))
        .collect()
}

/// A `serialized_key` request for `type_name`: the wire form
/// (`AsDeclared` + `Declaration`, `Recursive` nesting stays the
/// default), the given encoding/endianness, and `emit` set so the
/// caller hands in the full wire bytes including the encapsulation
/// header — the same convention `decode::decode` uses.
fn wire_key_request(type_name: &str, encoding: Encoding, endianness: Endianness) -> ProbeRequest {
    let mut request = ProbeRequest::new(type_name, encoding, endianness);
    request.key_holder = KeyHolderExtensibility::AsDeclared;
    request.key_holder_order = KeyHolderOrder::Declaration;
    request.emit = Emit::PayloadWithEncapsulation;
    request
}

/// The canonical KeyHash (XTypes 1.3 §7.6.8: big-endian PLAIN_CDR2,
/// `ForcedFinal` + `AscendingMemberId`, the defaults) of a
/// **full** sample value — independent of any wire bytes, so it is an
/// oracle for what `serialized_key`'s decoded (partial, defaulted) value
/// must still hash to.
fn full_sample_hash(ast: &Ast, type_name: &str, full: &Value) -> [u8; 16] {
    let mut request = ProbeRequest::new(type_name, Encoding::PlainCdr2, Endianness::Big);
    request.emit = Emit::KeyHash;
    let bytes = encode::encode::<Value>(ast, &request, full).expect("keyhash of the full sample");
    bytes.try_into().expect("16-byte KeyHash")
}

/// The same canonical KeyHash, computed from whatever `serialized_key`
/// decoded — key members real, everything else defaulted. XTypes 1.3
/// §7.6.8's Key Holder is built from the `@key` members alone, so a
/// defaulted non-key member must not move this.
fn decoded_key_hash(ast: &Ast, type_name: &str, decoded: &Value) -> [u8; 16] {
    full_sample_hash(ast, type_name, decoded)
}

#[test]
fn a_key_not_first_xcdr1_and_xcdr2() {
    let t = types();
    let full = json!({"x": 1, "id": 7, "y": 2});
    let expected = full_sample_hash(&t.ast, "A", &full);

    let xcdr1 = hex("0001000007000000");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("A", Encoding::PlainCdr1, Endianness::Little),
        &xcdr1,
    )
    .expect("A xcdr1 decodes");
    assert_eq!(decoded, json!({"x": 0, "id": 7, "y": 0}));
    assert_eq!(decoded_key_hash(&t.ast, "A", &decoded), expected);

    let xcdr2 = hex("000900000400000007000000");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("A", Encoding::PlainCdr2, Endianness::Little),
        &xcdr2,
    )
    .expect("A xcdr2 decodes");
    assert_eq!(decoded, json!({"x": 0, "id": 7, "y": 0}));
    assert_eq!(decoded_key_hash(&t.ast, "A", &decoded), expected);

    // The full sample this hash is checked against, decoded straight —
    // A xcdr2 data 000900000c000000010000000700000002000000 — must
    // name the same instance.
    let full_wire = hex("000900000c000000010000000700000002000000");
    let mut data_request = ProbeRequest::new("A", Encoding::PlainCdr2, Endianness::Little);
    data_request.emit = Emit::PayloadWithEncapsulation;
    let from_full: Value =
        zerodds_wire_codec::decode(&t.ast, &data_request, &full_wire).expect("full A decodes");
    assert_eq!(from_full, full);
}

#[test]
fn f_final_key_not_first_xcdr1_and_xcdr2() {
    let t = types();
    let full = json!({"x": 1, "id": 7});
    let expected = full_sample_hash(&t.ast, "F", &full);

    for (hex_bytes, encoding) in [
        ("0001000007000000", Encoding::PlainCdr1),
        ("0007000007000000", Encoding::PlainCdr2),
    ] {
        let bytes = hex(hex_bytes);
        let decoded: Value = serialized_key(
            &t.ast,
            &wire_key_request("F", encoding, Endianness::Little),
            &bytes,
        )
        .unwrap_or_else(|e| panic!("F {encoding:?} decodes: {e:?}"));
        assert_eq!(decoded, json!({"x": 0, "id": 7}), "{encoding:?}");
        assert_eq!(
            decoded_key_hash(&t.ast, "F", &decoded),
            expected,
            "{encoding:?}"
        );
    }
}

#[test]
fn m_mutable_key_not_first_xcdr1_and_xcdr2() {
    let t = types();
    let full = json!({"x": 1, "id": 7});
    let expected = full_sample_hash(&t.ast, "M", &full);

    let xcdr1 = hex("00030000017f0800010000000400000007000000027f0000");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("M", Encoding::PlainCdr1, Endianness::Little),
        &xcdr1,
    )
    .expect("M xcdr1 decodes");
    assert_eq!(decoded, json!({"x": 0, "id": 7}));
    assert_eq!(decoded_key_hash(&t.ast, "M", &decoded), expected);

    let xcdr2 = hex("000b0000080000000100002007000000");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("M", Encoding::PlainCdr2, Endianness::Little),
        &xcdr2,
    )
    .expect("M xcdr2 decodes");
    assert_eq!(decoded, json!({"x": 0, "id": 7}));
    assert_eq!(decoded_key_hash(&t.ast, "M", &decoded), expected);
}

#[test]
fn n_nested_keyed_member_recurses_xcdr2() {
    // Cyclone refuses the XCDR1 topic outright (-3) — no XCDR1 vector.
    let t = types();
    let full = json!({"x": 1, "k": {"a": 3, "b": 9}});
    let expected = full_sample_hash(&t.ast, "N", &full);

    let xcdr2 = hex("00090000080000000400000009000000");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("N", Encoding::PlainCdr2, Endianness::Little),
        &xcdr2,
    )
    .expect("N xcdr2 decodes");
    assert_eq!(decoded, json!({"x": 0, "k": {"a": 0, "b": 9}}));
    assert_eq!(decoded_key_hash(&t.ast, "N", &decoded), expected);
}

#[test]
fn s_two_key_members_around_a_non_key_one_xcdr1_and_xcdr2() {
    let t = types();
    let full = json!({"name": "ab", "x": 5, "id": 6});
    let expected = full_sample_hash(&t.ast, "S", &full);

    for (hex_bytes, encoding) in [
        ("00010002030000006162000006000000", Encoding::PlainCdr1),
        ("00070002030000006162000006000000", Encoding::PlainCdr2),
    ] {
        let bytes = hex(hex_bytes);
        let decoded: Value = serialized_key(
            &t.ast,
            &wire_key_request("S", encoding, Endianness::Little),
            &bytes,
        )
        .unwrap_or_else(|e| panic!("S {encoding:?} decodes: {e:?}"));
        assert_eq!(
            decoded,
            json!({"name": "ab", "x": 0, "id": 6}),
            "{encoding:?}"
        );
        assert_eq!(
            decoded_key_hash(&t.ast, "S", &decoded),
            expected,
            "{encoding:?}"
        );
    }
}

#[test]
fn r_final_keeps_declaration_order_despite_id_5_before_2() {
    let t = types();
    let full = json!({"a": 1, "x": 5, "b": 2});
    let expected = full_sample_hash(&t.ast, "R", &full);

    for (hex_bytes, encoding) in [
        ("000100000100000002000000", Encoding::PlainCdr1),
        ("000700000100000002000000", Encoding::PlainCdr2),
    ] {
        let bytes = hex(hex_bytes);
        let decoded: Value = serialized_key(
            &t.ast,
            &wire_key_request("R", encoding, Endianness::Little),
            &bytes,
        )
        .unwrap_or_else(|e| panic!("R {encoding:?} decodes: {e:?}"));
        assert_eq!(decoded, json!({"a": 1, "x": 0, "b": 2}), "{encoding:?}");
        assert_eq!(
            decoded_key_hash(&t.ast, "R", &decoded),
            expected,
            "{encoding:?}"
        );
    }
}

#[test]
fn q_appendable_keeps_declaration_order_despite_id_5_before_2() {
    let t = types();
    let full = json!({"a": 1, "x": 5, "b": 2});
    let expected = full_sample_hash(&t.ast, "Q", &full);

    let xcdr1 = hex("000100000100000002000000");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("Q", Encoding::PlainCdr1, Endianness::Little),
        &xcdr1,
    )
    .expect("Q xcdr1 decodes");
    assert_eq!(decoded, json!({"a": 1, "x": 0, "b": 2}));
    assert_eq!(decoded_key_hash(&t.ast, "Q", &decoded), expected);

    let xcdr2 = hex("00090000080000000100000002000000");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("Q", Encoding::PlainCdr2, Endianness::Little),
        &xcdr2,
    )
    .expect("Q xcdr2 decodes");
    assert_eq!(decoded, json!({"a": 1, "x": 0, "b": 2}));
    assert_eq!(decoded_key_hash(&t.ast, "Q", &decoded), expected);
}

/// Big-endian is not measured — Cyclone writes native (little-endian, on
/// the x86 test host) order for every vector above. This checks the
/// codec path is byte-order generic by round-tripping through it, not
/// against a second measurement: `encode::encode` writes the same
/// declaration-order wire form big-endian, and `serialized_key` reads it
/// back. Unmeasured against a real vendor.
#[test]
fn big_endian_is_unmeasured_but_the_codec_path_is_byte_order_generic() {
    // Cyclone writes native (little-endian, on the x86 test host)
    // order for every measured vector in this file — nothing here was
    // captured big-endian. This is the F xcdr1 vector
    // (`0001000007000000`) byte-swapped by hand: kind `0001` (CDR_LE) ->
    // `0000` (CDR_BE), `id = 7` little-endian `07000000` -> big-endian
    // `00000007`. It checks that the codec path this decode takes has
    // no little-endian-only step (`Reader::with_max_align` takes the
    // endianness as a parameter throughout, same as `decode`/`encode`
    // elsewhere in this crate) — not a second vendor measurement.
    let t = types();
    let full = json!({"x": 1, "id": 7});
    let expected = full_sample_hash(&t.ast, "F", &full);

    let be = hex("0000000000000007");
    let decoded: Value = serialized_key(
        &t.ast,
        &wire_key_request("F", Encoding::PlainCdr1, Endianness::Big),
        &be,
    )
    .expect("F xcdr1 decodes big-endian");
    assert_eq!(decoded, json!({"x": 0, "id": 7}));
    assert_eq!(decoded_key_hash(&t.ast, "F", &decoded), expected);
}

/// `encode::serialized_key` is the exact inverse of `decode::serialized_key`
/// — every Cyclone vector measured above, encoded from just the `@key`
/// member(s), must come back byte-identical. The non-key fields' values
/// are irrelevant (never read: `write_aggregate` walks the already
/// key-filtered plan), so each value below carries only the key(s).
#[test]
fn encode_a_key_not_first_matches_the_measured_bytes() {
    let t = types();
    let value = json!({"id": 7});
    let xcdr1 = encode::serialized_key(
        &t.ast,
        &wire_key_request("A", Encoding::PlainCdr1, Endianness::Little),
        &value,
    )
    .expect("encode A xcdr1");
    assert_eq!(xcdr1, hex("0001000007000000"));

    let xcdr2 = encode::serialized_key(
        &t.ast,
        &wire_key_request("A", Encoding::DelimitedCdr, Endianness::Little),
        &value,
    )
    .expect("encode A xcdr2");
    assert_eq!(xcdr2, hex("000900000400000007000000"));
}

#[test]
fn encode_f_final_key_not_first_matches_the_measured_bytes() {
    let t = types();
    let value = json!({"id": 7});
    let xcdr1 = encode::serialized_key(
        &t.ast,
        &wire_key_request("F", Encoding::PlainCdr1, Endianness::Little),
        &value,
    )
    .expect("encode F xcdr1");
    assert_eq!(xcdr1, hex("0001000007000000"));

    let xcdr2 = encode::serialized_key(
        &t.ast,
        &wire_key_request("F", Encoding::PlainCdr2, Endianness::Little),
        &value,
    )
    .expect("encode F xcdr2");
    assert_eq!(xcdr2, hex("0007000007000000"));
}

#[test]
fn encode_m_mutable_key_not_first_matches_the_measured_bytes() {
    let t = types();
    let value = json!({"id": 7});
    // The XCDR1 PID form and the must-understand flag are both dialect
    // axes (§7.4.1.2.3 permits the extended header above the short
    // form's limits, not exclusively; §7.2.2.4.4.4.6 leaves an
    // undeclared member's must-understand bit to the implementation):
    // the default dialect writes what RTI does (short PID header,
    // must-understand on every `@key` member); Cyclone always writes
    // the extended PID header and sets must-understand only where
    // `@must_understand` was declared. This vector is a Cyclone
    // capture, so the request is built in Cyclone's dialect explicitly
    // — decode does not need this (the wire states its own form),
    // which is why round 2's decode test needed no such override.
    let mut xcdr1_request = ProbeRequest::in_dialect(
        "M",
        Encoding::PlCdr1,
        Endianness::Little,
        zerodds_ast_layout::Dialect::Cyclone,
    );
    xcdr1_request.key_holder = KeyHolderExtensibility::AsDeclared;
    xcdr1_request.key_holder_order = KeyHolderOrder::Declaration;
    xcdr1_request.emit = Emit::PayloadWithEncapsulation;
    let xcdr1 = encode::serialized_key(&t.ast, &xcdr1_request, &value).expect("encode M xcdr1");
    assert_eq!(
        xcdr1,
        hex("00030000017f0800010000000400000007000000027f0000")
    );

    // Same dialect reason as the XCDR1 half above: the EMHEADER's
    // must-understand flag differs between the default (RTI-shaped)
    // dialect and Cyclone's for an undeclared `@key` member.
    let mut xcdr2_request = ProbeRequest::in_dialect(
        "M",
        Encoding::PlCdr2,
        Endianness::Little,
        zerodds_ast_layout::Dialect::Cyclone,
    );
    xcdr2_request.key_holder = KeyHolderExtensibility::AsDeclared;
    xcdr2_request.key_holder_order = KeyHolderOrder::Declaration;
    xcdr2_request.emit = Emit::PayloadWithEncapsulation;
    let xcdr2 = encode::serialized_key(&t.ast, &xcdr2_request, &value).expect("encode M xcdr2");
    assert_eq!(xcdr2, hex("000b0000080000000100002007000000"));
}

#[test]
fn encode_n_nested_keyed_member_matches_the_measured_bytes() {
    let t = types();
    let value = json!({"k": {"b": 9}});
    let xcdr2 = encode::serialized_key(
        &t.ast,
        &wire_key_request("N", Encoding::DelimitedCdr, Endianness::Little),
        &value,
    )
    .expect("encode N xcdr2");
    assert_eq!(xcdr2, hex("00090000080000000400000009000000"));
}

#[test]
fn encode_s_two_key_members_matches_the_measured_bytes() {
    let t = types();
    let value = json!({"name": "ab", "id": 6});
    let xcdr1 = encode::serialized_key(
        &t.ast,
        &wire_key_request("S", Encoding::PlainCdr1, Endianness::Little),
        &value,
    )
    .expect("encode S xcdr1");
    assert_eq!(xcdr1, hex("00010002030000006162000006000000"));

    let xcdr2 = encode::serialized_key(
        &t.ast,
        &wire_key_request("S", Encoding::PlainCdr2, Endianness::Little),
        &value,
    )
    .expect("encode S xcdr2");
    assert_eq!(xcdr2, hex("00070002030000006162000006000000"));
}

#[test]
fn encode_r_final_keeps_declaration_order_matches_the_measured_bytes() {
    let t = types();
    let value = json!({"a": 1, "b": 2});
    let xcdr1 = encode::serialized_key(
        &t.ast,
        &wire_key_request("R", Encoding::PlainCdr1, Endianness::Little),
        &value,
    )
    .expect("encode R xcdr1");
    assert_eq!(xcdr1, hex("000100000100000002000000"));

    let xcdr2 = encode::serialized_key(
        &t.ast,
        &wire_key_request("R", Encoding::PlainCdr2, Endianness::Little),
        &value,
    )
    .expect("encode R xcdr2");
    assert_eq!(xcdr2, hex("000700000100000002000000"));
}

#[test]
fn encode_q_appendable_keeps_declaration_order_matches_the_measured_bytes() {
    let t = types();
    let value = json!({"a": 1, "b": 2});
    let xcdr1 = encode::serialized_key(
        &t.ast,
        &wire_key_request("Q", Encoding::PlainCdr1, Endianness::Little),
        &value,
    )
    .expect("encode Q xcdr1");
    assert_eq!(xcdr1, hex("000100000100000002000000"));

    let xcdr2 = encode::serialized_key(
        &t.ast,
        &wire_key_request("Q", Encoding::DelimitedCdr, Endianness::Little),
        &value,
    )
    .expect("encode Q xcdr2");
    assert_eq!(xcdr2, hex("00090000080000000100000002000000"));
}

/// `@appendable struct ShapeType { @key string<128> color; long x; long y;
/// long shapesize; }` — the OMG `shape_main` type, `color = "BLUE"`. The
/// exact `serializedKey` octets of one RTI Connext 7.7.0 dispose, captured
/// RTI -> ZeroDDS over UDP (little-endian), `Test_FinalInstanceState_1`
/// (4 ShapeType instances disposed; RTI's form was the only one of the
/// three measured forms every one of Fast DDS 3.6.1, Cyclone 11.0.1 and
/// OpenDDS 3.35 accepted, 4/4 each).
#[test]
fn encode_rti_shapetype_serializedkey_matches_the_capture() {
    let mut m = Model::new();
    let long = m.primitive("Long", PrimitiveKind::Int32);
    let string = m.string("Str");
    let shape = m.structure("ShapeType", Extensibility::Appendable);
    let color = m.member(shape, "color", string);
    m.mark_key(color);
    m.member(shape, "x", long);
    m.member(shape, "y", long);
    m.member(shape, "shapesize", long);

    let value = json!({"color": "BLUE"});
    let xcdr2 = encode::serialized_key(
        &m.ast,
        &wire_key_request("ShapeType", Encoding::DelimitedCdr, Endianness::Little),
        &value,
    )
    .expect("encode ShapeType xcdr2");
    assert_eq!(xcdr2, hex("000900030900000005000000424c554500000000"));
}
