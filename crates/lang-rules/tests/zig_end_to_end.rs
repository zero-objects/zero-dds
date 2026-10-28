// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Zig, through every layer of the rebuild.
//!
//! The counterpart of `end_to_end.rs`, which does the same for Rust, of
//! `c_end_to_end.rs`, which does it for C, and of `go_end_to_end.rs`,
//! which does it for Go.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//!    A case that stopped rendering would otherwise be found by whichever
//!    probe ran next, which is a build directory away from the construct
//!    that caused it.
//! 2. **The generated Zig compiles**, under the real compiler. A
//!    snapshot says the emitter produced the text it produced last time;
//!    it does not say the text is Zig, that every name in it resolves,
//!    or that a field's type exists.
//!
//! ## What "compiles" means here, and why `zig build-obj` is not it
//!
//! Zig analyses a container's declarations only when something reaches
//! them. Measured on Zig 0.14.1: a file holding nothing but
//! `pub const X: u8 = 300;` passes `zig build-obj` with status 0 and no
//! output. A gate built on `build-obj` would therefore be green for a
//! generated types file that no compilation imports — which is every
//! generated types file at the moment it is generated.
//!
//! The check is [`zig_checks`]: `zig ast-check` for the grammar, then
//! `zig test --test-no-exec -fno-emit-bin` for the semantics. The second
//! reaches every declaration because the generated file ends in the
//! `test` block that references them all, and `--test-no-exec` stops
//! before the link step — which on macOS fails for reasons that have
//! nothing to do with the file under test. That the pair is not vacuous
//! is itself a test: `the_zig_check_catches_what_build_obj_misses`.
//!
//! `print_stderr` is allowed here on purpose, as it is in the sibling
//! columns' harnesses. A skipped compiler check and a corpus case the
//! frontend refused both have to be said out loud on a channel a passing
//! test does not use; a harness that swallowed either would be green for
//! having run nothing, which is the failure these tests exist to catch.
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
use std::sync::atomic::{AtomicU32, Ordering};
use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::zig::zig_file;
use zerodds_lang_rules::rule::zig;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Zig out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    // The same step the C, C++, TypeScript and Go columns take: a member
    // whose type is written as a scoped name arrives as an unresolved
    // reference, and a materialiser that asked `resolved()` without this
    // would report "the member type does not evaluate" for every
    // cross-module reference in the corpus.
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &zig::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = zig_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::zig::file(&file))
}

/// The generated text with every comment line taken out.
///
/// Every "this shape did **not** reach the output" assertion below runs
/// on this and not on the whole file, because this column's
/// documentation names the shapes it refused — `union(enum)`,
/// `packed struct`, `callconv`, `std.AutoHashMap` — and a substring
/// search over the comments would find all four in a file that contains
/// none of them.
fn code_only(generated: &str) -> String {
    generated
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n")
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_a_zig_struct() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(
        generated.contains("pub const Pose = struct {"),
        "{generated}"
    );
    assert!(generated.contains("    x: i32,"), "{generated}");
    assert!(generated.contains("    y: f64,"), "{generated}");
    assert!(generated.contains("    active: bool,"), "{generated}");
}

