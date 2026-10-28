// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The planner gated against bytes from three foreign implementations.
//!
//! Every number asserted here was produced by Cyclone DDS, RTI Connext
//! 7.7 or Fast DDS running their own serialiser over their own
//! generated code. The runs, the harness and the disagreements are in
//! `tests/fixtures/vendor_bytes.md`.
//!
//! **Serialiser, not socket.** Every entry point behind these numbers is
//! in-memory, so "on the wire" below means "out of the serialiser". The
//! two questions where that is known to differ are the encapsulation
//! options field (`docs/wire-feature-inventory.md` B5) and the content of
//! alignment padding (`tools/wire-capture`); neither is asserted here,
//! and a serialised payload's length on a socket is not a structure's
//! extent — the encapsulation padding of §7.6.3.1.2 comes after it.
//!
//! This is the gate `wire_vectors.rs` is not: those vectors are this
//! project's own earlier output, and one of them turned out to be
//! wrong.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, Bound, Extensibility, MemberSpec, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};
use zerodds_ast_layout::{
    CollectionDelimiterPolicy, DepCollector, Dialect, Encoding, Endianness, Extent,
    KeyHolderExtensibility, KeyHolderNesting, KeyHolderOrder, LayoutKey, LengthCodePolicy,
    MustUnderstandPolicy, PlCdr1HeaderPolicy, Purpose, TypeLayout, UnionLabelMatch, WcharPosition,
    WstringLengthPolicy, WstringUnitOrder, key_members_with, plan, primitive_layout,
};

struct Model {
    ast: Ast,
    root: Id<zerodds_ast::ModuleNode>,
    next: u32,
}

impl Model {
    fn new() -> Self {
        let ast = Ast::new();
        let root = ast.root_module();
        Self { ast, root, next: 0 }
    }

    /// A fresh name, so two primitives of the same kind can coexist.
    fn fresh(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("{stem}_{}", self.next)
    }

    fn primitive(&mut self, kind: PrimitiveKind) -> Id<TypeNode> {
        let name = self.fresh("P");
        self.ast
            .add_type(self.root, &name, TypeKind::Primitive(kind))
            .expect("primitive")
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

    /// A structure whose source said nothing about extensibility,
    /// which is the case the dialects answer differently.
    fn unannotated_structure(&mut self, name: &str) -> Id<TypeNode> {
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

    fn enumeration(&mut self, name: &str, literals: &[&str]) -> Id<TypeNode> {
        let id = self
            .ast
            .add_type(
                self.root,
                name,
                TypeKind::Enumeration {
                    enumerators: Vec::new(),
                },
            )
            .expect("enum");
        for literal in literals {
            let _ = self
                .ast
                .add_enumerator(id, literal, None)
                .expect("enumerator");
        }
        id
    }

    fn sequence(&mut self, element: Id<TypeNode>) -> Id<TypeNode> {
        let name = self.fresh("Seq");
        self.ast
            .add_type(
                self.root,
                &name,
                TypeKind::Sequence {
                    element: Ref::Resolved(element),
                    bound: Bound::Unbounded,
                },
            )
            .expect("sequence")
    }

    fn array(&mut self, element: Id<TypeNode>, count: i128) -> Id<TypeNode> {
        let dim = self.ast.add_int_expr(count).expect("dimension");
        let name = self.fresh("Arr");
        self.ast
            .add_type(
                self.root,
                &name,
                TypeKind::Array {
                    element: Ref::Resolved(element),
                    dims: alloc_vec(dim),
                },
            )
            .expect("array")
    }

    fn member(&mut self, aggregate: Id<TypeNode>, name: &str, type_: Id<TypeNode>) {
        let _ = self
            .ast
            .add_member(aggregate, MemberSpec::new(name, Ref::Resolved(type_)))
            .expect("member");
    }

    /// A member carrying `@key`.
    fn key_member(&mut self, aggregate: Id<TypeNode>, name: &str, type_: Id<TypeNode>) {
        let id = self
            .ast
            .add_member(aggregate, MemberSpec::new(name, Ref::Resolved(type_)))
            .expect("member");
        let any = self.ast.any_of_member(id).expect("any");
        self.ast
            .facets_mut(any)
            .expect("facets")
            .xtypes_mut()
            .is_key = true;
    }

    fn plan(&self, id: Id<TypeNode>, key: LayoutKey) -> TypeLayout {
        let mut deps = DepCollector::new();
        plan(&self.ast, id, key, &mut deps).expect("a plan")
    }

    fn extent(&self, id: Id<TypeNode>, key: LayoutKey) -> Extent {
        self.plan(id, key).extent
    }
}

fn alloc_vec(dim: Id<zerodds_ast::ConstExprNode>) -> Vec<Id<zerodds_ast::ConstExprNode>> {
    vec![dim]
}

fn xcdr1() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr1, Endianness::Little)
}

fn xcdr2() -> LayoutKey {
    LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
}

/// `@final struct Point { long x; double y; }` — the smallest form of
/// the alignment cap. Cyclone, RTI and Fast DDS all put the double at
/// offset 8 under XCDR1 and offset 4 under XCDR2.
#[test]
fn point_is_sixteen_bytes_under_xcdr1_and_twelve_under_xcdr2() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let double = m.primitive(PrimitiveKind::Float64);
    let point = m.structure("Point", Extensibility::Final);
    m.member(point, "x", long);
    m.member(point, "y", double);

    assert_eq!(m.extent(point, xcdr1()), Extent::Sized(16));
    assert_eq!(m.extent(point, xcdr2()), Extent::Sized(12));
    assert_eq!(
        m.plan(point, xcdr2()).members[1].offset,
        zerodds_ast_layout::Offset::Fixed(4)
    );
}

