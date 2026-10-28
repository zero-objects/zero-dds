//! Rules of DDS-XTypes 1.3 §7.2 that name a flag in a `TypeObject` or a
//! result of the is-assignable-from relation.
//!
//! One IDL text per case, lowered by the front end and not built here, so
//! that a rule is tested against the model a user's declaration produces.
//! `T1` is the reader and `T2` the writer, as in the clause;
//! `is_assignable` takes the writer first.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::uninlined_format_args,
    missing_docs
)]

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_types::assignability::{AssignabilityConfig, is_assignable, key_erased};
use zerodds_types::from_ast::{
    Form, FromAstError, complete_type_object, minimal_type_object, register_all, type_identifier,
};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::flags::{StructMemberFlag, UnionDiscriminatorFlag};
use zerodds_types::{CompleteTypeObject, MinimalTypeObject};

/// The model of `idl`; the lowering and resolution must be clean.
fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.is_error())
        .collect();
    assert!(errors.is_empty(), "the IDL does not parse: {errors:?}");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "rules.idl");
    let errors: Vec<_> = lowered
        .diagnostics
        .iter()
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "the IDL does not lower: {errors:?}");
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);
    ast
}

fn id(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type named `{name}`"))
}

/// Is the reader `T1` assignable from the writer `T2`, both in `idl`?
fn assignable(reader: &str, writer: &str, idl: &str) -> bool {
    let ast = model(idl);
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("registration");
    let r = type_identifier(&ast, id(&ast, reader), Form::Minimal).expect("reader identifier");
    let w = type_identifier(&ast, id(&ast, writer), Form::Minimal).expect("writer identifier");
    is_assignable(&w, &r, &registry, &AssignabilityConfig::default()).is_yes()
}

fn minimal_struct(ast: &Ast, name: &str) -> zerodds_types::type_object::minimal::MinimalStructType {
    match minimal_type_object(ast, id(ast, name)).expect("minimal object") {
        MinimalTypeObject::Struct(s) => s,
        other => panic!("`{name}` is no struct: {other:?}"),
    }
}

// -- Table 4: the value of an enumerated literal is an Int32 ---------------

/// Table 4: "a name and an Int32 value". A value past `i32` reaches the
/// builder when the front end's diagnostic is not heeded, and the builder
/// reports `OutOfRange` rather than narrowing it.
#[test]
fn an_enumerator_value_outside_int32_is_no_type_object() {
    let parsed = zerodds_idl_parse::parse(
        "enum Wide { @value(2147483648) A, B };\nenum Fits { @value(2147483647) A };",
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "wide.idl");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.severity == zerodds_idl_parse::Severity::Error),
        "the front end accepted an Int32 value of 2147483648"
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolution");
    for form in [Form::Minimal, Form::Complete] {
        let error = type_identifier(&ast, id(&ast, "Wide"), form).expect_err("no identifier");
        assert!(
            matches!(error, FromAstError::OutOfRange { .. }),
            "{form:?}: {error}"
        );
        assert!(type_identifier(&ast, id(&ast, "Fits"), form).is_ok());
    }
    assert!(minimal_type_object(&ast, id(&ast, "Wide")).is_err());
    assert!(complete_type_object(&ast, id(&ast, "Wide")).is_err());
}

// -- §7.2.2.4.4.4.6: the discriminator is always must understand ------------

#[test]
fn the_discriminator_of_a_union_is_must_understand_in_both_forms() {
    let ast = model(
        "union Plain switch (long) { case 1: long a; default: long b; };\n\
         union Keyed switch (@key long) { case 1: long a; default: long b; };",
    );
    for name in ["Plain", "Keyed"] {
        let keyed = name == "Keyed";
        let MinimalTypeObject::Union(minimal) =
            minimal_type_object(&ast, id(&ast, name)).expect("minimal")
        else {
            panic!("{name} is no union");
        };
        let flags = minimal.discriminator.common.member_flags;
        assert!(
            flags.0 & UnionDiscriminatorFlag::IS_MUST_UNDERSTAND != 0,
            "{name}: the minimal discriminator lacks IS_MUST_UNDERSTAND ({flags:?})"
        );
        assert_eq!(
            flags.0 & UnionDiscriminatorFlag::IS_KEY != 0,
            keyed,
            "{name}"
        );

        let CompleteTypeObject::Union(complete) =
            complete_type_object(&ast, id(&ast, name)).expect("complete")
        else {
            panic!("{name} is no union");
        };
        let flags = complete.discriminator.common.member_flags;
        assert!(
            flags.0 & UnionDiscriminatorFlag::IS_MUST_UNDERSTAND != 0,
            "{name}: the complete discriminator lacks IS_MUST_UNDERSTAND ({flags:?})"
        );
        assert_eq!(
            flags.0 & UnionDiscriminatorFlag::IS_KEY != 0,
            keyed,
            "{name}"
        );
    }
}

