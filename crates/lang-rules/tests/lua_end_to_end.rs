// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Lua, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go,
//! and of `end_to_end.rs`, which does it for Rust.
//!
//! Three gates and not two, because Lua checks less than any other
//! target here:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//! 2. **The generated Lua runs.** Not `luac -p`, which only parses. The
//!    chunk is *executed* — `assert(loadfile(f))()` — so every table
//!    constructor is built, every constructor function is called by the
//!    file's own trailing local, and the `assert`s in `M._resolved` fire
//!    if a descriptor names a type the file does not declare. A test
//!    below writes a file that parses and does not run, and shows that
//!    `luac -p` accepts it and this gate does not.
//! 3. **The member order survives**, measured by running the module and
//!    reading `M._descriptors[…].members` back. That is the fact this
//!    whole column rests on: a Lua table has no order, so the order is
//!    only there if it was written into the array part.
//!
//! What none of this proves is stated plainly, because it is the point
//! of the column: Lua enforces nothing about a value. A member of the
//! wrong width, the wrong sign or the wrong type is a table entry like
//! any other, and the generated file will hold it. The gate says the
//! file loads and that the descriptor is there to be read — not that
//! anything reads it.
// `print_stderr` is allowed here for one reason: an interpreter that is
// not on the path has to say so on a channel a passing test does not
// use. A green test that never ran `lua` is the failure these gates
// exist to catch.
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
use zerodds_lang_rules::adapter::lua::lua_file;
use zerodds_lang_rules::rule::lua;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Lua out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    // The same step the C, Go and TypeScript columns take: a member whose
    // type is written as a scoped name arrives as an unresolved
    // reference, and a materialiser that asked `resolved()` without this
    // would report "the member type does not evaluate" for every
    // cross-module reference in the corpus.
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &lua::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = lua_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::lua::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_becomes_a_descriptor_and_a_constructor() {
    // The two halves of this column. Neither alone carries the IDL: the
    // constructor holds no width and the descriptor holds no value.
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("M.Pose = {"), "{generated}");
    assert!(generated.contains("kind = \"struct\""), "{generated}");
    assert!(generated.contains("function M.Pose.new()"), "{generated}");
    assert!(
        generated.contains("return { x = 0, y = 0.0, active = false }"),
        "{generated}"
    );
    assert!(
        generated.contains("M._descriptors[\"::Pose\"] = M.Pose"),
        "{generated}"
    );
}

#[test]
fn the_member_list_is_the_array_part_and_keeps_declaration_order() {
    // A Lua table has no order: `next`, and therefore `pairs`, does not
    // specify one (Lua 5.4 reference §6.1). DDS-XTypes writes members in
    // declaration order (1.3 §7.4.1.1.5.1), so the order has to be the
    // array part of a table or it is gone.
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    let x = generated.find("name = \"x\"").expect("x");
    let y = generated.find("name = \"y\"").expect("y");
    let active = generated.find("name = \"active\"").expect("active");
    assert!(x < y && y < active, "the member order moved\n{generated}");
    // No key on the member entries: the index is the order.
    assert!(
        !generated.contains("members = {\n    x ="),
        "the members were keyed by name, which has no order\n{generated}"
    );
}

#[test]
fn every_member_carries_the_facts_lua_cannot_state() {
    let generated = generate("struct S { @key @id(7) long x; @optional octet b; };");
    assert!(
        generated.contains(
            "{ name = \"x\", key = \"x\", lua = \"integer\", kind = \"int\", bits = 32, \
             signed = true, wraps = false, optional = false, is_key = true, id = 7 }"
        ),
        "{generated}"
    );
    assert!(
        generated.contains("optional = true"),
        "the optional member is not marked\n{generated}"
    );
}

