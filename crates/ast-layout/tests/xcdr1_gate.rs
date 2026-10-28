// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The planner gated against the PL_CDR1 rules.
//!
//! The oracle is `crates/cdr/src/xcdr1.rs` at commit `e1f3af895`, whose
//! module header states the wire form directly from OMG XTypes 1.3
//! §7.4.1.2 and whose encoder implements it. It predates this rebuild
//! and nothing here produced it.
//!
//! The rules it pins, transcribed:
//!
//! - Standard header `[id:u16][length:u16]`, valid when
//!   `member_id <= 0x3F00` **and** `length <= 0xFFFF`.
//! - Extended header `[PID_EXTENDED:u16][8:u16][id:u32][length:u32]`,
//!   mandatory when `member_id > 0x3F00` **or** `length > 0xFFFF`.
//! - `PID_EXTENDED = 0x3F01`, `PID_LIST_END = 0x3F02`,
//!   `PID_SHORT_MAX_ID = 0x3F00`.
//!
//! # Where this file departs from its oracle
//!
//! The oracle wrote the extended header **at** 0x3F00 as well as above
//! it. §7.4.1.2.2 gives the short form's range as "from 0 to 16,128
//! (0x3F00) inclusive" and `PID_EXTENDED` is 0x3F01, so 0x3F00 is the
//! last usable id and not the first reserved one — and two foreign
//! implementations write the short form for it. Case `pid_3f00`,
//! 2026-08-10, `@mutable struct PidEdge { @id(16127) long a;
//! @id(16128) long b; };` XCDR1 little endian: RTI Connext 7.7 writes
//! `ff3e0400 07000000 003f0400 09000000 027f0000` and Fast DDS the same
//! behind its own sentinel. Cyclone writes the extended form for every
//! id and does not settle the edge. The oracle carried the same
//! off-by-one this rebuild inherited from it.
//! - The list terminates with the `PID_LIST_END` sentinel.
//! - Values pad to four bytes; the length field carries the **unpadded**
//!   length and the padding follows it uncounted.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{
    Ast, Bound, CharWidth, Extensibility, MemberSpec, PrimitiveKind, TypeKind, TypeNode,
};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{
    DepCollector, Dialect, Encoding, Endianness, Extent, LayoutKey, MemberHeader, PID_EXTENDED,
    PID_LIST_END, PID_SHORT_MAX_ID, PID_SHORT_MAX_LEN, PL_CDR1_ALIGNMENT, PlCdr1HeaderPolicy,
    ShortPidRange, TypeLayout, needs_extended_pid, plan,
};

/// The oracle's own rule is the one `Dialect::Specification` holds:
/// the short header wherever §7.4.1.2.2 allows it. That is also
/// `Dialect::default` now, so the `with_dialect` call is a no-op — kept
/// because this function's contract is the clause and not the default,
/// and the two parted once already. `Dialect::Cyclone` uses the extended
/// form throughout, which is a different question and the test below
/// asks it separately.
fn pl_cdr1() -> LayoutKey {
    LayoutKey::data(Encoding::PlCdr1, Endianness::Little).with_dialect(Dialect::Specification)
}

fn plain_cdr1() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
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

    fn mutable_struct(&mut self, name: &str) -> Id<TypeNode> {
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
            .extensibility = Some(Extensibility::Mutable);
        id
    }

    fn member_with_id(
        &mut self,
        aggregate: Id<TypeNode>,
        name: &str,
        type_: Id<TypeNode>,
        member_id: u32,
    ) -> Id<zerodds_ast::MemberNode> {
        let member = self
            .ast
            .add_member(aggregate, MemberSpec::new(name, Ref::Resolved(type_)))
            .expect("member");
        let any = self.ast.any_of_member(member).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .member_id = Some(zerodds_ast::MemberIdAssignment {
            effective: member_id,
            origin: zerodds_ast::MemberIdOrigin::Declared,
        });
        member
    }

    fn plan(&self, id: Id<TypeNode>, key: LayoutKey) -> TypeLayout {
        let mut deps = DepCollector::new();
        plan(&self.ast, id, key, &mut deps).expect("a plan")
    }
}

// -- the constants the oracle pins ---------------------------------------

