// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Completion by specification rule, one test per §5 row.
//!
//! Every row asserts the value **and** its origin, because the number
//! alone cannot tell a declaration from a derivation, and a member-id
//! collision is only diagnosable when that is known.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{AnyId, Ast, Extensibility, MemberIdOrigin, TypeKind};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_idl_lower::{HashedIdCounter, Options, hashed_member_id, lower, lower_with};
use zerodds_idl_parse::parse;

fn model(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    lower(&parsed.syntax(), "test.idl").ast
}

/// The same model, lowered with a `@hashid` that does not advance the
/// sequential counter — RTI Connext's reading of §7.3.1.2.1.1.
fn model_skipping_hashed_ids(source: &str) -> Ast {
    let parsed = parse(source);
    assert!(
        !parsed.has_errors(),
        "the fixture does not parse: {:?}",
        parsed.diagnostics()
    );
    let options = Options {
        hashed_id_advances_the_counter: HashedIdCounter::Skipped,
        ..Options::default()
    };
    lower_with(&parsed.syntax(), "test.idl", options).ast
}

fn type_of(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&ScopedName::absolute(alloc_name(name)))
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{name} is not in the model"))
}

fn alloc_name(name: &str) -> Vec<String> {
    vec![name.to_string()]
}

/// The effective member id and its origin, for member `index` of `type_name`.
fn member_id(ast: &Ast, type_name: &str, index: usize) -> (u32, MemberIdOrigin) {
    let id = type_of(ast, type_name);
    let member = ast.members_of(id)[index];
    let node = ast.member(member).expect("member");
    let assignment = ast
        .facets(AnyId::Member(member, node.node_id))
        .expect("facets")
        .xtypes
        .as_ref()
        .and_then(|x| x.member_id.clone())
        .unwrap_or_else(|| panic!("member {index} of {type_name} has no id"));
    (assignment.effective, assignment.origin)
}

// -- §5 row 1: member id -------------------------------------------------

#[test]
fn members_without_annotations_number_from_zero_upwards() {
    let ast = model("struct S { long a; long b; long c; };");
    for (index, expected) in [(0, 0), (1, 1), (2, 2)] {
        let (id, origin) = member_id(&ast, "S", index);
        assert_eq!(id, expected);
        assert_eq!(origin, MemberIdOrigin::PreviousPlusOne);
    }
}

#[test]
fn a_declared_id_shifts_the_numbering_of_what_follows() {
    // The rule is "previous member plus one", not "position": after
    // `@id(10)` the next member is 11. XTypes 1.3 §7.3.1.2.1.1.
    let ast = model("struct S { long a; @id(10) long b; long c; };");
    assert_eq!(
        member_id(&ast, "S", 0),
        (0, MemberIdOrigin::PreviousPlusOne)
    );
    assert_eq!(member_id(&ast, "S", 1), (10, MemberIdOrigin::Declared));
    assert_eq!(
        member_id(&ast, "S", 2),
        (11, MemberIdOrigin::PreviousPlusOne)
    );
}

#[test]
fn autoid_sequential_and_autoid_hash_differ() {
    let sequential = model("@autoid(SEQUENTIAL) struct S { long a; long b; };");
    assert_eq!(
        member_id(&sequential, "S", 0),
        (0, MemberIdOrigin::AutoSequential)
    );
    assert_eq!(
        member_id(&sequential, "S", 1),
        (1, MemberIdOrigin::AutoSequential)
    );

    let hashed = model("@autoid(HASH) struct S { long a; long b; };");
    let (first, origin) = member_id(&hashed, "S", 0);
    assert_eq!(origin, MemberIdOrigin::AutoHash);
    assert_eq!(first, hashed_member_id("a"));
    let (second, _) = member_id(&hashed, "S", 1);
    assert_eq!(second, hashed_member_id("b"));
    assert_ne!(first, second, "two names hashed to one id");
}

#[test]
fn a_hashid_uses_the_name_it_names_rather_than_the_members_own() {
    let ast = model("struct S { @hashid(\"other\") long a; };");
    let (id, origin) = member_id(&ast, "S", 0);
    assert_eq!(id, hashed_member_id("other"));
    assert_ne!(id, hashed_member_id("a"));
    assert_eq!(
        origin,
        MemberIdOrigin::Hashed {
            source_name: "other".to_string()
        }
    );
}