#[test]
fn every_idl_primitive_reaches_a_lua_holder_and_says_which() {
    // The mapping is total over `PrimitiveKind`; this shows each one
    // produces the holder and the width it should, rather than merely
    // producing something.
    let generated = generate(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n; long double o;
         };",
    );
    for expected in [
        "name = \"a\", key = \"a\", lua = \"boolean\", kind = \"boolean\", bits = 8",
        "name = \"b\", key = \"b\", lua = \"integer\", kind = \"int\", bits = 8, signed = false",
        "name = \"c\", key = \"c\", lua = \"integer\", kind = \"char\", bits = 8",
        "name = \"d\", key = \"d\", lua = \"integer\", kind = \"char\", bits = 16",
        "name = \"e\", key = \"e\", lua = \"integer\", kind = \"int\", bits = 8, signed = true",
        "name = \"g\", key = \"g\", lua = \"integer\", kind = \"int\", bits = 16, signed = true",
        "name = \"i\", key = \"i\", lua = \"integer\", kind = \"int\", bits = 32, signed = true",
        "name = \"k\", key = \"k\", lua = \"integer\", kind = \"int\", bits = 64, signed = true",
        "name = \"m\", key = \"m\", lua = \"number\", kind = \"float\", bits = 32",
        "name = \"n\", key = \"n\", lua = \"number\", kind = \"float\", bits = 64",
        "name = \"o\", key = \"o\", lua = \"string\", kind = \"float\", bits = 128",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn the_one_integer_that_does_not_fit_says_so() {
    // A Lua integer is 64-bit two's complement and wraps:
    // `math.maxinteger + 1 == math.mininteger` is true. So an
    // `unsigned long long` at or above 2^63 reads back negative. The
    // descriptor is where that is written down.
    let generated = generate("struct S { unsigned long long big; long long ok; };");
    assert!(
        generated.contains("bits = 64, signed = false, wraps = true"),
        "{generated}"
    );
    assert!(
        generated.contains("bits = 64, signed = true, wraps = false"),
        "{generated}"
    );
}

#[test]
fn an_unsigned_long_long_constant_is_written_in_hexadecimal() {
    // Lua 5.4 reference §3.1: a decimal numeral that does not fit in an
    // integer denotes a float, and a hexadecimal one wraps around to fit.
    // Measured: `math.type(18446744073709551615)` is `float`, and
    // `math.type(0xFFFFFFFFFFFFFFFF)` is `integer`. So the decimal form
    // would put a rounded float in the file.
    let generated = generate("const unsigned long long BIG = 18446744073709551615;");
    assert!(
        generated.contains("M.BIG = 0xFFFFFFFFFFFFFFFF"),
        "{generated}"
    );
    assert!(
        !generated.contains("18446744073709551615"),
        "the decimal form reached the output\n{generated}"
    );
}

#[test]
fn a_long_double_is_a_string_and_never_a_lua_float() {
    // A Lua float is a C double: 53 bits of significand against
    // binary128's 113. Writing one into a float loses sixty bits at the
    // assignment, before any codec runs.
    let generated = generate("struct S { long double big; };");
    assert!(
        generated.contains("lua = \"string\", kind = \"float\", bits = 128"),
        "{generated}"
    );
    assert!(generated.contains("big = \"0\""), "{generated}");
}

#[test]
fn a_double_default_is_written_so_that_math_type_answers_float() {
    // `0` is an integer in Lua 5.4 and `0.0` is a float (reference §3.1).
    // A `double` member defaulting to `0` would answer `"integer"`.
    let generated = generate("struct S { double d; long n; };");
    assert!(generated.contains("d = 0.0"), "{generated}");
    assert!(generated.contains("n = 0"), "{generated}");
}

#[test]
fn a_string_a_wstring_and_a_fixed_all_land_in_a_lua_string() {
    let generated = generate("struct S { string<7> a; wstring b; fixed<5,2> c; };");
    assert!(
        generated.contains("lua = \"string\", kind = \"string\", bits = 8, bound = 7"),
        "{generated}"
    );
    assert!(
        generated.contains("lua = \"string\", kind = \"string\", bits = 16, bound = 0"),
        "{generated}"
    );
    assert!(
        generated.contains("lua = \"string\", kind = \"decimal\", digits = 5, scale = 2"),
        "{generated}"
    );
    // The `fixed`'s default carries its scale, so the digit count is not
    // decided by whatever writes the value first.
    assert!(generated.contains("c = \"0.00\""), "{generated}");
}

#[test]
fn a_sequence_says_that_it_is_one_based() {
    // A Lua array starts at 1 and an IDL index at 0. Element i of a
    // sequence is t[i + 1], which is the single most likely defect in a
    // program written against this column, so the descriptor says so.
    let generated = generate("struct Bag { sequence<long, 4> ids; };");
    assert!(generated.contains("kind = \"sequence\""), "{generated}");
    assert!(generated.contains("base = 1"), "{generated}");
    assert!(generated.contains("bound = 4"), "{generated}");
    assert!(generated.contains("ids = {}"), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first_and_is_filled() {
    // `long grid[2][3]` is two rows of three. Reversing it silently
    // transposes the data; returning an empty table would return a value
    // of a different shape from the one declared.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(generated.contains("kind = \"array\""), "{generated}");
    assert!(generated.contains("dims = { 2, 3 }"), "{generated}");
    assert!(
        generated.contains("grid = {\n      { 0, 0, 0 },\n      { 0, 0, 0 },\n    }"),
        "{generated}"
    );
}

#[test]
fn a_map_becomes_an_array_of_pairs_and_never_a_lua_table_keyed_by_k() {
    // `pairs` has no defined order (Lua 5.4 reference §6.1) and a map
    // goes on the wire as an ordered sequence of pairs
    // (DDS-XTypes 1.3 §7.4.1.1.4). A Lua table also cannot hold a nil
    // value at all.
    let generated = generate("struct S { map<long, string> table_; };");
    assert!(generated.contains("kind = \"map\""), "{generated}");
    assert!(generated.contains("key_type = {"), "{generated}");
    assert!(generated.contains("value_type = {"), "{generated}");
    assert!(generated.contains("base = 1"), "{generated}");
}

#[test]
fn an_optional_member_gets_no_key_at_all() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7 makes absence the value, and in Lua
    // the absence of a value is the absence of the key. Writing a default
    // would make an absent member indistinguishable from one that arrived
    // with that value.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(generated.contains("return { c = 0 }"), "{generated}");
    assert!(
        generated.contains("optional = true"),
        "the descriptor does not record the absence\n{generated}"
    );
}

#[test]
fn a_member_named_after_a_lua_keyword_keeps_its_name() {
    // The one thing this column has that the other nine do not. Lua's
    // restriction is on the syntax `t.end`, not on the key: `t["end"]`
    // names the key `"end"` (Lua 5.4 reference §3.4.9). So nothing is
    // renamed and the IDL name is the Lua key.
    // `local` and `in` are IDL keywords too, so the pair measured here
    // is `end` and `then`: both are Lua keywords and both are ordinary
    // IDL identifiers.
    let generated = generate("struct S { long end; long then; };");
    assert!(
        generated.contains("name = \"end\", key = \"end\""),
        "{generated}"
    );
    assert!(generated.contains("[\"end\"] = 0"), "{generated}");
    assert!(generated.contains("[\"then\"] = 0"), "{generated}");
    assert!(
        !generated.contains("end_"),
        "a name was escaped, which this column never has to do\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_an_entry_pointing_at_its_target() {
    // Lua has no alias to declare. What is left of a typedef is the
    // entry, and every use takes the target's descriptor.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(generated.contains("kind = \"alias\""), "{generated}");
    assert!(generated.contains("target = {"), "{generated}");
    assert!(generated.contains("alias = \"::Distance\""), "{generated}");
}

#[test]
fn an_enum_writes_both_directions_and_its_values() {
    // `@value` admits gaps, so a position is not a value.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("kind = \"enum\""), "{generated}");
    assert!(
        generated.contains("values = { IDLE = 0, ACTIVE = 4, FAULT = 5 }"),
        "{generated}"
    );
    assert!(
        generated.contains("names = { [0] = \"IDLE\", [4] = \"ACTIVE\", [5] = \"FAULT\" }"),
        "{generated}"
    );
    assert!(
        generated.contains("{ name = \"IDLE\", value = 0 }"),
        "{generated}"
    );
}

#[test]
fn an_enum_defaults_to_its_first_literal_and_not_to_zero() {
    // `@value` admits gaps, so an enumeration need not have a literal
    // with the value 0 at all; a zero default would be a value the type
    // does not have.
    let generated = generate("enum Mode { @value(3) IDLE, ACTIVE }; struct S { Mode m; };");
    assert!(generated.contains("m = 3"), "{generated}");
}

#[test]
fn a_bitmask_writes_the_shift_and_the_holder_width() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("kind = \"bitmask\""), "{generated}");
    assert!(generated.contains("bit_bound = 16"), "{generated}");
    assert!(
        generated.contains("values = { READY = 1 << 0, MOVING = 1 << 4 }"),
        "{generated}"
    );
    assert!(
        generated.contains("{ name = \"MOVING\", position = 4, value = 1 << 4 }"),
        "{generated}"
    );
}

#[test]
fn a_bitset_states_the_declared_bit_width_and_rounds_nothing_up() {
    // Lua has no fixed-width integer, so nothing is rounded up to a
    // holder the way the C and Go columns have to.
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("kind = \"bitset\""), "{generated}");
    assert!(
        generated
            .contains("name = \"low\", key = \"low\", lua = \"integer\", kind = \"int\", bits = 3"),
        "{generated}"
    );
    assert!(generated.contains("bits = 12"), "{generated}");
}