#[test]
fn the_pl_cdr1_constants_match_the_oracle() {
    assert_eq!(PID_SHORT_MAX_ID, 0x3f00);
    assert_eq!(PID_EXTENDED, 0x3f01);
    assert_eq!(PID_LIST_END, 0x3f02);
    assert_eq!(PID_SHORT_MAX_LEN, 0xffff);
    assert_eq!(PL_CDR1_ALIGNMENT, 4);
}

/// The policy under which the extended header is mandatory and not
/// merely chosen, which is what this file's oracle describes.
const SPEC: PlCdr1HeaderPolicy = PlCdr1HeaderPolicy::Specification;

// -- the extended-header rule --------------------------------------------

#[test]
fn a_short_member_with_a_low_id_uses_the_standard_header() {
    assert!(!needs_extended_pid(
        1,
        Extent::Sized(4),
        SPEC,
        ShortPidRange::default()
    ));
    assert!(!needs_extended_pid(
        0x3eff,
        Extent::Sized(0xffff),
        SPEC,
        ShortPidRange::default()
    ));
}

#[test]
fn the_last_id_the_short_header_holds_is_the_threshold_itself() {
    // §7.4.1.2.2's range is inclusive of 0x3F00, and RTI and Fast DDS
    // both write the short form for it — see this file's header. The id
    // above it is `PID_EXTENDED` and is not a member id at all.
    assert!(!needs_extended_pid(
        0x3f00,
        Extent::Sized(4),
        SPEC,
        ShortPidRange::default()
    ));
    assert!(needs_extended_pid(
        0x3f01,
        Extent::Sized(4),
        SPEC,
        ShortPidRange::default()
    ));
    assert!(needs_extended_pid(
        0x4000,
        Extent::Sized(4),
        SPEC,
        ShortPidRange::default()
    ));
}

#[test]
fn a_value_longer_than_the_short_length_field_needs_the_extended_header() {
    // The second condition, and the one that is easy to miss: a low
    // member id does not save a member whose length cannot be stated in
    // sixteen bits.
    assert!(needs_extended_pid(
        1,
        Extent::Sized(0x1_0000),
        SPEC,
        ShortPidRange::default()
    ));
    assert!(!needs_extended_pid(
        1,
        Extent::Sized(0xffff),
        SPEC,
        ShortPidRange::default()
    ));
}

/// An unbounded member takes the **short** header, and the writer falls
/// back when a value actually overruns it.
///
/// This asserted the opposite, on the reasoning that a plan cannot
/// promise an unbounded value will stay under 0xFFFF. It cannot, and
/// that is not what settles it: RTI and Fast DDS write the short form
/// and fall back only on the sample that overruns. Case `mut_string`,
/// run 2026-08-09 — `01000800 04000000 61626300`, 24 bytes for the
/// whole type where an always-extended plan produced 32.
///
/// The fallback is `Writer::finish_pid`'s, tested in
/// `crates/wire/tests/wire.rs::a_short_parameter_header_becomes_extended_when_the_value_overruns_it`
/// and end to end in `wire-probe/tests/vendor_bytes.rs`.
#[test]
fn an_unbounded_member_takes_the_short_header_and_the_writer_falls_back() {
    assert!(!needs_extended_pid(
        1,
        Extent::Unbounded,
        SPEC,
        ShortPidRange::default()
    ));
    // The two reasons that *are* the plan's still are.
    assert!(needs_extended_pid(
        0x3f01,
        Extent::Unbounded,
        SPEC,
        ShortPidRange::default()
    ));
    assert!(needs_extended_pid(
        1,
        Extent::Unbounded,
        PlCdr1HeaderPolicy::AlwaysExtended,
        ShortPidRange::default()
    ));
}

/// Cyclone DDS's shape: the extended header for a member that needs
/// nothing of the sort. Measured — `tests/fixtures/vendor_bytes.md`,
/// `ext_mutable` under XCDR1, where every one of the three members goes
/// out behind `017f0800` although all three ids are below the threshold
/// and all three values fit sixteen bits.
#[test]
fn the_always_extended_policy_takes_the_long_header_for_a_member_that_fits_the_short_one() {
    let cyclone = PlCdr1HeaderPolicy::AlwaysExtended;
    assert!(needs_extended_pid(
        1,
        Extent::Sized(4),
        cyclone,
        ShortPidRange::default()
    ));
    assert!(needs_extended_pid(
        0,
        Extent::Sized(8),
        cyclone,
        ShortPidRange::default()
    ));
}

