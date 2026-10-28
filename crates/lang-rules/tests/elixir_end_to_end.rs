// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Elixir, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go,
//! and of `end_to_end.rs`, which does it for Rust.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//!    A case that stopped rendering would otherwise be found by
//!    whichever probe ran next, which is a build directory away from the
//!    construct that caused it.
//! 2. **The generated Elixir compiles**, under `elixirc`. A snapshot
//!    says the emitter produced the text it produced last time; it does
//!    not say the text is Elixir, that every `defstruct` builds, or that
//!    the `@type` and the `defstruct` agree about which fields exist —
//!    which `elixirc` checks and reports as "undefined field :z on
//!    struct". `elixirc` expands every macro, so this is a real check
//!    and a syntax-only one would not be.
//!
//! **Warnings are failures here.** `elixirc` exits 0 and writes to
//! standard error for a module attribute that is set and never read, for
//! a function clause that can never match, and for clauses of one name
//! that are not grouped. Each of those is a defect in what this column
//! generated, so anything on standard error fails the check.
// `print_stderr` is allowed here for one reason: a compiler that is not
// on the path has to say so on a channel a passing test does not use.
// A green test that never ran the compiler is the failure these gates
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
use zerodds_lang_rules::adapter::elixir::elixir_file;
use zerodds_lang_rules::rule::elixir;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Elixir out.
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
    let applied = apply(&model, &elixir::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = elixir_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::elixir::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_becomes_a_module_with_a_defstruct_and_a_type() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("defmodule Pose do"), "{generated}");
    assert!(generated.contains("x: 0,"), "{generated}");
    assert!(generated.contains("y: 0.0,"), "{generated}");
    assert!(generated.contains("active: false"), "{generated}");
    assert!(generated.contains("@type t :: %__MODULE__{"), "{generated}");
    assert!(generated.contains("x: integer(),"), "{generated}");
    assert!(generated.contains("y: float(),"), "{generated}");
    assert!(generated.contains("active: boolean()"), "{generated}");
}

#[test]
fn every_width_and_signedness_is_written_into_the_attribute() {
    // The point of this column. Elixir's `integer()` is arbitrary
    // precision and says nothing about what the wire holds, so a member
    // typed only `integer()` with the width nowhere is the
    // silent-substitution class this project is arranged against.
    let generated = generate(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n;
         };",
    );
    for expected in [
        "{:a, [idl: \"a\", kind: :boolean, bits: 8]}",
        "{:b, [idl: \"b\", kind: :octet, bits: 8, signed: false]}",
        "{:c, [idl: \"c\", kind: :char, bits: 8]}",
        "{:d, [idl: \"d\", kind: :wchar, bits: 16]}",
        "{:e, [idl: \"e\", kind: :int8, bits: 8, signed: true]}",
        "{:f, [idl: \"f\", kind: :uint8, bits: 8, signed: false]}",
        "{:g, [idl: \"g\", kind: :int16, bits: 16, signed: true]}",
        "{:h, [idl: \"h\", kind: :uint16, bits: 16, signed: false]}",
        "{:i, [idl: \"i\", kind: :int32, bits: 32, signed: true]}",
        "{:j, [idl: \"j\", kind: :uint32, bits: 32, signed: false]}",
        "{:k, [idl: \"k\", kind: :int64, bits: 64, signed: true]}",
        "{:l, [idl: \"l\", kind: :uint64, bits: 64, signed: false]}",
        "{:m, [idl: \"m\", kind: :float32, bits: 32]}",
        "{:n, [idl: \"n\", kind: :float64, bits: 64]}",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn every_attribute_the_output_writes_has_a_function_that_reads_it() {
    // A module attribute is a compile-time binding and is gone from the
    // compiled module, so a bare one is invisible to every other module
    // — and `elixirc` warns "module attribute @zd_members was set but
    // never used" for one with no reader.
    let generated = generate("struct S { long a; };");
    assert!(
        generated.contains("def __zd_idl__, do: @zd_idl"),
        "{generated}"
    );
    assert!(
        generated.contains("def __zd_members__, do: @zd_members"),
        "{generated}"
    );
}

#[test]
fn a_long_double_is_a_binary_and_never_a_float() {
    // An Elixir `float()` is an IEEE binary64 with 53 bits of
    // significand; an IDL `long double` is a binary128 with 113. Writing
    // one into a `float()` loses sixty bits at the assignment, before
    // any codec runs, with nothing to say it happened.
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("big: binary()"), "{generated}");
    assert!(
        generated.contains("kind: :float128, bits: 128, holder: :decimal_text"),
        "{generated}"
    );
    assert!(!generated.contains("big: float()"), "{generated}");
}

