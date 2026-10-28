// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR1 and XCDR2 layout planning. Spec §7, OMG XTypes 1.3 §7.4.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{
    Ast, Bound, CharWidth, Extensibility, MemberSpec, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};
use zerodds_ast_layout::{
    DepCollector, Encoding, Endianness, Extent, LayoutCache, LayoutError, LayoutKey, LengthCode,
    MemberHeader, Offset, TypeLayout, WcharPosition, length_code, plan, primitive_layout,
};

fn v1() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
}

fn v2() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

fn plan_of(ast: &Ast, id: Id<TypeNode>, key: LayoutKey) -> TypeLayout {
    let mut deps = DepCollector::new();
    plan(ast, id, key, &mut deps).expect("a plan")
}

/// A model with the primitive types most tests need.
struct Fixture {
    ast: Ast,
    root: zerodds_ast_core::Id<zerodds_ast::ModuleNode>,
}

impl Fixture {
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

    fn structure(&mut self, name: &str) -> Id<TypeNode> {
        self.ast
            .add_type(
                self.root,
                name,
                TypeKind::Structure {
                    base: None,
                    members: Vec::new(),
                },
            )
            .expect("struct")
    }

    fn set_extensibility(&mut self, id: Id<TypeNode>, value: Extensibility) {
        let any = self.ast.any_of_type(id).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .extensibility = Some(value);
    }

    fn member(&mut self, aggregate: Id<TypeNode>, name: &str, type_: Id<TypeNode>) {
        let _ = self
            .ast
            .add_member(aggregate, MemberSpec::new(name, Ref::Resolved(type_)))
            .expect("member");
    }
}

// -- primitives and alignment --------------------------------------------

#[test]
fn xcdr1_aligns_every_primitive_to_its_own_size() {
    for kind in PrimitiveKind::ALL {
        let (align, size) = primitive_layout(
            kind,
            LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
            WcharPosition::Member,
        );
        let expected = match kind {
            PrimitiveKind::Boolean
            | PrimitiveKind::Octet
            | PrimitiveKind::Char
            | PrimitiveKind::Int8
            | PrimitiveKind::UInt8 => 1,
            PrimitiveKind::Int16 | PrimitiveKind::UInt16 | PrimitiveKind::WChar => 2,
            PrimitiveKind::Int32 | PrimitiveKind::UInt32 | PrimitiveKind::Float32 => 4,
            _ => 8,
        };
        assert_eq!(align, expected, "{kind:?} alignment under XCDR1");
        assert!(size >= u32::from(align));
    }
}

#[test]
fn xcdr2_caps_alignment_at_four_and_leaves_sizes_alone() {
    // XTypes 1.3 §7.4.1: `MAXALIGN(VERSION2) = 4`, and `ALIGN(N)`
    // aligns to `MIN(N, XCDR.maxalign)`. A primitive keeps its size;
    // only what it aligns to changes.
    //
    // This test replaces one that asserted the opposite. That one was
    // written from wire vector V-3, which is not an OMG artefact —
    // see `Encoding::max_alignment` and
    // `tests/fixtures/vendor_bytes.md`.
    for kind in PrimitiveKind::ALL {
        let (v1_align, v1_size) = primitive_layout(
            kind,
            LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
            WcharPosition::Member,
        );
        let (v2_align, v2_size) = primitive_layout(
            kind,
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            WcharPosition::Member,
        );
        assert_eq!(v2_align, v1_align.min(4), "{kind:?} alignment under XCDR2");
        assert_eq!(v1_size, v2_size, "{kind:?} size across versions");
    }
    assert_eq!(
        primitive_layout(
            PrimitiveKind::Float64,
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            WcharPosition::Member
        )
        .0,
        4
    );
    assert_eq!(
        primitive_layout(
            PrimitiveKind::Float64,
            LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
            WcharPosition::Member
        )
        .0,
        8
    );
}

#[test]
fn long_double_is_sixteen_bytes() {
    let (_, size) = primitive_layout(
        PrimitiveKind::Float128,
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
        WcharPosition::Member,
    );
    assert_eq!(size, 16);
}

// -- struct layout -------------------------------------------------------

#[test]
fn a_final_struct_pads_between_members() {
    let mut f = Fixture::new();
    let octet = f.primitive("O", PrimitiveKind::Octet);
    let long = f.primitive("L", PrimitiveKind::Int32);
    let point = f.structure("Point");
    f.set_extensibility(point, Extensibility::Final);
    f.member(point, "a", octet);
    f.member(point, "b", long);

    let layout = plan_of(&f.ast, point, v1());
    // struct { octet a; int32 b; } — a at 0, b at 4 after three bytes of
    // padding.
    assert_eq!(layout.members[0].offset, Offset::Fixed(0));
    assert_eq!(layout.members[1].offset, Offset::Fixed(4));
    assert_eq!(layout.align, 4);
    assert_eq!(layout.extent, Extent::Sized(8));
}

#[test]
fn a_nested_struct_propagates_its_alignment() {
    let mut f = Fixture::new();
    let long_long = f.primitive("LL", PrimitiveKind::Int64);
    let inner = f.structure("Inner");
    f.set_extensibility(inner, Extensibility::Final);
    f.member(inner, "v", long_long);

    let outer = f.structure("Outer");
    f.set_extensibility(outer, Extensibility::Final);
    let octet = f.primitive("O", PrimitiveKind::Octet);
    f.member(outer, "flag", octet);
    f.member(outer, "inner", inner);

    let layout = plan_of(&f.ast, outer, v1());
    assert_eq!(layout.align, 8);
    // The nested struct starts at the next multiple of eight.
    assert_eq!(layout.members[1].offset, Offset::Fixed(8));
}

#[test]
fn a_sequence_makes_everything_after_it_dynamic() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let seq = f
        .ast
        .add_type(
            f.root,
            "LSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(long),
                bound: Bound::Unbounded,
            },
        )
        .expect("LSeq");
    let s = f.structure("Holder");
    f.set_extensibility(s, Extensibility::Final);
    f.member(s, "items", seq);
    f.member(s, "count", long);

    let layout = plan_of(&f.ast, s, v1());
    assert_eq!(layout.members[0].offset, Offset::Fixed(0));
    assert_eq!(layout.members[1].offset, Offset::Dynamic);
    assert_eq!(layout.extent, Extent::Unbounded);
}

