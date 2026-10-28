// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The planner gated against the pre-demolition wire vectors.
//!
//! The vectors are `crates/cdr/tests/xcdr2_wire_vectors.rs` at commit
//! `e1f3af895`, V-1 through V-12, documented in
//! `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6. They
//! predate this rebuild and nothing here produced them.
//!
//! # They are not an oracle
//!
//! This file used to call them one. They are this project's own earlier
//! output, and V-3 is measurably wrong — see the note above
//! `v3_final_mixed_primitives_plans_forty_four_under_xcdr2`. Where a
//! vector and a foreign implementation disagree, the foreign
//! implementation decides, and the run that decided it is recorded in
//! `tests/fixtures/vendor_bytes.md`.
//!
//! The expected bytes are transcribed in
//! `tests/fixtures/xcdr2_wire_vectors.md`. These tests assert that the
//! plan *predicts* them: the extent of the fixed-size vectors, the
//! DHEADER presence, the EMHEADER member-id and length-code sequence,
//! and the key-holder shape.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{
    Ast, Bound, CharWidth, Extensibility, MemberSpec, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{
    DepCollector, Encoding, Endianness, Extent, KeyHashForm, LayoutKey, LengthCode,
    LengthCodePolicy, MemberHeader, TypeLayout, key_hash_form, key_holder_plan, key_members, plan,
};

/// XCDR2 little-endian, the encoding every vector is written in.
fn xcdr2() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

/// The oracle's own framing: V-10 uses LC4 with an explicit NEXTINT,
/// which XTypes 1.3 §7.4.3.4.2 permits alongside the LC5 optimisation.
fn xcdr2_universal() -> LayoutKey {
    LayoutKey::data(Encoding::PlCdr2, Endianness::Little)
        .with_length_codes(LengthCodePolicy::UniversalNextInt)
}

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
                    width: CharWidth::Char,
                    bound: Bound::Unbounded,
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
    ) -> Id<zerodds_ast::MemberNode> {
        self.ast
            .add_member(aggregate, MemberSpec::new(name, Ref::Resolved(type_)))
            .expect("member")
    }

    fn set_member_id(&mut self, member: Id<zerodds_ast::MemberNode>, id: u32) {
        let any = self.ast.any_of_member(member).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .member_id = Some(zerodds_ast::MemberIdAssignment {
            effective: id,
            origin: zerodds_ast::MemberIdOrigin::Declared,
        });
    }

    fn mark_key(&mut self, member: Id<zerodds_ast::MemberNode>) {
        let any = self.ast.any_of_member(member).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .is_key = true;
    }

    fn mark_optional(&mut self, member: Id<zerodds_ast::MemberNode>) {
        let any = self.ast.any_of_member(member).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .is_optional = true;
    }

    fn plan(&self, id: Id<TypeNode>, key: LayoutKey) -> TypeLayout {
        let mut deps = DepCollector::new();
        plan(&self.ast, id, key, &mut deps).expect("a plan")
    }
}

// V-1: @final Empty {} — zero-byte payload.
#[test]
fn v1_empty_final_struct_plans_zero_bytes_and_no_dheader() {
    let mut m = Model::new();
    let empty = m.structure("Empty", Extensibility::Final);
    let layout = m.plan(empty, xcdr2());

    assert_eq!(layout.extent, Extent::Sized(0));
    assert!(!layout.needs_dheader);
    assert!(layout.members.is_empty());
}

// V-2: @final Point { long x; long y; } — eight bytes.
#[test]
fn v2_final_two_int32_plans_eight_bytes() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let point = m.structure("Point", Extensibility::Final);
    let _ = m.member(point, "x", long);
    let _ = m.member(point, "y", long);

    let layout = m.plan(point, xcdr2());
    assert_eq!(layout.extent, Extent::Sized(8));
    assert!(!layout.needs_dheader);
    assert_eq!(
        layout.members[1].offset,
        zerodds_ast_layout::Offset::Fixed(4)
    );
}