#[test]
fn a_union_has_a_discriminator_and_one_key_per_branch() {
    // Not a tag and a payload: an IDL discriminator may hold a value no
    // branch selects (IDL 4.2 §7.4.1.4.4.1).
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("kind = \"union\""), "{generated}");
    assert!(generated.contains("discriminator = {"), "{generated}");
    assert!(
        generated.contains("name = \"_d\", key = \"_d\""),
        "{generated}"
    );
    assert!(generated.contains("return { _d = 0 }"), "{generated}");
    assert!(generated.contains("name = \"idle\""), "{generated}");
    assert!(generated.contains("name = \"active\""), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_descriptor_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("kind = \"valuetype\""), "{generated}");
    assert!(generated.contains("name = \"counter\""), "{generated}");
    assert!(generated.contains("name = \"secret\""), "{generated}");
}

#[test]
fn a_constant_becomes_a_field_of_the_module_table() {
    let generated = generate("const octet LIMIT = 8; const string TAG = \"abc\";");
    assert!(generated.contains("M.LIMIT = 8"), "{generated}");
    assert!(generated.contains("M.TAG = \"abc\""), "{generated}");
}

#[test]
fn an_exception_becomes_a_descriptor_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("kind = \"exception\""), "{generated}");
    assert!(generated.contains("M.Failure = {"), "{generated}");
    assert!(
        generated.contains("return { code = 0, reason = \"\" }"),
        "{generated}"
    );
}

