// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Ada, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go,
//! and of `c_end_to_end.rs`, which does it for C.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//!    A case that stopped rendering would otherwise be found by whichever
//!    probe ran next, which is a build directory away from the construct
//!    that caused it.
//! 2. **The generated Ada compiles**, under GNAT. A snapshot says the
//!    emitter produced the text it produced last time; it does not say
//!    the text is Ada, that every name in it resolves, or that a
//!    component's subtype exists. GNAT is skipped where it is not on the
//!    path and the skip says so on standard error — a test that went
//!    green without running the compiler would be the failure it is
//!    meant to catch.
//!
//! GNAT derives the compilation unit name from the file name, so the
//! spec is always written to `zd_types.ads` and never anywhere else:
//! a package `Zd_Types` in `sample.ads` fails before a single
//! declaration is read, for a reason that has nothing to do with the
//! generated code.
// `print_stderr` is allowed here for one reason: a compiler that is not
// on the path has to say so on a channel a passing test does not use.
// A green test that never ran the compiler is the failure these gates
// exist to catch — and on this project's development machines GNAT is
// exactly what is missing.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;
#[path = "support/resolved.rs"]
mod resolved;

use std::process::Command;
use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::ada::{FILE_NAME, ada_file};
use zerodds_lang_rules::rule::ada;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Ada out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    // The same step the C, C++, Go and TypeScript columns take: a member
    // whose type is written as a scoped name arrives as an unresolved
    // reference, and a materialiser that asked `resolved()` without this
    // would report "the member type does not evaluate" for every
    // cross-module reference in the corpus.
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &ada::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = ada_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::ada::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_an_ada_record() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("type Pose is record"), "{generated}");
    assert!(
        generated.contains("X : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Y : Interfaces.IEEE_Float_64;"),
        "{generated}"
    );
    assert!(generated.contains("Active : Boolean;"), "{generated}");
    assert!(generated.contains("end record;"), "{generated}");
}

#[test]
fn every_idl_primitive_reaches_an_interfaces_type() {
    // The mapping is total over `PrimitiveKind`; this shows it produces
    // the type each one should, rather than merely producing something.
    let generated = generate(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n;
         };",
    );
    for expected in [
        "A : Boolean;",
        "B : Interfaces.Unsigned_8;",
        "C : Interfaces.Unsigned_8;",
        "D : Interfaces.Unsigned_16;",
        "E : Interfaces.Integer_8;",
        "F : Interfaces.Unsigned_8;",
        "G : Interfaces.Integer_16;",
        "H : Interfaces.Unsigned_16;",
        "I : Interfaces.Integer_32;",
        "J : Interfaces.Unsigned_32;",
        "K : Interfaces.Integer_64;",
        "L : Interfaces.Unsigned_64;",
        "M : Interfaces.IEEE_Float_32;",
        "N : Interfaces.IEEE_Float_64;",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
    // RM 10.1.2: the dependency is the compilation unit's, so the clause
    // is above the package and not inside it.
    assert!(generated.contains("with Interfaces;"), "{generated}");
}

#[test]
fn no_generated_type_has_an_implementation_defined_width() {
    // RM 3.5.4 fixes only a lower bound on `Integer`, and RM 3.5.7
    // leaves the format of `Float` to the implementation. A member
    // declared with either would change width between two compilers, for
    // a wire format that fixes it.
    let generated = generate(
        "struct P { long a; unsigned long b; long long c; float d; double e;
                    sequence<long> f; long g[2]; };",
    );
    for forbidden in [
        ": Integer;",
        ": Float;",
        ": Long_Float;",
        ": Long_Long_Float;",
        "of Integer;",
        "of Float;",
    ] {
        assert!(
            !generated.contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn a_long_double_is_text_and_never_an_ada_float() {
    // Ada names three predefined floating types and fixes the format of
    // none. `Long_Long_Float` is 80-bit x87 extended on x86-64 GNAT and
    // 64-bit where there is no wider hardware type, so one generated
    // spec would lose bits on some targets and not on others.
    let generated = generate("struct S { long double big; };");
    assert!(
        generated.contains("Big : Ada.Strings.Unbounded.Unbounded_String;"),
        "{generated}"
    );
    assert!(
        !generated.contains("Long_Long_Float"),
        "an implementation-defined float reached the output\n{generated}"
    );
}

#[test]
fn a_string_is_an_unbounded_string_and_never_an_ada_string() {
    // Ada's `String` is `array (Positive range <>) of Character`
    // (RM A.1) — an unconstrained array, and RM 3.8 requires a
    // component's subtype to be definite, so `Name : String;` is not a
    // legal component at all.
    let generated = generate("struct Bag { string name; wstring wide; };");
    assert!(
        generated.contains("Name : Ada.Strings.Unbounded.Unbounded_String;"),
        "{generated}"
    );
    assert!(
        generated.contains("Wide : Ada.Strings.Unbounded.Unbounded_String;"),
        "{generated}"
    );
    assert!(
        generated.contains("with Ada.Strings.Unbounded;"),
        "{generated}"
    );
    assert!(!generated.contains(": String;"), "{generated}");
}

#[test]
fn an_optional_member_becomes_a_discriminated_record() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. The absent state has no component
    // at all, so reading the value when `Present` is False raises
    // `Constraint_Error` (RM 3.8.1) rather than returning whatever was
    // there — which is what the C column's flag beside the value does.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(
        generated.contains("type Zd_Opt_Octet (Present : Boolean := False) is record"),
        "{generated}"
    );
    assert!(generated.contains("case Present is"), "{generated}");
    assert!(generated.contains("when True =>"), "{generated}");
    assert!(
        generated.contains("Value : Interfaces.Unsigned_8;"),
        "{generated}"
    );
    assert!(generated.contains("when False =>"), "{generated}");
    assert!(generated.contains("B : Zd_Opt_Octet;"), "{generated}");
    assert!(
        generated.contains("C : Interfaces.Integer_32;"),
        "{generated}"
    );
}