// V-3: @final mixed primitives.
//
// # The one vector this file does not follow
//
// V-3's own comment block at `e1f3af895` computes 48 bytes by padding
// the trailing double to align(8) and heading the table "OMG XTypes 1.3
// §7.4.2 alignment (PLAIN_CDR2)". Nothing in XTypes says that. §7.4.1
// says `MAXALIGN(VERSION2) = 4`; §7.6.8 repeats it as "the maximum
// alignment in PLAIN_CDR2 (i.e., 4)". The correct extent is 44.
//
// The same comment block also lists `l = -1234567` as `B9 29 ED FF`.
// That is 0xFFED29B9, which is -1234503. The two-place error is the
// clearest evidence that the table was computed by hand and never
// compared against an implementation.
//
// Two foreign implementations were asked for these exact bytes on
// 2026-08-09, recorded in `tests/fixtures/vendor_bytes.md`:
//
//   cyclone-xcdr2  01abc7cf31d40000 7929edff ceca2300 4f9721c5ffffffff
//                  15cd5b0700000000 00002040 6e861bf0f9210940   (44)
//   rti-xcdr2      … identical …                                (44)
//   cyclone-xcdr1  … with four padding bytes before the double  (48)
//   rti-xcdr1      … identical …                                (48)
//
// So 48 is the XCDR1 extent, which is what the hand table computed.
#[test]
fn v3_final_mixed_primitives_plans_forty_four_under_xcdr2() {
    let mut m = Model::new();
    let s = m.structure("Mixed", Extensibility::Final);
    // boolean, octet, short, ushort, long, ulong, longlong, ulonglong,
    // float, double — the oracle's V-3 member list.
    for (name, kind) in [
        ("b", PrimitiveKind::Boolean),
        ("o", PrimitiveKind::Octet),
        ("s", PrimitiveKind::Int16),
        ("us", PrimitiveKind::UInt16),
        ("l", PrimitiveKind::Int32),
        ("ul", PrimitiveKind::UInt32),
        ("ll", PrimitiveKind::Int64),
        ("ull", PrimitiveKind::UInt64),
        ("f", PrimitiveKind::Float32),
        ("d", PrimitiveKind::Float64),
    ] {
        let t = m.primitive(&format!("T_{name}"), kind);
        let _ = m.member(s, name, t);
    }

    let layout = m.plan(s, xcdr2());
    assert_eq!(
        layout.extent,
        Extent::Sized(44),
        "Cyclone and RTI both put 44 bytes on the wire"
    );

    let v1 = m.plan(s, LayoutKey::data(Encoding::PlainCdr1, Endianness::Little));
    assert_eq!(
        v1.extent,
        Extent::Sized(48),
        "the same members are 48 bytes under XCDR1, which is what the \
         hand-written table actually computed"
    );
}

// V-4: @final { string text; } — never fixed-size.
#[test]
fn v4_final_string_is_unbounded_in_extent() {
    let mut m = Model::new();
    let text = m.string("S");
    let s = m.structure("Message", Extensibility::Final);
    let _ = m.member(s, "text", text);

    let layout = m.plan(s, xcdr2());
    assert_eq!(layout.extent, Extent::Unbounded);
}

// V-5: @final Bag { sequence<long> ids; } — unbounded.
#[test]
fn v5_final_sequence_of_int32_is_unbounded() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let seq = m
        .ast
        .add_type(
            m.root,
            "LSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(long),
                bound: Bound::Unbounded,
            },
        )
        .expect("LSeq");
    let bag = m.structure("Bag", Extensibility::Final);
    let _ = m.member(bag, "ids", seq);

    let layout = m.plan(bag, xcdr2());
    assert_eq!(layout.extent, Extent::Unbounded);
}