#[test]
fn an_interface_becomes_a_table_of_operation_descriptors() {
    // Implemented rather than refused: an operation's parameters and its
    // result are a shape, and a shape is what this column carries. What
    // it is not is a call.
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(generated.contains("kind = \"interface\""), "{generated}");
    assert!(generated.contains("name = \"go\","), "{generated}");
    // `results` is an array of nought or one: a `void` operation and one
    // whose result key was never written would otherwise be the same
    // table, because a Lua key set to nil does not exist.
    assert!(generated.contains("results = {}"), "{generated}");
    assert!(generated.contains("name = \"status\""), "{generated}");
}

#[test]
fn a_native_declaration_is_opaque_and_says_so_with_a_value() {
    // `mapping = false` and not nil: a table entry set to nil is an entry
    // that does not exist, so nil could not be told from a key this file
    // never wrote.
    let generated = generate("native Handle;");
    assert!(generated.contains("kind = \"native\""), "{generated}");
    assert!(generated.contains("mapping = false"), "{generated}");
}

#[test]
fn a_module_nests_rather_than_flattening_onto_the_name() {
    // A Lua table key is a string and any string will do, so the IDL
    // scope survives whole. The Go column has to flatten because a Go
    // package is a directory, and `Common_Util_Location` cannot be read
    // back to its segments.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(generated.contains("M.common = {}"), "{generated}");
    assert!(generated.contains("M.common.util = {}"), "{generated}");
    assert!(
        generated.contains("M.common.util.Location = {"),
        "{generated}"
    );
    assert!(
        generated.contains("M._descriptors[\"::common::util::Location\"]"),
        "{generated}"
    );
}