#[test]
fn an_escaped_keyword_hashes_the_identifier_not_the_escape_spelling() {
    // IDL 4.2 §7.2.3.2: `_long` "is treated as if it were" `long` — the
    // underscore only turns off keyword checking. OpenDDS derives the
    // EMHEADER id 0x0364520F from MD5("long"); hashing the spelling
    // `_long` gives 0x04BF80B9 instead, which no other vendor agrees
    // with (measured in matrix/opendds/*/identifier-escaped-keyword).
    let ast = model("@mutable struct S { @hashid long _long; long plain; };");
    let (id, _) = member_id(&ast, "S", 0);
    assert_eq!(id, hashed_member_id("long"));
    assert_ne!(id, hashed_member_id("_long"));
}

#[test]
fn a_declared_id_wins_over_the_aggregates_autoid() {
    let ast = model("@autoid(HASH) struct S { @id(4) long a; };");
    assert_eq!(member_id(&ast, "S", 0), (4, MemberIdOrigin::Declared));
}

#[test]
fn a_hashed_id_stays_within_the_28_bit_member_id_space() {
    // Above 0x0FFFFFFF the id collides with the reserved parameter
    // range. XTypes 1.3 §7.3.1.2.1.1.
    let ast = model("@autoid(HASH) struct S { long alpha; long beta; long gamma; };");
    for index in 0..3 {
        let (id, _) = member_id(&ast, "S", index);
        assert_eq!(id & 0xF000_0000, 0, "member {index} left the id space");
    }
}

// -- §5 row 2: extensibility --------------------------------------------

#[test]
fn a_type_that_said_nothing_is_appendable_by_rule_and_silent_in_the_facet() {
    // Two questions with two answers: the facet says what was written,
    // the query says what holds. Conflating them would make "@appendable"
    // and "nothing" indistinguishable to a formatter.
    let ast = model("struct S { long a; };");
    let id = type_of(&ast, "S");
    let any = ast.any_of_type(id).expect("any");
    assert!(
        ast.facets(any)
            .expect("facets")
            .xtypes
            .as_ref()
            .is_none_or(|x| x.extensibility.is_none()),
        "the facet claimed an extensibility the source never wrote"
    );
    assert_eq!(
        ast.effective_extensibility(id),
        Extensibility::Appendable,
        "the rule's default is appendable, not final"
    );
}

#[test]
fn a_written_extensibility_is_what_the_rule_reports() {
    let ast = model("@final struct S { long a; };");
    assert_eq!(
        ast.effective_extensibility(type_of(&ast, "S")),
        Extensibility::Final
    );
}

/// A derived structure that says nothing has its base's extensibility:
/// DDS-XTypes 1.3 §7.2.2.4.5 lets it be unspecified or the base's, never
/// different. Cyclone DDS 11.0.1 frames this `Derived` with `DDS_OP_PLC`
/// (measured); the rule's appendable default gave it a DHEADER instead.
#[test]
fn a_derived_structure_that_said_nothing_has_its_bases_extensibility() {
    let ast = model(
        "@mutable struct Base { long a; }; struct Mid : Base { long b; }; \
         struct Leaf : Mid { long c; };",
    );
    for name in ["Base", "Mid", "Leaf"] {
        assert_eq!(
            ast.effective_extensibility(type_of(&ast, name)),
            Extensibility::Mutable,
            "{name}"
        );
    }
}