#[test]
fn every_idl_primitive_reaches_a_zig_type() {
    // The mapping is total over `PrimitiveKind`; this shows it produces
    // the type each one should, rather than merely producing something.
    let generated = generate(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n; long double o;
         };",
    );
    for expected in [
        "a: bool,", "b: u8,", "c: u8,", "d: u16,", "e: i8,", "f: u8,", "g: i16,", "h: u16,",
        "i: i32,", "j: u32,", "k: i64,", "l: u64,", "m: f32,", "n: f64,", "o: f128,",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn no_generated_type_has_a_target_dependent_width() {
    // `isize`, `usize` and the `c_*` family follow the target (Zig
    // language reference, "Primitive Types"). A member declared with one
    // would change size between a 64-bit host and a 32-bit
    // microcontroller, for a wire format that fixes every width.
    let generated = generate(
        "struct P { long a; unsigned long b; long long c; sequence<long> d; long e[2]; \
         string f; };",
    );
    for forbidden in [
        ": isize", ": usize", ": c_int", ": c_long", "[]isize", "[]usize",
    ] {
        assert!(
            !generated.contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn a_long_double_is_an_f128_because_zig_has_one() {
    // The one place this column differs from Go's and Python's, which
    // hold the decimal in a string: Zig's `f128` is a real IEEE 754
    // quadruple with the 113 bits of significand an IDL `long double`
    // has, so nothing is lost at the assignment and no text form is
    // needed.
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("big: f128,"), "{generated}");
    assert!(
        !generated.contains("big: []const u8"),
        "the decimal workaround reached a language that does not need it\n{generated}"
    );
}

#[test]
fn a_long_double_constant_is_written_as_a_number_and_not_as_text() {
    // The Go column refuses this constant by name, because its holder is
    // a string and CORBA gives a `long double` literal no spelling that
    // survives a `float64`. Zig's `comptime_float` is evaluated at f128,
    // so the literal is simply the number.
    let generated = generate("const long double BIG = 1.5;");
    assert!(
        generated.contains("pub const BIG: f128 = 1.5;"),
        "{generated}"
    );
}

#[test]
fn an_optional_member_becomes_a_question_mark_and_defaults_to_null() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. Zig's zero value is whatever the
    // constructor wrote, and a `long` that arrived as 0 is not
    // distinguishable from one that did not arrive unless the type says
    // so; `null` is the one state no value of `i32` can take.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(generated.contains("b: ?u8 = null,"), "{generated}");
    assert!(generated.contains("c: i32,"), "{generated}");
    assert!(!generated.contains("c: ?i32"), "{generated}");
}

#[test]
fn an_external_member_becomes_a_pointer_and_the_optional_goes_outside_it() {
    // `@external` says the member is stored outside the containing
    // object (DDS-XTypes 1.3 Table 21), which in Zig is a pointer and
    // needs no invented box type. The pointer goes on first and the
    // optional outside it, which is the shape that both breaks the
    // recursion and can say "absent".
    //
    // Found by the Zig compiler and by nothing else: without the
    // pointer, `next: ?m_Node` is a struct that contains itself and
    // `zig test` says `struct 'm_Node' depends on itself`. The tree is
    // finite either way, so no Rust assertion would have noticed.
    let generated = generate(
        "module m {
           struct Node;
           @mutable struct Node { long value; @optional @external Node next; };
         };",
    );
    assert!(generated.contains("next: ?*m_Node = null,"), "{generated}");
    assert!(
        !code_only(&generated).contains("next: ?m_Node"),
        "the pointer was dropped\n{generated}"
    );

    // `@external` on its own is the pointer and nothing else: the
    // member is always present and merely held by reference.
    let plain = generate("struct Inner { long a; }; struct S { @external Inner held; };");
    assert!(plain.contains("held: *Inner,"), "{plain}");
}

#[test]
fn a_member_named_after_a_zig_keyword_is_escaped_and_keeps_its_idl_name() {
    // A keyword cannot be an identifier anywhere: measured on Zig
    // 0.14.1, `struct { align: u8 }` is
    // `expected type expression, found 'align'`.
    let generated = generate("struct S { long align; long error; long try; };");
    for (spelled, idl) in [("align_", "align"), ("error_", "error"), ("try_", "try")] {
        assert!(
            generated.contains(&format!("{spelled}: i32,")),
            "missing {spelled}\n{generated}"
        );
        assert!(
            generated.contains(&format!("IDL: `{idl}`.")),
            "the IDL name of {idl} was not recorded\n{generated}"
        );
    }
}

#[test]
fn a_member_named_after_a_primitive_keeps_the_idl_spelling() {
    // Measured on Zig 0.14.1: `struct { type: u8, u8: i32, bool: u8 }`
    // compiles. A field name lives in the container's field namespace,
    // which the primitive types are not in, so escaping here would move
    // a member's spelling for a collision the language does not have.
    let generated = generate("struct S { long type; long u8; long bool; };");
    assert!(generated.contains("type: i32,"), "{generated}");
    assert!(generated.contains("u8: i32,"), "{generated}");
    assert!(generated.contains("bool: i32,"), "{generated}");
    assert!(
        !generated.contains("type_: i32"),
        "a field was escaped that Zig accepts\n{generated}"
    );
}

#[test]
fn a_declaration_named_after_a_primitive_is_escaped_because_zig_refuses_it() {
    // The other half of the same rule. `pub const u8 = i32;` is
    // `error: name shadows primitive 'u8'` — unlike Go, where shadowing
    // a predeclared identifier compiles and is merely unwise.
    let generated = generate("typedef long u8; typedef long type;");
    assert!(generated.contains("pub const u8_ = i32;"), "{generated}");
    assert!(generated.contains("pub const type_ = i32;"), "{generated}");
    assert!(generated.contains("IDL: `u8`."), "{generated}");
}

#[test]
fn two_members_that_would_reach_one_zig_name_are_refused_by_name() {
    // Not the case-only pair — IDL 4.2 §7.2.3.1 already forbids `value`
    // beside `Value` in one scope, and the frontend reports it. What is
    // left is the pair the keyword escape makes: `align` escapes to
    // `align_`, and a member already spelled `align_` is the same name.
    // Zig would say `duplicate struct member name`, naming a Zig
    // identifier and not the two IDL members behind it.
    let error = try_generate("struct S { long align; long align_; };", "collide.idl")
        .expect_err("the collision was not reported");
    assert!(error.contains("align_"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_sequence_becomes_a_slice_and_a_string_a_const_slice_of_octets() {
    let generated = generate("struct Bag { sequence<long> ids; string name; wstring wide; };");
    assert!(generated.contains("ids: []i32,"), "{generated}");
    assert!(generated.contains("name: []const u8,"), "{generated}");
    // A `wstring` holds UTF-8, which keeps the §7.4.1.1.2 surrogate rule
    // inside the runtime rather than copying another column's answer.
    assert!(generated.contains("wide: []const u8,"), "{generated}");
}

#[test]
fn a_fixed_holds_its_decimal_as_text() {
    let generated = generate("struct S { fixed<9,2> price; };");
    assert!(generated.contains("price: []const u8,"), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first() {
    // `long grid[2][3]` is two rows of three, which in Zig is
    // `[2][3]i32`. Reversing it silently transposes the data.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(generated.contains("grid: [2][3]i32,"), "{generated}");
}

#[test]
fn a_map_becomes_a_slice_of_entries_and_never_a_hash_map() {
    // A `map<K,V>` goes on the wire as an ordered sequence of pairs
    // (DDS-XTypes 1.3 §7.4.1.1.4). `std.AutoHashMap` iterates in bucket
    // order, which follows the hash and the insertion history, so one
    // value would produce different octets depending on how it was
    // built.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("table: []ZdMapEntry_long_string,"),
        "{generated}"
    );
    assert!(
        generated.contains("pub const ZdMapEntry_long_string = struct {"),
        "{generated}"
    );
    assert!(generated.contains("    key: i32,"), "{generated}");
    assert!(generated.contains("    value: []const u8,"), "{generated}");
    assert!(
        !code_only(&generated).contains("HashMap"),
        "a hash map reached the output\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_declaration_binding_the_same_type() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type". A Zig
    // declaration binds a name to a type value and creates none, so the
    // one declaration form is already the right one — where Go needs
    // `type A = B` to avoid declaring a second type.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(
        generated.contains("pub const Distance = i32;"),
        "{generated}"
    );
    assert!(generated.contains("d: Distance,"), "{generated}");
}

#[test]
fn an_enum_holds_its_variants_with_every_value_written_out() {
    // Not Zig's own numbering: it would number an unvalued variant
    // consecutively from zero and `@value` admits gaps, so leaving the
    // values out would renumber the wire.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(
        generated.contains("pub const Mode = enum(i32) {"),
        "{generated}"
    );
    assert!(generated.contains("    IDLE = 0,"), "{generated}");
    assert!(generated.contains("    ACTIVE = 4,"), "{generated}");
    assert!(generated.contains("    FAULT = 5,"), "{generated}");
}

#[test]
fn a_union_becomes_a_struct_with_a_discriminator_and_one_field_per_branch() {
    // Not a Zig `union(enum)`. A tagged union's tag is its active field,
    // and an IDL discriminator may take a value no case label selects
    // (IDL 4.2 §7.4.1.4.4.2.3), in which state no branch is live and
    // only the discriminator goes on the wire
    // (DDS-XTypes 1.3 §7.4.1.1.5.2).
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(
        generated.contains("pub const Payload = struct {"),
        "{generated}"
    );
    assert!(generated.contains("    _d: Mode,"), "{generated}");
    assert!(generated.contains("    idle: i32,"), "{generated}");
    assert!(generated.contains("    active: f64,"), "{generated}");
    assert!(
        !code_only(&generated).contains("union(enum)"),
        "a tagged union reached the output\n{generated}"
    );
}

/// An `@external` union branch takes the same pointer a structure's
/// `@external` member does — the branch declaration routes through the
/// same `member_field`, and the mirror in `idl-lower` is what lets it
/// see the annotation at all.
#[test]
fn an_external_union_branch_becomes_a_pointer() {
    let generated = generate(
        "struct Inner { long a; };
         union Payload switch (long) {
           case 1: @external Inner nested;
           default: long other;
         };",
    );
    assert!(generated.contains("nested: *Inner,"), "{generated}");
    assert!(
        !code_only(&generated).contains("nested: Inner,"),
        "the pointer was dropped on the branch\n{generated}"
    );
}

#[test]
fn a_bitmask_becomes_an_unsigned_integer_and_one_constant_per_bit() {
    // Not a `packed struct` of bools: that lays its fields out in
    // declaration order from the backing integer's least significant
    // bit, and a bit's place is its `@position`
    // (DDS-XTypes 1.3 §7.4.1.1.3.2), which admits gaps and any order.
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("pub const Flags = u16;"), "{generated}");
    assert!(
        generated.contains("pub const READY: Flags = 1 << 0;"),
        "{generated}"
    );
    assert!(
        generated.contains("pub const MOVING: Flags = 1 << 4;"),
        "{generated}"
    );
    assert!(
        !code_only(&generated).contains("packed struct"),
        "a packed struct reached the output\n{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_struct_of_holder_typed_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("pub const B = struct {"), "{generated}");
    assert!(generated.contains("    low: u8,"), "{generated}");
    assert!(generated.contains("    high: u16,"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_struct_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("pub const V = struct {"), "{generated}");
    assert!(generated.contains("    counter: i32,"), "{generated}");
    assert!(generated.contains("    secret: []const u8,"), "{generated}");
}

#[test]
fn a_constant_becomes_a_declaration_that_ascribes_its_type() {
    // The ascription is not decoration: an unascribed
    // `pub const LIMIT = 8;` is a `comptime_int` and takes whatever
    // width its first use wants.
    let generated = generate("const octet LIMIT = 8; const string TAG = \"abc\";");
    assert!(
        generated.contains("pub const LIMIT: u8 = 8;"),
        "{generated}"
    );
    assert!(
        generated.contains("pub const TAG: []const u8 = \"abc\";"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_struct_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(
        generated.contains("pub const Failure = struct {"),
        "{generated}"
    );
    assert!(generated.contains("    code: i32,"), "{generated}");
    assert!(generated.contains("    reason: []const u8,"), "{generated}");
}

#[test]
fn an_interface_becomes_a_struct_of_function_pointers() {
    // Covered here and not in the C column: Zig has a function-pointer
    // type that carries its parameter names, and the C tree has none —
    // in C the name sits inside the declarator, which that tree cannot
    // express (`coverage.rs`).
    //
    // No `callconv` is written. Measured on Zig 0.14.1, a
    // `callconv(.C)` function refuses a parameter of a generated
    // aggregate: "only extern structs and ABI sized packed structs are
    // extern compatible".
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void align(in Pose target);
           long status();
         };",
    );
    assert!(
        generated.contains("pub const Control = struct {"),
        "{generated}"
    );
    // `align` is a Zig keyword, so the operation escapes.
    assert!(
        generated.contains("align_: *const fn (target: Pose) void,"),
        "{generated}"
    );
    assert!(
        generated.contains("status: *const fn () i32,"),
        "{generated}"
    );
    assert!(
        !code_only(&generated).contains("callconv"),
        "a calling convention reached the output\n{generated}"
    );
}

#[test]
fn a_native_declaration_becomes_an_opaque_pointer() {
    // `anyopaque` itself cannot be a member's type — Zig gives it no
    // size — so the pointer is the declaration.
    let generated = generate("native Handle;");
    assert!(
        generated.contains("pub const Handle = *anyopaque;"),
        "{generated}"
    );
}

#[test]
fn a_module_flattens_onto_the_name_because_a_zig_file_does_not_nest() {
    // A Zig file is a struct and does not nest. Nothing is capitalised
    // on the way: Zig's visibility is the `pub` keyword and not the
    // first letter, so a rename here would be a rename with no cause.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("pub const common_util_Location = struct {"),
        "{generated}"
    );
    assert!(
        generated.contains("IDL: `Location`."),
        "the IDL name was not recorded\n{generated}"
    );
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Zig equivalent. What is left is
    // the file comment and the block that forces analysis.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(
        generated.contains("//! Generated by ZeroDDS"),
        "{generated}"
    );
    assert!(!generated.contains("pub const Sensitive"), "{generated}");
}

#[test]
fn every_declaration_is_pub_because_a_types_file_exists_to_be_imported() {
    let generated = generate(
        "module m { struct S { long a; }; enum E { A }; const long N = 1; };
         native H; typedef long T; bitmask B { X };",
    );
    for line in generated.lines() {
        if line.starts_with("const ") {
            panic!("a private declaration would have no consumer:\n{line}\n\n{generated}");
        }
    }
    assert!(
        generated.contains("pub const m_S = struct {"),
        "{generated}"
    );
}

#[test]
fn the_file_ends_in_the_block_that_forces_zig_to_analyse_it() {
    // Without it, `zig build-obj` over a generated types file reports
    // success for a file whose declarations it never looked at.
    let generated = generate("struct S { long a; };");
    assert!(generated.contains("test {"), "{generated}");
    // What the block has to do, not how it is spelled. It used to call
    // `std.testing.refAllDeclsRecursive`, which Zig 0.16 removed; the
    // walk is written out now. Asserting the old call by name pinned a
    // library function rather than the property, and the property is
    // that every declaration is reached — nested ones included, because
    // that is where a codec puts a union's arms and a map's entry type.
    assert!(
        generated.contains("meta.declarations(zd_T)"),
        "the block does not walk the declarations\n{generated}"
    );
    assert!(
        generated.contains("=> all(zd_field)"),
        "the walk does not descend into nested containers\n{generated}"
    );
    assert!(
        generated.trim_end().ends_with('}'),
        "the block is not last\n{generated}"
    );
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("pub const First").expect("First");
    let second = generated.find("pub const Second").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_catalogue_covers_produces_output() {
    // The catalogue says a rule covers it; this says the rule reaches
    // text.
    let sources = [
        ("struct S { long a; };", "pub const S = struct"),
        ("enum E { A };", "pub const E = enum(i32)"),
        ("typedef long T;", "pub const T = i32;"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "pub const U = struct",
        ),
        ("bitmask B { X };", "pub const B = u32;"),
        ("bitset T { bitfield<3> a; };", "pub const T = struct"),
        ("valuetype V { public long a; };", "pub const V = struct"),
        ("const long N = 1;", "pub const N: i32 = 1;"),
        ("exception E2 { long code; };", "pub const E2 = struct"),
        ("interface I { void ping(); };", "pub const I = struct"),
        ("native N2;", "pub const N2 = *anyopaque;"),
        (
            "module m { struct S { long a; }; };",
            "pub const m_S = struct",
        ),
        ("struct S { map<long,long> t; };", "pub const ZdMapEntry_"),
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

/// How many corpus cases have to reach Zig for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it: the corpus is 378
/// cases and the column has to carry substantially all of them.
const MUST_RENDER: usize = 350;

/// The names the Zig column itself refuses, each with the reason.
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
    let out = std::env::var("ZD_ZIG_OUT").ok();
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
                    std::fs::write(case.join("zd_types.zig"), text).expect("the output file");
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
            "{} corpus case(s) the frontend refuses, so Zig never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Zig; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// A counter, so two checks running at once do not share a directory.
static SERIAL: AtomicU32 = AtomicU32::new(0);

/// A scratch directory of this process's own, removed when the check is
/// done.
fn scratch() -> std::path::PathBuf {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("zd-zig-check-{}-{serial}", std::process::id()))
}

/// One generated file through the real compiler.
///
/// Two invocations, because neither alone is the check:
///
/// - `zig ast-check` parses and resolves identifiers. It reports an
///   undeclared name and a keyword used as a field, and it reports
///   nothing about a value that does not fit its type.
/// - `zig test --test-no-exec -fno-emit-bin` runs semantic analysis over
///   everything the file's `test` block reaches, which is every
///   declaration in it. `--test-no-exec` stops before the link, which on
///   macOS fails for want of libc symbols that have nothing to do with
///   the file under test; `-fno-emit-bin` stops before the object is
///   written.
///
/// `zig build-obj` is deliberately not used: Zig analyses container-level
/// declarations lazily, so a types file nothing references passes it
/// untouched. `the_zig_check_catches_what_build_obj_misses` measures
/// that.
fn zig_checks(source: &str) -> Result<(), String> {
    let dir = scratch();
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("zd_types.zig");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let result = run_zig(&["ast-check"], &file)
        .and_then(|()| run_zig(&["test", "--test-no-exec", "-fno-emit-bin"], &file));
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn run_zig(args: &[&str], file: &std::path::Path) -> Result<(), String> {
    let output = Command::new("zig")
        .args(args)
        .arg(file)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "zig {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// Whether `zig` is on the path at all.
fn zig_available() -> bool {
    Command::new("zig")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn the_zig_check_catches_what_build_obj_misses() {
    // A gate nobody has seen fail is a gate nobody should trust. Two
    // deliberate errors:
    //
    //  - one the grammar catches, so `ast-check` has something to do;
    //  - one only semantic analysis catches, and which `zig build-obj`
    //    over the same file reports success for — which is why this
    //    column's check is `zig test` and not `build-obj`.
    if !zig_available() {
        eprintln!("SKIPPED: `zig` is not on the path.");
        return;
    }

    let syntax_error = "pub const S = struct {\n    align: u8,\n};\n";
    assert!(
        zig_checks(syntax_error).is_err(),
        "a keyword used as a field passed the check"
    );

    // The same block the generator emits, because the point of the case
    // is that the block is what reaches the declaration. A different
    // block here would test a file this column never produces — and one
    // calling `std.testing.refAllDeclsRecursive`, as this did, stopped
    // reaching anything at all when Zig 0.16 removed that function: the
    // check then refused for the missing function instead of for the
    // value that does not fit, and said so.
    let semantic_error = concat!(
        "pub const X: u8 = 300;\n",
        "test {\n",
        "    const zd_walk = struct {\n",
        "        fn all(comptime zd_T: type) void {\n",
        "            inline for (comptime @import(\"std\").meta.declarations(zd_T)) |zd_decl| {\n",
        "                const zd_field = @field(zd_T, zd_decl.name);\n",
        "                if (@TypeOf(zd_field) == type) {\n",
        "                    switch (@typeInfo(zd_field)) {\n",
        "                        .@\"struct\", .@\"enum\", .@\"union\", .@\"opaque\" => all(zd_field),\n",
        "                        else => {},\n",
        "                    }\n",
        "                }\n",
        "                _ = &zd_field;\n",
        "            }\n",
        "        }\n",
        "    }.all;\n",
        "    zd_walk(@This());\n",
        "}\n"
    );
    let refused = zig_checks(semantic_error).expect_err("a value that does not fit passed");
    assert!(
        refused.contains("cannot represent integer value"),
        "the check refused for the wrong reason: {refused}"
    );

    // And the measurement that decided the harness: the same file, under
    // the command a first attempt would have reached for.
    let dir = scratch();
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    let file = dir.join("lazy.zig");
    std::fs::write(&file, "pub const X: u8 = 300;\n").expect("the scratch file");
    let lazy = Command::new("zig")
        .arg("build-obj")
        .arg(&file)
        .arg("-fno-emit-bin")
        .output()
        .expect("zig build-obj");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        lazy.status.success(),
        "`zig build-obj` has stopped being lazy; the harness note is stale:\n{}",
        String::from_utf8_lossy(&lazy.stderr)
    );
}

/// Every shape above, through the compiler.
///
/// One file per source rather than one concatenated file: two sources
/// here declare a `Pose` and a `T`, and Zig allows one declaration per
/// name in a container.
#[test]
fn the_generated_zig_compiles() {
    if !zig_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: `zig` is not on the path, so the generated Zig was not compiled. \
             Set it up or run this on a host that has it."
        );
        return;
    }
    let sources = [
        "struct Pose { long x; double y; boolean active; };",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
        "struct S { @optional octet b; long c; string name; wstring wide; \
         sequence<long> ids; long grid[2][3]; map<long, string> table; fixed<9,2> price; };",
        "struct T { long align; long error; long try; long type; long u8; };",
        "typedef long u8; typedef long type; typedef long align;",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "@bit_bound(64) bitmask Wide { @position(63) TOP };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const long double BIG = 1.5; \
         const float NARROW = .5; const fixed PRICE = 1.25d; const boolean ON = TRUE;",
        "exception Failure { long code; string reason; };",
        "struct Target { long x; }; interface Control { void align(in Target t); long status(); };",
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
        "@annotation Sensitive { string reason; };",
    ];
    for idl in sources {
        let generated = generate(idl);
        if let Err(why) = zig_checks(&generated) {
            panic!("{idl}\n\n{generated}\n\n{why}");
        }
    }
}

/// The generated text is what `zig fmt` would have written.
///
/// Not a style preference: `zig fmt` ships with the compiler and every
/// reader of a generated file runs it, so a file that moves under it is
/// a diff nobody asked for.
#[test]
fn the_generated_zig_is_already_formatted() {
    if !zig_available() {
        eprintln!("SKIPPED: `zig` is not on the path, so the layout was not checked.");
        return;
    }
    let sources = [
        "struct Pose { long x; double y; };",
        "enum Mode { IDLE, @value(4) ACTIVE };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "struct S { @optional octet b; map<long, string> t; long grid[2][3]; };",
        "struct Target { long x; }; interface Control { void ping(in Target t); long status(); };",
    ];
    for idl in sources {
        let generated = generate(idl);
        let dir = scratch();
        std::fs::create_dir_all(&dir).expect("the scratch directory");
        let file = dir.join("zd_types.zig");
        std::fs::write(&file, &generated).expect("the scratch file");
        let output = Command::new("zig")
            .arg("fmt")
            .arg("--check")
            .arg(&file)
            .output()
            .expect("zig fmt");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            output.status.success(),
            "{idl}\n\n{generated}\n\nzig fmt would rewrite it:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

/// The whole corpus through the compiler, one file per case.
///
/// `#[ignore]`, because it is 378 pairs of compiler invocations and
/// takes minutes. It is what proves the column rather than samples it,
/// so it is run with `--include-ignored` on a host that has Zig and its
/// result is what `docs/language-columns-status.md` records.
#[test]
#[ignore = "378 pairs of `zig` invocations; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        zig_available(),
        "the explicitly selected zig gate requires its compiler"
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
        if let Err(why) = zig_checks(&text) {
            failures.push(format!("{name}: {why}"));
        }
    }
    eprintln!("{checked} corpus case(s) went through the Zig compiler");
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

/// A type handle the model will not hand over is refused by name, not
/// answered with "this type has no Zig data form".
///
/// `body_of` says `Ok(None)` for a construct Nim cannot hold, and
/// `zig_defaulters_over` reads that as a reason to emit no defaulter and
/// walk on. A stale handle used to take the same door: the defaulter the
/// closure asked for was simply not in the batch, while the decoder that
/// calls it by name still was, and the omission surfaced — if at all —
/// as an undeclared identifier from a compiler further down the chain.
#[test]
fn a_type_the_model_does_not_hold_refuses_rather_than_going_missing() {
    let mut ast = one_struct_model();
    let stale = take_and_remove(&mut ast);
    let error = zerodds_lang_rules::default_zig::zig_defaulters_over(&ast, &[stale])
        .expect_err("a handle the model does not hold has no Table 9 value");
    let text = format!("{error:?}");
    assert!(
        text.contains("a type the model does not hold"),
        "the refusal does not name the case: {text}"
    );
}

/// A model holding `@final struct Point { long x; };`.
fn one_struct_model() -> Ast {
    let parsed = zerodds_idl_parse::parse("@final struct Point { long x; };");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    ast
}

/// `Point`'s handle, with `Point` taken back out of the model, so the
/// handle is one the arena will not answer for.
fn take_and_remove(ast: &mut Ast) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    let id = ast
        .types()
        .find(|(_, node)| node.name.as_str() == "Point")
        .map(|(id, _)| id)
        .expect("Point");
    let any = ast.any_of_type(id).expect("Point's identity");
    ast.remove(any).expect("remove Point");
    assert!(ast.type_(id).is_err(), "the handle is still answered");
    id
}

/// A bound is part of the hoisted codec routine's name.
///
/// `type_tag` names the routine a collection's codec is emitted under
/// and `decoder_over` emits one body per name, so a tag that dropped
/// the bound put `sequence<long>` and `sequence<long, 4>` on one
/// routine and kept whichever body came first. Measured on the parent
/// commit: with the unbounded member declared first the generated Zig
/// held no `zd_over` check at all, so the bounded member accepted any
/// length; with the bounded member first both got the bounded body,
/// which refuses and truncates a length the IDL puts no limit on.
/// `string` beside `string<4>` behaved the same way through `trimUtf8`.
#[test]
fn a_bounded_and_an_unbounded_collection_get_their_own_codec_routine() {
    for idl in [
        "@final struct S { sequence<long> b; sequence<long, 4> a; };",
        "@final struct S { sequence<long, 4> a; sequence<long> b; };",
    ] {
        let out = decoder_text(idl, "S");
        assert!(out.contains("fn zd_dec_seq_long("), "{idl}\n{out}");
        assert!(out.contains("fn zd_dec_seq_b4_long("), "{idl}\n{out}");
        assert_eq!(out.matches("zd_over").count(), 2, "{idl}\n{out}");
    }
    for idl in [
        "@final struct Txt { string one; string<4> two; };",
        "@final struct Txt { string<4> two; string one; };",
    ] {
        let out = decoder_text(idl, "Txt");
        assert!(out.contains("fn zd_dec_string("), "{idl}\n{out}");
        assert!(out.contains("fn zd_dec_string_b4("), "{idl}\n{out}");
        assert_eq!(out.matches("trimUtf8").count(), 1, "{idl}\n{out}");
    }
}

/// The unbounded forms keep the name they had, so nothing that reads
/// `zd_dec_seq_long` by name moves.
#[test]
fn an_unbounded_collection_keeps_its_unsuffixed_routine_name() {
    let out = decoder_text("@final struct S { sequence<long> b; string s; };", "S");
    assert!(out.contains("fn zd_dec_seq_long("), "{out}");
    assert!(out.contains("fn zd_dec_string("), "{out}");
    assert!(!out.contains("_b"), "{out}");
}

/// The decoder for one type, as Zig text.
fn decoder_text(idl: &str, root_name: &str) -> String {
    use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
    let parsed = zerodds_idl_parse::parse(idl);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == root_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let decls =
        zerodds_lang_rules::emit::decode::decoder_from::<zerodds_lang_rules::emit::zig::Zig>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end");
    decls
        .iter()
        .map(zerodds_lang_render::zig::decl)
        .collect::<Vec<String>>()
        .join("\n")
}
