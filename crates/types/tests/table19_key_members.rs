//! Table 19's key-member rules, and the two other assignability
//! findings of `docs/spec-coverage/dds-xtypes-1.3.gaps.md`.
//!
//! One IDL text per case, both types built from it, and
//! `is_assignable(writer, reader)` asked. The types are lowered rather
//! than constructed here on purpose: a rule tested only against types
//! the test built itself is a rule tested against the same
//! misunderstanding twice, and every one of these findings was a rule
//! that had no test at all.
//!
//! DDS-XTypes 1.3 writes the relation as "T1 is-assignable-from T2" with
//! **T1 the reader** and T2 the writer, so `m1` below is the reader's
//! member and `m2` the writer's. `is_assignable` takes the writer first.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_types::assignability::{AssignabilityConfig, is_assignable};
use zerodds_types::from_ast::{Form, register_all, type_identifier};
use zerodds_types::resolve::TypeRegistry;

/// Is the reader `T1` assignable from the writer `T2`, both built from
/// `idl`?
fn assignable(reader: &str, writer: &str, idl: &str) -> bool {
    let parsed = zerodds_idl_parse::parse(idl);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the IDL does not parse: {errors:?}");

    // `KeyOnUnionDiscriminator::Carried`, because Table 19's rule about a
    // union's discriminator only has a subject when the model carries the
    // annotation. Under the default the lowering *reports* `@key` on a
    // discriminator and drops it — every measured front end does — so both
    // sides of a keyed/unkeyed pair arrive unkeyed and compare assignable,
    // which is a statement about the axis and not about Table 19.
    let lowered = zerodds_idl_lower::lower_with(
        &parsed.syntax(),
        "table19.idl",
        zerodds_idl_lower::Options {
            key_on_union_discriminator: zerodds_idl_lower::KeyOnUnionDiscriminator::Carried,
            ..zerodds_idl_lower::Options::default()
        },
    );
    // Asserted, not discarded. The helper read only the *parser's*
    // diagnostics before this, so a declaration the lowering refused
    // reached the comparison as a model missing what the test was about
    // — which is how the keyed-discriminator case came to pass a rule it
    // was no longer exercising.
    let lowering_errors: Vec<_> = lowered
        .diagnostics
        .iter()
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .collect();
    assert!(
        lowering_errors.is_empty(),
        "the IDL does not lower: {lowering_errors:?}"
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);

    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("registration");
    let id = |name: &str| {
        ast.types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no type named `{name}`"))
    };
    let r = type_identifier(&ast, id(reader), Form::Minimal).expect("reader identifier");
    let w = type_identifier(&ast, id(writer), Form::Minimal).expect("writer identifier");
    is_assignable(&w, &r, &registry, &AssignabilityConfig::default()).is_yes()
}

/// The three bullets are an axis (`KeyMemberBounds`): the default is the
/// clause, and `Ignored` is what the OMG `dds-xtypes` suite's four
/// `struct_key_*` cases expect and what RTI Connext 7.7.0, OpenDDS 3.35,
/// CoreDX 6.15.3 and HDDS 1.3.0 were measured doing (2026-09-13, the
/// suite's release binaries in self-pairs). The axis reaches exactly the
/// bullets: the ordinary member rule still refuses what it refused.
#[test]
fn the_key_member_bullets_are_an_axis_whose_default_is_the_clause() {
    use zerodds_types::assignability::KeyMemberBounds;
    let idl = "\
@mutable struct R { @id(1) @key string<8> k; };
@mutable struct W { @id(1) @key string<16> k; };";
    let parsed = zerodds_idl_parse::parse(idl);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "axis.idl");
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolution");
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("registration");
    let id = |name: &str| {
        ast.types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no type named `{name}`"))
    };
    let r = type_identifier(&ast, id("R"), Form::Minimal).expect("reader identifier");
    let w = type_identifier(&ast, id("W"), Form::Minimal).expect("writer identifier");

    let enforced = AssignabilityConfig::default();
    assert_eq!(enforced.key_member_bounds, KeyMemberBounds::Enforced);
    assert!(!is_assignable(&w, &r, &registry, &enforced).is_yes());

    let ignored = AssignabilityConfig {
        key_member_bounds: KeyMemberBounds::Ignored,
        ..AssignabilityConfig::default()
    };
    assert!(is_assignable(&w, &r, &registry, &ignored).is_yes());

    // The axis does not reach the member rule: a @final pair with a
    // differing key member type is refused under both.
    let final_idl = "\
@final struct R { @key long k; };
@final struct W { @key short k; };";
    let parsed = zerodds_idl_parse::parse(final_idl);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "axis2.idl");
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolution");
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("registration");
    let id = |name: &str| {
        ast.types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no type named `{name}`"))
    };
    let r = type_identifier(&ast, id("R"), Form::Minimal).expect("reader identifier");
    let w = type_identifier(&ast, id("W"), Form::Minimal).expect("writer identifier");
    assert!(!is_assignable(&w, &r, &registry, &ignored).is_yes());
}

// ---------------------------------------------------------------------
// Table 19, STRUCTURE_TYPE — the three key rules that read a length or
// a literal set
// ---------------------------------------------------------------------