#[test]
fn a_luacats_block_is_written_above_each_type_and_is_only_a_comment() {
    // `lua-language-server` reads it. Nothing in the Lua runtime does,
    // and this test is where that is stated rather than assumed.
    let generated = generate("struct Pose { long x; double y; @optional string tag; };");
    assert!(generated.contains("---@class Pose"), "{generated}");
    assert!(generated.contains("---@field x integer"), "{generated}");
    assert!(generated.contains("---@field y number"), "{generated}");
    assert!(generated.contains("---@field tag? string"), "{generated}");
    // Every line mentioning a LuaCATS tag is a comment. Nothing in the
    // Lua runtime reads one, and this is where that is stated rather
    // than assumed.
    for line in generated
        .lines()
        .filter(|line| line.contains("@class") || line.contains("@field"))
    {
        assert!(
            line.trim_start().starts_with("--"),
            "an annotation escaped its comment: {line}"
        );
    }
}

#[test]
fn a_named_reference_is_resolved_at_load_and_not_left_to_a_reader() {
    // Reading a missing key in Lua answers nil and carries on, so a
    // descriptor naming a type this module does not declare would be
    // found by whatever read it next.
    let generated = generate("struct Point { long x; }; struct Pose { Point p; };");
    assert!(generated.contains("ref = \"::Point\""), "{generated}");
    assert!(generated.contains("M._resolved = {"), "{generated}");
    assert!(
        generated.contains("assert(M._descriptors[\"::Point\"]"),
        "{generated}"
    );
}

