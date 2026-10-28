// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Nim, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go,
//! and of `c_end_to_end.rs`, which does it for C.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//!    A case that stopped rendering would otherwise be found by
//!    whichever probe ran next, which is a build directory away from the
//!    construct that caused it.
//! 2. **The generated Nim passes `nim check`**, which is the semantic
//!    pass with no code generation — it resolves every name, checks
//!    every type and stops before the C backend runs, which is exactly
//!    what a types-only file needs proving about it. A snapshot says the
//!    emitter produced the text it produced last time; it does not say
//!    the text is Nim, that every name in it resolves, or that a field's
//!    type exists. `nim` is skipped where it is not on the path and the
//!    skip says so on standard error — a test that went green without
//!    running it would be the failure it is meant to catch.
//!
//! Nim is indentation-sensitive and its identifiers are compared after
//! normalisation, so both failure modes this column has are silent in a
//! way the brace languages' are not: a wrong indent is a different
//! program and two members that normalise together are one field. The
//! assertions below check the text itself and not merely that something
//! came out.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;
#[path = "support/resolved.rs"]
mod resolved;

use std::process::Command;
use zerodds_lang_rules::adapter::nim::nim_file;
use zerodds_lang_rules::rule::nim;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Nim out.
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
    let applied = apply(&model, &nim::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = nim_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::nim::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_a_nim_object() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("  Pose* = object\n"), "{generated}");
    assert!(generated.contains("    x*: int32\n"), "{generated}");
    assert!(generated.contains("    y*: float64\n"), "{generated}");
    assert!(generated.contains("    active*: bool\n"), "{generated}");
}

#[test]
fn every_declaration_carries_the_export_marker_that_makes_it_reachable() {
    // Nim's `*` (Nim manual, "Export marker"). A declaration without one
    // is invisible outside its module, and a types column whose types no
    // other module can name has no consumer. Unlike the Go column's
    // capitalisation it renames nothing, so the IDL spelling survives.
    let generated = generate("struct Pose { long x; }; const octet LIMIT = 8;");
    assert!(generated.contains("Pose* = object"), "{generated}");
    assert!(generated.contains("x*: int32"), "{generated}");
    assert!(generated.contains("LIMIT*: uint8 = 8"), "{generated}");
    // And the IDL spelling is the spelling: nothing was capitalised.
    assert!(
        !generated.contains("X*:"),
        "a member was renamed\n{generated}"
    );
}