#[test]
fn a_string_a_wstring_and_a_fixed_are_all_binaries_with_their_facts_beside_them() {
    let generated = generate("struct S { string<7> a; wstring b; fixed<5,2> c; string d; };");
    assert!(generated.contains("a: binary(),"), "{generated}");
    assert!(generated.contains("b: binary(),"), "{generated}");
    assert!(generated.contains("c: binary(),"), "{generated}");
    assert!(
        generated.contains("kind: :string, bits: 8, bound: 7"),
        "{generated}"
    );
    assert!(
        generated.contains("kind: :wstring, bits: 16, bound: 0"),
        "{generated}"
    );
    assert!(
        generated.contains("kind: :fixed, digits: 5, scale: 2, holder: :decimal_text"),
        "{generated}"
    );
}

#[test]
fn an_optional_member_is_nullable_and_the_others_are_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7 makes absence the value, not a zero.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(generated.contains("b: integer() | nil,"), "{generated}");
    assert!(generated.contains("c: integer()"), "{generated}");
    assert!(!generated.contains("c: integer() | nil"), "{generated}");
    assert!(generated.contains("b: nil,"), "{generated}");
    assert!(generated.contains("optional: true"), "{generated}");
}

#[test]
fn a_member_whose_type_has_no_default_is_enforced_instead() {
    // A nested struct's type is `%Other{}` and `nil` is not one of its
    // values, so the field is in `@enforce_keys` and `%S{}` is a compile
    // error rather than a sample with a hole in it.
    let generated = generate("struct Inner { long a; }; struct S { Inner i; long b; };");
    assert!(generated.contains("@enforce_keys [:i]"), "{generated}");
    assert!(generated.contains("i: Inner.t(),"), "{generated}");
    assert!(generated.contains("i: nil,"), "{generated}");
}

#[test]
fn a_member_named_after_an_elixir_reserved_word_is_escaped_and_keeps_its_idl_name() {
    // Lowering runs before the escape: `End` lowers to `end`, which is
    // reserved, so escaping first would leave the escape nothing to do.
    let generated = generate("struct S { long end; long When; };");
    assert!(generated.contains("{:end_, [idl: \"end\""), "{generated}");
    assert!(generated.contains("{:when_, [idl: \"When\""), "{generated}");
}

#[test]
fn an_idl_member_with_a_capital_becomes_a_lower_case_atom_that_records_its_spelling() {
    // The atom does not invert to the IDL name, and a member's ID under
    // @autoid(HASH) is the MD5 of the IDL name
    // (DDS-XTypes 1.3 §7.3.1.2.1.1).
    let generated = generate("struct S { long X; };");
    assert!(generated.contains("{:x, [idl: \"X\""), "{generated}");
    assert!(generated.contains("x: integer()"), "{generated}");
}