// V-6: @final Labelled { sequence<string> tags; } — unbounded, and its
// element is not a primitive, which is what makes it LC5 under the
// compact policy.
//
// The aggregate was called `Tags` until the §7.2.3 case rule was
// enforced, which is when the vector turned out to be untranslatable:
// `Tags` and `tags` share a scope. It measured what it meant to measure,
// through IDL no peer's compiler would have accepted.
#[test]
fn v6_final_sequence_of_string_is_unbounded_and_frames_compactly_as_lc5() {
    let mut m = Model::new();
    let text = m.string("S");
    let seq = m
        .ast
        .add_type(
            m.root,
            "StrSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(text),
                bound: Bound::Unbounded,
            },
        )
        .expect("StrSeq");
    let tags = m.structure("Labelled", Extensibility::Mutable);
    let member = m.member(tags, "tags", seq);
    m.set_member_id(member, 1);

    let layout = m.plan(tags, xcdr2());
    assert_eq!(layout.extent, Extent::Unbounded);

    let mutable = m.plan(tags, LayoutKey::data(Encoding::PlCdr2, Endianness::Little));
    let MemberHeader::EmHeader { lc, .. } = mutable.members[0].header else {
        panic!("expected an EMHEADER");
    };
    assert_eq!(lc, LengthCode::Lc5);
}

// V-7: @final Outer::Inner::S { long v; } — nesting does not change the
// wire form.
#[test]
fn v7_nested_modules_do_not_change_the_wire_form() {
    let mut m = Model::new();
    let outer = m.ast.ensure_module(m.root, "Outer").expect("Outer");
    let inner = m.ast.ensure_module(outer, "Inner").expect("Inner");
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m
        .ast
        .add_type(
            inner,
            "S",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("S");
    // The vector says `@final`, and the model keeps "said nothing" apart
    // from "said final": without the annotation the type is appendable
    // and carries a DHEADER, which is a different wire form.
    let any = m.ast.any_of_type(s).expect("any");
    m.ast
        .facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .extensibility = Some(Extensibility::Final);
    let _ = m
        .ast
        .add_member(s, MemberSpec::new("v", Ref::Resolved(long)))
        .expect("v");

    let layout = m.plan(s, xcdr2());
    assert_eq!(layout.extent, Extent::Sized(4));
}

// V-8: @final keyed Sensor { @key long id; double value; } — a
// four-byte key holder, zero-padded into the KeyHash.
#[test]
fn v8_keyed_sensor_plans_a_four_byte_zero_padded_key_holder() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let double = m.primitive("D", PrimitiveKind::Float64);
    let sensor = m.structure("Sensor", Extensibility::Final);
    let id = m.member(sensor, "id", long);
    let _ = m.member(sensor, "value", double);
    m.mark_key(id);

    let mut deps = DepCollector::new();
    let holder = key_holder_plan(&m.ast, sensor, Encoding::PlainCdr2, &mut deps).expect("holder");

    // Only the @key member is in the holder.
    assert_eq!(holder.members.len(), 1);
    assert_eq!(holder.extent, Extent::Sized(4));
    assert_eq!(key_hash_form(&holder), KeyHashForm::ZeroPadded);

    let keyed = key_members(&m.ast, sensor).expect("key members");
    assert_eq!(keyed.len(), 1);
}

// V-9: @appendable { long a; long b; } — a DHEADER precedes the body.
#[test]
fn v9_appendable_plans_a_dheader() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.structure("Appendable", Extensibility::Appendable);
    let _ = m.member(s, "a", long);
    let _ = m.member(s, "b", long);

    let layout = m.plan(s, xcdr2());
    assert!(layout.needs_dheader, "V-9: appendable carries a DHEADER");
    // The oracle's body is eight bytes behind a four-byte DHEADER.
    assert_eq!(layout.members.len(), 2);
}