/// The caller's default fills the root of a base chain, not each link: a
/// derived structure keeps its base's extensibility, whatever the default.
#[test]
fn the_callers_default_extensibility_does_not_override_a_bases() {
    let parsed = parse(
        "@mutable struct Base { long a; }; struct Derived : Base { long b; }; \
         struct Silent { long a; }; struct SilentDerived : Silent { long b; };",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let options = Options {
        default_extensibility: Some(Extensibility::Final),
        ..Options::default()
    };
    let ast = lower_with(&parsed.syntax(), "test.idl", options).ast;
    assert_eq!(
        ast.effective_extensibility(type_of(&ast, "Derived")),
        Extensibility::Mutable,
        "the base's, not the caller's default"
    );
    assert_eq!(
        ast.effective_extensibility(type_of(&ast, "Silent")),
        Extensibility::Final,
        "the default fills a root that said nothing"
    );
    assert_eq!(
        ast.effective_extensibility(type_of(&ast, "SilentDerived")),
        Extensibility::Final,
        "and reaches its derived structure through the base"
    );
}

// -- §5 rows 3 and 4: enumerator values and bit positions ---------------

#[test]
fn enumerators_without_values_run_from_zero_upwards() {
    let ast = model("enum E { A, B, C };");
    let e = type_of(&ast, "E");
    let TypeKind::Enumeration { enumerators } = &ast.type_(e).expect("E").kind else {
        panic!("expected an enumeration");
    };
    for (index, expected) in [(0, 0), (1, 1), (2, 2)] {
        assert_eq!(
            ast.effective_enumerator_value(enumerators[index]),
            Some(expected),
            "enumerator {index}"
        );
    }
}

#[test]
fn an_explicit_enumerator_value_shifts_the_ones_after_it() {
    let ast = model("enum E { A, @value(10) B, C };");
    let e = type_of(&ast, "E");
    let TypeKind::Enumeration { enumerators } = &ast.type_(e).expect("E").kind else {
        panic!("expected an enumeration");
    };
    assert_eq!(ast.effective_enumerator_value(enumerators[0]), Some(0));
    assert_eq!(ast.effective_enumerator_value(enumerators[1]), Some(10));
    assert_eq!(ast.effective_enumerator_value(enumerators[2]), Some(11));
}

#[test]
fn bit_positions_run_from_zero_and_an_explicit_one_shifts_the_rest() {
    let ast = model("bitmask B { A, @position(4) C, D };");
    let b = type_of(&ast, "B");
    let TypeKind::Bitmask { bits } = &ast.type_(b).expect("B").kind else {
        panic!("expected a bitmask");
    };
    assert_eq!(ast.effective_bit_position(bits[0]), Some(0));
    assert_eq!(ast.effective_bit_position(bits[1]), Some(4));
    assert_eq!(ast.effective_bit_position(bits[2]), Some(5));
}

// -- §5 row 5: bit bound -------------------------------------------------

#[test]
fn an_enum_without_a_bit_bound_is_thirty_two_bits() {
    let ast = model("enum E { A };");
    assert_eq!(ast.effective_bit_bound(type_of(&ast, "E")), 32);
}

#[test]
fn a_written_bit_bound_is_what_the_rule_reports() {
    let ast = model("@bit_bound(8) enum E { A };");
    assert_eq!(ast.effective_bit_bound(type_of(&ast, "E")), 8);
    let ast = model("@bit_bound(16) bitmask B { A };");
    assert_eq!(ast.effective_bit_bound(type_of(&ast, "B")), 16);
}

// -- §5 row 6: constant type ---------------------------------------------

#[test]
fn a_constant_takes_its_declared_type_not_the_expressions() {
    // `const octet N = 1;` is an octet holding 1, not a long. The
    // expression is checked against the type, never used to derive it.
    // IDL 4.2 §7.4.1.4.2.
    let ast = model("const octet N = 1;");
    let any = ast
        .find_by_scoped_name(&ScopedName::absolute(alloc_name("N")))
        .expect("N");
    let AnyId::Const(id, _) = any else {
        panic!("expected a constant, got {any:?}");
    };
    let node = ast.constant(id).expect("N");
    let type_id = node.type_.resolved().expect("a resolved type");
    assert_eq!(
        ast.type_(type_id).expect("type").kind,
        TypeKind::Primitive(zerodds_ast::PrimitiveKind::Octet)
    );
    assert_eq!(ast.eval_integral(node.expr).expect("value"), 1);
}

// -- §5 row 7: array dimensions -----------------------------------------

#[test]
fn an_array_dimension_is_the_evaluated_constant_expression() {
    let mut ast = model("const long N = 2; struct S { long a[N * 3]; };");
    // `N` is pending until the resolve pass runs; the dimension is the
    // expression that was written, not a folded number.
    let _ = ast.resolve_all_by_name().expect("a resolve pass");

    let s = type_of(&ast, "S");
    let member = ast.members_of(s)[0];
    let array = ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("an array type");
    let dims = match &ast.type_(array).expect("array").kind {
        TypeKind::Array { dims, .. } => dims.clone(),
        other => panic!("expected an array, got {other:?}"),
    };
    assert_eq!(dims.len(), 1);
    assert_eq!(ast.eval_integral(dims[0]).expect("evaluates"), 6);
}

#[test]
fn a_multi_dimensional_array_keeps_its_dimensions_in_order() {
    let ast = model("struct S { long a[2][3]; };");
    let s = type_of(&ast, "S");
    let member = ast.members_of(s)[0];
    let array = ast
        .member(member)
        .expect("member")
        .type_
        .resolved()
        .expect("an array");
    let TypeKind::Array { dims, .. } = &ast.type_(array).expect("array").kind else {
        panic!("expected an array");
    };
    assert_eq!(dims.len(), 2);
    assert_eq!(ast.eval_integral(dims[0]).expect("first"), 2);
    assert_eq!(ast.eval_integral(dims[1]).expect("second"), 3);
}

// -- bounds --------------------------------------------------------------

#[test]
fn a_bounded_string_and_sequence_carry_their_bound_expressions() {
    let ast = model("struct S { string<10> a; sequence<long, 4> b; };");
    let s = type_of(&ast, "S");
    let members = ast.members_of(s);

    let string = ast
        .member(members[0])
        .expect("a")
        .type_
        .resolved()
        .expect("a string");
    let TypeKind::String { bound, .. } = &ast.type_(string).expect("string").kind else {
        panic!("expected a string");
    };
    let expr = bound.expr().expect("a bound");
    assert_eq!(ast.eval_integral(expr).expect("evaluates"), 10);

    let sequence = ast
        .member(members[1])
        .expect("b")
        .type_
        .resolved()
        .expect("a sequence");
    let TypeKind::Sequence { bound, .. } = &ast.type_(sequence).expect("sequence").kind else {
        panic!("expected a sequence");
    };
    let expr = bound.expr().expect("a bound");
    assert_eq!(ast.eval_integral(expr).expect("evaluates"), 4);
}

#[test]
fn an_unbounded_collection_says_so_rather_than_carrying_a_zero() {
    let ast = model("struct S { string a; sequence<long> b; };");
    let s = type_of(&ast, "S");
    for (index, name) in [(0, "string"), (1, "sequence")] {
        let member = ast.members_of(s)[index];
        let type_id = ast
            .member(member)
            .expect("member")
            .type_
            .resolved()
            .expect("a type");
        let bound = match &ast.type_(type_id).expect("type").kind {
            TypeKind::String { bound, .. } | TypeKind::Sequence { bound, .. } => *bound,
            other => panic!("expected a {name}, got {other:?}"),
        };
        assert!(!bound.is_bounded(), "{name} gained a bound");
    }
}

#[test]
fn a_fixed_type_carries_its_digits_and_scale() {
    let ast = model("struct S { fixed<9,2> a; };");
    let s = type_of(&ast, "S");
    let member = ast.members_of(s)[0];
    let fixed = ast
        .member(member)
        .expect("a")
        .type_
        .resolved()
        .expect("a fixed");
    assert_eq!(
        ast.type_(fixed).expect("fixed").kind,
        TypeKind::Fixed {
            digits: 9,
            scale: 2
        }
    );
}

// -- §7.3.1.2.1.1, read twice --------------------------------------------

#[test]
fn a_hashed_id_advances_the_counter_by_default_and_is_skipped_for_rti() {
    // The clause states the rule twice and not the same way. One
    // sentence numbers a member from "the most-recently specified ID
    // (using the @id annotation …)", which leaves a hashed id out of the
    // count; the next from "the member ID of the previous member of the
    // same Aggregated Type plus one", which does not.
    //
    // Measured, `crates/ast-layout/tests/fixtures/vendor_bytes.md` §7:
    // for `@hashid long alpha; long b;` Cyclone and Fast DDS give `b`
    // `0343172D`, the hash plus one, and RTI gives it `00000000`. Three
    // of the four take the second reading, so it is the default.
    let source = "@mutable struct S { @hashid long alpha; long b; };";
    let alpha = hashed_member_id("alpha");

    let advancing = model(source);
    assert_eq!(member_id(&advancing, "S", 0).0, alpha);
    assert_eq!(member_id(&advancing, "S", 1).0, alpha + 1);

    let skipping = model_skipping_hashed_ids(source);
    assert_eq!(member_id(&skipping, "S", 0).0, alpha);
    assert_eq!(
        member_id(&skipping, "S", 1).0,
        0,
        "with nothing declared before it, RTI numbers from zero"
    );
}

#[test]
fn a_skipped_hashed_id_leaves_the_last_declared_one_standing() {
    // Not a reset to zero: RTI's counter keeps whatever the last
    // *declared* id was. `@id(10) long a; @hashid long alpha; long b;`
    // gives `b` `0000000B` and not `00000000` — the same fixture, §7.
    let source = "@mutable struct S { @id(10) long a; @hashid long alpha; long b; };";
    let alpha = hashed_member_id("alpha");

    let advancing = model(source);
    assert_eq!(member_id(&advancing, "S", 2).0, alpha + 1);

    let skipping = model_skipping_hashed_ids(source);
    assert_eq!(member_id(&skipping, "S", 1).0, alpha);
    assert_eq!(member_id(&skipping, "S", 2).0, 11);
}

#[test]
fn an_autoid_hash_member_is_skipped_the_same_way() {
    // `@autoid(HASH)` derives the same kind of id from the same hash, so
    // the counter question is the same question. A rule that read the
    // annotation rather than the origin would answer it only for
    // `@hashid`.
    let source = "@autoid(HASH) @mutable struct S { long alpha; @id(4) long b; long c; };";
    let skipping = model_skipping_hashed_ids(source);
    assert_eq!(member_id(&skipping, "S", 0).0, hashed_member_id("alpha"));
    assert_eq!(member_id(&skipping, "S", 1).0, 4);
    assert_eq!(member_id(&skipping, "S", 2).0, hashed_member_id("c"));
}

// -- DDS-XTypes 1.3 §7.3.1.2.1.1, Examples 1 and 2 -------------------------

#[test]
fn example_1_of_the_member_id_clause_numbers_a_derived_chain_0_1_2_3() {
    // §7.3.1.2.1.1 Example 1: ID(s1_m1)=0, ID(s1_m2)=1, ID(s2_m1)=2,
    // ID(s3_m1)=3.
    let ast = model(
        "@autoid(SEQUENTIAL) struct MyStruct1 { long s1_m1; float s1_m2; };\n\
         @autoid(SEQUENTIAL) struct MyStruct2 : MyStruct1 { string s2_m1; };\n\
         @autoid(SEQUENTIAL) struct MyStruct3 : MyStruct2 { long s3_m1; };",
    );
    assert_eq!(member_id(&ast, "MyStruct1", 0).0, 0);
    assert_eq!(member_id(&ast, "MyStruct1", 1).0, 1);
    assert_eq!(member_id(&ast, "MyStruct2", 0).0, 2);
    assert_eq!(member_id(&ast, "MyStruct3", 0).0, 3);
}

#[test]
fn example_2_of_the_member_id_clause_gives_the_two_published_ids() {
    // §7.3.1.2.1.1 Example 2: ID(color) = 0x0FA5DD70 from the member
    // name, ID(size) = 0x047790DA from `@hashid("shapesize")`.
    let ast = model(
        "@autoid(HASH) struct MyStruct1 { string color; @hashid(\"shapesize\") long size; };",
    );
    assert_eq!(member_id(&ast, "MyStruct1", 0).0, 0x0FA5_DD70);
    assert_eq!(member_id(&ast, "MyStruct1", 1).0, 0x0477_90DA);
    assert_eq!(hashed_member_id("shapesize"), 0x0477_90DA);
}