#[test]
fn a_bounded_string_is_still_unbounded_in_extent() {
    let mut f = Fixture::new();
    let bound = f.ast.add_int_expr(8).expect("8");
    let s = f
        .ast
        .add_type(
            f.root,
            "Name",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Const(bound),
            },
        )
        .expect("Name");

    // The bound caps the content, not the written length, so the extent
    // is not fixed.
    let layout = plan_of(&f.ast, s, v1());
    assert_eq!(layout.extent, Extent::Unbounded);
}

#[test]
fn a_fixed_size_array_multiplies_its_element_extent() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let dim = f.ast.add_int_expr(4).expect("4");
    let array = f
        .ast
        .add_type(
            f.root,
            "Quad",
            TypeKind::Array {
                element: Ref::Resolved(long),
                dims: vec![dim],
            },
        )
        .expect("Quad");

    let layout = plan_of(&f.ast, array, v1());
    assert_eq!(layout.extent, Extent::Sized(16));
}

#[test]
fn an_unresolved_member_type_fails_loudly_rather_than_guessing_a_size() {
    let mut f = Fixture::new();
    let s = f.structure("Holder");
    let _ = f
        .ast
        .add_member(
            s,
            MemberSpec::new(
                "x",
                Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(vec![
                    "Absent".to_string(),
                ]))),
            ),
        )
        .expect("x");

    let mut deps = DepCollector::new();
    assert!(matches!(
        plan(&f.ast, s, v1(), &mut deps),
        Err(LayoutError::Unresolved { .. })
    ));
}

// -- extensibility and headers -------------------------------------------

#[test]
fn xcdr2_gives_a_dheader_to_anything_that_is_not_final() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);

    for (extensibility, expected) in [
        (Extensibility::Final, false),
        (Extensibility::Appendable, true),
        (Extensibility::Mutable, true),
    ] {
        let s = f.structure("S");
        f.set_extensibility(s, extensibility);
        f.member(s, "x", long);
        let layout = plan_of(&f.ast, s, v2());
        assert_eq!(
            layout.needs_dheader, expected,
            "{extensibility:?} under XCDR2"
        );
    }
}

#[test]
fn xcdr1_plans_no_dheader_at_all() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);

    // XCDR1 has no delimiter header; an appendable type carries its
    // extensibility some other way or not at all.
    for extensibility in Extensibility::ALL {
        let s = f.structure("S");
        f.set_extensibility(s, extensibility);
        f.member(s, "x", long);
        let layout = plan_of(&f.ast, s, v1());
        assert!(!layout.needs_dheader, "{extensibility:?} under XCDR1");
    }
}

#[test]
fn a_mutable_struct_frames_every_member_with_a_header() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let s = f.structure("Mutable");
    f.set_extensibility(s, Extensibility::Mutable);
    f.member(s, "x", long);
    f.member(s, "y", long);

    let v1_layout = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlCdr1, Endianness::Little),
    );
    for member in &v1_layout.members {
        assert!(matches!(member.header, MemberHeader::PidCdr1 { .. }));
    }

    let v2_layout = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little),
    );
    for member in &v2_layout.members {
        assert!(matches!(member.header, MemberHeader::EmHeader { .. }));
    }
}

#[test]
fn a_member_id_above_the_short_pid_range_needs_the_extended_form() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let s = f.structure("Big");
    f.set_extensibility(s, Extensibility::Mutable);
    f.member(s, "x", long);

    let member = *f.ast.members_of(s).first().expect("member");
    let any = f.ast.any_of_member(member).expect("any");
    f.ast
        .facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .member_id = Some(zerodds_ast::MemberIdAssignment {
        effective: 0x4000,
        origin: zerodds_ast::MemberIdOrigin::Declared,
    });

    let layout = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlCdr1, Endianness::Little),
    );
    let MemberHeader::PidCdr1 { extended, .. } = layout.members[0].header else {
        panic!("expected a PID header");
    };
    assert!(extended);
}

#[test]
fn must_understand_is_set_exactly_when_the_facet_says_so() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let s = f.structure("Mutable");
    f.set_extensibility(s, Extensibility::Mutable);
    f.member(s, "plain", long);
    f.member(s, "required", long);

    let required = f.ast.members_of(s)[1];
    let any = f.ast.any_of_member(required).expect("any");
    f.ast
        .facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .is_must_understand = true;

    let layout = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little),
    );
    let MemberHeader::EmHeader {
        must_understand: first,
        ..
    } = layout.members[0].header
    else {
        panic!("expected an EMHEADER");
    };
    let MemberHeader::EmHeader {
        must_understand: second,
        ..
    } = layout.members[1].header
    else {
        panic!("expected an EMHEADER");
    };
    assert!(!first);
    assert!(second);
}

#[test]
fn an_optional_member_carries_a_header_in_every_extensibility() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);

    for extensibility in Extensibility::ALL {
        let s = f.structure("S");
        f.set_extensibility(s, extensibility);
        f.member(s, "maybe", long);
        let member = *f.ast.members_of(s).first().expect("member");
        let any = f.ast.any_of_member(member).expect("any");
        f.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .is_optional = true;

        let layout = plan_of(&f.ast, s, v2());
        assert!(
            !matches!(layout.members[0].header, MemberHeader::NoHeader),
            "an @optional member under {extensibility:?} needs a header"
        );
    }
}

// -- length codes, XTypes 1.3 §7.4.2 -------------------------------------

fn lc(ast: &Ast, id: Id<TypeNode>) -> LengthCode {
    let mut deps = DepCollector::new();
    length_code(
        ast,
        id,
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little),
        &mut deps,
        0,
    )
    .expect("a length code")
}