#[test]
fn every_idl_primitive_reaches_a_nim_type() {
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
        "a*: bool",
        "b*: uint8",
        "c*: uint8",
        "d*: uint16",
        "e*: int8",
        "f*: uint8",
        "g*: int16",
        "h*: uint16",
        "i*: int32",
        "j*: uint32",
        "k*: int64",
        "l*: uint64",
        "m*: float32",
        "n*: float64",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn no_generated_type_has_an_implementation_defined_width() {
    // Nim's `int` and `uint` are "the same size as a pointer" (Nim
    // manual, "Pre-defined integer types"), so a member declared with
    // one would change size between a build for amd64 and one for a
    // 32-bit ARM, for a wire format that fixes every width. `float` is
    // an alias for `float64` and hides which of the two was asked for.
    let generated = generate(
        "struct P { long a; unsigned long b; long long c; sequence<long> d; long e[2]; \
         float f; double g; };",
    );
    for forbidden in [
        ": int\n",
        ": uint\n",
        ": float\n",
        "seq[int]",
        "array[2, int]",
    ] {
        assert!(
            !generated.contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn an_idl_char_is_a_uint8_and_never_a_nim_char() {
    // Nim's `char` is exactly one byte, which is the right width, and it
    // is a distinct ordinal type with no arithmetic: `c + 1` does not
    // compile, so a codec adding an offset to an octet would need a
    // conversion at every use.
    let generated = generate("struct S { char c; octet o; };");
    assert!(generated.contains("c*: uint8"), "{generated}");
    assert!(
        !generated.contains(": char"),
        "a Nim char reached the output\n{generated}"
    );
}

#[test]
fn a_long_double_is_a_string_and_never_a_float64() {
    // Nim has `float32`, `float64` and no binary128. A `float64` has 53
    // bits of significand; an IDL `long double` is a binary128 with 113.
    // Writing one into a `float64` loses sixty bits at the assignment,
    // before any codec runs, with nothing to say it happened.
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("big*: string"), "{generated}");
}

#[test]
fn an_optional_member_becomes_an_option_and_the_others_do_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. Nim's zero value is
    // indistinguishable from a written zero, so a `long` that arrived as
    // 0 and one that did not arrive at all would be the same value;
    // `isNone` is the one state no written value can take. Not a
    // `ref T`: a `ref` is a traced pointer, so two copies of a decoded
    // value would share one heap cell.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(generated.contains("b*: Option[uint8]"), "{generated}");
    assert!(generated.contains("c*: int32"), "{generated}");
    assert!(
        !generated.contains("ref uint8"),
        "a ref reached the output\n{generated}"
    );
}

#[test]
fn an_external_member_is_held_by_reference_whether_or_not_it_recurses() {
    // DDS-XTypes 1.3 Table 21: the member is held outside the containing
    // object, and the indirection is the language mapping's — it changes
    // no octet. `ref` and not `ptr`: a `ref` is traced, so the runtime
    // owns the cell, where a `ptr` would make every `@external` member
    // an ownership question the IDL does not answer.
    //
    // Every `@external` member takes one, not only the ones that would
    // otherwise recurse. A mapping that reached for a reference when a
    // cycle forced it would be a repair and not a rule.
    let generated = generate(
        "struct Inner { long a; };
         struct Sample {
           @external long a;
           @external string text;
           @external sequence<long> ids;
           @external Inner nested;
           long tail;
         };",
    );
    assert!(generated.contains("a*: ref int32"), "{generated}");
    assert!(generated.contains("text*: ref string"), "{generated}");
    assert!(generated.contains("ids*: ref seq[int32]"), "{generated}");
    assert!(generated.contains("nested*: ref Inner"), "{generated}");
    // And a member with no annotation is still held inline.
    assert!(generated.contains("tail*: int32"), "{generated}");
    assert!(
        !generated.contains("tail*: ref"),
        "a plain member took a reference\n{generated}"
    );
}

#[test]
fn a_recursive_optional_external_member_is_finite() {
    // `tests/interop-corpus/recursive-optional-external`, which is the
    // case that measured this. Nim's `Option[T]` stores its `T` inline,
    // so `Option[m_Node]` inside `m_Node` is an object containing
    // itself: Nim 2.2.10 answers `illegal recursion in type 'm_Node'`.
    // The `ref` has the same size whatever it points at, which is what
    // terminates the declaration.
    //
    // The reference goes on first and the `Option` outside it. The two
    // annotations answer different questions — Table 21 says where the
    // member is held, §7.2.2.4.4.4.7 says whether it is there at all —
    // and the outer form has to be the one that can say "absent".
    let generated = generate(
        "module m {
           struct Node;
           @mutable struct Node {
             long value;
             @optional @external Node next;
           };
         };",
    );
    assert!(generated.contains("m_Node* = object"), "{generated}");
    assert!(
        generated.contains("next*: Option[ref m_Node]"),
        "{generated}"
    );
    assert!(
        !generated.contains("next*: Option[m_Node]"),
        "the member is held inline and the type contains itself\n{generated}"
    );
    assert!(
        !generated.contains("next*: ref Option"),
        "a reference to an optional says neither thing\n{generated}"
    );
}

#[test]
fn the_options_import_appears_exactly_when_an_option_does() {
    // `Option` is `std/options` and not a built-in, so a file using one
    // has to import the module — and a file that does not would take an
    // `imported and not used` warning for nothing.
    let with = generate("struct S { @optional octet b; };");
    assert!(with.contains("import std/options"), "{with}");
    let without = generate("struct S { octet b; };");
    assert!(
        !without.contains("import std/options"),
        "an unused import reached the output\n{without}"
    );
}

#[test]
fn a_member_named_after_a_nim_keyword_is_stropped_and_keeps_its_spelling() {
    // Backticks, which is Nim's own syntax for using a keyword as a
    // name. The trailing underscore the Go and Python columns append is
    // not available: Nim's identifier production is
    // `letter ( ['_'] (letter | digit) )*`, so `type_` does not lex at
    // all — the compiler's message for it is `invalid token: trailing
    // underscore`. Stropping renames nothing, so the IDL spelling is
    // still in the file.
    let generated = generate("struct S { long type; long proc; long addr; };");
    assert!(generated.contains("`type`*: int32"), "{generated}");
    assert!(generated.contains("`proc`*: int32"), "{generated}");
    assert!(generated.contains("`addr`*: int32"), "{generated}");
}

#[test]
fn a_name_that_only_normalises_onto_a_keyword_is_stropped_too() {
    // Nim's lexer looks a symbol up in the keyword table by the
    // normalised key — first character's case kept, the rest lower-cased
    // with the underscores removed — so `t_ype` *is* the keyword `type`
    // and not an identifier that resembles it. The backticks hold the
    // spelling the IDL wrote; Nim resolves it to the one symbol either
    // way.
    let generated = generate("struct S { long t_ype; };");
    assert!(generated.contains("`t_ype`*: int32"), "{generated}");
    // And the escape is recorded, because the spelling moved.
    assert!(generated.contains("IDL: `t_ype`."), "{generated}");
}

#[test]
fn a_trailing_underscore_is_repaired_because_nim_has_no_name_for_it() {
    // `count_` is a legal IDL identifier (IDL 4.2 §7.2.3 gives
    // `[A-Za-z_][A-Za-z0-9_]*`) and not a Nim one. The repair is
    // invisible to Nim — it drops interior underscores when it compares
    // identifiers anyway — and the IDL spelling goes in the comment,
    // because this is the one thing in this column that does rename.
    let generated = generate("struct S { long count_; };");
    assert!(generated.contains("count*: int32"), "{generated}");
    assert!(generated.contains("IDL: `count_`."), "{generated}");
}

#[test]
fn two_members_nim_cannot_tell_apart_are_refused_by_name() {
    // The column's central hazard, and it is unique among these
    // languages. Nim compares identifiers after normalisation: the first
    // character keeps its case, the rest is lower-cased and every
    // underscore is dropped, so `foo_bar` and `fooBar` are one
    // identifier. IDL 4.2 §7.2.3.1 forbids only a pair that differs in
    // case alone, so the pair is legal IDL and the frontend passes it
    // through. Nim would report a redefinition naming one field; this
    // names both IDL members.
    let error = try_generate("struct S { long foo_bar; long fooBar; };", "collide.idl")
        .expect_err("the collision was not reported");
    assert!(error.contains("foo_bar"), "{error}");
    assert!(error.contains("fooBar"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_pair_nim_does_tell_apart_is_not_refused() {
    // The first character's case *is* significant in Nim, so `foo_bar`
    // and `Foobar` are two identifiers and the check must not fire on
    // them. IDL compares identifiers without regard to case
    // (IDL 4.2 §7.2.3.1), so this is also the only shape the pair can
    // take: the frontend rejects `fooBar` beside `FooBar` before any
    // column sees it. A check that refused this pair would be refusing
    // legal IDL for a Nim rule that does not exist.
    let generated = generate("struct S { long foo_bar; long Foobar; };");
    assert!(generated.contains("foo_bar*: int32"), "{generated}");
    assert!(generated.contains("Foobar*: int32"), "{generated}");
}

#[test]
fn two_declarations_that_flatten_onto_one_module_scope_name_are_refused() {
    // One Nim module holds every declaration in the file, so this is a
    // whole-file check and not a per-scope one. `a_b::C` and `a::b_C`
    // both flatten to `a_b_C`, and Nim would report a redefinition
    // naming neither IDL declaration.
    let error = try_generate(
        "module a_b { struct C { long x; }; };
         module a { struct b_C { long y; }; };",
        "flatten.idl",
    )
    .expect_err("the collision was not reported");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_sequence_becomes_a_seq_and_a_string_a_string() {
    let generated = generate("struct Bag { sequence<long> ids; string name; };");
    assert!(generated.contains("ids*: seq[int32]"), "{generated}");
    assert!(generated.contains("name*: string"), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first() {
    // `long grid[2][3]` is two rows of three, which in Nim is
    // `array[2, array[3, int32]]`. Nim indexes an `array` whose length
    // is given as a plain integer from zero, which is what an IDL array
    // does; reversing the dimensions silently transposes the data.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("grid*: array[2, array[3, int32]]"),
        "{generated}"
    );
}

#[test]
fn a_map_becomes_a_seq_of_entries_and_never_a_table() {
    // A `std/tables` table iterates in bucket order, which depends on
    // the hash of the keys and on the capacity. A `map<K,V>` goes on the
    // wire as a sequence of pairs (DDS-XTypes 1.3 §7.4.1.1.4), so a
    // `Table` would give a different octet stream for two values that
    // differ only in the order they were built.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("table*: seq[ZdMapEntry_Long_String]"),
        "{generated}"
    );
    assert!(
        generated.contains("ZdMapEntry_Long_String* = object"),
        "{generated}"
    );
    assert!(
        !generated.contains("Table["),
        "a Nim Table reached the output\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_transparent_alias_and_not_a_distinct_type() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type".
    // `type Distance* = distinct int32` would, and a value of it would
    // then need an explicit conversion to reach an `int32`.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(generated.contains("Distance* = int32"), "{generated}");
    assert!(generated.contains("d*: Distance"), "{generated}");
    assert!(
        !generated.contains("distinct"),
        "a distinct type reached the output\n{generated}"
    );
}

#[test]
fn an_enum_becomes_one_enum_with_every_ordinal_written_out() {
    // Nim's enum fields take explicit ordinals, so unlike the C and Go
    // columns there is no defined integer type and no constant per
    // enumerator. Every ordinal is written because `@value` admits gaps
    // and a field with none takes its predecessor's plus one — the gap
    // makes it a *holey* enum, which Nim accepts and only withdraws `..`
    // iteration and array indexing from.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("Mode* = enum"), "{generated}");
    assert!(generated.contains("    IDLE = 0\n"), "{generated}");
    assert!(generated.contains("    ACTIVE = 4\n"), "{generated}");
    assert!(generated.contains("    FAULT = 5\n"), "{generated}");
    // No export marker on the fields: the `enumDecl` production takes a
    // `symbol` where the object production takes an `identVis`, and the
    // fields of an exported enumeration are reachable without one.
    assert!(
        !generated.contains("IDLE* ="),
        "an export marker reached an enum field\n{generated}"
    );
}

#[test]
fn an_enumerator_that_does_not_ascend_is_refused_by_name() {
    // "The ordinal values have to be in ascending order" (Nim manual,
    // "Enumeration types"). `@value` does not have to. Refused rather
    // than renumbered: an enumerator whose number changed would be a
    // different value on the wire.
    let error = try_generate("enum E { @value(5) A, @value(1) B };", "descending.idl")
        .expect_err("the descending enumeration was not reported");
    assert!(error.contains("Nim"), "{error}");
    assert!(
        error.contains("ascend") || error.contains("exceed"),
        "{error}"
    );
}

#[test]
fn a_union_becomes_an_object_with_a_discriminator_and_optional_branches() {
    // Not a Nim object variant: a decoder constructs one from a
    // discriminator it read at run time, which Nim cannot check at
    // compile time, and IDL 4.2 §7.4.1.4.4.1 lets the discriminator hold
    // a value that selects no branch at all. The branch that was written
    // is the one that is `isSome`.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("Payload* = object"), "{generated}");
    assert!(generated.contains("d*: Mode"), "{generated}");
    assert!(generated.contains("idle*: Option[int32]"), "{generated}");
    assert!(
        generated.contains("active*: Option[float64]"),
        "{generated}"
    );
    assert!(
        !generated.contains("case d"),
        "an object variant reached the output\n{generated}"
    );
}

/// An `@external` union branch keeps its `ref` inside the branch's
/// `Option`: `Option[ref Inner]`, the same order the struct member
/// takes — Table 21 says where the value is held, and the `Option` is
/// the branch's own "selected or not" state.
#[test]
fn an_external_union_branch_is_held_by_reference_inside_its_option() {
    let generated = generate(
        "struct Inner { long a; };
         union Payload switch (long) {
           case 1: @external Inner nested;
           default: long other;
         };",
    );
    assert!(
        generated.contains("nested*: Option[ref Inner]"),
        "{generated}"
    );
    assert!(
        !generated.contains("nested*: Option[Inner]"),
        "the reference was dropped on the branch\n{generated}"
    );
}

#[test]
fn a_union_branch_that_would_take_the_discriminators_field_is_refused() {
    // IDL 4.2 §7.4.1.4.4.1 gives the discriminator no member name of its
    // own, so `d` is this column's — and `_d`, which every other column
    // uses, is not a Nim identifier at all, because the production
    // starts at a letter. A branch called `d` would take the same field.
    let error = try_generate(
        "enum Mode { IDLE };
         union Payload switch (Mode) { case IDLE: long d; };",
        "discriminator.idl",
    )
    .expect_err("the collision was not reported");
    assert!(error.contains("discriminator"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_bitmask_becomes_an_unsigned_alias_and_one_constant_per_bit() {
    // Not a Nim `set[T]`: a set's base type is bounded at 2^16 elements
    // where DDS-XTypes 1.3 §7.4.1.1.3.2 admits a `@bit_bound(64)`, and
    // the bit a member occupies in a set is the compiler's layout rather
    // than the `@position` the IDL wrote.
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("Flags* = uint16"), "{generated}");
    assert!(generated.contains("READY* = Flags(1) shl 0"), "{generated}");
    assert!(
        generated.contains("MOVING* = Flags(1) shl 4"),
        "{generated}"
    );
    assert!(
        !generated.contains("set["),
        "a Nim set reached the output\n{generated}"
    );
}

#[test]
fn a_bitmask_constant_shifts_in_the_masks_own_width() {
    // `1 shl 63` is an `int` shift and overflows into a negative number
    // before anything could convert it, which is exactly the bit a
    // `@bit_bound(64)` mask needs. The conversion happens first.
    let generated = generate("@bit_bound(64) bitmask Wide { @position(63) TOP };");
    assert!(generated.contains("Wide* = uint64"), "{generated}");
    assert!(generated.contains("TOP* = Wide(1) shl 63"), "{generated}");
}

#[test]
fn a_bitset_becomes_an_object_of_holder_typed_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("B* = object"), "{generated}");
    assert!(generated.contains("low*: uint8"), "{generated}");
    assert!(generated.contains("high*: uint16"), "{generated}");
}

#[test]
fn a_valuetype_becomes_an_object_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("V* = object"), "{generated}");
    assert!(generated.contains("counter*: int32"), "{generated}");
    assert!(generated.contains("secret*: string"), "{generated}");
}

#[test]
fn a_constant_becomes_a_nim_constant_of_its_declared_type() {
    let generated = generate("const octet LIMIT = 8;");
    assert!(generated.contains("const\n"), "{generated}");
    assert!(generated.contains("  LIMIT*: uint8 = 8\n"), "{generated}");
}

#[test]
fn a_string_constant_is_written_in_ascii_one_escape_per_octet() {
    // A Nim `\xHH` escape inserts one byte, and a Nim `string` is a run
    // of octets with no encoding of its own, so the UTF-8 octets are
    // what put the same bytes in the string that the source had.
    let generated = generate("const string TAG = \"a\\u00e4b\";");
    assert!(generated.contains("TAG*: string = "), "{generated}");
    assert!(generated.is_ascii(), "the file is not ASCII\n{generated}");
}

#[test]
fn an_exception_becomes_an_object_of_its_members() {
    // A plain object and not one inheriting `CatchableError`: only a
    // `ref object` can be raised in Nim, and a `ref` would give the type
    // reference semantics no other member of a generated file has.
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("Failure* = object"), "{generated}");
    assert!(generated.contains("code*: int32"), "{generated}");
    assert!(generated.contains("reason*: string"), "{generated}");
}

#[test]
fn an_interface_becomes_an_object_of_procedural_fields() {
    // Nim has no interface construct. `concept` is the closest and is
    // documented as experimental, so a generated file depending on one
    // would be a file whose meaning is a compiler release away from
    // moving. An object of procedural fields is ordinary Nim.
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void start(in Pose target);
           long status();
         };",
    );
    assert!(generated.contains("Control* = object"), "{generated}");
    assert!(
        generated.contains("start*: proc (target: Pose)"),
        "{generated}"
    );
    assert!(generated.contains("status*: proc (): int32"), "{generated}");
}

#[test]
fn a_native_declaration_becomes_an_opaque_pointer_alias() {
    // `pointer` and not `ref`: a `ref` is traced, and an IDL `native`
    // says nothing that would let this runtime take ownership of what it
    // names.
    let generated = generate("native Handle;");
    assert!(generated.contains("Handle* = pointer"), "{generated}");
}

#[test]
fn a_module_flattens_onto_the_name_because_a_nim_module_is_a_file() {
    // Nim's unit of scope is the module and a module *is* a file: there
    // is no syntax for nesting one inside another, so nested IDL modules
    // cannot nest here.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("common_util_Location* = object"),
        "{generated}"
    );
    assert!(
        generated.contains("IDL: `Location`."),
        "the IDL name was not recorded\n{generated}"
    );
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Nim equivalent. What is left is
    // the file comment, and no section at all — a `type` keyword with no
    // indented block after it is a parse error, not an empty section.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(generated.contains("Generated by ZeroDDS"), "{generated}");
    assert!(
        !generated.contains("type\n"),
        "an empty section reached the output\n{generated}"
    );
}