/// > For any string key member m2 in T2, the m1 member of T1 with the
/// > same member ID verifies `m1.type.length >= m2.type.length`.
///
/// A reader whose `@key string<8>` matched a writer's `@key string<16>`
/// received keys it cannot hold: two instances whose first eight octets
/// agree become one, and under `KEEP_LAST(1)` the samples of one
/// overwrite the samples of the other.
#[test]
fn a_shorter_string_key_in_the_reader_is_not_assignable() {
    assert!(!assignable(
        "R",
        "W",
        "\
@mutable struct R { @id(1) @key string<8> k; };
@mutable struct W { @id(1) @key string<16> k; };"
    ));
    // The other way round the reader holds every key the writer can
    // send, and the rule says nothing against it.
    assert!(assignable(
        "R",
        "W",
        "\
@mutable struct R { @id(1) @key string<16> k; };
@mutable struct W { @id(1) @key string<8> k; };"
    ));
    // Equal bounds are the ordinary case and stay assignable.
    assert!(assignable(
        "R",
        "W",
        "\
@mutable struct R { @id(1) @key string<8> k; };
@mutable struct W { @id(1) @key string<8> k; };"
    ));
}

/// The same rule stated for a member that is **not** a key: a
/// `string<8>` reader is assignable from a `string<16>` writer, because
/// `@try_construct` decides what happens to a value that does not fit.
/// Only being a key makes the loss inadmissible, which is why the rule
/// could not live in the general member check.
#[test]
fn a_shorter_string_member_that_is_not_a_key_is_still_assignable() {
    assert!(assignable(
        "R",
        "W",
        "\
@mutable struct R { @id(1) string<8> s; };
@mutable struct W { @id(1) string<16> s; };"
    ));
}

/// > For any enumerated key member m2 in T2, the m1 member of T1 with
/// > the same member ID verifies that all literals in m2.type appear as
/// > literals in m1.type.
#[test]
fn a_writer_enum_key_literal_the_reader_lacks_is_not_assignable() {
    assert!(!assignable(
        "R",
        "W",
        // Each enum in its own module: IDL 4.2 §7.4.1.4.4.2.1 puts an
        // enumerator in the *enclosing* scope, not inside the enum, so
        // two enums in one scope cannot share a literal name — and the
        // rule under test is about literals that appear on both sides.
        "\
module r { @appendable enum E { A }; };
module w { @appendable enum E { A, B }; };
@mutable struct R { @id(1) @key r::E k; };
@mutable struct W { @id(1) @key w::E k; };"
    ));
    // A reader that knows more literals than the writer can send is
    // fine: every value that arrives has a name.
    assert!(assignable(
        "R",
        "W",
        "\
module r { @appendable enum E { A, B }; };
module w { @appendable enum E { A }; };
@mutable struct R { @id(1) @key r::E k; };
@mutable struct W { @id(1) @key w::E k; };"
    ));
}

/// > For any sequence or map key member m2 in T2, the m1 member of T1
/// > with the same member ID verifies
/// > `m1.type.length >= m2.type.length`.
#[test]
fn a_shorter_sequence_key_in_the_reader_is_not_assignable() {
    assert!(!assignable(
        "R",
        "W",
        "\
@mutable struct R { @id(1) @key sequence<long,4> k; };
@mutable struct W { @id(1) @key sequence<long,8> k; };"
    ));
    assert!(assignable(
        "R",
        "W",
        "\
@mutable struct R { @id(1) @key sequence<long,8> k; };
@mutable struct W { @id(1) @key sequence<long,4> k; };"
    ));
}

// ---------------------------------------------------------------------
// Table 19, UNION_TYPE — the discriminator key
// ---------------------------------------------------------------------

/// > Either the discriminators of both T1 and T2 are keys or neither
/// > are keys.
///
/// A keyed discriminator makes the union an instance-carrying type and
/// an unkeyed one does not, so a pair that disagrees has one side
/// splitting the samples into instances and the other holding them all
/// as one.
#[test]
fn one_keyed_union_discriminator_and_one_unkeyed_are_not_assignable() {
    assert!(!assignable(
        "R",
        "W",
        "\
@final union R switch (@key long) { case 1: long a; };
@final union W switch (long) { case 1: long a; };"
    ));
    assert!(!assignable(
        "R",
        "W",
        "\
@final union R switch (long) { case 1: long a; };
@final union W switch (@key long) { case 1: long a; };"
    ));
    // Both keyed, and both unkeyed, are the pairs the rule admits.
    assert!(assignable(
        "R",
        "W",
        "\
@final union R switch (@key long) { case 1: long a; };
@final union W switch (@key long) { case 1: long a; };"
    ));
    assert!(assignable(
        "R",
        "W",
        "\
@final union R switch (long) { case 1: long a; };
@final union W switch (long) { case 1: long a; };"
    ));
}

// ---------------------------------------------------------------------
// Table 18, BITMASK_TYPE — the bound and nothing else
// ---------------------------------------------------------------------