#[test]
fn all_eight_length_codes_are_representable() {
    assert_eq!(LengthCode::ALL.len(), 8);
    assert_eq!(LengthCode::Lc5.bits(), 5);
    assert!(LengthCode::Lc5.reuses_leading_length());
    assert!(!LengthCode::Lc4.reuses_leading_length());
}

#[test]
fn fixed_size_members_map_onto_lc0_through_lc3() {
    let mut f = Fixture::new();
    let one = f.primitive("O", PrimitiveKind::Octet);
    let two = f.primitive("S", PrimitiveKind::Int16);
    let four = f.primitive("L", PrimitiveKind::Int32);
    let eight = f.primitive("LL", PrimitiveKind::Int64);

    assert_eq!(lc(&f.ast, one), LengthCode::Lc0);
    assert_eq!(lc(&f.ast, two), LengthCode::Lc1);
    assert_eq!(lc(&f.ast, four), LengthCode::Lc2);
    assert_eq!(lc(&f.ast, eight), LengthCode::Lc3);
}

#[test]
fn a_string_member_frames_as_lc5() {
    // The rule that diverged across the emitters this rebuild replaces:
    // a variable-length member reuses its own leading length word as the
    // NEXTINT. LC4 there writes a redundant length and misplaces every
    // member after it.
    let mut f = Fixture::new();
    let s = f
        .ast
        .add_type(
            f.root,
            "Str",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
        )
        .expect("Str");
    assert_eq!(lc(&f.ast, s), LengthCode::Lc5);
}

#[test]
fn a_wstring_member_frames_as_lc5() {
    let mut f = Fixture::new();
    let s = f
        .ast
        .add_type(
            f.root,
            "WStr",
            TypeKind::String {
                width: CharWidth::WChar,
                bound: Bound::Unbounded,
            },
        )
        .expect("WStr");
    assert_eq!(lc(&f.ast, s), LengthCode::Lc5);
}

/// A map frames as Lc5 only when it has a DHEADER to reuse.
///
/// Measured on 2026-08-09 on the Linux host. `@mutable struct { map<long,
/// long> prim; map<long, Cell> agg; long tail; }`: Fast DDS writes
/// `00000040 14000000` over the primitive map — an explicit NEXTINT of
/// twenty over a bare count of two and four longs — and `01000050
/// 10000000` over the aggregate one, reusing the DHEADER. OpenDDS 3.34.0
/// agrees on the first. Rule (14) exempts a map whose key and value are
/// both primitive from the delimiter, and with no delimiter the leading
/// word is the entry count, which Lc5 would read as a byte length.
#[test]
fn a_map_frames_as_lc5_only_when_it_carries_a_dheader() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let prim = f
        .ast
        .add_type(
            f.root,
            "Lookup",
            TypeKind::Map {
                key: Ref::Resolved(long),
                value: Ref::Resolved(long),
                bound: Bound::Unbounded,
            },
        )
        .expect("Lookup");
    assert_eq!(lc(&f.ast, prim), LengthCode::Lc4);

    let cell = f.structure("Cell");
    f.member(cell, "a", long);
    let agg = f
        .ast
        .add_type(
            f.root,
            "CellLookup",
            TypeKind::Map {
                key: Ref::Resolved(long),
                value: Ref::Resolved(cell),
                bound: Bound::Unbounded,
            },
        )
        .expect("CellLookup");
    assert_eq!(lc(&f.ast, agg), LengthCode::Lc5);
}

/// An array of a non-primitive element reuses its DHEADER.
///
/// Measured on 2026-08-09. `@mutable struct { Cell arr[2]; }` is
/// `06000050 10000000` and then the four longs in Cyclone and in Fast
/// DDS, and `06000040 10000000` and then the same four in RTI, which
/// writes no array DHEADER. This planner used to answer Lc4 while still
/// writing the DHEADER, so it spent four bytes on a NEXTINT that
/// duplicated the delimiter behind it — a framing no implementation
/// emits.
#[test]
fn an_array_of_aggregates_reuses_its_dheader_as_the_nextint() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let cell = f.structure("Cell");
    f.member(cell, "a", long);
    f.member(cell, "b", long);
    let two = f.ast.add_int_expr(2).expect("2");
    let arr = f
        .ast
        .add_type(
            f.root,
            "Cells",
            TypeKind::Array {
                element: Ref::Resolved(cell),
                dims: vec![two],
            },
        )
        .expect("Cells");
    assert_eq!(lc(&f.ast, arr), LengthCode::Lc5);
}

#[test]
fn a_sequence_of_non_primitives_frames_as_lc5() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let inner = f.structure("Inner");
    f.member(inner, "v", long);
    let seq = f
        .ast
        .add_type(
            f.root,
            "InnerSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(inner),
                bound: Bound::Unbounded,
            },
        )
        .expect("InnerSeq");
    assert_eq!(lc(&f.ast, seq), LengthCode::Lc5);
}

#[test]
fn the_sequence_length_code_follows_the_element_width() {
    // Transcribed from Cyclone's get_length_code_seq and cross-checked
    // against Fast-CDR's get_serialized_member_size. A size-based rule
    // gets the two-byte case wrong, which is why each width is asserted
    // rather than inferred.
    let mut f = Fixture::new();
    for (name, kind, expected) in [
        ("O", PrimitiveKind::Octet, LengthCode::Lc5),
        ("B", PrimitiveKind::Boolean, LengthCode::Lc5),
        ("S", PrimitiveKind::Int16, LengthCode::Lc4),
        ("W", PrimitiveKind::WChar, LengthCode::Lc4),
        ("L", PrimitiveKind::Int32, LengthCode::Lc6),
        ("F", PrimitiveKind::Float32, LengthCode::Lc6),
        ("LL", PrimitiveKind::Int64, LengthCode::Lc7),
        ("D", PrimitiveKind::Float64, LengthCode::Lc7),
    ] {
        let element = f.primitive(name, kind);
        let seq = f
            .ast
            .add_type(
                f.root,
                &format!("{name}Seq"),
                TypeKind::Sequence {
                    element: Ref::Resolved(element),
                    bound: Bound::Unbounded,
                },
            )
            .expect("sequence");
        assert_eq!(lc(&f.ast, seq), expected, "sequence of {kind:?}");
    }
}