#[test]
fn the_chunk_returns_the_module_table() {
    // A Lua module is what its chunk returns (Lua 5.4 reference §6.3).
    // A chunk that returned nothing would load, run, and hand back
    // `true`.
    let generated = generate("struct S { long a; };");
    assert!(generated.trim_end().ends_with("return M"), "{generated}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Lua form. What is left is the
    // preamble, the empty registry and the return.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(generated.contains("local M = {}"), "{generated}");
    assert!(generated.contains("M._names = {}"), "{generated}");
    assert!(!generated.contains("kind = \"struct\""), "{generated}");
}

#[test]
fn a_member_of_a_type_lua_would_happily_hold_is_refused_by_name() {
    // Lua would hold an `any` as readily as anything else, which is
    // exactly why it is refused: a table entry with no wire form behind
    // it, from a generator that reported success, is the
    // silent-substitution class this project is arranged against.
    let error = try_generate("struct S { any payload; };", "any.idl")
        .expect_err("an `any` member was accepted");
    assert!(error.contains("materialise:"), "{error}");
    assert!(error.contains("Lua has no form for"), "{error}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("M.First = {").expect("First");
    let second = generated.find("M.Second = {").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_column_claims_produces_output() {
    let sources = [
        ("struct S { long a; };", "kind = \"struct\""),
        ("enum E { A };", "kind = \"enum\""),
        ("typedef long T;", "kind = \"alias\""),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "kind = \"union\"",
        ),
        ("bitmask B { X };", "kind = \"bitmask\""),
        ("bitset T { bitfield<3> a; };", "kind = \"bitset\""),
        ("valuetype V { public long a; };", "kind = \"valuetype\""),
        ("const long N = 1;", "M.N = 1"),
        ("exception E2 { long code; };", "kind = \"exception\""),
        ("interface I { void go(); };", "kind = \"interface\""),
        ("native N2;", "kind = \"native\""),
        ("module m { struct S { long a; }; };", "M.m = {}"),
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

/// How many corpus cases have to reach Lua for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust and Go columns' corpus gates report the
/// same ones — so those are counted and printed rather than failed on
/// here. What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it.
const MUST_RENDER: usize = 350;

/// The names the Lua column itself refuses, each with the reason.
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
    let out = std::env::var("ZD_LUA_OUT").ok();
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
                    std::fs::write(case.join("zd_types.lua"), text).expect("the output file");
                }
            }
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
            }
            (Err(why), None) => {
                // The stage that refused is what decides whose failure it
                // is. `parse` and `lower` are the frontend's and are the
                // same for every column; `apply` and `materialise` are
                // this one's.
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
            "{} corpus case(s) the frontend refuses, so Lua never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Lua; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the interpreter ------------------------------------------------------

/// Whether `lua` is on the path at all.
fn lua_available() -> bool {
    Command::new("lua")
        .arg("-v")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Whether `luac` is on the path at all.
fn luac_available() -> bool {
    Command::new("luac")
        .arg("-v")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A generated chunk written to a file of its own.
fn write_chunk(source: &str, tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zd-lua-{}-{}-{}",
        std::process::id(),
        tag,
        source.len()
    ));
    std::fs::create_dir_all(&dir).expect("the chunk directory");
    let path = dir.join("zd_types.lua");
    std::fs::write(&path, source).expect("the chunk");
    path
}

/// Run `script` against the module the chunk returns, and hand back what
/// it printed.
///
/// The chunk is **executed**, not parsed: `loadfile` compiles it and the
/// call runs it, so every table constructor is built, the trailing local
/// calls every constructor, and the `assert`s in `M._resolved` fire.
fn lua_run(source: &str, tag: &str, script: &str) -> Result<String, String> {
    let path = write_chunk(source, tag);
    let program = format!(
        "local m = assert(loadfile({}))()\n\
         assert(type(m) == \"table\", \"the chunk did not return a table\")\n\
         {script}",
        lua_string(&path.display().to_string())
    );
    let output = Command::new("lua")
        .arg("-e")
        .arg(&program)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
    result
}

/// One generated chunk, loaded and run.
fn lua_loads(source: &str, tag: &str) -> Result<(), String> {
    lua_run(source, tag, "").map(|_| ())
}

/// One generated chunk through `luac -p`, which only parses it.
fn luac_parses(source: &str, tag: &str) -> Result<(), String> {
    let path = write_chunk(source, tag);
    let output = Command::new("luac")
        .arg("-p")
        .arg(&path)
        .output()
        .map_err(|error| error.to_string());
    let result = match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(String::from_utf8_lossy(&output.stderr).into_owned()),
        Err(error) => Err(error),
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
    result
}

/// A Lua string literal holding `text`.
fn lua_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The sources every runtime gate below is measured on.
const RUNTIME_SOURCES: &[&str] = &[
    "struct Pose { long x; double y; boolean active; };",
    "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
     unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
     float m; double n; long double o; };",
    "struct S { @optional octet b; long c; string name; sequence<long> ids; \
     long grid[2][3]; map<long, string> pairs; fixed<5,2> price; };",
    "struct T { long end; long then; long repeat; };",
    "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
     union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
    "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
    "bitset Bits { bitfield<3> low; bitfield<12> high; };",
    "valuetype V { public long counter; private string secret; };",
    "const octet LIMIT = 8; const string TAG = \"abc\"; \
     const unsigned long long BIG = 18446744073709551615;",
    "exception Failure { long code; string reason; };",
    "struct Target { long x; }; interface Control { void go(in Target t); long status(); };",
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
    "struct Point { long x; }; struct Pose2 { Point p; sequence<Point> path; };",
];

#[test]
fn the_generated_lua_loads() {
    if !lua_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the interpreter is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: `lua` is not on the path, so the generated Lua was never run. \
             Set it up or run this on a host that has it."
        );
        return;
    }
    for (index, idl) in RUNTIME_SOURCES.iter().enumerate() {
        let generated = generate(idl);
        if let Err(why) = lua_loads(&generated, &format!("run{index}")) {
            panic!("{idl}\n\n{generated}\n\nlua said:\n{why}");
        }
    }
}

#[test]
fn running_the_chunk_catches_what_parsing_it_does_not() {
    // The reason the gate above runs the chunk instead of calling
    // `luac -p`. This file is syntactically perfect Lua and names a
    // descriptor that is not there — which is the defect a column whose
    // whole content is a descriptor table can actually commit.
    if !lua_available() || !luac_available() {
        eprintln!("SKIPPED: `lua` or `luac` is not on the path.");
        return;
    }
    let broken = "local M = {}\n\
                  M._descriptors = {}\n\
                  M.Pose = { idl = \"::Pose\", kind = \"struct\" }\n\
                  M._descriptors[\"::Pose\"] = M.Pose\n\
                  M._resolved = {\n\
                  \x20 [\"::Point\"] = assert(M._descriptors[\"::Point\"], \
                  \"::Pose names ::Point, which this module does not declare\"),\n\
                  }\n\
                  return M\n";
    luac_parses(broken, "broken-parse").expect("the broken chunk is valid Lua and should parse");
    let why = lua_loads(broken, "broken-run").expect_err("running the broken chunk succeeded");
    assert!(
        why.contains("does not declare"),
        "the failure was not the missing descriptor: {why}"
    );
}