// V-10: @mutable { @id(1) long a; @id(2) string b; } — a DHEADER, then
// an EMHEADER per member. The oracle frames the string with LC4 and an
// explicit NEXTINT, which is what the universal policy produces.
#[test]
fn v10_mutable_plans_the_oracle_member_id_and_length_code_sequence() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let text = m.string("S");
    let s = m.structure("Mutable", Extensibility::Mutable);
    let a = m.member(s, "a", long);
    let b = m.member(s, "b", text);
    m.set_member_id(a, 1);
    m.set_member_id(b, 2);

    let layout = m.plan(s, xcdr2_universal());
    assert!(layout.needs_dheader);
    assert_eq!(layout.members.len(), 2);

    // Member ids 1 and 2, in that order.
    assert_eq!(layout.members[0].member_id, 1);
    assert_eq!(layout.members[1].member_id, 2);

    // Both LC4 under the oracle's policy: the int32 because four bytes
    // fits, the string because the universal form carries an explicit
    // NEXTINT.
    for member in &layout.members {
        let MemberHeader::EmHeader {
            lc,
            must_understand,
        } = member.header
        else {
            panic!("expected an EMHEADER");
        };
        assert_eq!(lc, LengthCode::Lc4, "V-10 pins lc=4 for both members");
        assert!(!must_understand);
    }
}

// The same type under the compact policy: the string frames as LC5.
// Both are conformant; they are not the same bytes, which is why the
// policy is explicit.
#[test]
fn v10_under_the_compact_policy_the_string_member_frames_as_lc5() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let text = m.string("S");
    let s = m.structure("Mutable", Extensibility::Mutable);
    let a = m.member(s, "a", long);
    let b = m.member(s, "b", text);
    m.set_member_id(a, 1);
    m.set_member_id(b, 2);

    let layout = m.plan(s, LayoutKey::data(Encoding::PlCdr2, Endianness::Little));
    let MemberHeader::EmHeader { lc: first, .. } = layout.members[0].header else {
        panic!("expected an EMHEADER");
    };
    let MemberHeader::EmHeader { lc: second, .. } = layout.members[1].header else {
        panic!("expected an EMHEADER");
    };
    assert_eq!(first, LengthCode::Lc2, "a four-byte member is LC2");
    assert_eq!(second, LengthCode::Lc5, "a string reuses its length word");
}

// V-11a and V-11b: @mutable O { @id(1) @optional long maybe; } — the
// member carries a header whether it is present or absent, because the
// reader has to be told which.
#[test]
fn v11_optional_member_plans_a_header_regardless_of_presence() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.structure("Optional", Extensibility::Mutable);
    let maybe = m.member(s, "maybe", long);
    m.set_member_id(maybe, 1);
    m.mark_optional(maybe);

    let layout = m.plan(s, xcdr2_universal());
    assert!(layout.needs_dheader, "V-11: a DHEADER precedes the body");
    let MemberHeader::EmHeader { lc, .. } = layout.members[0].header else {
        panic!("V-11: an optional member needs an EMHEADER");
    };
    assert_eq!(lc, LengthCode::Lc4);
    assert_eq!(layout.members[0].member_id, 1);
}

// V-12: empty @mutable — a DHEADER of zero and no members.
#[test]
fn v12_empty_mutable_plans_a_dheader_and_no_members() {
    let mut m = Model::new();
    let s = m.structure("EmptyMutable", Extensibility::Mutable);

    let layout = m.plan(s, xcdr2());
    assert!(layout.needs_dheader, "V-12: the DHEADER is present at zero");
    assert!(layout.members.is_empty());
}

// -- key holder rules beyond V-8 -----------------------------------------