#[test]
fn a_member_named_after_a_reserved_word_escapes_with_a_letter() {
    // RM 2.3(4/3) forbids an identifier that ends in an underscore, so
    // the `type_` every other column writes is not an Ada identifier.
    // The escape is a letter suffix instead, and the IDL spelling is not
    // lost: it reads back from the Ada name minus the suffix.
    let generated = generate("struct S { long type; long range; long access; };");
    assert!(
        generated.contains("Type_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Range_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Access_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
    for spelled in ["Type_ ", "Range_ ", "Access_ "] {
        assert!(
            !generated.contains(spelled),
            "a trailing underscore reached the output\n{generated}"
        );
    }
}

#[test]
fn the_reserved_word_check_folds_case_because_ada_does() {
    // RM 2.9 lists the reserved words in lower case and RM 2.3(5/2)
    // folds before comparing, so `TYPE`, `Type` and `type` are one
    // reserved word and all three have to move. A case-sensitive check
    // would let `TYPE` through and produce a spec that does not parse.
    let generated = generate("struct S { long TYPE; long Record; };");
    assert!(
        generated.contains("TYPE_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Record_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
}

#[test]
fn a_member_named_after_a_unit_the_spec_relies_on_is_escaped_too() {
    // Ada permits the shadowing and the spec would compile until the
    // next line named `Interfaces.Integer_32`, which would then resolve
    // to the new declaration. Every member type in a generated spec is
    // reached through one of these names.
    let generated = generate("struct S { long Interfaces; long System; long Natural; };");
    assert!(
        generated.contains("Interfaces_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("System_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Natural_Idl : Interfaces.Integer_32;"),
        "{generated}"
    );
}

#[test]
fn two_members_that_fold_onto_one_ada_identifier_are_refused_by_name() {
    // The case-insensitive check, and the only one that catches this.
    // IDL 4.2 §7.2.3.1 already forbids `value` beside `Value` in one
    // scope and the frontend reports that. What is left is the pair the
    // escape makes across a case difference: `type` escapes to
    // `Type_Idl`, and a member already spelled `TYPE_IDL` is the same
    // Ada identifier (RM 2.3(5/2)). A column comparing written
    // spellings would emit both and GNAT would answer `"TYPE_IDL"
    // conflicts with declaration at line …`, naming an Ada identifier
    // and neither of the two IDL members behind it.
    let error = try_generate("struct S { long type; long TYPE_IDL; };", "collide.idl")
        .expect_err("the collision was not reported");
    assert!(error.contains("type"), "{error}");
    assert!(error.contains("TYPE_IDL"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn two_declarations_that_flatten_onto_one_name_are_refused_by_name() {
    // Two IDL scopes can reach one Ada identifier: `a::b_C` and
    // `a_b::C` both flatten to `A_B_C`. GNAT would report one
    // identifier and neither declaration.
    let error = try_generate(
        "module a { module b_c { struct D { long x; }; }; };
         module a_b { module c { struct D { long y; }; }; };",
        "flatten.idl",
    )
    .expect_err("the collision was not reported");
    assert!(error.contains("A_B_C_D"), "{error}");
}

#[test]
fn a_sequence_becomes_an_access_to_an_unconstrained_array() {
    // Ada has no built-in dynamic array. `Natural range <>` leaves the
    // length out of the type, which is where the wire form keeps it; a
    // component's subtype must be definite (RM 3.8), so the member holds
    // the access type.
    let generated = generate("struct Bag { sequence<long> ids; };");
    assert!(
        generated.contains(
            "type Zd_Seq_Long_Array is array (Natural range <>) of Interfaces.Integer_32;"
        ),
        "{generated}"
    );
    assert!(
        generated.contains("type Zd_Seq_Long is access Zd_Seq_Long_Array;"),
        "{generated}"
    );
    assert!(generated.contains("Ids : Zd_Seq_Long;"), "{generated}");
    // Never a generic instantiation: `Ada.Containers.Vectors` would need
    // one package per element type and drags the containers hierarchy
    // into every generated spec.
    assert!(
        !generated.contains("Ada.Containers"),
        "a container instantiation reached the output\n{generated}"
    );
}

#[test]
fn an_array_declares_one_type_per_dimension_outermost_first() {
    // `long grid[2][3]` is two rows of three. Ada has no anonymous array
    // type in a component position, so each dimension is a declaration
    // of its own; reversing them silently transposes the data.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("type Zd_Arr_Long_3 is array (0 .. 2) of Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("type Zd_Arr_Long_2_3 is array (0 .. 1) of Zd_Arr_Long_3;"),
        "{generated}"
    );
    assert!(generated.contains("Grid : Zd_Arr_Long_2_3;"), "{generated}");
    // The inner type comes first: Ada resolves nothing forward (RM 3.11).
    let inner = generated
        .find("type Zd_Arr_Long_3 ")
        .expect("the inner type");
    let outer = generated
        .find("type Zd_Arr_Long_2_3 ")
        .expect("the outer type");
    assert!(inner < outer, "the outer type was declared first");
}

#[test]
fn a_map_becomes_a_sequence_of_entries_and_never_a_hashed_container() {
    // DDS-XTypes 1.3 §7.4.1.1.4 defines a map on the wire as a sequence
    // of key/value pairs, which is ordered. `Ada.Containers.Hashed_Maps`
    // iterates in an order the implementation chooses, so a column
    // holding a map in one would produce a different octet stream for
    // one value.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("type Zd_Map_Entry_Long_String is record"),
        "{generated}"
    );
    assert!(
        generated.contains("Key : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Value : Ada.Strings.Unbounded.Unbounded_String;"),
        "{generated}"
    );
    assert!(
        generated.contains(
            "type Zd_Map_Long_String_Array is array (Natural range <>) of \
             Zd_Map_Entry_Long_String;"
        ),
        "{generated}"
    );
    assert!(
        generated.contains("Table : Zd_Map_Long_String;"),
        "{generated}"
    );
    assert!(
        !generated.contains("Hashed_Maps") && !generated.contains("Ordered_Maps"),
        "a container reached the output\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_subtype_and_not_a_derived_type() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type". RM
    // 3.2.2 makes a subtype another name for one type; RM 3.4 makes a
    // derived type a new one, and a value of it would need an explicit
    // conversion to reach an `Interfaces.Integer_32`.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(
        generated.contains("subtype Distance is Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        !generated.contains("type Distance is new"),
        "a derived type reached the output\n{generated}"
    );
    assert!(generated.contains("D : Distance;"), "{generated}");
}

#[test]
fn an_enum_carries_its_numbering_in_a_representation_clause() {
    // The single most important thing this column does. RM 3.5.1
    // numbers the literals 0, 1, 2 in declaration order, so without the
    // clause of RM 13.4 an IDL `@value(4)` arrives as 1 and every value
    // written from it is off the wire numbering with nothing in the file
    // to say it happened.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(
        generated.contains("type Mode is (IDLE, ACTIVE, FAULT);"),
        "{generated}"
    );
    assert!(
        generated.contains("for Mode use (IDLE => 0, ACTIVE => 4, FAULT => 5);"),
        "{generated}"
    );
    // The clause has to follow the type it applies to.
    let type_ = generated.find("type Mode is").expect("the type");
    let clause = generated.find("for Mode use").expect("the clause");
    assert!(type_ < clause, "the clause came before the type");
}

#[test]
fn an_enum_whose_values_do_not_ascend_is_refused_by_name() {
    // RM 13.4 requires the codes of a representation clause to satisfy
    // the type's own ordering. IDL puts no such constraint on `@value`.
    // Emitting the clause anyway produces a spec GNAT rejects; dropping
    // it silently renumbers every value on the wire, which is the
    // failure this refusal exists to prevent.
    let error = try_generate("enum Down { @value(5) A, @value(1) B };", "descend.idl")
        .expect_err("the descending enumeration was not reported");
    assert!(error.contains("13.4"), "{error}");
    assert!(error.contains("do not ascend"), "{error}");
}

#[test]
fn a_union_becomes_a_discriminated_record_with_a_variant_part() {
    // RM 3.8.1. The branches are inside the type and `D` is part of the
    // value, so reading a branch the discriminant does not select raises
    // `Constraint_Error` rather than returning whatever was there. Only
    // the selected branch is written (DDS-XTypes 1.3 §7.4.1.1.5.2).
    let generated = generate(
        "enum Mode { IDLE, ACTIVE, FAULT };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(
        generated.contains("type Payload (D : Mode := IDLE) is record"),
        "{generated}"
    );
    assert!(generated.contains("case D is"), "{generated}");
    assert!(generated.contains("when IDLE =>"), "{generated}");
    assert!(
        generated.contains("Idle : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(generated.contains("when ACTIVE =>"), "{generated}");
    assert!(
        generated.contains("Active : Interfaces.IEEE_Float_64;"),
        "{generated}"
    );
    // `FAULT` selects nothing, so the arm that covers it carries no
    // component. That is the honest answer to a discriminator taking a
    // value no branch selects.
    assert!(generated.contains("when others =>"), "{generated}");
    assert!(generated.contains("null;"), "{generated}");
    assert!(generated.contains("end case;"), "{generated}");
}

#[test]
fn a_union_whose_branches_cover_the_discriminant_writes_no_others_arm() {
    // RM 3.8.1 makes `others` cover the values the earlier choices did
    // not. Where they covered every literal of the enumeration there is
    // nothing left for it, so the arm is not written.
    let generated = generate(
        "enum Two { A, B };
         union U switch (Two) { case A: long a; case B: long b; };",
    );
    assert!(
        generated.contains("type U (D : Two := A) is record"),
        "{generated}"
    );
    assert!(
        !generated.contains("when others =>"),
        "an empty others arm reached the output\n{generated}"
    );
}

#[test]
fn a_union_default_branch_becomes_the_others_arm() {
    // An IDL `default:` is exactly Ada's `others`, which RM 3.8.1
    // requires to be alone and last.
    let generated = generate("union U switch (long) { case 1: long a; default: double b; };");
    assert!(
        generated.contains("type U (D : Interfaces.Integer_32 := 0) is record"),
        "{generated}"
    );
    assert!(generated.contains("when 1 =>"), "{generated}");
    assert!(generated.contains("when others =>"), "{generated}");
    assert!(
        generated.contains("B : Interfaces.IEEE_Float_64;"),
        "{generated}"
    );
}

#[test]
fn a_negative_case_label_brings_the_operator_of_its_type_into_view() {
    // Measured on GNAT 14.2.0 and not reasoned: eleven corpus cases
    // failed with `operator for type "Interfaces.Integer_16" is not
    // directly visible`. A predefined operator of a type declared in
    // another package is not directly visible where the type is only
    // reached as a qualified name (RM 8.4), and RM 4.4 makes the `-` of
    // `-13` an operator of the expected type rather than part of the
    // literal — so a union with a negative case label applies one.
    let generated = generate("union U switch (short) { case -13: long a; };");
    assert!(generated.contains("with Interfaces;"), "{generated}");
    assert!(
        generated.contains("use type Interfaces.Integer_16;"),
        "{generated}"
    );
    assert!(generated.contains("when -13 =>"), "{generated}");
    // `use type` and never `use Interfaces;`: the second would make
    // `Integer_32`, `Unsigned_8` and the rest directly visible too, and
    // a generated declaration of one of those names would be a homograph
    // of a predefined one.
    assert!(
        !generated.contains("use Interfaces;"),
        "a whole-package use clause reached the output\n{generated}"
    );
}

#[test]
fn a_negative_constant_brings_the_operator_of_its_type_into_view() {
    // The same clause, in the other position this column writes a
    // negative literal in.
    let generated = generate("const long N = -5;");
    assert!(
        generated.contains("use type Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("N : constant Interfaces.Integer_32 := -5;"),
        "{generated}"
    );
}

#[test]
fn a_spec_that_applies_no_operator_gets_no_use_clause() {
    // A `use type` for a type nothing applies an operator to is a clause
    // a reader has to account for and cannot.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union U switch (Mode) { case IDLE: long a; };
         struct S { long x; double y; };",
    );
    assert!(
        !generated.contains("use type"),
        "an unneeded use clause reached the output\n{generated}"
    );
}

#[test]
fn an_enumeration_representation_clause_needs_no_use_clause() {
    // Its codes are of `universal_integer` (RM 13.4), whose operators
    // come from `Standard` and are always directly visible. GNAT 14.2.0
    // accepted `for M_E use (LOW => -5, …)` with no clause, which is why
    // the walk that collects them leaves representation clauses out.
    let generated = generate("enum E { @value(-5) LOW, @value(0) MID, @value(1000) HIGH };");
    assert!(
        generated.contains("for E use (LOW => -5, MID => 0, HIGH => 1000);"),
        "{generated}"
    );
    assert!(!generated.contains("use type"), "{generated}");
}

#[test]
fn an_identifier_that_begins_with_an_underscore_takes_a_letter_in_front() {
    // IDL 4.2 §7.2.3.2: the escape underscore "ONLY turns off keyword
    // checking" and is not part of the identifier, so the front end
    // hands this backend `long`, `double`, `switch` — no leading
    // underscore survives to collide with RM 2.3(2) (GNAT 14.2.0:
    // `identifier cannot start with underline`). Ada identifiers are
    // case-insensitive (RM 2.3(5/2)), so each declaration reads back
    // as the IDL name unchanged, and a member's ID under @autoid(HASH)
    // is the MD5 of that bare name (DDS-XTypes 1.3 §7.3.1.2.1.1).
    let generated =
        generate("struct Sample { long _long; double _double; short _switch; long plain; };");
    assert!(
        generated.contains("Long : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Double : Interfaces.IEEE_Float_64;"),
        "{generated}"
    );
    assert!(
        generated.contains("Switch : Interfaces.Integer_16;"),
        "{generated}"
    );
    assert!(
        generated.contains("Plain : Interfaces.Integer_32;"),
        "{generated}"
    );
    // The Ada spelling matches the IDL identifier case-insensitively,
    // so no `IDL:` comment is needed for these members.
    assert!(!generated.contains("IDL: `_long`."), "{generated}");
    for line in generated.lines() {
        assert!(
            !line.trim_start().starts_with('_'),
            "a leading underscore reached the output: {line}"
        );
    }
}

#[test]
fn no_generated_identifier_breaks_a_lexical_rule_of_the_language() {
    // RM 2.3 in one place: an identifier begins with a letter, does not
    // end in an underscore, and holds no two in a row. All three are
    // repaired by one marker, so a reader meeting `Idl` in a generated
    // name knows the same thing wherever it stands.
    let generated = generate(
        "module m {
           struct Sample { long _long; long type; long a_b; };
           enum E { _switch, PLAIN };
           @bit_bound(8) bitmask F { _range };
           const long _limit = 1;
           exception _fail { long code; };
           interface _iface { void _go(in long _arg); };
         };",
    );
    for line in generated.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") || trimmed.is_empty() {
            continue;
        }
        for word in trimmed.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')) {
            if word.is_empty() {
                continue;
            }
            assert!(
                !word.starts_with('_'),
                "{word:?} starts with an underscore\n{generated}"
            );
            assert!(
                !word.ends_with('_'),
                "{word:?} ends with an underscore\n{generated}"
            );
            assert!(
                !word.contains("__"),
                "{word:?} holds two underscores in a row\n{generated}"
            );
        }
    }
}

#[test]
fn a_bitmask_becomes_a_derived_modular_type_and_one_constant_per_bit() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(
        generated.contains("type Flags is new Interfaces.Unsigned_16;"),
        "{generated}"
    );
    assert!(
        generated.contains("READY : constant Flags := 1;"),
        "{generated}"
    );
    assert!(
        generated.contains("MOVING : constant Flags := 16;"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_record_of_holder_typed_components() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("type B is record"), "{generated}");
    assert!(
        generated.contains("Low : Interfaces.Unsigned_8;"),
        "{generated}"
    );
    assert!(
        generated.contains("High : Interfaces.Unsigned_16;"),
        "{generated}"
    );
}

#[test]
fn a_valuetype_becomes_a_record_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("type V is record"), "{generated}");
    assert!(
        generated.contains("Counter : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Secret : Ada.Strings.Unbounded.Unbounded_String;"),
        "{generated}"
    );
}

#[test]
fn an_aggregate_with_no_members_is_a_null_record() {
    // RM 3.8 gives `null record` as its own form: a `component_list` may
    // not be empty, so `record end record` is not Ada. Unlike the C
    // column, which has to refuse the case — C23 §6.7.2.1p8 leaves a
    // structure with no named member undefined — Ada has a spelling for
    // it and the corpus contains it.
    let generated = generate("struct Empty { };");
    assert!(
        generated.contains("type Empty is null record;"),
        "{generated}"
    );
}

#[test]
fn a_constant_becomes_a_constant_object_of_its_declared_type() {
    let generated = generate("const octet LIMIT = 8;");
    assert!(
        generated.contains("LIMIT : constant Interfaces.Unsigned_8 := 8;"),
        "{generated}"
    );
}

#[test]
fn a_string_constant_is_a_call_because_unbounded_string_is_private() {
    // `Unbounded_String` is a private type (RM A.4.5) and Ada has no
    // literal for one, so the value is a function call evaluated when
    // the package is elaborated.
    let generated = generate("const string TAG = \"abc\";");
    assert!(
        generated.contains(
            "TAG : constant Ada.Strings.Unbounded.Unbounded_String := \
             Ada.Strings.Unbounded.To_Unbounded_String (\"abc\");"
        ),
        "{generated}"
    );
}

#[test]
fn a_floating_constant_carries_a_point_because_ada_needs_one() {
    // RM 2.4.1: a numeric literal with no point is an *integer* literal,
    // exponent or not, and an integer literal is not a value of a
    // floating-point type. `X : constant IEEE_Float_64 := 1e5;` does not
    // compile; `1.0e5` does, and is the same number.
    let generated = generate("const double SCALE = 1e5; const float HALF = 0.5;");
    assert!(
        generated.contains("SCALE : constant Interfaces.IEEE_Float_64 := 1.0e5;"),
        "{generated}"
    );
    assert!(
        generated.contains("HALF : constant Interfaces.IEEE_Float_32 := 0.5;"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_record_of_its_members() {
    // Not an Ada `exception`: RM 11.1 gives one no components at all,
    // and only a message string travels with it.
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("type Failure is record"), "{generated}");
    assert!(
        generated.contains("Code : Interfaces.Integer_32;"),
        "{generated}"
    );
    assert!(
        generated.contains("Reason : Ada.Strings.Unbounded.Unbounded_String;"),
        "{generated}"
    );
}

#[test]
fn an_interface_becomes_an_ada_interface_type_with_abstract_operations() {
    // Ada 2005 has real interface types (RM 3.9.4): a type with no
    // components whose primitive operations are abstract, which is what
    // an IDL interface declares.
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(
        generated.contains("type Control is interface;"),
        "{generated}"
    );
    assert!(
        generated.contains("procedure Go (Self : in Control; Target : in Pose) is abstract;"),
        "{generated}"
    );
    assert!(
        generated.contains(
            "function Status (Self : in Control) return Interfaces.Integer_32 is abstract;"
        ),
        "{generated}"
    );
    // The type comes before its primitive operations (RM 3.11).
    let type_ = generated.find("type Control is").expect("the type");
    let operation = generated.find("procedure Go").expect("the operation");
    assert!(type_ < operation, "an operation came before the type");
}

#[test]
fn an_operation_keeps_the_idl_parameter_direction() {
    // The one mapping in this crate that does. RM 6.1 has all three
    // modes, and Ada 2012 admits `out` and `in out` on a function as
    // well as on a procedure; C, C++, Go and Rust all have to spell
    // `out` as a pointer or a reference and lose the direction.
    let generated = generate("interface I { void mix(in long a, inout long b, out double c); };");
    assert!(
        generated.contains("A : in Interfaces.Integer_32"),
        "{generated}"
    );
    assert!(
        generated.contains("B : in out Interfaces.Integer_32"),
        "{generated}"
    );
    assert!(
        generated.contains("C : out Interfaces.IEEE_Float_64"),
        "{generated}"
    );
}

#[test]
fn a_native_declaration_becomes_a_derived_address_type() {
    let generated = generate("native Handle;");
    assert!(
        generated.contains("type Handle is new System.Address;"),
        "{generated}"
    );
    assert!(generated.contains("with System;"), "{generated}");
}

#[test]
fn a_module_flattens_onto_the_name_because_a_declarative_part_is_contiguous() {
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("type Common_Util_Location is record"),
        "{generated}"
    );
    assert!(
        generated.contains("IDL: `Location`."),
        "the IDL name was not recorded\n{generated}"
    );
}

#[test]
fn a_recursive_type_is_cut_with_an_incomplete_type_declaration() {
    // RM 3.10.1. `Node` cannot be declared before it is named, so the
    // name is declared first and the type follows. The array of `Node`
    // is split the same way, because an incomplete type may not be an
    // array element — only the designated subtype of an access type,
    // which is what `Zd_Seq_Node` is.
    let generated = generate("struct Node { sequence<Node> children; };");
    assert!(generated.contains("type Node;"), "{generated}");
    assert!(generated.contains("type Zd_Seq_Node_Array;"), "{generated}");
    assert!(
        generated.contains("type Zd_Seq_Node is access Zd_Seq_Node_Array;"),
        "{generated}"
    );
    assert!(generated.contains("type Node is record"), "{generated}");
    assert!(
        generated.contains("type Zd_Seq_Node_Array is array (Natural range <>) of Node;"),
        "{generated}"
    );
    // The order is the one RM 3.10.1 requires: the name, then the access
    // type that may designate it, then the completion, then the array
    // whose element the completion made definite.
    let name = generated.find("type Node;").expect("the incomplete type");
    let access = generated
        .find("is access Zd_Seq_Node_Array;")
        .expect("the access");
    let body = generated
        .find("type Node is record")
        .expect("the completion");
    let elements = generated
        .find("type Zd_Seq_Node_Array is array")
        .expect("the array");
    assert!(name < access, "{generated}");
    assert!(access < body, "{generated}");
    assert!(body < elements, "{generated}");
}

#[test]
fn mutual_recursion_needs_only_one_incomplete_declaration() {
    // Cutting one type of the cycle is enough: the other is reached
    // through the sequence the cut made writable.
    let generated = generate(
        "struct B; struct A { long value; sequence<B> bs; };
         struct B { long weight; sequence<A> as; };",
    );
    // Which of the two the walk cuts at is the model's arena order and
    // not this column's; exactly one of them is declared incomplete.
    let cut_a = generated.contains("type A;\n");
    let cut_b = generated.contains("type B;\n");
    assert!(cut_a != cut_b, "expected exactly one cut\n{generated}");
    assert!(generated.contains("type A is record"), "{generated}");
    assert!(generated.contains("type B is record"), "{generated}");
    // Whichever was cut, its sequence's element type is completed after
    // the record and the access type is not.
    let cut = if cut_a { "A" } else { "B" };
    let name = generated
        .find(&format!("type {cut};\n"))
        .expect("the incomplete type");
    let body = generated
        .find(&format!("type {cut} is record"))
        .expect("the completion");
    let elements = generated
        .find(&format!(
            "type Zd_Seq_{cut}_Array is array (Natural range <>) of {cut};"
        ))
        .expect("the array");
    assert!(name < body, "{generated}");
    assert!(body < elements, "{generated}");
}

#[test]
fn a_recursion_through_a_map_splits_the_entry_record_too() {
    // The entry record has a component of the value type, and a record
    // component may not be incomplete (RM 3.10.1). Both the entry and
    // the array of entries are declared incomplete; the access type the
    // member holds is complete straight away.
    let generated = generate("struct Node { long value; map<long, Node> children; };");
    assert!(generated.contains("type Node;"), "{generated}");
    assert!(
        generated.contains("type Zd_Map_Entry_Long_Node;"),
        "{generated}"
    );
    assert!(
        generated.contains("type Zd_Map_Long_Node is access Zd_Map_Long_Node_Array;"),
        "{generated}"
    );
    assert!(
        generated.contains("type Zd_Map_Entry_Long_Node is record"),
        "{generated}"
    );
}

#[test]
fn a_recursive_optional_member_becomes_an_access_type() {
    // The cycle passes through no collection: what terminates it is the
    // member being absent (DDS-XTypes 1.3 §7.3.1.2.1.3), and `@external`
    // says it is held by reference (Table 21). An Ada record cannot
    // contain itself by value at all, and RM 3.10.1 admits an incomplete
    // type only as the designated subtype of an access type — so the
    // access type is both what the annotations ask for and the only
    // spelling available.
    let generated =
        generate("struct Node; struct Node { long value; @optional @external Node next; };");
    assert!(generated.contains("type Node;"), "{generated}");
    assert!(
        generated.contains("type Zd_Ref_Node is access Node;"),
        "{generated}"
    );
    assert!(generated.contains("Next : Zd_Ref_Node;"), "{generated}");
    // Not the `Present` wrapper: that record holds the value inline and
    // would be as unbounded as the record it sits in.
    assert!(!generated.contains("Zd_Opt_Node"), "{generated}");
}

#[test]
fn a_recursion_with_no_collection_and_no_reference_is_refused_by_name() {
    // A record whose component is itself has no bound, and there is no
    // annotation here to make the component a reference. Refused by
    // name rather than emitted in whatever order the walk reached, which
    // would fail with `"Node" is undefined` against a line the generator
    // chose.
    let error = try_generate(
        "struct Node; struct Node { long value; Node next; };",
        "unbounded.idl",
    )
    .expect_err("the unbounded recursion was not reported");
    assert!(error.contains("Node"), "{error}");
    assert!(error.contains("no bound"), "{error}");
}

#[test]
fn a_type_is_declared_before_it_is_named() {
    // RM 3.11: a declaration is visible from its own end onwards, so
    // unlike the Go column the output order is not the model's.
    let generated = generate("struct Outer { Inner i; }; struct Inner { long x; };");
    let inner = generated.find("type Inner is").expect("Inner");
    let outer = generated.find("type Outer is").expect("Outer");
    assert!(inner < outer, "the output named Inner before declaring it");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Ada equivalent. What is left is
    // the package and the file comment.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(generated.contains("package Zd_Types is"), "{generated}");
    assert!(generated.contains("end Zd_Types;"), "{generated}");
    assert!(!generated.contains("type "), "{generated}");
}

#[test]
fn every_generated_character_is_one_gnat_reads_the_same_way() {
    // GNAT's default source representation is Latin-1, not UTF-8: a
    // multi-byte character in a comment or a string literal would be
    // read byte by byte, and two of the three bytes of an em dash fall
    // in the range Latin-1 leaves unassigned. Every generated spec is
    // therefore printable ASCII, which every source representation GNAT
    // accepts reads identically.
    let generated = generate(
        "module m { enum Mode { IDLE, @value(4) ACTIVE };
           union U switch (Mode) { case IDLE: long a; };
           struct S { @optional string text; sequence<long> q; long g[2]; long type; };
           @bit_bound(8) bitmask F { BIT };
           exception E { long code; };
           interface I { void go(in S arg); };
           native H; const string TAG = \"abc\"; };",
    );
    for (index, ch) in generated.char_indices() {
        assert!(
            ch == '\n' || (' '..='~').contains(&ch),
            "byte {index} is {ch:?}, which GNAT reads as Latin-1\n{generated}"
        );
    }
}

/// The codec generator, for the shapes whose defect is only visible in
/// the routines and not in the declarations.
fn codec(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    zerodds_lang_rules::generate::source(
        &ast,
        zerodds_lang_cst::Language::Ada,
        zerodds_ast_layout::LayoutKey::data(
            zerodds_ast_layout::Encoding::PlainCdr2,
            zerodds_ast_layout::Endianness::Little,
        ),
    )
    .expect("a codec")
}

/// A bound belongs to the routine the collection is hoisted into.
///
/// The unbounded member is declared first on purpose: the hoisted
/// routine is keyed by `type_tag`, so with the bound left out of it the
/// bounded member was decoded by the unbounded member's routine and its
/// length check disappeared entirely.
#[test]
fn a_sequence_bound_survives_an_unbounded_sequence_of_the_same_element() {
    let generated = codec("struct S { sequence<long> open; sequence<long, 4> capped; };");
    assert!(
        generated.contains("zd_n > 4"),
        "the bound of `sequence<long, 4>` is nowhere in the codec\n{generated}"
    );
    assert!(
        generated.contains("procedure zd_enc_Seq_Of_Long ")
            && generated.contains("procedure zd_enc_Seq_Of_4_Long "),
        "{generated}"
    );
}

/// The same for a `string`, whose bound is carried the same way.
#[test]
fn a_string_bound_survives_an_unbounded_string_beside_it() {
    let generated = codec("struct S { string open; string<4> capped; };");
    assert!(
        generated.contains("procedure zd_enc_String ")
            && generated.contains("procedure zd_enc_String_4 "),
        "the bounded string shares the unbounded one's routine\n{generated}"
    );
}

/// A boxed value whose target does not resolve is refused by name.
///
/// It used to answer `Ok(())`, which declares nothing and says so
/// nowhere: `valuetype VB Missing;` produced an empty package. Every
/// other unresolved reference in `declare_type` — a sequence's element,
/// a map's key, a typedef's target, a member's type — is `Unevaluated`,
/// and this one is now too.
#[test]
fn a_boxed_value_with_an_unresolved_target_is_refused() {
    let parsed = zerodds_idl_parse::parse("valuetype VB Missing;");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let mut ast: Ast = lowered.ast;
    let _ = ast.resolve_all_by_name();
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &ada::catalogue()).expect("apply");
    let error = ada_file(&ast, &applied).expect_err("a refusal");
    let text = error.to_string();
    assert!(
        text.contains("boxed value target"),
        "unexpected refusal: {text}"
    );
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_table_claims_for_ada_produces_output() {
    // The table says a rule covers it; this says the rule reaches text.
    let sources = [
        ("struct S { long a; };", "type S is record"),
        ("enum E { A };", "type E is (A);"),
        ("enum E2 { A };", "for E2 use (A => 0);"),
        ("typedef long T;", "subtype T is Interfaces.Integer_32;"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "type U (D : M := A) is record",
        ),
        ("bitmask B { X };", "type B is new Interfaces.Unsigned_32;"),
        ("bitset T { bitfield<3> a; };", "type T is record"),
        ("valuetype V { public long a; };", "type V is record"),
        (
            "const long N = 1;",
            "N : constant Interfaces.Integer_32 := 1;",
        ),
        ("exception E2 { long code; };", "type E2 is record"),
        ("interface I { void go(); };", "type I is interface;"),
        ("native N2;", "type N2 is new System.Address;"),
        (
            "struct S { sequence<long> a; };",
            "type Zd_Seq_Long is access Zd_Seq_Long_Array;",
        ),
        (
            "struct S { map<long, long> m; };",
            "type Zd_Map_Entry_Long_Long is record",
        ),
    ];
    for (idl, expected) in sources {
        let generated = generate(idl);
        assert!(
            generated.contains(expected),
            "{idl:?} produced no {expected:?}:\n{generated}"
        );
    }
}

// -- the corpus ----------------------------------------------------------

/// Every corpus case, by name.
fn corpus() -> Vec<(String, std::path::PathBuf)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).expect("the corpus directory") {
        let entry = entry.expect("a corpus entry");
        let idl = entry.path().join("type.idl");
        if idl.is_file() {
            out.push((entry.file_name().to_string_lossy().into_owned(), idl));
        }
    }
    out.sort();
    out
}

/// How many corpus cases have to reach Ada for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it: the corpus is 378
/// cases and the column has to carry substantially all of them.
const MUST_RENDER: usize = 350;

/// The names the Ada column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
/// The five recursive cases the cut of RM 3.10.1 made renderable are
/// not here — each of them renders, and `recursive-typedef-sequence`
/// joins them because a subtype on a cycle is *deferred* past its
/// target's completion rather than refused. The one that is names the
/// place the clause has no cut and no deferral to offer: an array whose
/// element reaches the array itself admits no incomplete type and no
/// reordering, and the layout planner refuses `recursive-array` for
/// every column anyway (IDL 4.2 §7.4.1.4.4.4.4).
const REFUSED: &[(&str, &str)] = &[(
    "recursive-array",
    "an incomplete type as an array element, which RM 3.10.1 does not admit",
)];

#[test]
fn every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let out = std::env::var("ZD_ADA_OUT").ok();
    let mut failures = Vec::new();
    let mut frontend = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut rendered = 0_usize;
    for (name, path) in cases {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let expected = REFUSED.iter().find(|(case, _)| *case == name);
        match (try_generate(&source, &path.display().to_string()), expected) {
            (Ok(text), None) => {
                rendered += 1;
                if let Some(dir) = &out {
                    let case = std::path::Path::new(dir).join(&name);
                    std::fs::create_dir_all(&case).expect("the output directory");
                    std::fs::write(case.join(FILE_NAME), text).expect("the output file");
                }
            }
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
            }
            (Err(why), None) => {
                // The stage that refused is what decides whose failure
                // it is. `parse` and `lower` are the frontend's and are
                // the same for every column; `apply` and `materialise`
                // are this one's.
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
            }
            (Err(_), Some(_)) => {}
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    if !frontend.is_empty() {
        // Named on standard error rather than swallowed. They are not
        // this column's failures and they are not nothing either.
        eprintln!(
            "{} corpus case(s) the frontend refuses, so Ada never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Ada; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// One generated spec through GNAT.
///
/// `gnatmake -c` compiles the units named and stops before binding,
/// which is what a spec with no body needs: there is nothing to bind and
/// nothing to link. The file is written under [`FILE_NAME`] because GNAT
/// derives the unit name from it — a package `Zd_Types` in any other
/// file fails before a declaration is read.
fn gnat_compiles(source: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-ada-compile-{}-{tag}-{}",
        std::process::id(),
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    std::fs::write(dir.join(FILE_NAME), source).map_err(|error| error.to_string())?;
    let output = Command::new("gnatmake")
        .arg("-c")
        .arg("-gnat12")
        .arg(FILE_NAME)
        .current_dir(&dir)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Whether GNAT is on the path at all.
fn gnat_available() -> bool {
    Command::new("gnatmake")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The sources the compile gate covers, one construct or family each.
///
/// A label per source, because each is written to the sample directory
/// under a file name of its own when `ZD_ADA_SAMPLES` is set.
const SAMPLES: &[(&str, &str)] = &[
    ("pose", "struct Pose { long x; double y; boolean active; };"),
    (
        "primitives",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
    ),
    (
        "collections",
        "struct S { @optional octet b; long c; string name; wstring wide; \
         sequence<long> ids; long grid[2][3]; map<long, string> table; };",
    ),
    (
        "reserved",
        "struct T { long type; long range; long access; long RECORD; long Interfaces; };",
    ),
    (
        "enum_union",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
    ),
    (
        "union_default",
        "union U switch (long) { case 1: long a; case 2: long b; default: double c; };
         union V switch (boolean) { case TRUE: long t; default: double f; };",
    ),
    (
        "union_covered",
        "enum Two { A, B }; union W switch (Two) { case A: long a; case B: long b; };",
    ),
    (
        "bitmask",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
    ),
    (
        "bitset",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
    ),
    (
        "valuetype",
        "valuetype V { public long counter; private string secret; };",
    ),
    (
        "constants",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const double SCALE = 1e5; \
         const boolean ON = TRUE;",
    ),
    (
        "exception",
        "exception Failure { long code; string reason; };",
    ),
    (
        "interface",
        "struct Target { long x; };
         interface Control { void go(in Target t); long status(); \
         void mix(inout long a, out double b); };",
    ),
    (
        "interface_attributes_and_raises",
        "exception Overrun { long limit; };
         interface Base { readonly attribute long counter; };
         interface Derived : Base {
           attribute long memory;
           long f(in long x) raises (Overrun);
         };",
    ),
    ("native", "native Handle;"),
    (
        "module",
        "module common { module util { struct Location { long x; }; }; };",
    ),
    (
        "typedef",
        "typedef long Distance; struct Dist { Distance d; };",
    ),
    ("empty", "struct Empty { }; bitset AllPad { };"),
    (
        "nested",
        "struct Inner { long x; }; struct Middle { Inner i; sequence<Inner> many; }; \
         struct Outer { Middle m; Middle grid[2]; };",
    ),
    // The two shapes GNAT 14.2.0 rejected on the first run, so a
    // regression in either is a compile failure and not a diff.
    (
        // No negative floating-point constant here: the frontend does
        // not evaluate `const double X = -1.5;` today and reports the
        // constant as unevaluated, which is not this column's failure
        // and is not something a sample should carry. `needs_operator`
        // covers the shape anyway, so the clause will be there when the
        // frontend answers.
        "negative_labels",
        "union U switch (short) { case -13: long a; case 127: double b; };
         union V switch (long long) { case -987654321987: long c; };
         const long N = -5; const short M = -1;",
    ),
    (
        "escaped_identifier",
        "struct Sample { long _long; double _double; short _switch; long plain; };",
    ),
    // The five shapes of RM 3.10.1 cut, one each. These are what the
    // compile gate is most needed for: everything above is ordinary Ada,
    // and an incomplete type declaration is the one construct here whose
    // legality depends on where it stands.
    (
        "recursive_sequence",
        "struct Node { long value; sequence<Node> children; };",
    ),
    (
        "recursive_map",
        "struct Node { long value; map<long, Node> children; };",
    ),
    (
        "recursive_optional",
        "struct Node; struct Node { long value; @optional @external Node next; };",
    ),
    (
        "recursive_mutual",
        "struct B; struct A { long value; sequence<B> bs; }; \
         struct B { long weight; sequence<A> as; };",
    ),
    (
        "recursive_union",
        "struct Node;
         union Branch switch (long) { case 0: long leaf; default: sequence<Node> nodes; };
         struct Node { long value; Branch link; };",
    ),
];

/// Every sample above, compiled.
///
/// One `gnatmake` per sample rather than one for all of them: the
/// samples declare types with names of their own but each is a package
/// spec, and two packages of one name cannot share a directory.
#[test]
fn the_generated_ada_compiles() {
    // Written whether or not GNAT is here, so a host that has it can be
    // handed exactly the text this host produced.
    if let Ok(dir) = std::env::var("ZD_ADA_SAMPLES") {
        write_samples(&dir);
    }
    if !gnat_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: `gnatmake` is not on the path, so the generated Ada was not \
             compiled. Looked for `gnatmake --version`; install GNAT (Debian: \
             `gnat`, Alire: `gnat_native`) or run this on a host that has it."
        );
        return;
    }
    for (label, idl) in SAMPLES {
        let generated = generate(idl);
        if let Err(why) = gnat_compiles(&generated, label) {
            panic!("{label}: {idl}\n\n{generated}\n\ngnatmake said:\n{why}");
        }
    }
}

/// The samples, each in a file GNAT will accept the unit name from.
///
/// Every generated spec declares `Zd_Types`, so one directory cannot
/// hold two of them under the name GNAT requires. The package is renamed
/// per sample here — the only edit made to generated text anywhere in
/// this column, and it is made so the files can sit side by side for a
/// compiler on another host.
fn write_samples(dir: &str) {
    let root = std::path::Path::new(dir);
    std::fs::create_dir_all(root).expect("the sample directory");
    for (label, idl) in SAMPLES {
        // GNAT folds the unit name to lower case to find the file, so
        // `Zd_Sample_Recursive_Map` has to be in
        // `zd_sample_recursive_map.ads` and nowhere else.
        let unit = format!("Zd_Sample_{}", zerodds_lang_cst::ada::ada_case(label));
        let file = format!("{}.ads", unit.to_lowercase());
        let generated = generate(idl)
            .replace("package Zd_Types is", &format!("package {unit} is"))
            .replace("end Zd_Types;", &format!("end {unit};"))
            .replace("zd_types.ads", &file);
        std::fs::write(root.join(&file), generated).expect("the sample file");
    }
}

/// The whole corpus through GNAT, one package per case.
///
/// `#[ignore]`, because it is one invocation of a compiler per case and
/// takes minutes. It is what proves the column rather than samples it,
/// so it is run with `--include-ignored` on a host that has GNAT.
#[test]
#[ignore = "one `gnatmake` invocation per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        gnat_available(),
        "the explicitly selected ada gate requires its compiler"
    );
    let mut failures = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = try_generate(&source, &path.display().to_string())
            .unwrap_or_else(|why| panic!("accepted corpus case {name} failed generation: {why}"));
        checked += 1;
        if let Err(why) = gnat_compiles(&text, &name) {
            failures.push(format!("{name}: {why}"));
        }
    }
    eprintln!("{checked} corpus case(s) reached ada_end_to_end");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(
        checked >= MUST_RENDER,
        "too few corpus cases reached the selected gate: {checked}"
    );
}