// -- §7.2.2.4.4.4.8: a key member is always must understand ------------------

#[test]
fn a_key_member_is_must_understand_in_both_forms() {
    let ast = model("@mutable struct Keyed { @id(1) @key long k; @id(2) long v; };");
    let minimal = minimal_struct(&ast, "Keyed");
    let flags = |index: usize| minimal.member_seq[index].common.member_flags;
    assert!(flags(0).has(StructMemberFlag::IS_KEY));
    assert!(
        flags(0).has(StructMemberFlag::IS_MUST_UNDERSTAND),
        "the key member is not must understand in the minimal object"
    );
    assert!(!flags(1).has(StructMemberFlag::IS_KEY));
    assert!(!flags(1).has(StructMemberFlag::IS_MUST_UNDERSTAND));

    let CompleteTypeObject::Struct(complete) =
        complete_type_object(&ast, id(&ast, "Keyed")).expect("complete")
    else {
        panic!("Keyed is no struct");
    };
    let flags = |index: usize| complete.member_seq[index].common.member_flags;
    assert!(flags(0).has(StructMemberFlag::IS_KEY));
    assert!(
        flags(0).has(StructMemberFlag::IS_MUST_UNDERSTAND),
        "the key member is not must understand in the complete object"
    );
    assert!(!flags(1).has(StructMemberFlag::IS_MUST_UNDERSTAND));
}

// -- §7.2.2.4.6: KeyErased(T) ------------------------------------------------

const AIRCRAFT: &str = "\
struct Coordinates { float x; float y; float z; };
struct Aircraft {
    @key string airline;
    @key int32 flight_number;
    Coordinates current_position;
};
enum CommandKind { START, STOP, GO_LEFT, GO_RIGHT };
union Command switch (@key CommandKind) {
case START: float delay;
case STOP: float distance;
case GO_LEFT:
case GO_RIGHT: float angle;
};";

/// §7.2.2.4.6: "KeyErased(T) [is] obtained from T by removing the key
/// designation from any member that had it. In the case the type T had no
/// members designated key members then KeyErased(T) is the same type T."
/// The clause's own example: `Aircraft_KeyErased` has the members of
/// `Aircraft` without `@key`, `Coordinates` is unchanged, and the union
/// `Command` keeps its members and loses the key on its discriminator.
#[test]
fn key_erased_removes_the_key_designation_and_nothing_else() {
    let ast = model(AIRCRAFT);

    let aircraft = minimal_type_object(&ast, id(&ast, "Aircraft")).expect("aircraft");
    let MinimalTypeObject::Struct(keyed) = &aircraft else {
        panic!("not a struct");
    };
    assert_eq!(
        keyed
            .member_seq
            .iter()
            .filter(|m| m.common.member_flags.has(StructMemberFlag::IS_KEY))
            .count(),
        2
    );
    let MinimalTypeObject::Struct(erased) = key_erased(aircraft.clone()) else {
        panic!("not a struct");
    };
    assert_eq!(erased.member_seq.len(), keyed.member_seq.len());
    for (before, after) in keyed.member_seq.iter().zip(&erased.member_seq) {
        assert!(!after.common.member_flags.has(StructMemberFlag::IS_KEY));
        assert_eq!(before.common.member_id, after.common.member_id);
        assert_eq!(before.common.member_type_id, after.common.member_type_id);
        assert_eq!(before.detail, after.detail);
        // Every other flag stays as it was.
        assert_eq!(
            before.common.member_flags.0 & !StructMemberFlag::IS_KEY,
            after.common.member_flags.0
        );
    }

    let coordinates = minimal_type_object(&ast, id(&ast, "Coordinates")).expect("coordinates");
    assert_eq!(
        key_erased(coordinates.clone()),
        coordinates,
        "a type without key members is its own KeyErased"
    );

    let command = minimal_type_object(&ast, id(&ast, "Command")).expect("command");
    let MinimalTypeObject::Union(keyed) = &command else {
        panic!("not a union");
    };
    assert_ne!(
        keyed.discriminator.common.member_flags.0 & UnionDiscriminatorFlag::IS_KEY,
        0
    );
    let MinimalTypeObject::Union(erased) = key_erased(command.clone()) else {
        panic!("not a union");
    };
    assert_eq!(
        erased.discriminator.common.member_flags.0 & UnionDiscriminatorFlag::IS_KEY,
        0
    );
    assert_eq!(erased.member_seq, keyed.member_seq);
    assert_eq!(
        erased.discriminator.common.type_id,
        keyed.discriminator.common.type_id
    );

    // "The related KeyErased type definition only applies to aggregated
    // types": an enumeration is returned as it is.
    let kind = minimal_type_object(&ast, id(&ast, "CommandKind")).expect("enum");
    assert_eq!(key_erased(kind.clone()), kind);
}