/// A type with no key member has no KeyHash, and `key_members` is where
/// that is decided.
///
/// The holder plan itself keeps every member, which is the second half
/// of XTypes 1.3 §7.6.8 Step 1 — "otherwise do not remove any members" —
/// and what RTI's XCDR interpreter produces when it is driven past the
/// plugin (`tests/fixtures/vendor_bytes.md` §5). The plan cannot make
/// the refusal instead, because it is re-entered at every nested
/// aggregate: a keyless struct reached through a `@key` member is the
/// same call, and there it *must* contribute all of its members.
///
/// So the split is deliberate. `key_members` answers "is there a key at
/// all", both callers ask it first, and neither reaches the holder for a
/// keyless type.
#[test]
fn a_type_with_no_key_member_has_no_key_members_and_a_whole_type_holder() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.structure("Unkeyed", Extensibility::Final);
    let _ = m.member(s, "v", long);

    assert!(
        key_members(&m.ast, s).expect("a key set").is_empty(),
        "no key member, so no KeyHash is emitted for it at all"
    );

    let mut deps = DepCollector::new();
    let holder = key_holder_plan(&m.ast, s, Encoding::PlainCdr2, &mut deps).expect("holder");
    assert_eq!(
        holder.members.len(),
        1,
        "Step 1's second half: nothing is removed from a keyless holder"
    );
    assert_eq!(holder.extent, Extent::Sized(4));
}

#[test]
fn a_key_holder_over_sixteen_bytes_is_hashed() {
    let mut m = Model::new();
    let long_long = m.primitive("LL", PrimitiveKind::Int64);
    let s = m.structure("WideKey", Extensibility::Final);
    for name in ["a", "b", "c"] {
        let member = m.member(s, name, long_long);
        m.mark_key(member);
    }

    let mut deps = DepCollector::new();
    let holder = key_holder_plan(&m.ast, s, Encoding::PlainCdr2, &mut deps).expect("holder");
    assert_eq!(key_hash_form(&holder), KeyHashForm::Md5);
}

#[test]
fn an_unbounded_key_member_is_always_hashed() {
    let mut m = Model::new();
    let text = m.string("S");
    let s = m.structure("StringKey", Extensibility::Final);
    let member = m.member(s, "name", text);
    m.mark_key(member);

    let mut deps = DepCollector::new();
    let holder = key_holder_plan(&m.ast, s, Encoding::PlainCdr2, &mut deps).expect("holder");
    assert_eq!(key_hash_form(&holder), KeyHashForm::Md5);
}

#[test]
fn key_members_come_back_in_member_id_order_not_declaration_order() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.structure("Keyed", Extensibility::Mutable);
    // Declared b then a, but with ids 2 and 1.
    let b = m.member(s, "b", long);
    let a = m.member(s, "a", long);
    m.set_member_id(b, 2);
    m.set_member_id(a, 1);
    m.mark_key(b);
    m.mark_key(a);

    let keyed = key_members(&m.ast, s).expect("key members");
    let a_node = m.ast.member(a).expect("a").node_id;
    let b_node = m.ast.member(b).expect("b").node_id;
    // Two peers that disagree about this order compute different hashes
    // for the same sample.
    assert_eq!(keyed, vec![a_node, b_node]);
}

#[test]
fn nested_keyed_members_contribute_their_own_keys() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let inner = m.structure("Inner", Extensibility::Final);
    let inner_key = m.member(inner, "ik", long);
    m.mark_key(inner_key);

    let outer = m.structure("Outer", Extensibility::Final);
    let nested = m.member(outer, "inner", inner);
    m.mark_key(nested);

    let keyed = key_members(&m.ast, outer).expect("key members");
    assert_eq!(keyed.len(), 2, "the nested key member contributes too");
}

#[test]
fn a_little_endian_key_request_is_rejected() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.structure("Keyed", Extensibility::Final);
    let member = m.member(s, "id", long);
    m.mark_key(member);

    // Honouring it would produce a hash no peer agrees with.
    let little = LayoutKey {
        endianness: Endianness::Little,
        ..LayoutKey::key(Encoding::PlainCdr2)
    };
    let mut deps = DepCollector::new();
    assert!(matches!(
        plan(&m.ast, s, little, &mut deps),
        Err(zerodds_ast_layout::LayoutError::KeyMustBeBigEndian)
    ));
}