/// The ten-primitive struct: 48 bytes under XCDR1, 44 under XCDR2, from
/// all three implementations.
#[test]
fn the_mixed_primitive_struct_matches_all_three_vendors() {
    let mut m = Model::new();
    let s = m.structure("Mixed", Extensibility::Final);
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
        let t = m.primitive(kind);
        m.member(s, name, t);
    }

    assert_eq!(m.extent(s, xcdr1()), Extent::Sized(48));
    assert_eq!(m.extent(s, xcdr2()), Extent::Sized(44));
}

/// `@final struct Arr { long a[3]; double d[2]; }` — 32 and 28 bytes.
/// An array of a primitive takes no delimiter under either version, so
/// all three agree.
#[test]
fn an_array_of_primitives_takes_no_delimiter() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let double = m.primitive(PrimitiveKind::Float64);
    let longs = m.array(long, 3);
    let doubles = m.array(double, 2);
    let s = m.structure("Arr", Extensibility::Final);
    m.member(s, "a", longs);
    m.member(s, "d", doubles);

    assert!(!m.plan(longs, xcdr2()).needs_dheader);
    assert_eq!(m.extent(s, xcdr1()), Extent::Sized(32));
    assert_eq!(m.extent(s, xcdr2()), Extent::Sized(28));
}

/// `Cell items[2]` where `Cell` is a two-`long` `@final` struct.
///
/// Cyclone 20 bytes, Fast DDS 20, RTI 16. The default follows the
/// specification and the two-vendor majority.
#[test]
fn an_array_of_structs_is_delimited_unless_the_policy_says_otherwise() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let cell = m.structure("Cell", Extensibility::Final);
    m.member(cell, "a", long);
    m.member(cell, "b", long);
    let cells = m.array(cell, 2);
    let outer = m.structure("ArrN", Extensibility::Final);
    m.member(outer, "items", cells);

    assert!(m.plan(cells, xcdr2()).needs_dheader);
    assert_eq!(
        m.extent(outer, xcdr2()),
        Extent::Sized(20),
        "Cyclone, Fast DDS"
    );
    assert_eq!(m.extent(outer, xcdr1()), Extent::Sized(16), "all three");

    let rti = xcdr2().with_collection_delimiters(CollectionDelimiterPolicy::Never);
    assert!(!m.plan(cells, rti).needs_dheader);
    assert_eq!(m.extent(outer, rti), Extent::Sized(16), "RTI Connext 7.7");
}

/// `Colour cs[2]` — 12 bytes from Cyclone, 8 from RTI and Fast DDS.
///
/// The enumeration is where Fast DDS parts from Cyclone: it reads an
/// enum as primitive for this rule, which the specification's own
/// rationale for the exemption supports but its normative text does
/// not.
#[test]
fn an_array_of_enumerations_splits_the_vendors_three_ways() {
    let mut m = Model::new();
    let colour = m.enumeration("Colour", &["RED", "GREEN", "BLUE"]);
    let colours = m.array(colour, 2);
    let outer = m.structure("ArrE", Extensibility::Final);
    m.member(outer, "cs", colours);

    assert_eq!(m.extent(outer, xcdr2()), Extent::Sized(12), "Cyclone");

    let fastdds = xcdr2().with_collection_delimiters(CollectionDelimiterPolicy::EnumIsPrimitive);
    assert_eq!(m.extent(outer, fastdds), Extent::Sized(8), "Fast DDS");

    let rti = xcdr2().with_collection_delimiters(CollectionDelimiterPolicy::Never);
    assert_eq!(m.extent(outer, rti), Extent::Sized(8), "RTI Connext 7.7");

    assert_eq!(m.extent(outer, xcdr1()), Extent::Sized(8), "all three");
}

/// A sequence is never fixed-size, so only the delimiter flag is
/// observable in the plan. `sequence<Cell>` carries one, `sequence<long>`
/// does not.
#[test]
fn a_sequence_is_delimited_by_the_same_rule_as_an_array() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let cell = m.structure("Cell", Extensibility::Final);
    m.member(cell, "a", long);
    let of_cells = m.sequence(cell);
    let of_longs = m.sequence(long);

    assert!(m.plan(of_cells, xcdr2()).needs_dheader);
    assert!(!m.plan(of_longs, xcdr2()).needs_dheader);
    assert!(!m.plan(of_cells, xcdr1()).needs_dheader);

    let colour = m.enumeration("Colour", &["RED", "BLUE"]);
    let of_colours = m.sequence(colour);
    assert!(m.plan(of_colours, xcdr2()).needs_dheader, "Cyclone");
    let fastdds = xcdr2().with_collection_delimiters(CollectionDelimiterPolicy::EnumIsPrimitive);
    assert!(
        !m.plan(of_colours, fastdds).needs_dheader,
        "Fast DDS and RTI"
    );
}

/// `@final { long a; double b; short c; }` — 18 bytes under XCDR1, 14
/// under XCDR2, from all three.
#[test]
fn the_three_member_struct_matches_all_three_vendors() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let double = m.primitive(PrimitiveKind::Float64);
    let short = m.primitive(PrimitiveKind::Int16);
    let s = m.structure("Ext", Extensibility::Final);
    m.member(s, "a", long);
    m.member(s, "b", double);
    m.member(s, "c", short);

    assert_eq!(m.extent(s, xcdr1()), Extent::Sized(18));
    assert_eq!(m.extent(s, xcdr2()), Extent::Sized(14));
}