/// Table 19, STRUCTURE row: "KeyErased(m1.type) is-assignable-from
/// KeyErased(m2.type)". Two structures that differ only in the key
/// designation of the structure that is the type of a member are
/// assignable; they would not be if the members' types were compared with
/// their keys.
#[test]
fn the_types_of_members_are_compared_with_their_keys_erased() {
    let idl = "\
@mutable struct InnerKeyed { @id(1) @key long k; @id(2) long v; };
@mutable struct InnerPlain { @id(1) long k; @id(2) long v; };
@mutable struct OuterReader { @id(1) InnerPlain m; };
@mutable struct OuterWriter { @id(1) InnerKeyed m; };";
    assert!(assignable("OuterReader", "OuterWriter", idl));
    assert!(assignable("OuterWriter", "OuterReader", idl));
}

// -- Table 15: primitive types from a bitmask --------------------------------

/// Table 15: a `UINT8` is assignable from a `BITMASK_TYPE` iff the bound is
/// between 1 and 8, `UINT16` for 9 to 16, `UINT32` for 17 to 32 and
/// `UINT64` for 33 to 64.
#[test]
fn an_unsigned_integer_is_assignable_from_a_bitmask_of_its_width_only() {
    let bounds = [
        (1u32, 8u32),
        (8, 8),
        (9, 16),
        (16, 16),
        (12, 16),
        (17, 32),
        (32, 32),
        (33, 64),
        (64, 64),
    ];
    let integers = [
        (8u32, "uint8"),
        (16, "uint16"),
        (32, "uint32"),
        (64, "uint64"),
    ];
    for (bound, width) in bounds {
        for (integer_width, integer) in integers {
            let idl = format!(
                "@bit_bound({bound}) bitmask Mask {{ A }};\n\
                 @mutable struct R {{ @id(1) {integer} v; }};\n\
                 @mutable struct W {{ @id(1) Mask v; }};"
            );
            assert_eq!(
                assignable("R", "W", &idl),
                width == integer_width,
                "{integer} from a bitmask of bound {bound}"
            );
        }
    }
}

// -- §7.2.4.4.5.1: strings --------------------------------------------------

/// §7.2.4.4.5.1: "any string type of narrow characters is assignable from
/// any other string type of narrow characters. Any string type of wide
/// characters is assignable from any other string type of wide characters.
/// However, string types of narrow characters are not assignable from
/// string types of wide characters".
#[test]
fn narrow_and_wide_strings_are_not_assignable_to_each_other() {
    let idl = "\
@mutable struct Narrow { @id(1) string s; };
@mutable struct Narrow5 { @id(1) string<5> s; };
@mutable struct Narrow10 { @id(1) string<10> s; };
@mutable struct Wide { @id(1) wstring s; };
@mutable struct Wide5 { @id(1) wstring<5> s; };";
    assert!(assignable("Narrow", "Narrow5", idl));
    assert!(assignable("Narrow5", "Narrow", idl));
    assert!(assignable("Narrow5", "Narrow10", idl));
    assert!(assignable("Narrow10", "Narrow5", idl));
    assert!(assignable("Wide", "Wide5", idl));
    assert!(assignable("Wide5", "Wide", idl));
    assert!(!assignable("Narrow", "Wide", idl));
    assert!(!assignable("Wide", "Narrow", idl));
    assert!(!assignable("Narrow5", "Wide5", idl));
    assert!(!assignable("Wide5", "Narrow5", idl));
}

// -- Table 19, UNION: the same extensibility --------------------------------