#[test]
fn the_executed_module_reports_its_members_in_idl_declaration_order() {
    // The fact this whole column rests on, measured by running the module
    // rather than by reading the text: a Lua table has no order, so the
    // order is only there if it was written into the array part.
    if !lua_available() {
        eprintln!("SKIPPED: `lua` is not on the path, so the member order was not measured.");
        return;
    }
    let generated = generate(
        "struct Sample { long first; double second; string third; @optional octet fourth; };",
    );
    let script = "local d = assert(m._descriptors[\"::Sample\"])\n\
                  local out = {}\n\
                  for i = 1, #d.members do out[#out + 1] = d.members[i].name end\n\
                  print(table.concat(out, \",\"))\n";
    let printed = lua_run(&generated, "order", script).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(printed.trim(), "first,second,third,fourth");
}

#[test]
fn the_executed_module_reports_the_widths_lua_itself_cannot() {
    // `math.type` is the whole of Lua's type information about a number.
    // Everything else — the width, the signedness, the bound — is in the
    // descriptor, and this reads it back out of a running module.
    if !lua_available() {
        eprintln!("SKIPPED: `lua` is not on the path, so the descriptor was not read back.");
        return;
    }
    let generated =
        generate("struct W { octet small; unsigned long long big; double d; string<7> s; };");
    let script = "local d = assert(m._descriptors[\"::W\"])\n\
                  local by = {}\n\
                  for i = 1, #d.members do by[d.members[i].name] = d.members[i] end\n\
                  assert(by.small.bits == 8 and by.small.signed == false)\n\
                  assert(by.big.bits == 64 and by.big.signed == false and by.big.wraps == true)\n\
                  assert(by.d.lua == \"number\" and by.d.bits == 64)\n\
                  assert(by.s.bound == 7)\n\
                  local v = m.W.new()\n\
                  print(math.type(v.small), math.type(v.d), type(v.s))\n";
    let printed = lua_run(&generated, "widths", script).unwrap_or_else(|why| panic!("{why}"));
    // The one thing Lua does distinguish, and the reason a `double`
    // default is written `0.0` and not `0`.
    assert_eq!(printed.trim(), "integer\tfloat\tstring");
}

#[test]
fn the_executed_module_reaches_a_member_named_after_a_lua_keyword() {
    // `t.end` does not parse and `t["end"]` does, and both name the key
    // `"end"`. Measured, because it is the claim that this column renames
    // nothing.
    if !lua_available() {
        eprintln!("SKIPPED: `lua` is not on the path.");
        return;
    }
    let generated = generate("struct K { long end; long payload; };");
    let script = "local v = m.K.new()\n\
                  print(v[\"end\"], v.payload)\n\
                  local d = assert(m._descriptors[\"::K\"])\n\
                  print(d.members[1].name, d.members[1].key)\n";
    let printed = lua_run(&generated, "keyword", script).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(printed.trim(), "0\t0\nend\tend");
}

/// The whole corpus, run one chunk at a time.
///
/// `#[ignore]`, because it is one process per case and takes minutes. It
/// is what proves the column rather than samples it, so it is run with
/// `--include-ignored` and its result is what
/// `docs/language-columns-status.md` records.
#[test]
#[ignore = "one `lua` process per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_loads() {
    assert!(
        lua_available(),
        "the explicitly selected lua gate requires its compiler"
    );
    let mut failures = Vec::new();
    let mut loaded = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = try_generate(&source, &path.display().to_string())
            .unwrap_or_else(|why| panic!("accepted corpus case {name} failed generation: {why}"));
        match lua_loads(&text, &name) {
            Ok(()) => loaded += 1,
            Err(why) => failures.push(format!("{name}: {why}")),
        }
    }
    eprintln!("{loaded} corpus case(s) loaded under `lua`");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not load:\n{failures:#?}",
        failures.len()
    );
    assert!(
        loaded >= MUST_RENDER,
        "too few corpus cases reached the selected gate: {loaded}"
    );
}