/// The same members as `@appendable`: a delimiter and nothing else
/// changes, so 18 bytes under XCDR2. XCDR1 has no delimited encoding,
/// so it stays at 18 too — for a different reason.
#[test]
fn appendable_adds_a_delimiter_under_xcdr2_and_nothing_under_xcdr1() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let double = m.primitive(PrimitiveKind::Float64);
    let short = m.primitive(PrimitiveKind::Int16);
    let s = m.structure("Ext", Extensibility::Appendable);
    m.member(s, "a", long);
    m.member(s, "b", double);
    m.member(s, "c", short);

    assert!(m.plan(s, xcdr2()).needs_dheader);
    assert_eq!(m.extent(s, xcdr2()), Extent::Sized(18));
    assert!(!m.plan(s, xcdr1()).needs_dheader);
    assert_eq!(m.extent(s, xcdr1()), Extent::Sized(18));
}

/// The IDL from GitHub issues #41 and #42, unannotated throughout, so
/// every type takes the specification's default of appendable.
///
/// RTI reads it the same way and serialises 60 bytes: a 56-byte outer
/// delimiter, then twice a 24-byte delimiter and three doubles. Cyclone
/// reads the same IDL as final and serialises 48.
/// Neither is a bug in the other; the IDL does not say.
#[test]
fn the_reported_robot_type_plans_the_sixty_bytes_rti_writes() {
    let mut m = Model::new();
    let double = m.primitive(PrimitiveKind::Float64);
    let location = m.structure("Location", Extensibility::Appendable);
    for name in ["x", "y", "z"] {
        m.member(location, name, double);
    }
    let orientation = m.structure("Orientation", Extensibility::Appendable);
    for name in ["x", "y", "z"] {
        m.member(orientation, name, double);
    }
    let robot = m.structure("Robot", Extensibility::Appendable);
    m.member(robot, "location", location);
    m.member(robot, "orientation", orientation);

    assert_eq!(m.extent(location, xcdr2()), Extent::Sized(28), "4 + 3 * 8");
    assert_eq!(m.extent(robot, xcdr2()), Extent::Sized(60), "RTI Connext");

    // Read as final instead, which is how Cyclone's `idlc` reads it.
    let mut c = Model::new();
    let double = c.primitive(PrimitiveKind::Float64);
    let location = c.structure("Location", Extensibility::Final);
    for name in ["x", "y", "z"] {
        c.member(location, name, double);
    }
    let orientation = c.structure("Orientation", Extensibility::Final);
    for name in ["x", "y", "z"] {
        c.member(orientation, name, double);
    }
    let robot = c.structure("Robot", Extensibility::Final);
    c.member(robot, "location", location);
    c.member(robot, "orientation", orientation);
    assert_eq!(c.extent(robot, xcdr2()), Extent::Sized(48), "Cyclone DDS");

    // Under XCDR1 the two readings coincide, which is why the same
    // pair of peers interoperates there and not under XCDR2.
    assert_eq!(m.extent(robot, xcdr1()), Extent::Sized(48));
    assert_eq!(c.extent(robot, xcdr1()), Extent::Sized(48));
}

/// `@final struct Odd { long a; short b; }` — six bytes, not eight.
///
/// All three serialisers write `010000000200` under both versions. A
/// structure ends after its last member; nothing pads it out to its own
/// alignment. The *payload* a peer receives is padded out to four and
/// the count stated in the encapsulation options — §7.6.3.1.2, row B5,
/// measured on a capture — which is a rule about the encapsulation and
/// not about the structure's extent.
#[test]
fn a_structure_carries_no_trailing_padding() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let short = m.primitive(PrimitiveKind::Int16);
    let odd = m.structure("Odd", Extensibility::Final);
    m.member(odd, "a", long);
    m.member(odd, "b", short);

    assert_eq!(m.extent(odd, xcdr1()), Extent::Sized(6));
    assert_eq!(m.extent(odd, xcdr2()), Extent::Sized(6));
    assert_eq!(m.plan(odd, xcdr2()).align, 4, "it still aligns to four");
}

/// `Odd items[2]; octet tail;` — the padding that a six-byte,
/// four-aligned element needs between elements but not after the last.
///
/// All three serialisers write `01000000 0200 0000 03000000 0400 77`
/// under XCDR1: fifteen bytes, with the octet at offset 14. Under XCDR2
/// Cyclone and Fast DDS prefix the array's delimiter for nineteen; RTI
/// does not. Fifteen is the serialised extent; the encapsulation pads it
/// to sixteen on the way out, which is B5's field and not this one.
#[test]
fn array_elements_pad_between_themselves_but_not_after_the_last() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let short = m.primitive(PrimitiveKind::Int16);
    let octet = m.primitive(PrimitiveKind::Octet);
    let odd = m.structure("Odd", Extensibility::Final);
    m.member(odd, "a", long);
    m.member(odd, "b", short);
    let items = m.array(odd, 2);
    let outer = m.structure("ArrOdd", Extensibility::Final);
    m.member(outer, "items", items);
    m.member(outer, "tail", octet);

    assert_eq!(
        m.extent(items, xcdr1()),
        Extent::Sized(14),
        "8 for the padded first element, 6 for the unpadded second"
    );
    assert_eq!(m.extent(outer, xcdr1()), Extent::Sized(15));
    assert_eq!(
        m.plan(outer, xcdr1()).members[1].offset,
        zerodds_ast_layout::Offset::Fixed(14),
        "the octet sits right after the array, not after a pad"
    );

    assert_eq!(
        m.extent(outer, xcdr2()),
        Extent::Sized(19),
        "Cyclone, Fast DDS"
    );
    let rti = xcdr2().with_collection_delimiters(CollectionDelimiterPolicy::Never);
    assert_eq!(m.extent(outer, rti), Extent::Sized(15), "RTI Connext 7.7");
}