/// Table 19, UNION row: "T1.extensibility == T2.extensibility."
#[test]
fn two_unions_of_different_extensibility_are_not_assignable() {
    let idl = "\
@final union Fa switch (long) { case 1: long a; case 2: short b; };
@final union Fb switch (long) { case 1: long a; case 2: short b; };
@appendable union Aa switch (long) { case 1: long a; case 2: short b; };
@appendable union Ab switch (long) { case 1: long a; case 2: short b; };
@mutable union Ma switch (long) { case 1: long a; case 2: short b; };
@mutable union Mb switch (long) { case 1: long a; case 2: short b; };";
    for (reader, writer) in [("Fa", "Fb"), ("Aa", "Ab"), ("Ma", "Mb")] {
        assert!(assignable(reader, writer, idl), "{reader} from {writer}");
    }
    for (reader, writer) in [
        ("Fa", "Aa"),
        ("Fa", "Ma"),
        ("Aa", "Fa"),
        ("Aa", "Ma"),
        ("Ma", "Fa"),
        ("Ma", "Aa"),
    ] {
        assert!(!assignable(reader, writer, idl), "{reader} from {writer}");
    }
}

// -- Table 19, STRUCTURE: members marked as key appear in both ---------------

/// Table 19, STRUCTURE row: "Members marked as key in either T1 or T2
/// appear (i.e., have a corresponding member of the same member ID) in both
/// T1 and T2."
#[test]
fn a_key_member_that_one_side_lacks_makes_the_structures_not_assignable() {
    let idl = "\
@mutable struct KeyReader { @id(1) @key long k; @id(2) long v; };
@mutable struct KeyWriter { @id(1) @key long k; @id(2) long v; @id(3) long x; };
@mutable struct PlainWriter { @id(1) long k; @id(2) long v; };
@mutable struct NoKeyWriter { @id(2) long v; @id(3) long x; };
@mutable struct NoKeyReader { @id(2) long v; };
@mutable struct KeyWriterOnly { @id(1) @key long k; @id(2) long v; };";
    // The member is present on both sides; it is a key on one only.
    assert!(assignable("KeyReader", "PlainWriter", idl));
    assert!(assignable("PlainWriter", "KeyReader", idl));
    assert!(assignable("KeyReader", "KeyWriter", idl));
    // The key member of the reader has no counterpart in the writer.
    assert!(!assignable("KeyReader", "NoKeyWriter", idl));
    // The key member of the writer has no counterpart in the reader.
    assert!(!assignable("NoKeyReader", "KeyWriterOnly", idl));
    // Without the key marking the same pair is assignable.
    assert!(assignable("NoKeyReader", "PlainWriter", idl));
}

// -- §7.2.4.4.8.1 to §7.2.4.4.8.3: the clause's examples --------------------

/// §7.2.4.4.8.1: "Coordinate2D is assignable from Coordinate3D".
#[test]
fn coordinate2d_is_assignable_from_coordinate3d() {
    let idl = "\
struct Coordinate2D { long x; long y; };
struct Coordinate3D { long x; long y; long z; };";
    assert!(assignable("Coordinate2D", "Coordinate3D", idl));
}

/// §7.2.4.4.8.2: "LandVehicle is assignable from Vehicle. Any consumer of
/// the latter that receives an instance of the former will observe the
/// value of the member km_per_hour and ignore the member num_wheels." The
/// two sentences give the direction both ways round, and a structure
/// compares by the members its base type adds to it.
#[test]
fn a_structure_and_its_derived_structure_are_assignable() {
    let idl = "\
struct Vehicle { long km_per_hour; };
struct LandVehicle : Vehicle { long num_wheels; };";
    assert!(assignable("Vehicle", "LandVehicle", idl));
    assert!(assignable("LandVehicle", "Vehicle", idl));
}

/// §7.2.4.4.8.3: "Because the is-assignable-from relationship is evaluated
/// as if all member definitions were flattened into the types under
/// evaluation, both versions of the Giraffe type are assignable to one
/// another."
#[test]
fn both_versions_of_giraffe_are_assignable_to_one_another() {
    let idl = "\
struct Animal1 { long body_length; long num_legs; };
struct Giraffe1 : Animal1 { long neck_length; };
struct Animal2 { long body_length; };
struct Mammal2 : Animal2 { long num_legs; };
struct Giraffe2 : Mammal2 { long neck_length; };
struct Snake2 : Animal2 { long num_scales; };";
    assert!(assignable("Giraffe1", "Giraffe2", idl));
    assert!(assignable("Giraffe2", "Giraffe1", idl));
    // Not an accident of the flattening: a snake has no legs, and a giraffe
    // is not a snake.
    assert!(!assignable("Giraffe2", "Snake2", idl));
}