#[test]
fn a_union_member_frames_with_an_explicit_nextint() {
    // Its length depends on the active branch, so nothing in its leading
    // bytes states it. Cyclone returns LENGTH_CODE_NEXTINT here too.
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let u = f
        .ast
        .add_type(
            f.root,
            "Value",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Value");
    assert_eq!(lc(&f.ast, u), LengthCode::Lc4);
}

#[test]
fn a_typedef_of_a_string_still_frames_as_lc5() {
    // A typedef is transparent on the wire, so the code must follow
    // through it rather than treating the alias as an opaque type.
    let mut f = Fixture::new();
    let s = f
        .ast
        .add_type(
            f.root,
            "Str",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
        )
        .expect("Str");
    let alias = f
        .ast
        .add_type(
            f.root,
            "Name",
            TypeKind::Alias {
                aliased: Ref::Resolved(s),
            },
        )
        .expect("Name");
    assert_eq!(lc(&f.ast, alias), LengthCode::Lc5);
}

// -- endianness ----------------------------------------------------------

#[test]
fn offsets_are_shared_between_little_and_big_endian() {
    let mut f = Fixture::new();
    let octet = f.primitive("O", PrimitiveKind::Octet);
    let long = f.primitive("L", PrimitiveKind::Int32);
    let s = f.structure("Point");
    f.set_extensibility(s, Extensibility::Final);
    f.member(s, "a", octet);
    f.member(s, "b", long);

    // Byte order changes the bytes, not where they sit.
    let little = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Little),
    );
    let big = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlainCdr1, Endianness::Big),
    );
    let little_offsets: Vec<Offset> = little.members.iter().map(|m| m.offset).collect();
    let big_offsets: Vec<Offset> = big.members.iter().map(|m| m.offset).collect();
    assert_eq!(little_offsets, big_offsets);
    assert_eq!(little.extent, big.extent);
}

// -- cache ---------------------------------------------------------------

#[test]
fn a_repeated_query_is_served_from_the_cache() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let mut cache = LayoutCache::new();

    let _ = cache.layout(&f.ast, long, v2()).expect("first");
    let _ = cache.layout(&f.ast, long, v2()).expect("second");
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.misses(), 1);
    assert_eq!(cache.hits(), 1);
}

#[test]
fn both_byte_orders_share_one_cache_entry() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let mut cache = LayoutCache::new();

    let _ = cache
        .layout(
            &f.ast,
            long,
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
        )
        .expect("little");
    let _ = cache
        .layout(
            &f.ast,
            long,
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Big),
        )
        .expect("big");
    assert_eq!(cache.len(), 1);
}

#[test]
fn a_change_evicts_the_type_and_its_dependents_and_nothing_else() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let a = f.structure("A");
    f.member(a, "v", long);
    let b = f.structure("B");
    f.member(b, "inner", a);
    let c = f.structure("C");
    f.member(c, "v", long);

    let mut cache = LayoutCache::new();
    for id in [a, b, c] {
        let _ = cache.layout(&f.ast, id, v2()).expect("plan");
    }
    assert_eq!(cache.len(), 3);

    let c_node = f.ast.type_(c).expect("C").node_id;
    // Change A: A and B-which-contains-A go, C stays.
    let changeset = {
        let mut tx = f.ast.begin();
        let _ = tx
            .ast()
            .add_member(a, MemberSpec::new("extra", Ref::Resolved(long)))
            .expect("extra");
        tx.commit().expect("commit")
    };
    cache.invalidate(&changeset);

    assert!(cache.contains(c_node, v2()), "C should have survived");
    assert_eq!(cache.len(), 1);
}

#[test]
fn a_commit_that_changed_nothing_evicts_nothing() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let mut cache = LayoutCache::new();
    let _ = cache.layout(&f.ast, long, v2()).expect("plan");

    let empty = f.ast.begin().commit().expect("empty commit");
    cache.invalidate(&empty);
    assert_eq!(cache.len(), 1);
}

// -- the length-code policy is configuration, spec §7.1 -------------------

#[test]
fn the_default_policy_is_the_one_the_other_implementations_emit() {
    // Fast DDS and Cyclone both join the NEXTINT with the member's own
    // length word. ZeroDDS follows them by default; the other form is a
    // flag, not the baseline.
    assert_eq!(
        zerodds_ast_layout::LengthCodePolicy::default(),
        zerodds_ast_layout::LengthCodePolicy::Compact
    );
    assert!(zerodds_ast_layout::LengthCodePolicy::Compact.is_default());
    assert_eq!(
        v2().length_codes,
        zerodds_ast_layout::LengthCodePolicy::Compact
    );
    assert_eq!(
        LayoutKey::key(Encoding::PlainCdr2).length_codes,
        zerodds_ast_layout::LengthCodePolicy::Compact
    );
}

#[test]
fn the_policy_parses_from_a_configuration_value_and_rejects_a_typo() {
    use zerodds_ast_layout::LengthCodePolicy;
    for policy in LengthCodePolicy::ALL {
        assert_eq!(LengthCodePolicy::parse(policy.as_str()), Some(policy));
    }
    // A wire-affecting setting must not silently fall back on a typo.
    assert_eq!(LengthCodePolicy::parse("compakt"), None);
    assert_eq!(LengthCodePolicy::parse(""), None);
}

#[test]
fn the_two_policies_produce_different_framing_for_the_same_type() {
    // If they agreed there would be nothing to configure.
    use zerodds_ast_layout::LengthCodePolicy;
    let mut f = Fixture::new();
    let text = f
        .ast
        .add_type(
            f.root,
            "S",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
        )
        .expect("S");
    let s = f.structure("M");
    f.set_extensibility(s, Extensibility::Mutable);
    f.member(s, "text", text);

    let compact = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little),
    );
    let universal = plan_of(
        &f.ast,
        s,
        LayoutKey::data(Encoding::PlCdr2, Endianness::Little)
            .with_length_codes(LengthCodePolicy::UniversalNextInt),
    );
    assert_ne!(compact.members[0].header, universal.members[0].header);
}