#[test]
fn one_type_section_holds_every_declaration() {
    // Nim groups declarations into sections, and within one `type`
    // section the declarations may name each other in any order,
    // including mutually. That is what lets this column walk the model
    // in declaration order where `adapter/c.rs` has to walk the type
    // graph post-order and the OCaml column has to thread an `and`.
    let generated = generate(
        "struct First { long a; }; enum E { X }; typedef long T; struct Second { long b; };",
    );
    let sections = generated.lines().filter(|line| *line == "type").count();
    assert_eq!(sections, 1, "not one type section\n{generated}");
}

#[test]
fn a_type_may_name_itself_inside_the_one_section() {
    let generated = generate("struct Node { sequence<Node> children; };");
    assert!(generated.contains("Node* = object"), "{generated}");
    assert!(generated.contains("children*: seq[Node]"), "{generated}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("First*").expect("First");
    let second = generated.find("Second*").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn the_const_section_comes_after_the_type_section() {
    // Nim resolves a module-scope name only after its declaration, and
    // only a `type` section is exempt from that. A constant whose type
    // is a declared one would not compile above it.
    let generated = generate("typedef long Distance; const Distance D = 5;");
    let types = generated.find("\ntype\n").expect("the type section");
    let consts = generated.find("\nconst\n").expect("the const section");
    assert!(
        types < consts,
        "the sections are the wrong way round\n{generated}"
    );
}

#[test]
fn no_line_of_the_output_is_indented_with_a_tab() {
    // Nim rejects a tab in the indentation of a line outright (Nim
    // manual, Lexical Analysis). This is a whole-file failure and not an
    // untidy one.
    let generated = generate(
        "module m { struct S { long a; }; };
         @bit_bound(8) bitmask B { X };",
    );
    for line in generated.lines() {
        assert!(!line.contains('\t'), "a tab reached the output: {line:?}");
        assert_eq!(line, line.trim_end(), "trailing whitespace: {line:?}");
    }
}

#[test]
fn the_whole_output_is_ascii_including_the_prose_in_its_comments() {
    // The renderer escapes a code point above 0x7F to `\xHH` per octet,
    // which is what a Nim string literal needs and what a reader of a
    // comment cannot read. Every string this column writes into a
    // comment is therefore ASCII already, so the escape never fires
    // there — an em dash in the prose would reach the file as
    // `\xe2\x80\x94`.
    let generated = generate(
        "struct S { @optional long a; map<long, string> m; };
         enum E { A }; union U switch (E) { case A: long a; };
         @bit_bound(8) bitmask B { X };
         bitset Bits { bitfield<3> low; };
         exception F { long code; };
         native H;
         module m { struct Inner { long x; }; };",
    );
    assert!(generated.is_ascii(), "the file is not ASCII\n{generated}");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_table_claims_for_nim_produces_output() {
    // The table says a rule covers it; this says the rule reaches text.
    let sources = [
        ("struct S { long a; };", "S* = object"),
        ("enum E { A };", "E* = enum"),
        ("typedef long T;", "T* = int32"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "U* = object",
        ),
        ("bitmask B { X };", "B* = uint32"),
        ("bitset T { bitfield<3> a; };", "T* = object"),
        ("valuetype V { public long a; };", "V* = object"),
        ("const long N = 1;", "N*: int32 = 1"),
        ("exception E2 { long code; };", "E2* = object"),
        ("interface I { void start(); };", "I* = object"),
        ("native N2;", "N2* = pointer"),
        ("module m { struct S { long a; }; };", "m_S* = object"),
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

/// How many corpus cases have to reach Nim for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it: the corpus is 378
/// cases and the column has to carry substantially all of them.
const MUST_RENDER: usize = 350;

/// The names the Nim column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too — an exemption nothing checks is an exemption that outlives its
/// cause.
const REFUSED: &[(&str, &str)] = &[];

#[test]
fn every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let out = std::env::var("ZD_NIM_OUT").ok();
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
                    std::fs::write(case.join("zd_types.nim"), text).expect("the output file");
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
            "{} corpus case(s) the frontend refuses, so Nim never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Nim; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// One generated file through `nim check`.
///
/// `nim check` and not `nim compile`: it is the semantic pass on its
/// own — it resolves every name and checks every type and then stops,
/// with no C backend, no linker and no binary. A types-only file has
/// nothing to run, so everything a compile would add is time.
fn nim_checks(source: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-nim-check-{}-{}",
        std::process::id(),
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("zd_types.nim");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("nim")
        .arg("check")
        .arg("--hints:off")
        .arg("--colors:off")
        .arg("zd_types.nim")
        .current_dir(&dir)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        // Nim writes its diagnostics to standard output, not standard
        // error, so both are reported: a failure whose message went to
        // the channel this did not read would be a failure with no text.
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Whether `nim` is on the path at all.
fn nim_available() -> bool {
    Command::new("nim")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Every shape above, checked.
///
/// One test and not one per shape, because each `nim check` costs a
/// second and the sources are checked as separate files anyway — they
/// cannot be concatenated, since two of them declare a `S`.
#[test]
fn the_generated_nim_compiles() {
    if !nim_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch. It is the expected state on a machine with no
        // Nim toolchain: the proof is outstanding until this runs on one
        // that has it.
        eprintln!(
            "SKIPPED: `nim` is not on the path, so the generated Nim was not checked. \
             Install Nim 2.x or run this on a host that has it."
        );
        return;
    }
    for idl in SAMPLES {
        let generated = generate(idl);
        if let Err(why) = nim_checks(&generated) {
            panic!("{idl}\n\n{generated}\n\nnim check said:\n{why}");
        }
    }
}

/// The sources the compile gate covers, one per construct.
///
/// Public to this file and named, because `write_the_samples` writes the
/// same list to disk for a host that has Nim and does not have this test
/// suite.
const SAMPLES: &[&str] = &[
    "struct Pose { long x; double y; boolean active; };",
    "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
     unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
     float m; double n; long double o; };",
    "struct S { @optional octet b; long c; string name; sequence<long> ids; \
     long grid[2][3]; map<long, string> table; };",
    "struct T { long type; long proc; long addr; long count_; };",
    "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
     union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
    "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
    "@bit_bound(64) bitmask Wide { @position(63) TOP };",
    "bitset Bits { bitfield<3> low; bitfield<12> high; };",
    "valuetype V { public long counter; private string secret; };",
    "const octet LIMIT = 8; const string TAG = \"abc\"; const double RATE = 1.5;",
    "exception Failure { long code; string reason; };",
    "struct Target { long x; }; interface Control { void start(in Target t); long status(); };",
    "exception Overrun { long limit; };\n\
     interface Base { readonly attribute long counter; };\n\
     interface Derived : Base {\n\
     \x20   attribute long memory;\n\
     \x20   long f(in long x) raises (Overrun);\n\
     };",
    "native Handle;",
    "module common { module util { struct Location { long x; }; }; };",
    "typedef long Distance; struct Dist { Distance d; };",
    "struct Node { sequence<Node> children; };",
    "struct Empty { };",
    // `@external` on a primitive, a string, a sequence and an aggregate.
    // DDS-XTypes 1.3 Table 21.
    "struct Inner { long a; };
     struct Ext { @external long a; @external string text; \
      @external sequence<long> ids; @external Inner nested; long tail; };",
    // The case `nim check` refused before this column honoured Table 21:
    // `Option[m_Node]` inside `m_Node` is an object containing itself.
    // `tests/interop-corpus/recursive-optional-external`.
    "module m {
       struct Node;
       @mutable struct Node { long value; @optional @external Node next; };
     };",
];

/// Write the sample output where a host with a Nim toolchain can read
/// it.
///
/// `#[ignore]`, because it writes files. It exists so that the compile
/// proof can be carried out on a machine that has Nim without carrying
/// this crate's build there: `ZD_NIM_SAMPLES=<dir> cargo test -p
/// zerodds-lang-rules --test nim_end_to_end -- --ignored
/// write_the_samples`.
#[test]
#[ignore = "writes files; set ZD_NIM_SAMPLES and run with --ignored"]
fn write_the_samples() {
    let dir = std::env::var("ZD_NIM_SAMPLES").expect("the selected export needs ZD_NIM_SAMPLES");
    let root = std::path::Path::new(&dir);
    std::fs::create_dir_all(root).expect("the sample directory");
    for (index, idl) in SAMPLES.iter().enumerate() {
        let text = generate(idl);
        let path = root.join(format!("sample_{:02}.nim", index + 1));
        std::fs::write(&path, text).expect("a sample file");
    }
    eprintln!(
        "{} Nim sample(s) written to {}",
        SAMPLES.len(),
        root.display()
    );
}

/// The whole corpus through `nim check`, one file per case.
///
/// `#[ignore]`, because it is 378 invocations of a compiler and takes
/// minutes. It is what proves the column rather than samples it, so it
/// is run with `--include-ignored` on a host that has Nim and its result
/// is what `docs/language-columns-status.md` records.
#[test]
#[ignore = "378 `nim check` invocations; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        nim_available(),
        "the explicitly selected nim gate requires its compiler"
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
        if let Err(why) = nim_checks(&text) {
            failures.push(format!("{name}: {why}"));
        }
    }
    eprintln!("{checked} corpus case(s) reached nim check");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not check:\n{failures:#?}",
        failures.len()
    );
    assert!(
        checked >= MUST_RENDER,
        "too few corpus cases reached the selected gate: {checked}"
    );
}