/// The policy names round-trip, and a typo is rejected rather than
/// silently taken for the default.
#[test]
fn the_collection_policy_parses_from_a_configuration_value() {
    for policy in CollectionDelimiterPolicy::ALL {
        assert_eq!(
            CollectionDelimiterPolicy::parse(policy.as_str()),
            Some(policy)
        );
    }
    assert_eq!(CollectionDelimiterPolicy::parse("Never"), None);
    assert_eq!(CollectionDelimiterPolicy::parse(""), None);
    assert!(CollectionDelimiterPolicy::default().is_default());
}

// -- the dialects --------------------------------------------------------

/// `LayoutKey::data` starts from the clause and not from a product.
///
/// The project's default is the clause-by-clause reading, because a
/// default is what somebody gets who chose nothing and its warrant is
/// the text. `docs/specs/zerodds-interop-suite-1.0.md` §7 states it and
/// records the reversal: the default was `Dialect::Cyclone`, and once
/// each product's departures moved into that product's own dialect, the
/// default was writing Cyclone's two.
///
/// The two departures are asserted here by name and not only through the
/// dialect equality, so that a future default which happened to carry
/// one of them fails with the axis in the message.
#[test]
fn a_fresh_layout_key_is_in_the_specification_dialect() {
    assert!(Dialect::default().is_default());
    assert_eq!(Dialect::default(), Dialect::Specification);
    for encoding in Encoding::ALL {
        assert_eq!(
            LayoutKey::data(encoding, Endianness::Little),
            LayoutKey::in_dialect(
                Dialect::Specification,
                encoding,
                Endianness::Little,
                Purpose::Data
            )
        );
        assert_eq!(
            LayoutKey::key(encoding),
            LayoutKey::in_dialect(
                Dialect::Specification,
                encoding,
                Endianness::Big,
                Purpose::Key
            ),
            "the key holder takes the dialect too: what an unannotated \
             keyed aggregate is decides whether its holder is delimited"
        );
    }
    let fresh = LayoutKey::data(Encoding::PlainCdr1, Endianness::Big);
    assert_eq!(
        fresh.union_label,
        UnionLabelMatch::SignExtended,
        "the default drops the branch a negative narrow case label selects"
    );
    assert_eq!(
        fresh.wstring_unit_order,
        WstringUnitOrder::Stream,
        "the default writes a `wstring`'s code units in the host's order"
    );
    assert_eq!(
        fresh.default_extensibility,
        Extensibility::Appendable,
        "the default reads an unannotated aggregate as `@final`, which is \
         §7.3.1.2.1.8 read the other way"
    );
    // The dialect keeps every one of them and loses only its standing.
    assert!(!Dialect::Cyclone.is_default());
    assert_eq!(
        Dialect::Cyclone.union_label(),
        UnionLabelMatch::NegativeLabelUnmatchedBelow32
    );
    assert_eq!(
        Dialect::Cyclone.wstring_unit_order(),
        WstringUnitOrder::HostUnswapped
    );
    assert_eq!(
        Dialect::Cyclone.default_extensibility(),
        Extensibility::Final
    );
}

/// Every cell of the table in `Dialect`'s own documentation, against
/// the rows of `tests/fixtures/vendor_bytes.md` and
/// `docs/refused-constructs.md` that produced it.
///
/// Written out rather than derived from the accessors: a test that asks
/// the code what the code says would pass whatever the code said.
#[test]
fn each_dialect_holds_what_its_implementation_was_measured_writing() {
    // Collection delimiters — `array_enum`, `seq_enum`, `array_nested`,
    // `seq_struct`, `seq_appendable`.
    assert_eq!(
        Dialect::Cyclone.collection_delimiters(),
        CollectionDelimiterPolicy::Specification
    );
    assert_eq!(
        Dialect::Rti.collection_delimiters(),
        CollectionDelimiterPolicy::Never
    );
    assert_eq!(
        Dialect::FastDds.collection_delimiters(),
        CollectionDelimiterPolicy::EnumIsPrimitive
    );
    assert_eq!(
        Dialect::Specification.collection_delimiters(),
        CollectionDelimiterPolicy::Specification
    );

    // PL_CDR1 headers — `ext_mutable` and `mut_optional` under XCDR1.
    assert_eq!(
        Dialect::Cyclone.pl_cdr1_headers(),
        PlCdr1HeaderPolicy::AlwaysExtended
    );
    // RTI writes the short header where the specification allows it,
    // until the sample's type can exceed what one states — then every
    // constructed member takes `PID_EXTENDED`;
    // `crates/wire-probe/tests/rti_large_sample_headers.rs` holds RTI's
    // octets either side of that step.
    assert_eq!(
        Dialect::Rti.pl_cdr1_headers(),
        PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples
    );
    assert_eq!(
        Dialect::FastDds.pl_cdr1_headers(),
        PlCdr1HeaderPolicy::ShortTrueLength
    );
    assert_eq!(
        Dialect::Specification.pl_cdr1_headers(),
        PlCdr1HeaderPolicy::Specification
    );

    // Default extensibility — the `robot` rows. Fast DDS has none:
    // `fastddsgen` rejects that IDL, so the preset takes §7.3.1.2.1.8's
    // appendable rather than guessing.
    assert_eq!(
        Dialect::Cyclone.default_extensibility(),
        Extensibility::Final
    );
    assert_eq!(
        Dialect::Rti.default_extensibility(),
        Extensibility::Appendable
    );
    assert_eq!(
        Dialect::FastDds.default_extensibility(),
        Extensibility::Appendable
    );
    assert_eq!(
        Dialect::Specification.default_extensibility(),
        Extensibility::Appendable
    );

    // Length codes — `mut_lengthcodes` and `mut_lc_nested`, run
    // 2026-08-09 through `wire-oracle/gate`. All three agree on
    // LC5/6/7 where a leading word can be reused and on LC0..LC3 for a
    // member whose kind fixes the width; they part twice, and the two
    // splits nest. RTI reuses a nested aggregate's own DHEADER where
    // Cyclone writes LC4; Fast DDS does that *and* takes a short code
    // whenever the member's serialized size is 1, 2, 4 or 8.
    assert_eq!(Dialect::Cyclone.length_codes(), LengthCodePolicy::Compact);
    assert_eq!(
        Dialect::Rti.length_codes(),
        LengthCodePolicy::ReuseNestedDelimiter
    );
    assert_eq!(
        Dialect::FastDds.length_codes(),
        LengthCodePolicy::ShortWhereItFits
    );
    assert_eq!(
        Dialect::Specification.length_codes(),
        LengthCodePolicy::Compact
    );

    // Must-understand — `mut_mustunderstand`, both encoding versions.
    // Cyclone and RTI write the flag in whichever of the three places
    // the framing puts it; Fast DDS writes it in none of them.
    assert_eq!(
        Dialect::FastDds.must_understand(),
        MustUnderstandPolicy::Omitted
    );
    for dialect in [Dialect::Cyclone, Dialect::Rti] {
        assert_eq!(dialect.must_understand(), MustUnderstandPolicy::Declared);
    }
    // The specification dialect parts from all three vendors here, and
    // it is the only line in this table where it does. §7.2.2.4.4.4.8:
    // "Key members shall never be optional, and they shall always have
    // their 'must understand' attribute set to true." Measured on
    // `evolution-mutable-key` under XCDR2: Cyclone, RTI and Fast DDS
    // write `20000001` for `@id(1) @key long a` and OpenDDS writes
    // `a0000001`. Three implementations against a clause with one
    // reading, so the clause-by-clause column takes the clause and the
    // vendor columns take their vendors.
    assert_eq!(
        Dialect::OpenDds.must_understand(),
        MustUnderstandPolicy::DeclaredAndKey
    );
    // The specification dialect also writes §7.2.2.4.4.4.6's flag on the
    // discriminator of a `@mutable` union, which RTI and OpenDDS leave
    // clear.
    assert_eq!(
        Dialect::Specification.must_understand(),
        MustUnderstandPolicy::DeclaredKeyAndDiscriminator
    );
}