/// A bitset that inherits is sized over the whole chain, not over the
/// bitfields it declares itself.
///
/// The anchor width is the only wire question a bitset asks, and DDS-
/// XTypes 1.3 answers none of it: §7.4.3.5.3 runs from rule (1) to rule
/// (30) and no left-hand side is a bitset, and §7.4.1.1 stops at
/// structures. The rule below is therefore measured, not read.
///
/// eProsima Fast DDS 3.6.1 / Fast CDR 2.3.5 on the Linux test host,
/// 2026-08-09. It is the only one of the three foreign implementations
/// that parses `bitset` at all — Cyclone DDS 11.0.1's `idlc` reports a
/// syntax error on the keyword and RTI Connext 7.7's `rtiddsgen`
/// reports `mismatched input 'bitset' expecting EOF`, in every target
/// language.
///
/// ```idl
/// bitset B1 { bitfield<8> a; };
/// bitset B2 : B1 { bitfield<8> b; };
/// bitset B3 : B2 { bitfield<4> c; };
/// @final struct Sample { B3 value; octet tail; };
/// ```
///
/// with `a = 0x11, b = 0x22, c = 0x3, tail = 0xAB`, Fast DDS writes
/// `11220300 ab` in little endian and `00032211 ab` in big endian under
/// both XCDR1 and XCDR2. Twenty bits, so a four-byte anchor and the
/// trailing octet at offset four; `fastddsgen` emits a
/// `std::bitset<20>` and packs `a` at 0..8, `b` at 8..16 and `c` at
/// 16..20, base first.
///
/// Summing only `B3`'s own four bits gave a one-byte anchor and put the
/// octet at offset one.
#[test]
fn a_bitset_that_inherits_is_sized_over_the_whole_chain() {
    let mut f = Fixture::new();
    let octet = f.primitive("octet", PrimitiveKind::UInt8);

    let b1 = f
        .ast
        .add_type(
            f.root,
            "B1",
            TypeKind::Bitset {
                base: None,
                fields: Vec::new(),
            },
        )
        .expect("B1");
    f.ast.add_bitfield(b1, Some("a"), 8, None).expect("a");

    let b2 = f
        .ast
        .add_type(
            f.root,
            "B2",
            TypeKind::Bitset {
                base: Some(Ref::Resolved(b1)),
                fields: Vec::new(),
            },
        )
        .expect("B2");
    f.ast.add_bitfield(b2, Some("b"), 8, None).expect("b");

    let b3 = f
        .ast
        .add_type(
            f.root,
            "B3",
            TypeKind::Bitset {
                base: Some(Ref::Resolved(b2)),
                fields: Vec::new(),
            },
        )
        .expect("B3");
    f.ast.add_bitfield(b3, Some("c"), 4, None).expect("c");

    // Base first, then own, over the whole chain.
    let all: Vec<u16> = f
        .ast
        .all_bitfields_of(b3)
        .iter()
        .map(|id| f.ast.bitfield(*id).expect("a bitfield").bits)
        .collect();
    assert_eq!(all, vec![8, 8, 4]);

    for key in [v1(), v2()] {
        assert_eq!(
            plan_of(&f.ast, b3, key).extent,
            Extent::Sized(4),
            "twenty bits take a four-byte anchor",
        );
        assert_eq!(
            plan_of(&f.ast, b1, key).extent,
            Extent::Sized(1),
            "eight bits take a one-byte anchor",
        );
        assert_eq!(
            plan_of(&f.ast, b2, key).extent,
            Extent::Sized(2),
            "sixteen bits take a two-byte anchor",
        );
    }

    // And the octet behind it lands at offset four, which is where Fast
    // DDS put the `ab`.
    let sample = f.structure("Sample");
    f.set_extensibility(sample, Extensibility::Final);
    f.member(sample, "value", b3);
    f.member(sample, "tail", octet);
    for key in [v1(), v2()] {
        let plan = plan_of(&f.ast, sample, key);
        assert_eq!(plan.members[0].offset, Offset::Fixed(0));
        assert_eq!(plan.members[1].offset, Offset::Fixed(4));
        assert_eq!(plan.extent, Extent::Sized(5));
    }

    // The anchor aligns to its own width. `@final struct { octet head;
    // B3 value; octet tail; }` with `head = 1` came back from Fast DDS
    // as `01000000 11220300 ab` under both versions: three bytes of
    // padding, the anchor at offset four, the octet at eight.
    let padded = f.structure("Padded");
    f.set_extensibility(padded, Extensibility::Final);
    f.member(padded, "head", octet);
    f.member(padded, "value", b3);
    f.member(padded, "tail", octet);
    for key in [v1(), v2()] {
        let plan = plan_of(&f.ast, padded, key);
        assert_eq!(plan.members[1].offset, Offset::Fixed(4));
        assert_eq!(plan.members[2].offset, Offset::Fixed(8));
        assert_eq!(plan.extent, Extent::Sized(9));
    }
}

// -- what has no wire form is refused by name ----------------------------

#[test]
fn any_is_refused_at_planning_and_the_reason_names_the_construct() {
    // Accepted in the model — Building Block Any is one of IDL 4.2's
    // fifteen — and refused here, because DDS-XTypes has no `any` at
    // all. Refusing by name is a different statement from failing to
    // parse: the author of `struct S { any a; };` gets told which
    // construct cannot be encoded and in which encoding, rather than a
    // node id with an unresolved reference.
    let mut fixture = Fixture::new();
    let any = fixture
        .ast
        .add_type(fixture.root, "any", TypeKind::Any)
        .expect("any");

    for key in [v1(), v2()] {
        let mut deps = DepCollector::new();
        match plan(&fixture.ast, any, key, &mut deps) {
            Err(LayoutError::UnsupportedForEncoding { reason, .. }) => {
                assert!(reason.contains("any"), "{reason}");
                assert!(reason.contains("CORBA CDR"), "{reason}");
            }
            other => panic!("`any` was planned for {key:?}: {other:?}"),
        }
    }
}