#[test]
fn the_planner_applies_the_extended_rule_to_a_real_member() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let text = m
        .ast
        .add_type(
            m.root,
            "S",
            TypeKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
        )
        .expect("S");
    let s = m.mutable_struct("M");
    let _ = m.member_with_id(s, "small", long, 1);
    let _ = m.member_with_id(s, "big_id", long, 0x3f01);
    let _ = m.member_with_id(s, "unbounded", text, 2);

    let layout = m.plan(s, pl_cdr1());
    let extended: Vec<bool> = layout
        .members
        .iter()
        .map(|member| match member.header {
            MemberHeader::PidCdr1 { extended, .. } => extended,
            other => panic!("expected a PID header, got {other:?}"),
        })
        .collect();
    assert_eq!(
        extended,
        vec![false, true, false],
        "only the id above the threshold takes the extended form; an unbounded \
         value takes the short one and the writer rewrites it if a sample \
         overruns sixteen bits"
    );
}

// -- the sentinel --------------------------------------------------------

#[test]
fn a_mutable_struct_plans_the_pid_list_end_sentinel() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.mutable_struct("M");
    let _ = m.member_with_id(s, "x", long, 1);

    // A reader that stops at the last member it recognises instead of at
    // the sentinel misreads whatever follows the parameter list.
    let layout = m.plan(s, pl_cdr1());
    assert!(layout.needs_sentinel);
}

#[test]
fn an_empty_mutable_struct_still_plans_the_sentinel() {
    let mut m = Model::new();
    let s = m.mutable_struct("Empty");
    let layout = m.plan(s, pl_cdr1());
    assert!(layout.needs_sentinel);
    assert!(layout.members.is_empty());
}

#[test]
fn plain_cdr1_plans_no_sentinel() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m
        .ast
        .add_type(
            m.root,
            "Final",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Final");
    let _ = m
        .ast
        .add_member(s, MemberSpec::new("x", Ref::Resolved(long)))
        .expect("x");

    let layout = m.plan(s, plain_cdr1());
    assert!(!layout.needs_sentinel, "only a parameter list has one");
    assert!(!layout.needs_dheader, "XCDR1 has no delimiter header");
}

#[test]
fn xcdr2_plans_no_sentinel_because_it_has_no_parameter_list_terminator() {
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.mutable_struct("M");
    let _ = m.member_with_id(s, "x", long, 1);

    let layout = m.plan(s, LayoutKey::data(Encoding::PlCdr2, Endianness::Little));
    assert!(!layout.needs_sentinel);
    assert!(layout.needs_dheader, "XCDR2 delimits instead");
}

// -- alignment, XCDR1 side -----------------------------------------------

#[test]
fn xcdr1_aligns_sixty_four_bit_primitives_to_eight() {
    let mut m = Model::new();
    let octet = m.primitive("O", PrimitiveKind::Octet);
    let double = m.primitive("D", PrimitiveKind::Float64);
    let s = m
        .ast
        .add_type(
            m.root,
            "Padded",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Padded");
    let _ = m
        .ast
        .add_member(s, MemberSpec::new("flag", Ref::Resolved(octet)))
        .expect("flag");
    let _ = m
        .ast
        .add_member(s, MemberSpec::new("value", Ref::Resolved(double)))
        .expect("value");

    let layout = m.plan(s, plain_cdr1());
    assert_eq!(
        layout.members[1].offset,
        zerodds_ast_layout::Offset::Fixed(8)
    );
    assert_eq!(layout.extent, Extent::Sized(16));
}

#[test]
fn a_member_id_beyond_u16_still_produces_a_usable_plan() {
    // The short PID field is sixteen bits; an id past that must take the
    // extended form rather than truncate into a different member.
    let mut m = Model::new();
    let long = m.primitive("L", PrimitiveKind::Int32);
    let s = m.mutable_struct("M");
    let _ = m.member_with_id(s, "far", long, 0x0001_0001);

    let layout = m.plan(s, pl_cdr1());
    let MemberHeader::PidCdr1 { extended, .. } = layout.members[0].header else {
        panic!("expected a PID header");
    };
    assert!(extended);
    assert_eq!(layout.members[0].member_id, 0x0001_0001);
}