/// RTI is the one implementation whose `wstring` shape depends on the
/// encoding version: the byte count under XCDR2, a NUL-terminated
/// character count of four-byte characters under XCDR1.
/// `docs/refused-constructs.md`, "the wire underneath it is not agreed".
#[test]
fn the_wstring_setting_follows_the_encoding_where_the_vendor_does() {
    for encoding in Encoding::ALL {
        assert_eq!(
            Dialect::Cyclone.wstring_length(encoding),
            WstringLengthPolicy::SPECIFICATION
        );
        assert_eq!(
            Dialect::FastDds.wstring_length(encoding),
            WstringLengthPolicy::CHARACTER_COUNT
        );
        let expected = if encoding.is_v1() {
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED
        } else {
            WstringLengthPolicy::SPECIFICATION
        };
        assert_eq!(Dialect::Rti.wstring_length(encoding), expected);
    }
}

/// The unannotated `common::Robot` of GitHub issues #41 and #42, read
/// by each dialect in turn.
///
/// The same model, four answers, one flag apart — which is the point of
/// the preset. Cyclone's 48 bytes and RTI's 60 are both measured;
/// `tests/fixtures/vendor_bytes.md`, the `robot` rows.
#[test]
fn an_unannotated_aggregate_takes_the_dialect_s_word_for_what_it_is() {
    let mut m = Model::new();
    let double = m.primitive(PrimitiveKind::Float64);
    let location = m.unannotated_structure("Location");
    for name in ["x", "y", "z"] {
        m.member(location, name, double);
    }
    let orientation = m.unannotated_structure("Orientation");
    for name in ["x", "y", "z"] {
        m.member(orientation, name, double);
    }
    let robot = m.unannotated_structure("Robot");
    m.member(robot, "location", location);
    m.member(robot, "orientation", orientation);

    let extent = |dialect: Dialect, encoding: Encoding| {
        m.extent(
            robot,
            LayoutKey::in_dialect(dialect, encoding, Endianness::Little, Purpose::Data),
        )
    };

    assert_eq!(
        extent(Dialect::Cyclone, Encoding::PlainCdr2),
        Extent::Sized(48),
        "Cyclone reads it as final: three doubles twice, no delimiter"
    );
    assert_eq!(
        extent(Dialect::Rti, Encoding::PlainCdr2),
        Extent::Sized(60),
        "RTI reads it as appendable: an outer delimiter and two inner"
    );
    assert_eq!(
        extent(Dialect::Specification, Encoding::PlainCdr2),
        Extent::Sized(60),
        "§7.3.1.2.1.8 says appendable, which is RTI's reading"
    );
    assert_eq!(
        extent(Dialect::FastDds, Encoding::PlainCdr2),
        Extent::Sized(60),
        "Fast DDS reads it as appendable too: `fastddsgen` rejects this \
         particular IDL, but the corpus answers it without this case — \
         `primitive-mixed-unannotated` under XCDR2 is `30000000` and then \
         the members, where Cyclone writes no delimiter"
    );

    // XCDR1 has no delimited encoding, so the reading does not show.
    // This is why the same pair of peers interoperates there.
    for dialect in Dialect::ALL {
        assert_eq!(extent(dialect, Encoding::PlainCdr1), Extent::Sized(48));
    }
}