#[test]
fn an_object_reference_is_refused_at_planning_and_the_reason_names_it() {
    // Reachable from IDL since `Object` was given rules (117) and (118),
    // and refused here for the reason `any` is: DDS-XTypes has no object
    // reference and no type identifier for one.
    let mut fixture = Fixture::new();
    let object = fixture
        .ast
        .add_type(
            fixture.root,
            "Object",
            TypeKind::ObjectRef { interface: None },
        )
        .expect("Object");

    for key in [v1(), v2()] {
        let mut deps = DepCollector::new();
        match plan(&fixture.ast, object, key, &mut deps) {
            Err(LayoutError::UnsupportedForEncoding { reason, .. }) => {
                assert!(reason.contains("object reference"), "{reason}");
                assert!(reason.contains("IOR"), "{reason}");
            }
            other => panic!("`Object` was planned for {key:?}: {other:?}"),
        }
    }
}

#[test]
fn valuebase_is_refused_at_planning_and_the_reason_names_it() {
    let mut fixture = Fixture::new();
    let base = fixture
        .ast
        .add_type(fixture.root, "ValueBase", TypeKind::ValueBase)
        .expect("ValueBase");

    for key in [v1(), v2()] {
        let mut deps = DepCollector::new();
        match plan(&fixture.ast, base, key, &mut deps) {
            Err(LayoutError::UnsupportedForEncoding { reason, .. }) => {
                assert!(reason.contains("ValueBase"), "{reason}");
                assert!(reason.contains("CORBA CDR"), "{reason}");
            }
            other => panic!("`ValueBase` was planned for {key:?}: {other:?}"),
        }
    }
}

// -- recursion -----------------------------------------------------------

/// A type that names itself through a sequence is planned, not refused.
///
/// XTypes 1.3 §7.2.2.4.3 admits it and the wire stays finite: the
/// elements sit behind a length, so a value terminates even though the
/// type does not. The planner used to descend into the element and hit
/// the depth limit, reporting a legal type as `TooDeep`.
///
/// Measured against OpenDDS, the one other compiler in the interop matrix
/// that accepts the same IDL: it writes the same octets for the same
/// sample. Cyclone's `idlc` is killed by the OOM reaper on this file, and
/// `fastddsgen` and `rtiddsgen` refuse it.
#[test]
fn a_sequence_of_the_enclosing_type_is_planned() {
    let mut fixture = Fixture::new();
    let long = fixture.primitive("Long", PrimitiveKind::Int32);
    let node = fixture.structure("Node");
    fixture.set_extensibility(node, Extensibility::Appendable);
    let children = fixture
        .ast
        .add_type(
            fixture.root,
            "NodeSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(node),
                bound: Bound::Unbounded,
            },
        )
        .expect("NodeSeq");
    fixture.member(node, "value", long);
    fixture.member(node, "children", children);

    for key in [v1(), v2()] {
        let layout = plan_of(&fixture.ast, node, key);
        assert_eq!(
            layout.extent,
            Extent::Unbounded,
            "a recursive type has no fixed size, whatever the encoding"
        );
    }
}

/// Two types that reach each other through sequences, which is the same
/// cycle spread over a forward declaration.
///
/// Separate from the self-referential case because the descent enters at
/// a different point: `A` is not on the path when `B`'s sequence asks
/// about it until one full turn has been taken.
#[test]
fn two_types_that_name_each_other_through_sequences_are_planned() {
    let mut fixture = Fixture::new();
    let long = fixture.primitive("Long", PrimitiveKind::Int32);
    let a = fixture.structure("A");
    let b = fixture.structure("B");
    fixture.set_extensibility(a, Extensibility::Appendable);
    fixture.set_extensibility(b, Extensibility::Appendable);
    let bs = fixture
        .ast
        .add_type(
            fixture.root,
            "BSeq",
            TypeKind::Sequence {
                element: Ref::Resolved(b),
                bound: Bound::Unbounded,
            },
        )
        .expect("BSeq");
    let as_ = fixture
        .ast
        .add_type(
            fixture.root,
            "ASeq",
            TypeKind::Sequence {
                element: Ref::Resolved(a),
                bound: Bound::Unbounded,
            },
        )
        .expect("ASeq");
    fixture.member(a, "value", long);
    fixture.member(a, "bs", bs);
    fixture.member(b, "weight", long);
    fixture.member(b, "as", as_);

    for key in [v1(), v2()] {
        assert_eq!(plan_of(&fixture.ast, a, key).extent, Extent::Unbounded);
        assert_eq!(plan_of(&fixture.ast, b, key).extent, Extent::Unbounded);
    }
}

/// A cycle that does not pass through a collection stays an error.
///
/// `struct S { S inner; }` has no finite size — there is no length to
/// sit behind. IDL 4.2 §7.4.1.4.4.4.4 admits a recursive type only
/// via sequence members (a map's length terminates one the same way),
/// so the refusal names the clause instead of tripping the descent
/// limit; the cycle break must not turn it into a plan either way.
/// The same answer covers `@optional` and `@external` members —
/// XTypes 1.3 §7.3.1.2.1.4 calls an @external cycle "not
/// serializable", and RTI refuses the combination outright ("external
/// members can't be marked as optional", measured in
/// matrix/rti/*/recursive-optional-external).
#[test]
fn a_cycle_without_a_collection_is_still_refused() {
    let mut fixture = Fixture::new();
    let s = fixture.structure("S");
    fixture.member(s, "inner", s);

    let mut deps = DepCollector::new();
    match plan(&fixture.ast, s, v2(), &mut deps) {
        Err(LayoutError::RecursionOutsideCollection { .. }) => {}
        other => panic!("a type of infinite size was planned: {other:?}"),
    }
}