#[test]
fn two_members_that_would_reach_one_atom_are_refused_by_name() {
    // What the escape makes: `end` escapes to `end_`, and a member
    // already spelled `end_` takes the same atom. Elixir would take the
    // second silently — a struct key given twice is a rebinding — so a
    // column that emitted both would lose one member from a file that
    // compiles.
    let error = try_generate("struct S { long end; long end_; };", "collide.idl")
        .expect_err("the collision was not reported");
    assert!(error.contains("end_"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_sequence_is_a_list_and_an_array_is_a_tuple() {
    // A tuple's arity is part of its type and Dialyzer checks it, which
    // makes the array's length the one IDL fact Elixir's own type system
    // enforces. A `list()` for both would give that up for nothing.
    let generated = generate("struct S { sequence<long> ids; long grid[2][3]; };");
    assert!(generated.contains("ids: list(integer()),"), "{generated}");
    assert!(
        generated.contains(
            "grid: {{integer(), integer(), integer()}, {integer(), integer(), integer()}}"
        ),
        "{generated}"
    );
    assert!(
        generated.contains("grid: {{0, 0, 0}, {0, 0, 0}}"),
        "{generated}"
    );
    assert!(
        generated.contains("kind: :array, dims: [2, 3]"),
        "{generated}"
    );
}

#[test]
fn an_array_of_a_thousand_is_written_out_and_nothing_truncates_it() {
    // Stated rather than hidden: there is no size at which the spelling
    // changes, because a threshold would mean one IDL construct had two
    // Elixir forms depending on a number nobody chose.
    let generated = generate("const long N = 1000; struct S { long a[N]; };");
    assert_eq!(
        generated.matches("integer()").count(),
        1000,
        "the tuple type is not a thousand wide"
    );
    assert!(
        generated.contains("dims: [1000]"),
        "the dimension is not stated"
    );
}

#[test]
fn a_map_is_a_list_of_pairs_and_never_an_elixir_map() {
    // An Elixir map's iteration order is not the sample's, and a
    // `map<K,V>` goes on the wire as an ordered sequence of pairs
    // (DDS-XTypes 1.3 §7.4.1.1.4). Two entries with one key would also
    // silently become one.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("table: list({integer(), binary()})"),
        "{generated}"
    );
    assert!(generated.contains("defstruct [table: []]"), "{generated}");
    assert!(
        generated.contains("kind: :map, bound: 0, key: [kind: :int32"),
        "{generated}"
    );
    assert!(
        !generated.contains("%{"),
        "an Elixir map reached the output\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_module_with_a_type_and_the_use_names_it() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type". The
    // module declares no struct, so `Distance.t()` expands to the target
    // and nothing else.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(generated.contains("defmodule Distance do"), "{generated}");
    assert!(generated.contains("@type t :: integer()"), "{generated}");
    assert!(generated.contains("d: Distance.t()"), "{generated}");
    assert!(
        generated.contains("kind: :alias, module: Distance"),
        "{generated}"
    );
}

#[test]
fn an_enum_becomes_a_module_of_atoms_with_its_numbering_written_out() {
    // `@value` admits gaps, so a numbering this file inferred would
    // renumber the wire.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("defmodule Mode do"), "{generated}");
    assert!(
        generated.contains("@type t :: :idle | :active | :fault"),
        "{generated}"
    );
    assert!(generated.contains("def idle, do: 0"), "{generated}");
    assert!(generated.contains("def active, do: 4"), "{generated}");
    assert!(generated.contains("def fault, do: 5"), "{generated}");
    assert!(
        generated.contains("def value(:active), do: 4"),
        "{generated}"
    );
    assert!(
        generated.contains("def from_value(4), do: {:ok, :active}"),
        "{generated}"
    );
    assert!(
        generated.contains("def from_value(other), do: {:error, {:unknown_value, other}}"),
        "{generated}"
    );
    assert!(
        generated.contains("{:idle, 0, \"IDLE\"}"),
        "the IDL spelling is not recorded\n{generated}"
    );
}

#[test]
fn a_member_of_an_enumeration_is_an_integer_and_not_the_enumeration() {
    // An @appendable writer may send an enumerator the reader's IDL does
    // not list, and there is no atom for it. Which enumeration it is
    // stays readable, in the attribute.
    let generated = generate("enum Mode { IDLE }; struct S { Mode m; };");
    assert!(generated.contains("m: integer()"), "{generated}");
    assert!(!generated.contains("m: Mode.t()"), "{generated}");
    assert!(
        generated.contains("kind: :enum, module: Mode, bits: 32"),
        "{generated}"
    );
}

#[test]
fn a_union_is_a_struct_with_a_discriminator_and_not_a_tagged_tuple() {
    // The load-bearing decision of this column. An IDL union's
    // discriminator may take a value no branch selects
    // (IDL 4.2 §7.4.1.4.4.1), and a tagged tuple has no tag for that
    // state.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE, FAULT };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("defmodule Payload do"), "{generated}");
    assert!(generated.contains("_d: integer(),"), "{generated}");
    assert!(generated.contains("idle: integer() | nil,"), "{generated}");
    assert!(generated.contains("active: float() | nil"), "{generated}");
    assert!(generated.contains("_d: 0,"), "{generated}");
    assert!(generated.contains("idle: nil,"), "{generated}");
    assert!(
        generated.contains("@zd_discriminator [kind: :enum, module: Mode, bits: 32]"),
        "{generated}"
    );
    assert!(generated.contains("labels: [0]"), "{generated}");
}

#[test]
fn a_default_case_is_recorded_as_the_default_and_not_as_a_number() {
    let generated = generate("union U switch (long) { case 1: long a; default: double b; };");
    assert!(generated.contains("labels: [1]"), "{generated}");
    assert!(generated.contains("labels: [:default]"), "{generated}");
}

#[test]
fn a_union_branch_is_nullable_without_being_optional() {
    // Two different statements: `@optional` says absence is a value the
    // member can take, and a branch says only the selected one is on the
    // wire. An attribute that conflated them would tell a codec to write
    // an absence flag for a member that has none.
    let generated = generate("union U switch (long) { case 1: long a; };");
    assert!(generated.contains("a: integer() | nil"), "{generated}");
    assert!(
        !generated.contains("optional: true"),
        "a branch was written as optional\n{generated}"
    );
}

#[test]
fn a_bitmask_becomes_a_module_of_bit_values_with_its_bound_stated() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("defmodule Flags do"), "{generated}");
    assert!(
        generated.contains("kind: :bitmask, bits: 16"),
        "{generated}"
    );
    assert!(generated.contains("@type t :: integer()"), "{generated}");
    assert!(generated.contains("def ready, do: 1"), "{generated}");
    assert!(generated.contains("def moving, do: 16"), "{generated}");
    assert!(
        generated.contains("{:moving, 4, 16, \"MOVING\"}"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_struct_whose_field_widths_are_stated() {
    // `bitfield<3>` is three bits. C and Java round that up to the
    // narrowest integer that holds it; Elixir has no fixed-width type at
    // all, so nothing is rounded and the attribute states three.
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("defmodule B do"), "{generated}");
    assert!(generated.contains("{:low, 3, \"low\"}"), "{generated}");
    assert!(generated.contains("{:high, 12, \"high\"}"), "{generated}");
    assert!(generated.contains("low: integer(),"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_struct_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("defmodule V do"), "{generated}");
    assert!(generated.contains("kind: :valuetype"), "{generated}");
    assert!(generated.contains("counter: integer(),"), "{generated}");
    assert!(generated.contains("secret: binary()"), "{generated}");
}

#[test]
fn a_constant_becomes_an_attribute_and_the_function_that_reads_it() {
    // A bare attribute is a compile-time binding and is gone from the
    // compiled module, so nothing outside the file could reach it.
    let generated = generate("const octet LIMIT = 8;");
    assert!(
        generated.contains("defmodule ZdConstants do"),
        "{generated}"
    );
    assert!(generated.contains("@limit 8"), "{generated}");
    assert!(generated.contains("def limit, do: @limit"), "{generated}");
    assert!(
        generated.contains("limit: [idl: \"LIMIT\", kind: :octet, bits: 8, signed: false]"),
        "{generated}"
    );
}

#[test]
fn a_constant_named_after_a_built_in_attribute_is_escaped() {
    // `@doc 8` is not a name error: Elixir reports "@doc is a built-in
    // module attribute for documentation … got: 8", and the whole file
    // stops compiling. `const long doc = 1;` is ordinary IDL.
    let generated = generate("const long doc = 1; const long type = 2;");
    assert!(generated.contains("@doc_ 1"), "{generated}");
    assert!(generated.contains("def doc_, do: @doc_"), "{generated}");
    assert!(generated.contains("@type_ 2"), "{generated}");
    assert!(generated.contains("doc_: [idl: \"doc\""), "{generated}");
}

#[test]
fn a_constant_in_a_module_lands_in_that_scope() {
    let generated = generate("module common { const long N = 1; };");
    assert!(
        generated.contains("defmodule Common.ZdConstants do"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_struct_and_not_a_defexception() {
    // `defexception` requires either a `:message` field or a `message/1`
    // callback, and an IDL exception has neither.
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("defmodule Failure do"), "{generated}");
    assert!(generated.contains("kind: :exception"), "{generated}");
    assert!(generated.contains("code: integer(),"), "{generated}");
    assert!(
        !generated
            .lines()
            .any(|line| line.trim_start().starts_with("defexception")),
        "a defexception reached the output\n{generated}"
    );
}

#[test]
fn an_interface_becomes_a_behaviour_of_callbacks() {
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(generated.contains("defmodule Control do"), "{generated}");
    assert!(
        generated.contains("@callback go(target :: Pose.t()) :: :ok"),
        "{generated}"
    );
    assert!(
        generated.contains("@callback status() :: integer()"),
        "{generated}"
    );
}

#[test]
fn a_native_declaration_becomes_an_opaque_type() {
    let generated = generate("native Handle;");
    assert!(generated.contains("defmodule Handle do"), "{generated}");
    assert!(generated.contains("@type t :: any()"), "{generated}");
}

#[test]
fn a_module_nests_because_elixir_module_names_nest() {
    // The C and Go columns have to flatten `::common::util::Location`
    // onto one identifier. Elixir's module names nest with `.`, and that
    // nesting is the language's own.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("defmodule Common.Util.Location do"),
        "{generated}"
    );
    assert!(
        generated.contains("scoped: \"::common::util::Location\""),
        "the IDL scoped name was not recorded\n{generated}"
    );
    assert!(
        !generated.contains("Common_Util_Location"),
        "the scope was flattened\n{generated}"
    );
}

#[test]
fn a_module_named_elixir_is_escaped_because_it_is_the_root_namespace() {
    // `defmodule S` and `defmodule Elixir.S` are one module: both write
    // `Elixir.S.beam` and the second replaces the first with no
    // diagnostic at all.
    let generated = generate("module elixir { struct S { long a; }; };");
    assert!(generated.contains("defmodule Elixir_.S do"), "{generated}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Elixir equivalent. What is left
    // is the file comment.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(
        !generated.contains("defmodule"),
        "something was emitted\n{generated}"
    );
}

#[test]
fn an_any_in_member_position_is_refused_by_name() {
    // `any()` is a real Elixir type that Dialyzer accepts everywhere,
    // which is exactly why refusing matters: a member typed `any()` is a
    // field no codec can write, from a generator that reported success.
    let error = try_generate("struct S { any payload; };", "any.idl")
        .expect_err("the `any` was not refused");
    assert!(error.contains("Elixir has no form"), "{error}");
    assert!(error.contains("any"), "{error}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("defmodule First").expect("First");
    let second = generated.find("defmodule Second").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn an_aggregate_with_no_members_is_a_defstruct_with_an_empty_list() {
    // Legal Elixir, so no refusal is needed.
    let generated = generate("struct Empty { };");
    assert!(generated.contains("defstruct []"), "{generated}");
    assert!(
        generated.contains("@type t :: %__MODULE__{}"),
        "{generated}"
    );
}

#[test]
fn every_construct_the_column_claims_produces_output() {
    let sources = [
        ("struct S { long a; };", "defmodule S do"),
        ("enum E { A };", "@type t :: :a"),
        ("typedef long T;", "@type t :: integer()"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "_d: integer()",
        ),
        ("bitmask B { X };", "def x, do: 1"),
        ("bitset T { bitfield<3> a; };", "{:a, 3, \"a\"}"),
        ("valuetype V { public long a; };", "kind: :valuetype"),
        ("const long N = 1;", "def n, do: @n"),
        ("exception E2 { long code; };", "kind: :exception"),
        ("interface I { void go(); };", "@callback go() :: :ok"),
        ("native N2;", "@type t :: any()"),
        ("module m { struct S { long a; }; };", "defmodule M.S do"),
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

/// How many corpus cases have to reach Elixir for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the sibling columns' corpus gates report the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it.
const MUST_RENDER: usize = 350;

/// The names the Elixir column itself refuses, each with the reason.
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
    let out = std::env::var("ZD_ELIXIR_OUT").ok();
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
                    std::fs::write(case.join("zd_types.ex"), text).expect("the output file");
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
            "{} corpus case(s) the frontend refuses, so Elixir never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Elixir; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// One generated file through `elixirc`.
///
/// `-o` into a directory of this run's own: `elixirc` writes a `.beam`
/// per module into the working directory otherwise, and a test that left
/// them in the crate root would be a test that changed the tree.
///
/// Anything on standard error is a failure, whatever the exit status,
/// except `elixirc`'s notice that a file is taking long to compile.
/// `elixirc` exits 0 and warns for an attribute nothing reads, a clause
/// that can never match and clauses of one name that are not grouped —
/// each of which is a defect in what this column generated rather than a
/// style note.
fn elixir_compiles(source: &str) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!(
        "zd-elixir-{}-{stamp}-{}",
        std::process::id(),
        source.len()
    ));
    let out = dir.join("ebin");
    std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let file = dir.join("zd_types.ex");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    // `--ignore-module-conflict`, because one file may be compiled twice
    // in one run of this suite and a redefinition is not what is being
    // measured here.
    let output = Command::new("elixirc")
        .arg("--ignore-module-conflict")
        .arg("-o")
        .arg(&out)
        .arg(&file)
        .output()
        .map_err(|error| error.to_string());
    let result = match output {
        Err(why) => Err(why),
        Ok(output) => {
            // Both streams. `elixirc` puts a compile error and a
            // warning on standard output as readily as on standard
            // error, and reading only the latter made this function
            // answer `Ok(())` for a source the compiler had complained
            // about — the exact blindness the deliberate-error test
            // below exists to catch, and which it caught: under the
            // Elixir the CI image carries (1.14.0) the message went to
            // standard output and this returned `Err("")`, an error
            // with no text.
            let said = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            // Except the line `Compiling <file> (it's taking more than
            // 10s)`, which `elixirc` prints when one file outlasts its
            // long-compilation threshold: it reports the host's load, not
            // the source.
            let said = said
                .lines()
                .filter(|line| {
                    !(line.starts_with("Compiling ") && line.contains("(it's taking more than "))
                })
                .collect::<Vec<_>>()
                .join("\n");
            if output.status.success() && said.trim().is_empty() {
                Ok(())
            } else {
                Err(said)
            }
        }
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Whether `elixirc` is on the path at all.
fn elixirc_available() -> bool {
    Command::new("elixirc")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn the_compiler_check_catches_a_deliberate_error() {
    // Before the check below is trusted. A `@type` naming a field the
    // `defstruct` does not declare is the cross-check `elixirc` performs
    // between the two halves of every declaration this column writes,
    // and it is the one no Rust test could make.
    if !elixirc_available() {
        eprintln!("SKIPPED: `elixirc` is not on the path.");
        return;
    }
    let broken = "defmodule ZdProbe do\n  defstruct [x: 0]\n  \
                  @type t :: %__MODULE__{x: integer(), z: integer()}\nend\n";
    let why = elixir_compiles(broken).expect_err("elixirc accepted an undefined struct field");
    assert!(why.contains("undefined field :z"), "{why}");

    // And a warning, which exits 0. An attribute nothing reads is
    // exactly what this column's readers exist to prevent.
    let warned = "defmodule ZdProbe2 do\n  @zd_members []\n  defstruct []\nend\n";
    let why = elixir_compiles(warned).expect_err("a bare attribute did not fail the check");
    assert!(why.contains("never used"), "{why}");
}

/// Every shape above, compiled.
///
/// One `elixirc` per source and not one for all of them: the sources
/// declare modules of the same name (`S` appears in several), and
/// concatenating them would measure the last definition rather than each.
#[test]
fn the_generated_elixir_compiles() {
    if !elixirc_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: `elixirc` is not on the path, so the generated Elixir was not \
             compiled. Set it up or run this on a host that has it."
        );
        return;
    }
    let sources = [
        "struct Pose { long x; double y; boolean active; };",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
        "struct S { @optional octet b; long c; string name; wstring w; fixed<5,2> f; \
         sequence<long> ids; long grid[2][3]; map<long, string> table; };",
        "struct T { long end; long When; long value; };",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; \
         default: string other; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const double R = 1.5; \
         const long doc = 1; const double E = 1e10; const string HASH = \"a#b\";",
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
        "module elixir { struct S { long a; }; };",
        "typedef long Distance; struct Dist { Distance d; };",
        "struct Node { sequence<Node> children; };",
        "struct Inner { long a; }; struct Outer { Inner i; sequence<Inner> more; };",
        "struct Empty { };",
    ];
    for idl in sources {
        let generated = generate(idl);
        if let Err(why) = elixir_compiles(&generated) {
            panic!("{idl}\n\n{generated}\n\nelixirc said:\n{why}");
        }
    }
}

/// The whole corpus through `elixirc`, one file per case.
///
/// `#[ignore]`, because `elixirc` starts in about a second and there are
/// hundreds of cases. It is what proves the column rather than samples
/// it, so it is run with `--include-ignored` on a host that has Elixir.
#[test]
#[ignore = "one `elixirc` per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        elixirc_available(),
        "the explicitly selected elixir gate requires its compiler"
    );
    let mut failures = Vec::new();
    let mut compiled = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = try_generate(&source, &path.display().to_string())
            .unwrap_or_else(|why| panic!("accepted corpus case {name} failed generation: {why}"));
        match elixir_compiles(&text) {
            Ok(()) => compiled += 1,
            Err(why) => failures.push(format!("{name}: {why}")),
        }
    }
    eprintln!("{compiled} corpus case(s) compiled");
    assert!(
        failures.is_empty(),
        "{} of the corpus did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(
        compiled >= MUST_RENDER,
        "too few corpus cases reached the selected gate: {compiled}"
    );
}