/// > `BITMASK_TYPE` if and only if `T1.bound == T2.bound`
///
/// and beside it: "Preserve bit values by index **for all bits
/// identified in both T1 and T2**" — a sentence that expects bits only
/// one side names. Two bitmasks of the same bound whose flags sit at
/// different positions, and a writer with one flag more than the reader,
/// were both refused.
#[test]
fn two_bitmasks_of_the_same_bound_are_assignable_whatever_their_flags() {
    assert!(assignable(
        "R",
        "W",
        "\
@bit_bound(8) bitmask R { @position(0) F0 };
@bit_bound(8) bitmask W { @position(1) F1 };"
    ));
    assert!(assignable(
        "R",
        "W",
        "\
@bit_bound(8) bitmask R { @position(0) F0 };
@bit_bound(8) bitmask W { @position(0) F0, @position(1) F1 };"
    ));
    // The bound is the wire width and there is no coercion of it.
    assert!(!assignable(
        "R",
        "W",
        "\
@bit_bound(8) bitmask R { @position(0) F0 };
@bit_bound(16) bitmask W { @position(0) F0 };"
    ));
}

// ---------------------------------------------------------------------
// §7.2.4.4.2 — `@non_serialized`
// ---------------------------------------------------------------------

/// > Members that are marked as non-serialized, see Sub Clause
/// > 7.3.1.2.1.14, shall be ignored during type compatibility checking.
///
/// The member is left out of the `TypeObject` entirely, which is both
/// halves at once: it is not on the wire, and the assignability rules
/// read the `TypeObject` and nothing else.
#[test]
fn a_non_serialized_member_does_not_enter_the_comparison() {
    assert!(assignable(
        "R",
        "W",
        "\
@final struct R { long a; };
@final struct W { long a; @non_serialized long b; };"
    ));
    // And the other way: the reader carries the extra member.
    assert!(assignable(
        "R",
        "W",
        "\
@final struct R { long a; @non_serialized long b; };
@final struct W { long a; };"
    ));
    // An ordinary extra member in a `@final` struct is still refused,
    // so the rule above is about the annotation and not about `@final`
    // having gone soft.
    assert!(!assignable(
        "R",
        "W",
        "\
@final struct R { long a; };
@final struct W { long a; long b; };"
    ));
}

// ---------------------------------------------------------------------
// Table 29 — TK_NONE
// ---------------------------------------------------------------------

/// Table 29's first row: `TK_NONE | N/A | Invalid identifier`.
///
/// §7.2.4.1's "a type is always assignable from itself" is a statement
/// about *types*, and `TK_NONE` names none — it is what a
/// `TypeInformation` writes for an equivalence form the sender left out.
/// The identity shortcut answered `Yes` for it, so two endpoints that
/// both carry the invalid identifier matched each other on the absence
/// of a type.
#[test]
fn tk_none_is_not_assignable_from_itself() {
    use zerodds_types::TypeIdentifier;
    let registry = TypeRegistry::new();
    let cfg = AssignabilityConfig::default();
    assert!(
        !is_assignable(
            &TypeIdentifier::None,
            &TypeIdentifier::None,
            &registry,
            &cfg
        )
        .is_yes()
    );
    // And it is not assignable to or from a real type either.
    let int32 = TypeIdentifier::Primitive(zerodds_types::PrimitiveKind::Int32);
    assert!(!is_assignable(&TypeIdentifier::None, &int32, &registry, &cfg).is_yes());
    assert!(!is_assignable(&int32, &TypeIdentifier::None, &registry, &cfg).is_yes());
    // The identity shortcut still works for an identifier that names a
    // type, so the guard above did not disable it.
    assert!(is_assignable(&int32, &int32, &registry, &cfg).is_yes());
}

// ---------------------------------------------------------------------
// §7.3.1.2.1.11 — @ignore_literal_names
// ---------------------------------------------------------------------

/// > If the `@ignore_literal_names` annotation has the value TRUE in
/// > both enumerated types, then the literal names are ignored for the
/// > purpose of checking the assignability of the two Enumerated Types.
/// > Otherwise the literal names are considered.
///
/// The annotation reached the front end and stopped there: `from_ast`
/// wrote `EnumTypeFlag(0)` unconditionally, so the comparison had
/// nothing to read and the two types were refused over names the author
/// had said to ignore. Both halves are here — the flag has to travel
/// with the type, and one side alone must not decide it.
#[test]
fn ignore_literal_names_needs_the_annotation_on_both_types() {
    // Same values, different names, annotated on both sides.
    assert!(assignable(
        "R",
        "W",
        "\
@ignore_literal_names @appendable enum R { A, B };
@ignore_literal_names @appendable enum W { X, Y };"
    ));
    // On one side only, the names still count.
    assert!(!assignable(
        "R",
        "W",
        "\
@ignore_literal_names @appendable enum R { A, B };
@appendable enum W { X, Y };"
    ));
    assert!(!assignable(
        "R",
        "W",
        "\
@appendable enum R { A, B };
@ignore_literal_names @appendable enum W { X, Y };"
    ));
    // On neither side, which is the default.
    assert!(!assignable(
        "R",
        "W",
        "\
@appendable enum R { A, B };
@appendable enum W { X, Y };"
    ));
}
