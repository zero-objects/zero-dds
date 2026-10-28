// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The IDL rules the model cannot express structurally. Spec §6.
//!
//! Each rule gets a test that trips it and a test that does not, because
//! a check that fires on everything is as useless as one that never
//! fires.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_lower::lower;
use zerodds_idl_parse::{Diagnostic, DiagnosticCode, parse};

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let parsed = parse(source);
    lower(&parsed.syntax(), "test.idl").diagnostics
}

/// Whether the source trips a given rule.
fn trips(source: &str, code: DiagnosticCode) -> bool {
    diagnostics(source).iter().any(|d| d.code == code)
}

/// Assert a rule fires, and print every diagnostic when it does not.
fn assert_trips(source: &str, code: DiagnosticCode) {
    let found = diagnostics(source);
    assert!(
        found.iter().any(|d| d.code == code),
        "{source:?} did not trip {}: {found:?}",
        code.as_str()
    );
}

fn assert_clean(source: &str) {
    let found = diagnostics(source);
    assert!(found.is_empty(), "{source:?} reported {found:?}");
}

// -- identifier collisions -----------------------------------------------

#[test]
fn two_names_differing_only_by_case_collide() {
    assert_trips(
        "struct Pose { long x; }; struct pose { long y; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn the_same_name_in_two_scopes_does_not_collide() {
    assert_clean(
        "module A { struct S { long x; }; };
         module B { struct S { long x; }; };",
    );
}

// -- one namespace per scope, §7.2.3.1 -----------------------------------
//
// The clause's own example is `typedef long Foo;` outside an operation
// and `in Foo foo` inside it, and it lists "Foo and foo collide" among
// that example's errors. An aggregate is the same shape: the type name
// is used in the scope the member name is declared in.

#[test]
fn a_member_named_like_the_type_it_uses_collides() {
    assert_trips(
        "struct Inner { long a; }; struct S { Inner inner; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn a_case_member_named_like_the_type_it_uses_collides() {
    assert_trips(
        "struct Cell { long a; };
         union Choice switch (long) { case 1: Cell cell; default: octet raw; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn a_member_named_like_the_discriminator_type_collides() {
    assert_trips(
        "enum Mode { IDLE, ACTIVE };
         union Choice switch (Mode) { case IDLE: long mode; default: octet raw; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn a_member_named_like_a_type_inside_a_collection_collides() {
    assert_trips(
        "struct Colour { long a; }; struct S { sequence<Colour> colour; };",
        DiagnosticCode::DuplicateName,
    );
}

// A union is a scope (IDL 4.2 §7.5.2), and "the name of a module,
// structure, union, … may not be redefined within the immediate scope"
// of it — `struct S { long s; }` was refused, this union was taken.
#[test]
fn a_case_member_named_like_its_union_collides() {
    assert_trips(
        "union Choice switch (long) { case 1: long choice; default: octet raw; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn a_case_member_named_unlike_its_union_is_clean() {
    assert_clean("union Choice switch (long) { case 1: long choices; default: octet raw; };");
}

#[test]
fn two_members_of_one_aggregate_differing_only_by_case_collide() {
    assert_trips(
        "struct S { long tail; short Tail; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn a_member_named_like_a_type_it_reaches_by_scoped_name_does_not_collide() {
    // `::common::First first;` puts no simple identifier in the member's
    // scope. `tao_idl` and `opendds_idl` accept exactly this and refuse
    // the unqualified form beside it.
    assert_clean(
        "module common { struct First { long a; }; };
         module common { struct S { ::common::First first; double b; }; };",
    );
}

#[test]
fn a_member_named_after_its_own_type_in_a_different_spelling_does_not_collide() {
    assert_clean("struct Inner { long a; }; struct S { Inner nested; };");
}

#[test]
fn a_parameter_named_like_its_type_collides() {
    // The clause's own example, written out.
    assert_trips(
        "typedef long Foo;
         interface Thing { void doit(in Foo foo); };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn an_attribute_named_like_its_type_collides() {
    assert_trips(
        "enum Mode { IDLE, ACTIVE };
         interface Control { readonly attribute Mode mode; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn a_parameter_named_like_a_type_used_only_by_another_operation_does_not_collide() {
    // Each operation is its own scope, so a name used in one does not
    // reach the parameters of the next.
    assert_clean(
        "typedef long Foo;
         interface Thing { void one(in Foo value); void two(in long foo); };",
    );
}

#[test]
fn a_member_named_like_a_type_no_other_scope_uses_does_not_collide() {
    // `Inner` is used in `Outer` and `inner` declared in `S`; two
    // different scopes, so the clause does not reach them.
    assert_clean(
        "struct Inner { long a; };
         struct Outer { Inner held; };
         struct S { long inner; };",
    );
}

// -- union discriminators ------------------------------------------------

#[test]
fn each_legal_discriminator_type_is_accepted() {
    for discriminator in ["long", "short", "boolean", "char", "octet", "unsigned long"] {
        let source =
            format!("union U switch ({discriminator}) {{ case 1: long a; default: long b; }};");
        assert!(
            !trips(&source, DiagnosticCode::InvalidDiscriminator),
            "{discriminator} was rejected"
        );
    }
}

#[test]
fn an_enum_discriminator_is_accepted() {
    assert!(!trips(
        "enum Mode { IDLE, ACTIVE };
         union U switch (Mode) { case IDLE: long a; };",
        DiagnosticCode::InvalidDiscriminator
    ));
}

#[test]
fn a_float_discriminator_is_rejected() {
    assert_trips(
        "union U switch (double) { case 1: long a; };",
        DiagnosticCode::InvalidDiscriminator,
    );
}

#[test]
fn a_string_discriminator_is_rejected() {
    assert_trips(
        "union U switch (string) { case 1: long a; };",
        DiagnosticCode::InvalidDiscriminator,
    );
}

#[test]
fn a_struct_discriminator_is_rejected() {
    assert_trips(
        "struct S { long x; };
         union U switch (S) { case 1: long a; };",
        DiagnosticCode::InvalidDiscriminator,
    );
}

#[test]
fn a_typedef_to_a_legal_discriminator_is_accepted() {
    assert!(!trips(
        "typedef long Discriminator;
         union U switch (Discriminator) { case 1: long a; };",
        DiagnosticCode::InvalidDiscriminator
    ));
}

// -- case labels ---------------------------------------------------------

#[test]
fn a_repeated_case_label_is_rejected() {
    assert_trips(
        "union U switch (long) { case 1: long a; case 1: long b; };",
        DiagnosticCode::InvalidCaseLabel,
    );
}

#[test]
fn distinct_case_labels_are_accepted() {
    assert!(!trips(
        "union U switch (long) { case 1: long a; case 2: long b; };",
        DiagnosticCode::InvalidCaseLabel
    ));
}

#[test]
fn two_default_cases_are_rejected() {
    assert_trips(
        "union U switch (long) { default: long a; default: long b; };",
        DiagnosticCode::InvalidCaseLabel,
    );
}

#[test]
fn a_case_label_outside_the_discriminators_range_is_rejected() {
    // 300 cannot be selected by an octet discriminator, so the branch is
    // unreachable and the source is wrong rather than merely odd.
    assert_trips(
        "union U switch (octet) { case 300: long a; };",
        DiagnosticCode::InvalidCaseLabel,
    );
}

#[test]
fn a_case_label_inside_the_range_is_accepted() {
    assert!(!trips(
        "union U switch (octet) { case 255: long a; };",
        DiagnosticCode::InvalidCaseLabel
    ));
}

#[test]
fn a_boolean_discriminator_admits_only_zero_and_one() {
    assert!(!trips(
        "union U switch (boolean) { case 0: long a; case 1: long b; };",
        DiagnosticCode::InvalidCaseLabel
    ));
    assert_trips(
        "union U switch (boolean) { case 2: long a; };",
        DiagnosticCode::InvalidCaseLabel,
    );
}

// -- annotations that contradict each other ------------------------------

#[test]
fn a_key_member_must_not_be_optional() {
    // A key is what identifies an instance; an absent key identifies
    // nothing. DDS-XTypes 1.3 §7.2.2.4.4.4.8: "Key members shall never
    // be optional".
    assert_trips(
        "struct S { @key @optional long a; };",
        DiagnosticCode::ConflictingAnnotations,
    );
}

#[test]
fn a_key_member_must_not_be_non_serialized() {
    assert_trips(
        "struct S { @key @non_serialized long a; };",
        DiagnosticCode::ConflictingAnnotations,
    );
}

#[test]
fn key_on_its_own_is_accepted() {
    assert_clean("struct S { @key long a; long b; };");
}

// -- what may be part of a key -------------------------------------------

#[test]
fn a_map_cannot_be_part_of_a_key() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.8: "Members of type map cannot be
    // included as part of the key." A map in a key holder gives one
    // instance two handles, because two samples whose maps hold the same
    // pairs in a different order serialise differently.
    for source in [
        // The member's own type.
        "struct S { @key map<long, long> m; };",
        // Through an alias — "Aliases to one of the previous types can
        // also be used as key members".
        "typedef map<long, long> M; struct S { @key M m; };",
        // Through a collection — "all the elements in the array or
        // sequence shall be considered part of the key of T".
        "struct S { @key sequence<map<long, long> > entries; };",
        "struct S { @key map<long, long> a[2]; };",
        // Through a nested type's own key — "only the key of K ... shall
        // be considered part of the key of T. This relationship is
        // recursive".
        "struct Inner { @key map<long, long> m; };
         struct S { @key Inner i; long tail; };",
    ] {
        assert_trips(source, DiagnosticCode::InvalidAnnotation);
    }
}

#[test]
fn a_map_that_is_not_in_the_key_is_accepted() {
    // The rule is about the key, not about the type. A map beside a key
    // member, and a map inside a keyed type that is not itself keyed,
    // are both outside the key holder.
    assert_clean("struct S { @key long id; map<long, long> m; };");
    assert_clean(
        "struct Inner { @key long k; map<long, long> m; };
         struct S { @key Inner i; };",
    );
}

#[test]
fn a_string_key_is_accepted_although_the_clause_does_not_list_it() {
    // §7.2.2.4.4.4.8 lists "primitive, aggregation, enumeration,
    // bitmask, array, and sequence" and omits `string`. Its own
    // Example 2 four pages later is `@key string<12> label`, the OMG
    // interop test's `ShapeType` keys on `@key string<128> color`, and
    // Cyclone, RTI and Fast DDS all accept one. The list is incomplete,
    // and only the sentence that forbids something is enforced.
    assert_clean("struct S { @key string<128> color; long x; };");
    assert_clean("struct S { @key sequence<long> path; };");
}

#[test]
fn a_key_member_whose_type_reaches_itself_does_not_hang_the_walk() {
    // Legal IDL: a keyed member whose type contains a sequence of
    // itself. The walk needs its cycle guard or it does not return.
    assert_clean(
        "struct Node;
         struct Node { @key long id; sequence<Node> children; };
         struct S { @key Node root; };",
    );
}

// -- unions --------------------------------------------------------------

#[test]
fn only_a_unions_discriminator_may_be_a_key() {
    // Table 21 lists `@key` against "Structure Members, Union
    // discriminator". A case member marked `@key` would put the branch
    // in the key holder, so the instance handle would depend on which
    // branch the sample selected.
    assert_trips(
        "union U switch (long) { case 1: @key long a; default: long b; };",
        DiagnosticCode::InvalidAnnotation,
    );
}

#[test]
fn no_union_member_is_optional() {
    // §7.2.2.4.4.4.7: "Union members, including the discriminator,
    // shall never be optional." An optional case member plans a
    // presence marker no implementation writes.
    assert_trips(
        "union U switch (long) { case 1: @optional long a; default: long b; };",
        DiagnosticCode::InvalidAnnotation,
    );
}

#[test]
fn a_union_with_plain_members_is_accepted() {
    assert_clean("union U switch (long) { case 1: long a; default: long b; };");
}

// -- @external off its one position --------------------------------------

/// `@external` on a type declaration is refused by name, never silently
/// dropped.
///
/// Table 21 applies `@external` to members. On a `typedef` or a `struct`
/// declaration the flag landed on the type node, where the planner, the
/// codec walks and the adapters — which all read it off the member —
/// never looked: the annotation vanished between the source and every
/// consumer without a word.
#[test]
fn external_on_a_type_declaration_is_refused_by_name() {
    for source in [
        "@external typedef long Handle;",
        "@external struct S { long a; };",
        "@external union U switch (long) { case 1: long a; };",
    ] {
        assert_trips(source, DiagnosticCode::InvalidAnnotation);
    }
}

/// `@external` on a member stays accepted — the refusal above is about
/// the declaration position, not the annotation.
#[test]
fn external_on_a_member_and_on_a_branch_is_accepted() {
    assert_clean(
        "struct Inner { long a; };
         struct S { @external Inner nested; };
         union U switch (long) { case 1: @external Inner i; default: long b; };",
    );
}

// -- member id collisions ------------------------------------------------

#[test]
fn two_declared_ids_that_are_equal_are_rejected() {
    assert_trips(
        "struct S { @id(1) long a; @id(1) long b; };",
        DiagnosticCode::DuplicateMemberId,
    );
}

#[test]
fn a_declared_id_colliding_with_a_derived_one_is_rejected_and_names_both_origins() {
    // `b` derives id 1 from `a`, and the declaration takes it too.
    let found = diagnostics("struct S { long a; long b; @id(1) long c; };");
    let collision = found
        .iter()
        .find(|d| d.code == DiagnosticCode::DuplicateMemberId)
        .unwrap_or_else(|| panic!("no collision reported: {found:?}"));
    assert!(
        collision.message.contains("@id") && collision.message.contains("previous member"),
        "the diagnostic does not say where each id came from: {}",
        collision.message
    );
}

#[test]
fn members_numbered_by_the_default_rule_do_not_collide() {
    assert_clean("struct S { long a; long b; long c; };");
}

// -- bounds, dimensions and precision ------------------------------------

#[test]
fn a_zero_string_bound_is_rejected() {
    assert_trips("struct S { string<0> a; };", DiagnosticCode::InvalidBound);
}

#[test]
fn a_negative_sequence_bound_is_rejected() {
    assert_trips(
        "struct S { sequence<long, -1> a; };",
        DiagnosticCode::InvalidBound,
    );
}

#[test]
fn a_positive_bound_is_accepted() {
    assert_clean("struct S { string<10> a; sequence<long, 4> b; };");
}

#[test]
fn a_zero_array_dimension_is_rejected() {
    assert_trips("struct S { long a[0]; };", DiagnosticCode::InvalidBound);
}

#[test]
fn a_positive_array_dimension_is_accepted() {
    assert_clean("struct S { long a[4]; };");
}

#[test]
fn a_fixed_with_more_than_thirty_one_digits_is_rejected() {
    assert_trips("struct S { fixed<32,2> a; };", DiagnosticCode::InvalidBound);
}

#[test]
fn a_fixed_whose_scale_exceeds_its_digits_is_rejected() {
    assert_trips(
        "struct S { fixed<10,11> a; };",
        DiagnosticCode::InvalidBound,
    );
}

#[test]
fn a_fixed_within_its_ranges_is_accepted() {
    assert_clean("struct S { fixed<31,31> a; fixed<9,2> b; };");
}

// -- forward declarations ------------------------------------------------

#[test]
fn a_forward_declaration_never_defined_is_rejected() {
    assert_trips("struct Pose;", DiagnosticCode::UndefinedForward);
}

#[test]
fn a_forward_declaration_that_is_defined_is_accepted() {
    assert_clean("struct Pose; struct Pose { long x; };");
}

// -- every diagnostic carries its paragraph ------------------------------

#[test]
fn every_diagnostic_names_a_specification_paragraph() {
    // A rule with no paragraph is ZeroDDS policy and has to say so;
    // an empty reference is neither. Spec §7.
    for source in [
        "struct Pose { long x; }; struct pose { long y; };",
        "union U switch (double) { case 1: long a; };",
        "union U switch (long) { case 1: long a; case 1: long b; };",
        "struct S { @key @optional long a; };",
        "struct S { @id(1) long a; @id(1) long b; };",
        "struct S { string<0> a; };",
        "struct S { fixed<32,2> a; };",
        "struct Pose;",
        "@extensibility(NONSENSE) struct S { long a; };",
    ] {
        for diagnostic in diagnostics(source) {
            assert!(
                !diagnostic.spec_reference.is_empty(),
                "{source:?} produced a diagnostic with no paragraph: {diagnostic:?}"
            );
        }
    }
}

#[test]
fn every_diagnostic_points_at_a_byte_range() {
    let found = diagnostics("union U switch (double) { case 1: long a; };");
    assert!(!found.is_empty());
    for diagnostic in found {
        assert!(
            diagnostic.primary.end > diagnostic.primary.start,
            "{diagnostic:?} points at an empty range"
        );
    }
}

#[test]
fn every_diagnostic_code_has_its_own_string() {
    let mut seen: Vec<&'static str> = Vec::new();
    for code in DiagnosticCode::ALL {
        let text = code.as_str();
        assert!(!seen.contains(&text), "{text} is used by two codes");
        seen.push(text);
    }
}

// -- a clean file stays clean --------------------------------------------

#[test]
fn a_file_using_the_whole_language_reports_nothing() {
    assert_clean(
        "module Robot {
           const long MAX = 8;
           @bit_bound(16) enum Mode { IDLE, @value(4) ACTIVE, FAULT };
           bitmask Flags { READY, @position(4) MOVING };
           @mutable struct Header { @key long id; string<32> name; };
           @mutable struct Pose : Header {
             @id(10) double x;
             double y, z;
             sequence<long, MAX> path;
             long grid[MAX][2];
           };
           union Payload switch (Mode) {
             case IDLE: long idle;
             case ACTIVE: Pose stance;
             default: octet raw;
           };
           exception Failure { long code; };
           interface Control {
             void go(in Pose target) raises (Failure);
             readonly attribute Mode setting;
           };
         };",
    );
}

// -- §7.2.3/§7.2.4: an identifier that collides with a keyword -----------

#[test]
fn an_identifier_differing_from_a_keyword_only_in_case_is_refused() {
    // IDL 4.2 §7.2.4, p. 20: "Keywords must be written exactly as shown
    // in the above list. Identifiers that collide with keywords (see
    // 7.2.3, Identifiers) are illegal." The clause's own example is
    // `typedef boolean BOOLEAN;`.
    assert_trips(
        "enum Ee { AA, CUSTOM, BB };",
        DiagnosticCode::KeywordCollision,
    );
    assert_trips("typedef boolean BOOLEAN;", DiagnosticCode::KeywordCollision);
    assert_trips("struct S { Long a; };", DiagnosticCode::KeywordCollision);
    assert_trips("const boolean B = true;", DiagnosticCode::KeywordCollision);
}

#[test]
fn the_diagnostic_names_the_keyword_and_the_clause() {
    let found = diagnostics("enum Ee { AA, CUSTOM, BB };");
    let reported = found
        .iter()
        .find(|d| d.code == DiagnosticCode::KeywordCollision)
        .expect("the collision is reported");
    assert!(reported.message.contains("CUSTOM"), "{reported}");
    assert!(reported.message.contains("custom"), "{reported}");
    assert_eq!(reported.spec_reference, "OMG IDL 4.2 §7.2.3, §7.2.4");
}

#[test]
fn the_underscore_escape_turns_the_keyword_check_off() {
    // §7.2.3.2: prefixing an underscore "is a purely lexical convention
    // that ONLY turns off keyword checking", and `_AnIdentifier` is the
    // name `AnIdentifier`.
    assert_clean("enum Ee { AA, _custom, BB };");
    assert_clean("struct S { long _long; };");
}

#[test]
fn a_name_that_collides_with_no_keyword_is_left_alone() {
    assert_clean("enum Ee { AA, COLOUR, BB };");
    assert!(!trips(
        "struct S { long value; };",
        DiagnosticCode::KeywordCollision
    ));
}

// -- rules (52), (57) and (204): repetitions that are one-or-more --------

#[test]
fn an_enumeration_with_no_enumerator_is_refused() {
    // (57) `<enum_dcl> ::= "enum" <identifier> "{" <enumerator> { ","
    // <enumerator> }* "}"` — one enumerator and then any number more,
    // never none. All seven foreign front ends measured refuse
    // `enum E { };`.
    assert_trips("enum E { };", DiagnosticCode::MissingToken);
    assert_clean("enum E { ONLY };");
}

#[test]
fn a_bitmask_with_no_bit_value_is_refused() {
    // (204) `<bitmask_dcl> ::= "bitmask" <identifier> "{" <bit_value>
    // { "," <bit_value> }* "}"` — the same shape one building block on.
    assert_trips("bitmask B { };", DiagnosticCode::MissingToken);
    assert_clean("bitmask B { FLAG };");
}

#[test]
fn a_union_with_no_case_is_refused() {
    // (52) `<switch_body> ::= <case>+`, the third of the same shape.
    assert_trips("union U switch (long) { };", DiagnosticCode::MissingToken);
    assert_clean("union U switch (long) { case 1: long a; };");
}

#[test]
fn the_two_bodies_the_grammar_does_let_be_empty_stay_empty() {
    // Not every empty body is a mistake, and a check that refused these
    // would refuse legal IDL. (198) adds `"struct" <identifier> "{" "}"`
    // outright, and (200) writes a bitset's body as `<bitfield>*`.
    assert_clean("struct S { };");
    assert_clean("bitset S { };");
}

// -- the declared type of a constant -------------------------------------

#[test]
fn a_constant_whose_value_is_not_of_its_declared_type_is_refused() {
    // IDL 4.2 §7.4.1.4.3: the value expression "shall be consistent with
    // the type declared for the constant". Without this the declared
    // type was discarded and the literal's own type took its place —
    // `const long K = "x";` reached the Rust backend as
    // `pub const K: &'static str = "x";`.
    assert_trips("const long K = \"x\";", DiagnosticCode::TypeMismatch);
    assert_trips("const boolean K = 1;", DiagnosticCode::TypeMismatch);
    assert_trips("const char K = \"x\";", DiagnosticCode::TypeMismatch);
    assert_trips("const long K = 1.5;", DiagnosticCode::TypeMismatch);
    assert_trips("const string K = 'a';", DiagnosticCode::TypeMismatch);
}

#[test]
fn the_wide_and_narrow_string_pair_is_the_case_the_clause_names() {
    // §7.2.6.3, in so many words: "Attempts to assign a wide string
    // literal to a non-wide string constant or to assign a non-wide
    // string literal to a wide string constant result in a compile-time
    // diagnostic."
    assert_trips("const string S = L\"x\";", DiagnosticCode::TypeMismatch);
    assert_trips("const wstring S = \"x\";", DiagnosticCode::TypeMismatch);
    assert_clean("const string S = \"x\";");
    assert_clean("const wstring S = L\"x\";");
}

#[test]
fn every_form_the_specification_admits_still_gets_through() {
    // The half that keeps the rule honest. A rejection that reaches too
    // far breaks valid IDL, which costs more than the defect it removes,
    // so each of these is a form IDL 4.2 allows and must keep allowing.
    for source in [
        // One per declared type, with the literal it takes.
        "const short K = 1;",
        "const long K = 1;",
        "const long long K = 1;",
        "const unsigned long K = 1;",
        "const octet K = 255;",
        "const int8 K = 1;",
        "const uint64 K = 1;",
        "const boolean K = TRUE;",
        "const char K = 'a';",
        "const wchar K = L'a';",
        // A narrow character literal is a wide one written narrowly.
        "const wchar K = 'a';",
        "const string K = \"x\";",
        "const string<8> K = \"x\";",
        "const wstring K = L\"x\";",
        "const double K = 1.5;",
        "const float K = 1.5;",
        "const long double K = 1.5;",
        // An integer where a float is declared: a whole number is a
        // number.
        "const double K = 1;",
        // Rule (43): `const fixed` takes its digits and scale from the
        // value, so there is no declared width to disagree with.
        "const fixed K = 1.23d;",
        // Forms whose type this stage does not settle, and which
        // therefore have to pass without a word rather than be guessed
        // at.
        "typedef long Ali;\nconst Ali K = 1;",
        "const long K1 = 1;\nconst long K2 = K1;",
        "enum Ea { EA1, EA2 };\nconst Ea K = EA1;",
        // Operators carry the form of their operands.
        "const long K = 1 + 2;",
        "const long K = -1;",
        "const long K = ~1;",
        "const long K = (1 + 2) * 3;",
        "const double K = 1 + 1.5;",
        "const long K1 = 1;\nconst long K2 = K1 + 1;",
    ] {
        assert!(
            !trips(source, DiagnosticCode::TypeMismatch),
            "{source:?} is IDL 4.2 and was refused"
        );
    }
}

// -- the bounds of §7.4.13 -----------------------------------------------

#[test]
fn a_bitfield_wider_than_sixty_four_bits_is_refused() {
    // §7.4.13.4.3.2: "The first one (<positive_int_const>) is the number
    // of bits that can be stored (its size). The maximum value is 64."
    // Before this the front end said nothing and the C writer refused it
    // one whole backend later, with "bitset wider than the sixty-four
    // bits an anchor holds".
    assert_trips(
        "bitset Ba { bitfield<65> f1; };",
        DiagnosticCode::InvalidBound,
    );
    assert_clean("bitset Ba { bitfield<64> f1; };");
}

#[test]
fn a_bitfield_wider_than_its_destination_type_is_refused() {
    // §7.4.13.4.3.2: "When the destination type is given, the number of
    // stored bits cannot exceed its size (i.e., 1 for boolean, 8 for
    // octet, …)."
    assert_trips(
        "bitset Ba { bitfield<9, octet> f1; };",
        DiagnosticCode::InvalidBound,
    );
    assert_trips(
        "bitset Ba { bitfield<2, boolean> f1; };",
        DiagnosticCode::InvalidBound,
    );
    assert_clean("bitset Ba { bitfield<8, octet> f1; };");
    assert_clean("bitset Ba { bitfield<1, boolean> f1; };");
    assert_clean("bitset Ba { bitfield<17, long> f1; };");
}

#[test]
fn a_bit_bound_over_sixty_four_is_refused() {
    // §7.4.13.4.3.3: `@bit_bound` "must be lower than or equal to 64".
    assert_trips(
        "@bit_bound(65) bitmask Ba { F1 };",
        DiagnosticCode::InvalidBound,
    );
    assert_clean("@bit_bound(64) bitmask Ba { F1 };");
}

#[test]
fn a_bitmask_with_more_values_than_its_size_is_refused() {
    // §7.4.13.4.3.3: "A bit mask must contain at least one value and no
    // more than its size expressed in bits", and with an explicit bound
    // "the number of values cannot then exceed the value given to
    // @bit_bound".
    assert_trips(
        "@bit_bound(2) bitmask Ba { F1, F2, F3 };",
        DiagnosticCode::InvalidBound,
    );
    assert_clean("@bit_bound(4) bitmask Ba { F1, F2, F3 };");

    // "By default, the size of a bit mask is 32" — so the 33rd value is
    // one too many even with no annotation in sight.
    let names = |n: usize| {
        (1..=n)
            .map(|i| format!("F{i:02}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    assert_trips(
        &format!("bitmask Ba {{ {} }};", names(33)),
        DiagnosticCode::InvalidBound,
    );
    assert_clean(&format!("bitmask Ba {{ {} }};", names(32)));
}

#[test]
fn a_bit_position_outside_the_bitmasks_size_is_refused() {
    // §7.4.13.4.3.3: `@position` — "Possible positions range from 0 …
    // up to (size – 1)".
    assert_trips(
        "@bit_bound(2) bitmask Ba { @position(5) F1 };",
        DiagnosticCode::InvalidBound,
    );
    assert_clean("@bit_bound(8) bitmask Ba { @position(7) F1 };");
    assert_clean("bitmask Ba { @position(31) F1 };");
}

#[test]
fn the_bit_forms_the_specification_admits_still_get_through() {
    // The half that keeps the five rules above honest.
    for source in [
        "bitmask Ba { F1 };",
        "bitmask Ba { F1, F2, F3 };",
        "@bit_bound(1) bitmask Ba { F1 };",
        "bitset Ba { bitfield<3> f1; bitfield<5> f2; };",
        // An anonymous bitfield is padding and still has a width.
        "bitset Ba { bitfield<4>; bitfield<3> f2; };",
        "bitset Ba { bitfield<1, boolean> f1; bitfield<8, octet> f2; };",
        "bitset Ba { bitfield<32, long> f1; };",
        "bitset Ba { bitfield<64, long long> f1; };",
        "@bit_bound(64) bitmask Ba { @position(63) F1 };",
    ] {
        assert!(
            !trips(source, DiagnosticCode::InvalidBound),
            "{source:?} is IDL 4.2 and was refused"
        );
    }
}

// -- §7.4.15.4.2: applying a declared annotation -------------------------

#[test]
fn an_annotation_member_with_no_default_takes_the_default_of_its_type() {
    // DDS-XTypes 1.3 §7.3.1.2.5: "If an omitted member does not have a
    // specified default, it will take the default value specified for its
    // type in Clause 7.2.2.4.4.4.7." (IDL 4.2 §7.4.15.4.2 reads the other
    // way; the front end of this specification follows its own.)
    assert_clean("@annotation Aa { long v; };\n@Aa struct Sa { long m; };");
    assert_clean("@annotation Aa { long v; string s; };\n@Aa(v = 1) struct Sa { long m; };");
    assert_clean("@annotation Aa { long v; };\n@Aa(v = 1) struct Sa { long m; };");
    assert_clean("@annotation Aa { long v; };\n@Aa(1) struct Sa { long m; };");
    assert_clean("@annotation Aa { long v default 0; };\n@Aa struct Sa { long m; };");
}

#[test]
fn the_names_of_annotation_members_are_not_case_sensitive() {
    // §7.3.1.2.5: "the name of an annotation and its members are not
    // case-sensitive."
    assert_clean(
        "@annotation Aa { long Value default 0; };\n@Aa(value = 1) struct Sa { long m; };",
    );
    assert_trips(
        "@annotation Aa { long v default 0; };\n@Aa(w = 1) struct Sa { long m; };",
        DiagnosticCode::InvalidAnnotation,
    );
}

#[test]
fn an_annotation_argument_has_to_have_the_members_type() {
    // "A constant expression, whose type must be compatible with the
    // member's declaration." The same reading as a constant's own value
    // — `Lowering::value_disagrees_with` answers both, so the two
    // clauses cannot drift apart.
    assert_trips(
        "@annotation Aa { long v default 0; };\n@Aa(\"x\") struct Sa { long m; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "@annotation Aa { string v default \"\"; };\n@Aa(1) struct Sa { long m; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("@annotation Aa { long v default 0; };\n@Aa(1) struct Sa { long m; };");
    assert_clean("@annotation Aa { string v default \"\"; };\n@Aa(\"x\") struct Sa { long m; };");
    assert_clean("@annotation Aa { boolean v default FALSE; };\n@Aa(TRUE) struct Sa { long m; };");
}

#[test]
fn an_argument_naming_no_member_is_refused() {
    // The clause's closing sentence: "malformed definitions shall be
    // treated as an error."
    assert_trips(
        "@annotation Aa { long v default 0; };\n@Aa(nosuch = 1) struct Sa { long m; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("@annotation Aa { long v default 0; };\n@Aa(v = 1) struct Sa { long m; };");
}

#[test]
fn the_short_form_is_only_for_an_annotation_with_one_member() {
    // "In case there is only one member, the annotation application may
    // be shortened to the name of the annotation prefixed with @ and
    // followed with the constant value of that unique member."
    assert_trips(
        "@annotation Aa { long v1 default 0; long v2 default 0; };\n\
         @Aa(1) struct Sa { long m; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean(
        "@annotation Aa { long v1 default 0; long v2 default 0; };\n\
         @Aa(v1 = 1, v2 = 2) struct Sa { long m; };",
    );
    assert_clean("@annotation Aa { long v default 0; };\n@Aa(1) struct Sa { long m; };");
}

#[test]
fn none_of_the_four_rules_reaches_an_unknown_annotation() {
    // The boundary is the clause's own: the same paragraph says an
    // unknown annotation "should be ignored by the IDL-processing
    // tools". A rule that fired on `@RTI_topic_kind(NO_KEY)` would break
    // every vendor file in the tree to enforce a declaration that does
    // not exist.
    for source in [
        "@nosuchthing struct Sa { long m; };",
        "@nosuchthing(1) struct Sa { long m; };",
        "@nosuchthing(v = 1) struct Sa { long m; };",
        "@nosuchthing(a = 1, b = 2) struct Sa { long m; };",
        "@RTI_resource_limits(initial_count = 10, max_count = 100) struct Sa { long m; };",
    ] {
        assert!(
            !trips(source, DiagnosticCode::InvalidAnnotation),
            "{source:?}: §7.4.15.4.2 says ignore it"
        );
    }
}

#[test]
fn an_annotations_declared_members_reach_the_model() {
    // The defect underneath all four rules above. Rule (222) writes an
    // annotation member's name with a bare identifier and the parser
    // wraps it in no `Declarator` node, so `Lowering::declared` — which
    // walks declarators — found none and returned an empty list. Every
    // annotation reached the model with zero members, and nothing
    // downstream had anything to check an application against.
    let parsed = parse("@annotation Aa { long v1 default 0; string v2; };");
    let ast = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl").ast;
    let zerodds_ast::AnyId::AnnotationDef(id, _) = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![String::from(
            "Aa",
        )]))
        .expect("the declaration")
    else {
        panic!("Aa is not an annotation declaration");
    };
    let members: Vec<String> = ast
        .annotation_def(id)
        .expect("the declaration")
        .members
        .iter()
        .filter_map(|member| ast.annotation_member(*member).ok())
        .map(|member| member.name.clone())
        .collect();
    assert_eq!(members, vec!["v1".to_string(), "v2".to_string()]);
}

// -- §7.5.2: a declaration may not reuse its scope's name ----------------

#[test]
fn a_declaration_may_not_reuse_the_name_of_its_own_scope() {
    // §7.5.2, with the clause's own example: `module M { typedef short
    // M; };` marked `// Error`.
    assert_trips(
        "module Ma { typedef short Ma; };",
        DiagnosticCode::DuplicateName,
    );
    assert_trips(
        "module common { @final struct common { long x; }; };",
        DiagnosticCode::DuplicateName,
    );
    // Case-insensitively, because §7.2.3 makes every identifier
    // comparison in IDL case-insensitive.
    assert_trips(
        "module Ma { typedef short MA; };",
        DiagnosticCode::DuplicateName,
    );
}

#[test]
fn only_the_immediate_scope_is_compared() {
    // The half that keeps the rule from breaking valid IDL. The clause
    // says "within the immediate scope", and a nested scope is a
    // different scope: refusing these would refuse a great deal of
    // legal IDL for a rule that does not say so.
    assert_clean("module Ma { module Mb { typedef short Ma; }; };");
    assert_clean("module Ma { module Mb { struct Ma { long x; }; }; };");
    assert_clean("module Ma { typedef short Sb; };");
    assert_clean("struct Sa { long m; };\nmodule Mb { typedef short Sa; };");
}

// -- DDS-XTypes 1.3 §7.2.2: rules of the type system ----------------------

/// Table 12 (§7.2.3): "Enumerated Types: ENUMERATION_TYPE, BITMASK_TYPE —
/// These types may be final or appendable on a type-by-type basis."
#[test]
fn an_enum_or_bitmask_is_never_mutable() {
    assert_trips(
        "@mutable enum E { A, B };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "@mutable bitmask M { A, B };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "@extensibility(MUTABLE) enum E { A, B };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("@final enum E { A, B };");
    assert_clean("@appendable enum E { A, B };");
    assert_clean("@final bitmask M { A, B };");
    assert_clean("@mutable struct S { long a; };");
}

/// Table 4: an enumerated literal has "a name and an Int32 value that are
/// each unique within that type".
#[test]
fn two_enumerators_of_one_value_are_rejected() {
    assert_trips(
        "enum E { @value(1) A, @value(1) B };",
        DiagnosticCode::InvalidAnnotation,
    );
    // An explicit value that meets an implicit one.
    assert_trips(
        "enum E { A, @value(0) B };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("enum E { @value(1) A, @value(2) B, C };");
}

/// Table 4: an enumerated literal has "a name and an Int32 value". A value
/// outside `i32` has no representation in a TypeObject
/// (`CommonEnumeratedLiteral.value` is an `int32`, §7.3.4.5.5).
#[test]
fn an_enumerator_value_outside_int32_is_rejected() {
    assert_trips(
        "enum E { @value(2147483648) A };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "enum E { @value(4294967296) A };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "enum E { @value(-2147483649) A };",
        DiagnosticCode::InvalidAnnotation,
    );
    // The values the implicit numbering reaches after the last Int32.
    assert_trips(
        "enum E { @value(2147483647) A, B };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("enum E { @value(2147483647) A };");
    assert_clean("enum E { @value(-2147483648) A, B };");
}

/// §7.2.2.4.4.4.4: "The remaining part of the member ID range—from 0 to
/// 268,402,687 (0x0FFFBFFF)—is available for use by application-defined
/// types". The ids of that range are accepted, at both ends. The text
/// above the range says 28 bits, and 0x0FFFC000 to 0x0FFFFFFF stay inside
/// them: the specification does not forbid an application the ids it does
/// not make available (DDSXTY14-8), and the recorded vendor corpus writes
/// `@id(268435455)`.
#[test]
fn an_application_member_id_in_the_available_range_is_accepted() {
    assert_clean("@mutable struct S { @id(0) long a; };");
    assert_clean("@mutable struct S { @id(268419071) long a; };");
    assert_clean("@mutable struct S { @id(1) long a; @id(268419071) long b; };");
}

/// §7.2.2.4.4.4.4: member IDs "have a range that can be represented in 28
/// bits: from zero to 268,435,455 (0x0FFFFFFF)".
#[test]
fn a_member_id_is_at_most_28_bits() {
    assert_trips(
        "@mutable struct S { @id(268435456) long a; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "@mutable struct S { @id(4294967295) long a; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("@mutable struct S { @id(268435455) long a; };");
    assert_clean("@mutable struct S { @id(0) long a; };");
}

/// §7.2.2.4.4.3: the name `discriminator` "is reserved for union types".
#[test]
fn a_union_member_is_not_named_discriminator() {
    assert_trips(
        "union U switch (long) { case 1: long discriminator; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean("union U switch (long) { case 1: long value; };");
    // The name is only reserved for unions.
    assert_clean("struct S { long discriminator; };");
}

/// §7.2.2.4.4.4.9: "Optional members are not allowed to associate an
/// explicit default value for the member."
#[test]
fn an_optional_member_has_no_default() {
    assert_trips(
        "struct S { @optional @default(5) long a; };",
        DiagnosticCode::ConflictingAnnotations,
    );
    assert_clean("struct S { @default(5) long a; };");
    assert_clean("struct S { @optional long a; };");
}

/// §7.2.2.6: "The types permitted for an AnnotationParameter are:
/// Primitive types, String types of Char8 or Char16 elements, Enumerated
/// types."
#[test]
fn an_annotation_member_is_a_primitive_a_string_or_an_enum() {
    assert_trips(
        "struct P { long x; };\n@annotation A { P part; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_trips(
        "union U switch (long) { case 1: long a; };\n@annotation A { U choice; };",
        DiagnosticCode::InvalidAnnotation,
    );
    assert_clean(
        "enum E { X, Y };\n\
         typedef long L;\n\
         @annotation A { long a; boolean b; string s; wstring w; wchar c; \
         long double d; E mode; L count; };",
    );
}