/// A dialect is a starting point and not a lock.
///
/// The order is the contract: `with_dialect` resets every setting, so
/// the individual override goes after it. The command-line probes apply
/// them in that order for this reason.
#[test]
fn an_individual_policy_overrides_the_dialect_under_it() {
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little)
        .with_dialect(Dialect::Rti)
        .with_collection_delimiters(CollectionDelimiterPolicy::Specification);
    assert_eq!(
        key.collection_delimiters,
        CollectionDelimiterPolicy::Specification,
        "the flag wins over the preset"
    );
    assert_eq!(
        key.default_extensibility,
        Extensibility::Appendable,
        "and changes nothing else about the preset"
    );
    assert_eq!(
        key.pl_cdr1_headers,
        PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples
    );
}

/// The names round-trip, and a typo is refused rather than taken for
/// the default.
#[test]
fn the_dialect_and_header_names_parse_from_a_configuration_value() {
    for dialect in Dialect::ALL {
        assert_eq!(Dialect::parse(dialect.as_str()), Some(dialect));
    }
    assert_eq!(Dialect::parse("Cyclone"), None);
    assert_eq!(Dialect::parse("fast-dds"), None);
    assert_eq!(Dialect::parse(""), None);

    for policy in PlCdr1HeaderPolicy::ALL {
        assert_eq!(PlCdr1HeaderPolicy::parse(policy.as_str()), Some(policy));
    }
    assert_eq!(PlCdr1HeaderPolicy::parse("always_extended"), None);
    assert!(PlCdr1HeaderPolicy::default().is_default());
}

/// The key holder is forced to `@final` everywhere but one place.
///
/// DDSI-RTPS 2.5 §9.6.4.8 Step 1 says the holder "shall have
/// extensibility kind 'FINAL'", and Cyclone, Fast DDS and RTI-XCDR2 all
/// do. RTI under XCDR1 keeps the parameter list for a `@mutable` type,
/// which puts the holder past sixteen bytes and hashes the framing
/// instead of the key. `KeyHolderExtensibility` carries the run.
#[test]
fn the_key_holder_is_final_everywhere_but_rti_under_xcdr1() {
    for encoding in Encoding::ALL {
        for dialect in Dialect::ALL {
            let expected = if dialect == Dialect::Rti && encoding.is_v1() {
                KeyHolderExtensibility::AsDeclared
            } else {
                KeyHolderExtensibility::ForcedFinal
            };
            assert_eq!(dialect.key_holder_extensibility(encoding), expected);
            assert_eq!(
                LayoutKey::in_dialect(dialect, encoding, Endianness::Big, Purpose::Key).key_holder,
                expected
            );
        }
    }

    // And the plan follows it: a `@mutable` keyed type holds sixteen
    // bytes of key under every dialect but that one, where the holder
    // is the parameter list and its extent is not fixed at all.
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);
    let foo = m.structure("Foo", Extensibility::Mutable);
    m.member(foo, "id", long);
    let any = m.ast.any_of_member(m.ast.members_of(foo)[0]).expect("any");
    m.ast.facets_mut(any).expect("facets").xtypes_mut().is_key = true;

    let holder = |dialect: Dialect, encoding: Encoding| {
        m.extent(
            foo,
            LayoutKey::in_dialect(dialect, encoding, Endianness::Big, Purpose::Key),
        )
    };
    assert_eq!(
        holder(Dialect::Cyclone, Encoding::PlCdr1),
        Extent::Sized(4),
        "forced final: one long, and the KeyHash is it zero-padded"
    );
    assert_eq!(
        holder(Dialect::Rti, Encoding::PlCdr2),
        Extent::Sized(4),
        "RTI forces final under XCDR2 like everyone else"
    );
    assert_eq!(
        holder(Dialect::Rti, Encoding::PlCdr1),
        Extent::Unbounded,
        "RTI keeps the parameter list under XCDR1, which is what sends \
         the holder down the MD5 branch"
    );
    for policy in KeyHolderExtensibility::ALL {
        assert_eq!(KeyHolderExtensibility::parse(policy.as_str()), Some(policy));
    }
    assert_eq!(KeyHolderExtensibility::parse("final"), None);
}

/// Divergence 6: how deep Step 1's member removal reaches.
///
/// XTypes 1.3 §7.6.8 Step 1 removes the non-key members "if there are
/// any key members. Otherwise do not remove any members", and Step 3
/// applies steps 1 and 2 "recursively to the members of FooKeyHolder".
/// Every number below was produced by Cyclone, RTI or Fast DDS over
/// their own generated code; `tests/fixtures/vendor_bytes.md` §6 has the
/// holders byte for byte.
#[test]
fn a_key_member_of_a_keyless_type_contributes_all_of_its_members() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);

    // @final struct NestedNoKey { long a; long b; };
    // @final struct OuterKeylessNested { @key NestedNoKey inner; long tail; };
    let nested_no_key = m.structure("NestedNoKey", Extensibility::Final);
    m.member(nested_no_key, "a", long);
    m.member(nested_no_key, "b", long);
    let outer_keyless = m.structure("OuterKeylessNested", Extensibility::Final);
    m.key_member(outer_keyless, "inner", nested_no_key);
    m.member(outer_keyless, "tail", long);

    // @final struct NestedSomeKey { @key long a; long b; };
    // @final struct OuterKeyedNested { @key NestedSomeKey inner; long tail; };
    let nested_some_key = m.structure("NestedSomeKey", Extensibility::Final);
    m.key_member(nested_some_key, "a", long);
    m.member(nested_some_key, "b", long);
    let outer_keyed = m.structure("OuterKeyedNested", Extensibility::Final);
    m.key_member(outer_keyed, "inner", nested_some_key);
    m.member(outer_keyed, "tail", long);

    for encoding in Encoding::ALL {
        for dialect in Dialect::ALL {
            let key = LayoutKey::in_dialect(dialect, encoding, Endianness::Big, Purpose::Key);
            assert_eq!(
                m.extent(outer_keyless, key),
                Extent::Sized(8),
                "{dialect:?}/{encoding:?}: a @key member whose type declares no \
                 key of its own puts both of that type's members in the holder; \
                 all three vendors give 0a0a0a0a0b0b0b0b"
            );
            assert_eq!(
                m.extent(outer_keyed, key),
                Extent::Sized(4),
                "{dialect:?}/{encoding:?}: and when the nested type does declare \
                 a key, only that member travels"
            );
        }
    }
}