/// The named refusal, not the depth limit, for every cycle-bearing
/// shape below — a reader of the error has to learn *why* the type has
/// no plan, and `TooDeep` reports a depth and not a reason.
fn expect_recursion_refusal(ast: &Ast, id: Id<TypeNode>, what: &str) {
    for key in [v1(), v2()] {
        let mut deps = DepCollector::new();
        match plan(ast, id, key, &mut deps) {
            Err(LayoutError::RecursionOutsideCollection { .. }) => {}
            other => panic!("{what} was not refused by name: {other:?}"),
        }
    }
}

/// A cycle through a map's *value* is legal: both halves sit behind the
/// map's element count, exactly as a sequence's elements do.
/// `matrix/cases/recursive-through-map` is the corpus form of the same
/// type.
#[test]
fn a_map_whose_value_is_the_enclosing_type_is_planned() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    f.set_extensibility(node, Extensibility::Appendable);
    let children = f
        .ast
        .add_type(
            f.root,
            "Children",
            TypeKind::Map {
                key: Ref::Resolved(long),
                value: Ref::Resolved(node),
                bound: Bound::Unbounded,
            },
        )
        .expect("Children");
    f.member(node, "value", long);
    f.member(node, "children", children);

    for key in [v1(), v2()] {
        assert_eq!(plan_of(&f.ast, node, key).extent, Extent::Unbounded);
    }
}

/// A cycle through a map's *key* half is behind the same element count
/// as one through its value, so it is planned the same way. Separate
/// from the value case because `plan_at` asks `closes_a_cycle` once per
/// half and a regression could break one and keep the other.
#[test]
fn a_map_whose_key_is_the_enclosing_type_is_planned() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    f.set_extensibility(node, Extensibility::Appendable);
    let index = f
        .ast
        .add_type(
            f.root,
            "Index",
            TypeKind::Map {
                key: Ref::Resolved(node),
                value: Ref::Resolved(long),
                bound: Bound::Unbounded,
            },
        )
        .expect("Index");
    f.member(node, "value", long);
    f.member(node, "index", index);

    for key in [v1(), v2()] {
        assert_eq!(plan_of(&f.ast, node, key).extent, Extent::Unbounded);
    }
}

/// A cycle that closes through a union branch *behind a sequence* is the
/// legal kind: the sequence in the branch is what makes it finite, and
/// the planner has to descend the union to find it.
/// `matrix/cases/recursive-union-branch` is the corpus form.
#[test]
fn a_union_branch_that_reaches_back_through_a_sequence_is_planned() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    f.set_extensibility(node, Extensibility::Appendable);
    let nodes = f
        .ast
        .add_type(
            f.root,
            "Nodes",
            TypeKind::Sequence {
                element: Ref::Resolved(node),
                bound: Bound::Unbounded,
            },
        )
        .expect("Nodes");
    let branch = f
        .ast
        .add_type(
            f.root,
            "Branch",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Branch");
    f.set_extensibility(branch, Extensibility::Appendable);
    let zero = f.ast.add_int_expr(0).expect("0");
    f.ast
        .add_case(
            branch,
            vec![zerodds_ast::CaseLabel::Value(zero)],
            MemberSpec::new("leaf", Ref::Resolved(long)),
        )
        .expect("leaf");
    f.ast
        .add_case(
            branch,
            vec![zerodds_ast::CaseLabel::Default],
            MemberSpec::new("nodes", Ref::Resolved(nodes)),
        )
        .expect("nodes");
    f.member(node, "value", long);
    f.member(node, "link", branch);

    for key in [v1(), v2()] {
        assert_eq!(plan_of(&f.ast, node, key).extent, Extent::Unbounded);
        assert_eq!(plan_of(&f.ast, branch, key).extent, Extent::Unbounded);
    }
}

/// A union branch that reaches the enclosing type back with no sequence
/// or map between them is the recursion IDL 4.2 §7.4.1.4.4.4.4 does not
/// admit, and the refusal names it — the branch arm of the planner has
/// its own check, separate from the aggregate members'.
#[test]
fn a_union_branch_that_closes_a_cycle_without_a_collection_is_refused() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    let branch = f
        .ast
        .add_type(
            f.root,
            "Branch",
            TypeKind::Union {
                discriminator: Ref::Resolved(long),
                cases: Vec::new(),
                default_case: None,
            },
        )
        .expect("Branch");
    let zero = f.ast.add_int_expr(0).expect("0");
    f.ast
        .add_case(
            branch,
            vec![zerodds_ast::CaseLabel::Value(zero)],
            MemberSpec::new("again", Ref::Resolved(node)),
        )
        .expect("again");
    f.member(node, "value", long);
    f.member(node, "link", branch);

    expect_recursion_refusal(&f.ast, node, "a union branch cycle without a collection");
}

/// An array has no length on the wire, so a cycle through one never
/// terminates — the array arm refuses it by name rather than letting
/// `MAX_DEPTH` report a depth.
#[test]
fn an_array_of_the_enclosing_type_is_refused() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    let dim = f.ast.add_int_expr(2).expect("2");
    let pair = f
        .ast
        .add_type(
            f.root,
            "Pair",
            TypeKind::Array {
                element: Ref::Resolved(node),
                dims: vec![dim],
            },
        )
        .expect("Pair");
    f.member(node, "value", long);
    f.member(node, "pair", pair);

    expect_recursion_refusal(&f.ast, node, "an array of the enclosing type");
}

/// `typedef Node Ref; struct Node { Ref again; };` is the direct cycle
/// written through an alias, and `closes_a_cycle_without_length` follows
/// the chain — the raw id of the alias would not match the active type.
#[test]
fn a_cycle_written_through_a_typedef_is_still_refused() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    let alias = f
        .ast
        .add_type(
            f.root,
            "NodeRef",
            TypeKind::Alias {
                aliased: Ref::Resolved(node),
            },
        )
        .expect("NodeRef");
    f.member(node, "value", long);
    f.member(node, "again", alias);

    expect_recursion_refusal(&f.ast, node, "a typedef-obscured cycle");
}