/// The recursion has no depth limit — except in RTI's XCDR1 holder,
/// which stops at the first keyless aggregate and takes it whole.
#[test]
fn step_3_recursion_is_unlimited_except_in_rtis_xcdr1_holder() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);

    // Keyed at every level: p alone, everywhere.
    let chain_leaf = m.structure("ChainLeaf", Extensibility::Final);
    m.key_member(chain_leaf, "p", long);
    m.member(chain_leaf, "q", long);
    let chain_mid = m.structure("ChainMid", Extensibility::Final);
    m.key_member(chain_mid, "inner", chain_leaf);
    m.member(chain_mid, "r", long);
    let chain_outer = m.structure("ChainOuter", Extensibility::Final);
    m.key_member(chain_outer, "mid", chain_mid);
    m.member(chain_outer, "tail", long);

    // A keyless level above a keyed one, which is where they part.
    let deep_leaf = m.structure("DeepLeaf", Extensibility::Final);
    m.key_member(deep_leaf, "p", long);
    m.member(deep_leaf, "q", long);
    let deep_mid = m.structure("DeepMid", Extensibility::Final);
    m.member(deep_mid, "leaf", deep_leaf);
    m.member(deep_mid, "r", long);
    let deep_outer = m.structure("DeepOuter", Extensibility::Final);
    m.key_member(deep_outer, "mid", deep_mid);
    m.member(deep_outer, "tail", long);

    for encoding in Encoding::ALL {
        for dialect in Dialect::ALL {
            let key = LayoutKey::in_dialect(dialect, encoding, Endianness::Big, Purpose::Key);
            assert_eq!(
                m.extent(chain_outer, key),
                Extent::Sized(4),
                "{dialect:?}/{encoding:?}: three keyed levels give p alone; \
                 every vendor and every encoding agrees"
            );

            // RTI's XCDR1 holder keeps DeepLeaf's q, so twelve bytes
            // instead of eight. `maxSizeSerializedKey V1 / V2 = 12 / 8`.
            let stops_at_keyless = dialect == Dialect::Rti && encoding.is_v1();
            let expected = if stops_at_keyless { 12 } else { 8 };
            assert_eq!(
                m.extent(deep_outer, key),
                Extent::Sized(expected),
                "{dialect:?}/{encoding:?}: a keyless level above a keyed one"
            );
        }
    }

    // The dialect table, and the setting it selects.
    for encoding in Encoding::ALL {
        for dialect in Dialect::ALL {
            let expected = if dialect == Dialect::Rti && encoding.is_v1() {
                KeyHolderNesting::KeylessNestedWhole
            } else {
                KeyHolderNesting::Recursive
            };
            assert_eq!(dialect.key_holder_nesting(encoding), expected);
            assert_eq!(
                LayoutKey::in_dialect(dialect, encoding, Endianness::Big, Purpose::Key)
                    .key_holder_nesting,
                expected
            );
        }
    }
    for policy in KeyHolderNesting::ALL {
        assert_eq!(KeyHolderNesting::parse(policy.as_str()), Some(policy));
    }
    assert_eq!(KeyHolderNesting::parse("unlimited"), None);
    assert!(KeyHolderNesting::default().is_default());
}

/// The topic type itself is exempt: a keyless one has no KeyHash at all.
///
/// Step 1's second half describes what a *nested* keyless aggregate
/// contributes. Applied to the type the algorithm was invoked on it
/// would give a keyless topic a hash over every member, and no vendor
/// produces one — `vendor_bytes.md` §5. `key_members` is where both
/// callers take that decision, so it is the function that has to keep
/// answering with an empty list.
#[test]
fn a_keyless_topic_type_still_reports_no_key_members() {
    let mut m = Model::new();
    let long = m.primitive(PrimitiveKind::Int32);

    let inner = m.structure("Inner", Extensibility::Final);
    m.member(inner, "a", long);
    let keyless = m.structure("Keyless", Extensibility::Final);
    m.member(keyless, "inner", inner);
    m.member(keyless, "tail", long);

    for nesting in KeyHolderNesting::ALL {
        assert!(
            key_members_with(&m.ast, keyless, nesting, KeyHolderOrder::default())
                .expect("a key set")
                .is_empty(),
            "{nesting:?}: a keyless topic type has no key members"
        );
    }

    // A keyed one names its own key member first, then what the nested
    // holder contributes — the order the holder is written in. The
    // keyless `Inner` contributes its member under both settings: RTI's
    // XCDR1 holder takes it whole, which is the same list one level down.
    let keyed = m.structure("Keyed", Extensibility::Final);
    m.key_member(keyed, "inner", inner);
    m.member(keyed, "tail", long);
    for nesting in KeyHolderNesting::ALL {
        assert_eq!(
            key_members_with(&m.ast, keyed, nesting, KeyHolderOrder::default())
                .expect("a key set")
                .len(),
            2,
            "{nesting:?}: the @key member, and the one member the keyless \
             nested type contributes"
        );
    }

    // Two levels down is where the settings part: `Whole` stops at the
    // keyless `Mid` and never reaches `Leaf`'s own key.
    let leaf = m.structure("Leaf", Extensibility::Final);
    m.key_member(leaf, "p", long);
    m.member(leaf, "q", long);
    let mid = m.structure("Mid", Extensibility::Final);
    m.member(mid, "leaf", leaf);
    m.member(mid, "r", long);
    let deep = m.structure("Deep", Extensibility::Final);
    m.key_member(deep, "mid", mid);
    assert_eq!(
        key_members_with(
            &m.ast,
            deep,
            KeyHolderNesting::Recursive,
            KeyHolderOrder::default(),
        )
        .expect("a key set")
        .len(),
        4,
        "mid, leaf, leaf's key p, and r"
    );
    assert_eq!(
        key_members_with(
            &m.ast,
            deep,
            KeyHolderNesting::KeylessNestedWhole,
            KeyHolderOrder::default(),
        )
        .expect("a key set")
        .len(),
        3,
        "mid, leaf taken whole, and r — RTI's XCDR1 holder does not \
         descend past the keyless level to pick p out of leaf"
    );
}

/// A `Char16` is the width its dialect writes, and the plan knows it.
///
/// `wchar_member`, run 2026-08-09: RTI Connext writes `61000000` under
/// XCDR1 and `6100` under XCDR2; Cyclone and Fast DDS write `6100`
/// everywhere. The probe's own bytes are asserted in
/// `wire-probe/tests/wstring_length_policy.rs`, and they come out right
/// from the writer alone — a `wchar` at the head of a type is four
/// bytes on the wire whether or not the plan agrees.
///
/// This asserts the half that does not show there. The plan's width is
/// what sizes an aggregate's extent, what picks a `wchar` member's
/// EMHEADER length code, and what a key holder is measured against for
/// §9.6.4.8's sixteen-byte branch. A plan that said two while the writer
/// wrote four would be wrong in all three and visible in none of them
/// until a type put something after the `wchar`.
#[test]
fn a_wchar_is_the_width_its_dialect_writes_in_the_plan_as_well() {
    for (dialect, encoding, width) in [
        (Dialect::Cyclone, Encoding::PlainCdr1, 2u32),
        (Dialect::Cyclone, Encoding::PlainCdr2, 2),
        (Dialect::Rti, Encoding::PlainCdr1, 4),
        (Dialect::Rti, Encoding::PlainCdr2, 2),
        (Dialect::FastDds, Encoding::PlainCdr1, 2),
        (Dialect::FastDds, Encoding::PlainCdr2, 2),
        // Table 31 says two, so the clause-by-clause dialect says two.
        (Dialect::Specification, Encoding::PlainCdr1, 2),
    ] {
        let key = LayoutKey::in_dialect(dialect, encoding, Endianness::Little, Purpose::Data);
        let expected = u8::try_from(width).expect("2 or 4");
        assert_eq!(
            primitive_layout(PrimitiveKind::WChar, key, WcharPosition::Member),
            (expected, width),
            "{dialect:?} / {encoding:?}"
        );
    }
}

/// The width reaches the extent, which is the offset of everything after.
#[test]
fn a_four_byte_wchar_moves_the_member_behind_it_in_the_plan() {
    let mut m = Model::new();
    let wide = m.primitive(PrimitiveKind::WChar);
    let long = m.primitive(PrimitiveKind::Int32);
    let s = m.structure("WN", Extensibility::Final);
    m.member(s, "c", wide);
    m.member(s, "n", long);

    let plan_in = |dialect| {
        let key = LayoutKey::in_dialect(
            dialect,
            Encoding::PlainCdr1,
            Endianness::Little,
            Purpose::Data,
        );
        let mut deps = DepCollector::new();
        plan(&m.ast, s, key, &mut deps).expect("a plan")
    };
    // Two bytes and two of padding, or four bytes and none: the `long`
    // is at offset 4 either way and the *extent* is eight either way.
    // What differs is the alignment the type carries, which is the
    // `wchar`'s own.
    assert_eq!(plan_in(Dialect::Cyclone).align, 4);
    assert_eq!(plan_in(Dialect::Rti).align, 4);

    // A `wchar` alone shows it: two bytes under Cyclone, four under RTI.
    let alone = m.structure("W1", Extensibility::Final);
    m.member(alone, "c", wide);
    let extent_of = |dialect| {
        let key = LayoutKey::in_dialect(
            dialect,
            Encoding::PlainCdr1,
            Endianness::Little,
            Purpose::Data,
        );
        let mut deps = DepCollector::new();
        plan(&m.ast, alone, key, &mut deps).expect("a plan").extent
    };
    assert_eq!(extent_of(Dialect::Cyclone), Extent::Sized(2));
    assert_eq!(extent_of(Dialect::Rti), Extent::Sized(4));
}

/// Two dialects that size a `wchar` differently do not share a cached
/// plan.
///
/// `LayoutKey::offset_key` left `WstringLengthPolicy` out on the
/// reasoning that a `wstring` plans as `Extent::Unbounded` under every
/// variant and moves no offset. True, and not the whole question: the
/// same policy fixes the width of a scalar `wchar`.
#[test]
fn the_wstring_policy_is_part_of_the_offset_key() {
    let cyclone = LayoutKey::in_dialect(
        Dialect::Cyclone,
        Encoding::PlainCdr1,
        Endianness::Little,
        Purpose::Data,
    );
    let rti = LayoutKey::in_dialect(
        Dialect::Rti,
        Encoding::PlainCdr1,
        Endianness::Little,
        Purpose::Data,
    );
    assert_ne!(
        cyclone.offset_key(),
        rti.offset_key(),
        "a two-byte plan would be served to a four-byte request"
    );
    // And a plan that differs in nothing else still differs here.
    let narrow = cyclone.with_wstring_length(WstringLengthPolicy::SPECIFICATION);
    let wide = cyclone.with_wstring_length(WstringLengthPolicy::CHARACTER_COUNT_TERMINATED);
    assert_ne!(narrow.offset_key(), wide.offset_key());
}