/// `sequence<Ref>` with `typedef Node Ref` is the legal cycle written
/// through an alias: the barrier is the sequence's, and the alias is
/// transparent to it.
#[test]
fn a_sequence_of_a_typedef_of_the_enclosing_type_is_planned() {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    f.set_extensibility(node, Extensibility::Appendable);
    let alias = f
        .ast
        .add_type(
            f.root,
            "NodeRef",
            TypeKind::Alias {
                aliased: Ref::Resolved(node),
            },
        )
        .expect("NodeRef");
    let children = f
        .ast
        .add_type(
            f.root,
            "Children",
            TypeKind::Sequence {
                element: Ref::Resolved(alias),
                bound: Bound::Unbounded,
            },
        )
        .expect("Children");
    f.member(node, "value", long);
    f.member(node, "children", children);

    for key in [v1(), v2()] {
        assert_eq!(plan_of(&f.ast, node, key).extent, Extent::Unbounded);
    }
}

/// `Node` with a `next` member of its own type, `@external` and
/// optionally `@optional`.
fn self_referential_node(optional: bool) -> (Fixture, Id<TypeNode>) {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let node = f.structure("Node");
    f.set_extensibility(node, Extensibility::Mutable);
    f.member(node, "value", long);
    let next = f
        .ast
        .add_member(node, MemberSpec::new("next", Ref::Resolved(node)))
        .expect("next");
    let any = f.ast.any_of_member(next).expect("any");
    let facets = f.ast.facets_mut(any).expect("facets");
    facets.xtypes_mut().is_optional = optional;
    facets.xtypes_mut().is_external = true;
    (f, node)
}

/// `@optional @external Node next` inside `Node` — the corpus case
/// `recursive-optional-external` — is planned.
///
/// The cycle crosses no sequence and no map, and it is admitted anyway,
/// because what makes a value finite is a terminator and an `@optional`
/// member is one: the member can be absent, and an absent member ends
/// the descent where a zero-length sequence would. IDL 4.2
/// §7.4.1.4.4.4.4 names the collection form of that property; XTypes
/// 1.3 §7.3.1.2.1.4 refuses a cyclic *value*, which is a different
/// question — the corpus sample is a two-node tree and has no cycle in
/// it.
///
/// Measured 2026-08-26: Fast DDS carries the type and writes 28 octets
/// for that sample under XCDR1 big-endian, which this planner's writer
/// now equals byte for byte.
#[test]
fn an_optional_cycle_is_planned() {
    let (f, node) = self_referential_node(true);

    for key in [v1(), v2()] {
        let plan = plan_of(&f.ast, node, key);
        // No finite size: the depth of a value is the value's.
        assert_eq!(plan.extent, Extent::Unbounded);
        // Both members are planned; the back edge is not dropped.
        assert_eq!(plan.members.len(), 2);
        // The long fixes the alignment; the back edge contributes
        // nothing to it, which is the fixpoint and not a guess.
        assert_eq!(plan.align, 4);
    }
}

/// `A { long x; @optional B tail; }` and `B { long y; A head; }`: one
/// cycle over two types, terminated by the one `@optional` member on it.
fn mutual_cycle(
    optional: bool,
    a: Extensibility,
    b: Extensibility,
) -> (Fixture, [Id<TypeNode>; 2]) {
    let mut f = Fixture::new();
    let long = f.primitive("L", PrimitiveKind::Int32);
    let type_a = f.structure("A");
    let type_b = f.structure("B");
    f.set_extensibility(type_a, a);
    f.set_extensibility(type_b, b);
    f.member(type_a, "x", long);
    let tail = f
        .ast
        .add_member(type_a, MemberSpec::new("tail", Ref::Resolved(type_b)))
        .expect("tail");
    let any = f.ast.any_of_member(tail).expect("any");
    f.ast
        .facets_mut(any)
        .expect("facets")
        .xtypes_mut()
        .is_optional = optional;
    f.member(type_b, "y", long);
    f.member(type_b, "head", type_a);
    (f, [type_a, type_b])
}

/// The cycle is admitted whatever the two types' extensibility, from
/// either end. Under `@mutable` the EMHEADER's length code plans each
/// member's type a second time, and that descent used to run outside the
/// `@optional` member's barrier: `@mutable struct A` was refused with
/// `RecursionOutsideCollection` where `@final` passed (corpus case
/// `recursive-mutual-optional`, `zerodds-idlc check` IDL0014).
#[test]
fn a_cycle_over_two_types_with_one_optional_member_is_planned() {
    use Extensibility::{Appendable, Final, Mutable};
    for (a, b) in [
        (Final, Final),
        (Mutable, Final),
        (Final, Mutable),
        (Mutable, Mutable),
        (Appendable, Mutable),
    ] {
        let (f, types) = mutual_cycle(true, a, b);
        for id in types {
            for key in [v1(), v2()] {
                let mut deps = DepCollector::new();
                let planned = plan(&f.ast, id, key, &mut deps);
                assert!(
                    planned.is_ok(),
                    "A {a:?}, B {b:?}, planned from {id:?} under {:?}: {planned:?}",
                    key.encoding
                );
            }
        }
    }
}

/// The same cycle without the `@optional` stays refused, `@mutable` or not.
#[test]
fn a_cycle_over_two_types_without_a_terminator_is_refused() {
    for extensibility in [Extensibility::Final, Extensibility::Mutable] {
        let (f, [type_a, type_b]) = mutual_cycle(false, extensibility, extensibility);
        expect_recursion_refusal(&f.ast, type_a, "A of a two-type cycle without @optional");
        expect_recursion_refusal(&f.ast, type_b, "B of a two-type cycle without @optional");
    }
}

/// `@external Node next` without `@optional` stays refused by name.
///
/// `@external` says where the value is held, not whether it is there.
/// Nothing about a value can end the descent, so the type has no finite
/// value at all — the refusal IDL 4.2 §7.4.1.4.4.4.4 asks for, and not
/// a depth limit reporting a number.
#[test]
fn an_external_cycle_without_optional_is_still_refused() {
    let (f, node) = self_referential_node(false);

    expect_recursion_refusal(&f.ast, node, "an @external cycle with no @optional on it");
}
