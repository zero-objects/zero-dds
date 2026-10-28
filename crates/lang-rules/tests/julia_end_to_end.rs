// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Julia, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go, and
//! of `c_end_to_end.rs`, which does it for C.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused. A
//!    case that stopped rendering would otherwise be found by whichever
//!    probe ran next, which is a build directory away from the construct
//!    that caused it.
//! 2. **The generated Julia loads**, under `julia --eval 'include(…)'`.
//!    A parse check would not do: Julia parses `struct A; b::B; end`
//!    before `B` exists perfectly happily, and the failure mode of this
//!    column is exactly a name that is not yet defined. `include`
//!    evaluates every `struct` definition, resolves every field type and
//!    stops with an `UndefVarError` — which is why
//!    `the_load_check_catches_a_forward_reference` feeds it a file that
//!    is known to be wrong before any of the others trust it.
//!
//! `julia` is skipped where it is not on the path and the skip says so on
//! standard error — a test that went green without running it would be
//! the failure it is meant to catch. `print_stderr` is allowed here for
//! exactly that: a skip and a frontend refusal have to be visible on a
//! passing run, and standard error is the channel a passing test does not
//! otherwise use.
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
use zerodds_lang_rules::adapter::julia::julia_file;
use zerodds_lang_rules::rule::julia;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Julia out.
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
    let applied = apply(&model, &julia::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = julia_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::julia::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_a_julia_struct() {
    // `struct` and not `mutable struct`: an immutable struct is Julia's
    // default form, and a decoded sample is a value rather than a handle
    // a second reference can change underneath the first.
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(
        generated.contains("struct Pose\n    x::Int32\n    y::Float64\n    active::Bool\nend"),
        "{generated}"
    );
    assert!(
        !generated.contains("mutable struct"),
        "a mutable struct reached the output\n{generated}"
    );
}

#[test]
fn every_idl_primitive_reaches_a_julia_type() {
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
        "a::Bool",
        "b::UInt8",
        "c::UInt8",
        "d::UInt16",
        "e::Int8",
        "f::UInt8",
        "g::Int16",
        "h::UInt16",
        "i::Int32",
        "j::UInt32",
        "k::Int64",
        "l::UInt64",
        "m::Float32",
        "n::Float64",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn no_generated_type_has_a_platform_defined_width() {
    // `Int` is `Int64` on a 64-bit platform and `Int32` on a 32-bit one
    // (Julia manual, "Integers and Floating-Point Numbers"). A member
    // declared with it would change width between the host that
    // generated the file and the target that runs it, for a wire format
    // that fixes every width.
    let generated = generate(
        "struct P { long a; unsigned long b; long long c; sequence<long> d; long e[2]; };",
    );
    for forbidden in ["::Int\n", "::UInt\n", "Vector{Int}", "NTuple{2, Int}"] {
        assert!(
            !generated.contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn a_long_double_is_a_string_and_never_a_float() {
    // `Float64` has 53 bits of significand; an IDL `long double` is a
    // binary128 with 113. `BigFloat` is not the answer either: its
    // precision is a run-time global set by `setprecision` and not part
    // of the type — measured, `precision(BigFloat)` is 256 by default —
    // so a declaration cannot say 113 bits. The member holds the decimal.
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("big::String"), "{generated}");
    assert!(
        !generated.contains("BigFloat"),
        "a BigFloat reached the output\n{generated}"
    );
}

#[test]
fn an_optional_member_becomes_a_union_with_nothing_and_the_others_do_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. An all-bits-zero `UInt8` and a
    // member that did not arrive would otherwise be the same value;
    // `Nothing` has exactly one value and no `UInt8` is it. The union is
    // a real Julia type a method can dispatch on, so a decoder that
    // handles the two cases does it with two methods rather than a
    // branch.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(
        generated.contains("b::Union{UInt8, Nothing}"),
        "{generated}"
    );
    assert!(generated.contains("c::Int32"), "{generated}");
    assert!(!generated.contains("c::Union"), "{generated}");
}

#[test]
fn a_member_named_after_a_julia_keyword_is_escaped_and_keeps_its_idl_name() {
    // `struct S; end::Int32; end` does not declare a field called `end`
    // at all — it does not parse as one. The IDL name goes in the comment
    // above the field, because Julia has no counterpart of Go's
    // `reflect.StructTag`.
    let generated = generate("struct S { long end; long function; };");
    assert!(generated.contains("end_::Int32"), "{generated}");
    assert!(generated.contains("function_::Int32"), "{generated}");
    assert!(generated.contains("# IDL: `end`."), "{generated}");
    assert!(generated.contains("# IDL: `function`."), "{generated}");
}

#[test]
fn a_field_named_after_a_base_type_is_left_alone_because_a_field_shadows_nothing() {
    // Measured on Julia 1.12.6: `struct S; Int32::Int32; String::String; end`
    // declares two fields and both annotations still resolve to the
    // `Core` types. Escaping them would rename a member the wire format
    // names, for a collision that does not exist.
    // `_Int32` names `Int32`: IDL 4.2 §7.2.3.2 says the leading
    // underscore is not part of the identifier, and it is needed here
    // only because IDL compares identifiers without regard to case and
    // `int32` is one of its own keywords.
    let generated = generate("struct S { long _Int32; string _String; };");
    assert!(generated.contains("Int32::Int32"), "{generated}");
    assert!(generated.contains("String::String"), "{generated}");
}

#[test]
fn a_declared_name_that_would_shadow_the_language_is_escaped() {
    // The case the parser does not catch and Julia does not warn about.
    // `const Vector = Int32` at the top of a file is accepted, and every
    // later `Vector{Int32}` is then a call on an integer — measured on
    // Julia 1.12.6, where the same thing done with `String` also loads
    // and quietly turns every string member into an `Int32`.
    //
    // `Vector` and not `String` in the source here because IDL compares
    // identifiers without regard to case and `string` is one of its own
    // keywords: the frontend refuses `typedef long String;` before this
    // column sees it. `Vector`, `Nothing` and `NTuple` reach it.
    let generated = generate("typedef long Vector; struct S { Vector a; sequence<long> b; };");
    assert!(generated.contains("const Vector_ = Int32"), "{generated}");
    assert!(generated.contains("a::Vector_"), "{generated}");
    assert!(generated.contains("b::Vector{Int32}"), "{generated}");

    let absence = generate("typedef long Nothing; struct S { Nothing a; @optional long b; };");
    assert!(absence.contains("const Nothing_ = Int32"), "{absence}");
    assert!(absence.contains("b::Union{Int32, Nothing}"), "{absence}");
}

#[test]
fn two_declarations_that_would_reach_one_julia_name_are_refused_by_name() {
    // What the escape makes: `end` escapes to `end_`, and a member
    // already spelled `end_` reaches the same name. Julia would accept
    // neither the struct nor a diagnostic naming the two IDL members.
    let error = try_generate("struct S { long end; long end_; };", "collide.idl")
        .expect_err("the collision was not reported");
    assert!(error.contains("end_"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn two_modules_that_flatten_onto_one_name_are_refused_by_name() {
    // `a_b::C` and `a::b_C` both give `a_b_C`. This is the check the
    // interpreter cannot make for us: measured on Julia 1.12.6, a file
    // declaring `struct S; x::Int32; end` and then
    // `struct S; y::Float64; end` loads, prints no warning, exits 0, and
    // `fieldnames(S)` is `(:y,)`. The first type is simply gone — a
    // generated file that loads and is missing a declaration.
    let error = try_generate(
        "module a_b { struct C { long x; }; };
         module a { struct b_C { double y; }; };",
        "flatten.idl",
    )
    .expect_err("the collision was not reported");
    assert!(error.contains("a_b_C"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_sequence_becomes_a_vector_and_a_string_a_string() {
    // A Julia `String` is UTF-8 natively (manual, "Strings") and carries
    // its own length, so unlike the C column's `const char *` there is no
    // separate length to keep in step and no terminator to agree on.
    let generated = generate("struct Bag { sequence<long> ids; string name; };");
    assert!(generated.contains("ids::Vector{Int32}"), "{generated}");
    assert!(generated.contains("name::String"), "{generated}");
}

#[test]
fn an_array_becomes_an_ntuple_and_the_outermost_dimension_is_outermost() {
    // `long grid[2][3]` is two rows of three, which in Julia is
    // `NTuple{2, NTuple{3, Int32}}`. Reversing it silently transposes the
    // data.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("grid::NTuple{2, NTuple{3, Int32}}"),
        "{generated}"
    );
}

#[test]
fn an_array_of_a_thousand_writes_its_length_and_nothing_truncates_it() {
    // The length is part of the type, which is the one IDL fact Julia's
    // own type system enforces without a codec being asked: a tuple of
    // any other length is a different type. An `Array{Int32, 1}` would
    // accept every length and put the number in a comment.
    let generated = generate("struct S { long a[1000]; };");
    assert!(generated.contains("a::NTuple{1000, Int32}"), "{generated}");
    assert!(
        !generated.contains("Array{"),
        "an Array reached the output\n{generated}"
    );
}

#[test]
fn a_map_becomes_a_vector_of_entries_and_never_a_dict() {
    // A `Dict` is a hash table and its iteration order is its layout's. A
    // `map<K,V>` goes on the wire as a sequence of pairs (DDS-XTypes 1.3
    // §7.4.1.1.4), so a `Dict` would give an octet stream ordered by the
    // hash of the keys.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("table::Vector{ZdMapEntry_long_string}"),
        "{generated}"
    );
    assert!(
        generated.contains("struct ZdMapEntry_long_string\n    key::Int32\n    value::String\nend"),
        "{generated}"
    );
    assert!(
        !generated.contains("Dict{"),
        "a Dict reached the output\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_a_const_binding() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type", which is
    // exactly what `const Distance = Int32` is. `const` and not a plain
    // global: a non-`const` global holding a type makes every annotation
    // that uses it a run-time lookup.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(generated.contains("const Distance = Int32"), "{generated}");
    assert!(generated.contains("d::Distance"), "{generated}");
}

#[test]
fn an_enum_becomes_an_at_enum_with_its_base_and_every_value_written_out() {
    // `@enum` is in `Base` and needs no import. Every value is written
    // because `@enum` numbers from zero when they are omitted and
    // `@value` admits gaps.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(generated.contains("@enum Mode::Int32 begin"), "{generated}");
    assert!(generated.contains("IDLE = 0"), "{generated}");
    assert!(generated.contains("ACTIVE = 4"), "{generated}");
    assert!(generated.contains("FAULT = 5"), "{generated}");
}

#[test]
fn a_union_becomes_a_struct_with_a_discriminator_and_optional_branches() {
    // Not a Julia `Union{…}`, although Julia has one and it is a real
    // type: an IDL discriminator may take a value that selects no branch
    // (IDL 4.2 §7.4.1.4.4.1), and a `Union{A, B}` value is an A or a B
    // with nowhere to record that.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("struct Payload"), "{generated}");
    assert!(generated.contains("_d::Mode"), "{generated}");
    assert!(
        generated.contains("idle::Union{Int32, Nothing}"),
        "{generated}"
    );
    assert!(
        generated.contains("active::Union{Float64, Nothing}"),
        "{generated}"
    );
}

#[test]
fn a_bitmask_becomes_an_unsigned_alias_and_one_shifted_constant_per_bit() {
    // Never an `@enum`: an `@enum` value is one member, and a bitmask
    // holds several at once. The one is converted before it is shifted,
    // so the constant has the bitmask's width rather than the platform's.
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("const Flags = UInt16"), "{generated}");
    assert!(
        generated.contains("const READY = Flags(1) << 0"),
        "{generated}"
    );
    assert!(
        generated.contains("const MOVING = Flags(1) << 4"),
        "{generated}"
    );
    assert!(
        !generated.contains("@enum Flags"),
        "a bitmask reached an @enum\n{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_struct_of_holder_typed_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("struct B"), "{generated}");
    assert!(generated.contains("low::UInt8"), "{generated}");
    assert!(generated.contains("high::UInt16"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_struct_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("struct V"), "{generated}");
    assert!(generated.contains("counter::Int32"), "{generated}");
    assert!(generated.contains("secret::String"), "{generated}");
}

#[test]
fn a_constant_is_written_as_a_conversion_and_never_as_a_bare_literal() {
    // `const LIMIT = 8` binds an `Int64` on a 64-bit host and an `Int32`
    // on a 32-bit one, because a Julia integer literal takes the
    // platform's `Int`. `UInt8(8)` binds what the IDL declared,
    // everywhere. A string needs no conversion and gets none.
    let generated = generate("const octet LIMIT = 8; const string TAG = \"abc\";");
    assert!(generated.contains("const LIMIT = UInt8(8)"), "{generated}");
    assert!(generated.contains("const TAG = \"abc\""), "{generated}");
}

#[test]
fn a_string_constant_escapes_julias_interpolation_marker() {
    // `"$HOME"` in Julia is an interpolation and not four characters
    // (manual, "Strings"). A constant carrying it, written through the
    // C-family escape alone, would either stop the file with
    // `UndefVarError: HOME` or bind whatever that name happens to hold.
    // No other column has this to do.
    let generated = generate("const string PROMPT = \"$HOME\";");
    assert!(
        generated.contains("const PROMPT = \"\\$HOME\""),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_struct_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("struct Failure"), "{generated}");
    assert!(generated.contains("code::Int32"), "{generated}");
    assert!(generated.contains("reason::String"), "{generated}");
}

#[test]
fn an_interface_becomes_an_abstract_type_and_a_function_stub_per_operation() {
    // Julia has no interface construct at all: there is no declaration
    // that says a subtype must provide these methods. The language's own
    // answer is an abstract type plus a documented method contract, and
    // `function go end` is a generic function with no methods — it binds
    // the name and adds nothing. The signature goes in the comment
    // because a function declared this way has no argument list.
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(
        generated.contains("abstract type Control end"),
        "{generated}"
    );
    assert!(
        generated.contains("# go(target::Pose)::Nothing"),
        "{generated}"
    );
    assert!(generated.contains("function go end"), "{generated}");
    assert!(generated.contains("# status()::Int32"), "{generated}");
    assert!(generated.contains("function status end"), "{generated}");
}

#[test]
fn a_native_declaration_becomes_an_opaque_const() {
    let generated = generate("native Handle;");
    assert!(generated.contains("const Handle = Any"), "{generated}");
}

#[test]
fn a_module_flattens_onto_the_name_because_a_module_cannot_be_reopened() {
    // Julia's `module` nests properly and would be the natural form. It
    // is not used because a module body is one contiguous block and a
    // second `module Common … end` rebinds the name — measured on Julia
    // 1.12.6, the first module's declarations are dropped with no warning
    // and exit status 0. Declarations here are in dependency order, which
    // interleaves modules in general.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("struct common_util_Location"),
        "{generated}"
    );
    assert!(
        generated.contains("IDL: `Location`."),
        "the IDL name was not recorded\n{generated}"
    );
    assert!(
        !generated.contains("module "),
        "a module reached the output\n{generated}"
    );
}

#[test]
fn an_empty_aggregate_is_a_singleton_type_and_not_a_refusal() {
    // Julia allows `struct S end` — a singleton type — so there is
    // nothing to refuse here.
    let generated = generate("struct Empty { };");
    assert!(generated.contains("struct Empty\nend"), "{generated}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Julia equivalent. What is left is
    // the file comment.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(generated.contains("Generated by ZeroDDS"), "{generated}");
    // A declaration starts at column zero; the file comment mentions the
    // word and is not one.
    assert!(!generated.contains("\nstruct "), "{generated}");
    assert!(!generated.contains("\nconst "), "{generated}");
}

#[test]
fn the_output_is_in_dependency_order_and_not_source_order() {
    // The C column's reason, not the Go column's: a struct's field types
    // are evaluated when the struct is defined, so a name declared later
    // in the file is an `UndefVarError`.
    let generated = generate("struct Outer { long a; }; struct Inner { long b; };");
    let outer = generated.find("struct Outer").expect("Outer");
    let inner = generated.find("struct Inner").expect("Inner");
    assert!(outer < inner, "the walk lost the source order for free");

    let dependent = generate("struct Held; struct Holder { Held h; }; struct Held { long a; };");
    let held = dependent.find("struct Held").expect("Held");
    let holder = dependent.find("struct Holder").expect("Holder");
    assert!(
        held < holder,
        "the dependency was declared after its user\n{dependent}"
    );
}

#[test]
fn a_self_recursive_type_needs_no_forward_declaration() {
    // A struct naming itself is the one forward reference Julia accepts:
    // the type's own name is bound before its field types are evaluated.
    // Measured, and it is why this case gets no abstract type.
    let generated = generate("struct Node { sequence<Node> children; };");
    assert!(generated.contains("children::Vector{Node}"), "{generated}");
    assert!(
        !generated.contains("ZdFwd_"),
        "a self-recursive type got a forward declaration it does not need\n{generated}"
    );
}

#[test]
fn a_mutually_recursive_pair_gets_an_abstract_forward_declaration() {
    // The measurement that decides this: `struct A; b::Vector{B}; end`
    // before `struct B` stops with ``UndefVarError: `B` not defined``,
    // exactly as a bare `b::B` does. A `Vector{…}` position is evaluated
    // at the same moment as any other. An `abstract type` has no fields
    // and therefore no forward reference of its own.
    let generated = generate(
        "struct B;
         struct A { sequence<B> bs; };
         struct B { sequence<A> as; };",
    );
    assert!(
        generated.contains("abstract type ZdFwd_"),
        "no forward declaration was emitted\n{generated}"
    );
    let abstract_at = generated
        .find("abstract type ZdFwd_")
        .expect("the abstract");
    let first_struct = generated.find("\nstruct ").expect("a struct");
    assert!(
        abstract_at < first_struct,
        "the forward declaration came after the struct that names it\n{generated}"
    );
    assert!(generated.contains(" <: ZdFwd_"), "{generated}");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_table_claims_for_julia_produces_output() {
    // The table says a rule covers it; this says the rule reaches text.
    let sources = [
        ("struct S { long a; };", "struct S"),
        ("enum E { A };", "@enum E::Int32 begin"),
        ("typedef long T;", "const T = Int32"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "struct U",
        ),
        ("bitmask B { X };", "const B = UInt32"),
        ("bitset T { bitfield<3> a; };", "struct T"),
        ("valuetype V { public long a; };", "struct V"),
        ("const long N = 1;", "const N = Int32(1)"),
        ("exception E2 { long code; };", "struct E2"),
        ("interface I { void go(); };", "abstract type I end"),
        ("native N2;", "const N2 = Any"),
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

/// How many corpus cases have to reach Julia for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it: the corpus is 378
/// cases and the column has to carry substantially all of them.
const MUST_RENDER: usize = 350;

/// The names the Julia column itself refuses, each with the reason.
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
    let out = std::env::var("ZD_JULIA_OUT").ok();
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
                    std::fs::write(case.join("zd_types.jl"), text).expect("the output file");
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
            "{} corpus case(s) the frontend refuses, so Julia never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Julia; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the interpreter -----------------------------------------------------

/// A directory of this test run's own, named for what is in it.
///
/// A counter and not the source's length: the tests run in parallel
/// threads of one process, and two sources of the same length would
/// otherwise share a directory and delete each other's file.
fn scratch(tag: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seed = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("zd-julia-{tag}-{}-{seed}", std::process::id()))
}

/// One file through `julia --eval 'include(…)'`.
///
/// `include` and not `--check-bounds=no` or a parse-only flag: Julia has
/// no parse-only mode that resolves anything, and a file whose fields
/// name types that do not exist parses perfectly. `include` evaluates
/// every `struct` definition, which is where the field types are
/// resolved and where an `UndefVarError` is raised.
///
/// `--startup-file=no` because a user's `~/.julia/config/startup.jl`
/// would otherwise be part of the check, and it is not part of the
/// generated file.
fn julia_loads(source: &str) -> Result<(), String> {
    let dir = scratch("load");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join("zd_types.jl");
    std::fs::write(&path, source).map_err(|error| error.to_string())?;
    let output = Command::new("julia")
        .arg("--startup-file=no")
        .arg("--color=no")
        .arg("--eval")
        .arg(format!("include(\"{}\")", path.display()))
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Whether `julia` is on the path at all.
fn julia_available() -> bool {
    Command::new("julia")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn the_load_check_catches_a_forward_reference() {
    // Before anything trusts [`julia_loads`], it is shown to fail on the
    // file this column exists to avoid. Both halves matter: the bare
    // forward reference and the one inside a `Vector{…}`, because the
    // second is the one people expect Julia to defer and it does not.
    if !julia_available() {
        eprintln!("SKIPPED: `julia` is not on the path, so the load check was not proven.");
        return;
    }
    let bare = julia_loads("struct A\n    b::B\nend\nstruct B\n    x::Int32\nend\n")
        .expect_err("a bare forward reference loaded");
    assert!(bare.contains("UndefVarError"), "{bare}");

    let wrapped = julia_loads("struct A\n    b::Vector{B}\nend\nstruct B\n    a::Vector{A}\nend\n")
        .expect_err("a forward reference inside a Vector loaded");
    assert!(wrapped.contains("UndefVarError"), "{wrapped}");

    // And it passes what it should: the self-recursive form Julia does
    // accept, which is why the column does not forward-declare it.
    julia_loads("struct Node\n    children::Vector{Node}\nend\n")
        .expect("a self-recursive struct did not load");
}

/// Every shape above, loaded.
///
/// One `julia` process per source. Start-up is about 0.3 s, which for
/// this many sources is cheaper than the machinery that would avoid it;
/// the whole-corpus test below is where that stops being true.
#[test]
fn the_generated_julia_loads() {
    if !julia_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the interpreter is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: `julia` is not on the path, so the generated Julia was not loaded. \
             Set it up or run this on a host that has it."
        );
        return;
    }
    let sources = [
        "struct Pose { long x; double y; boolean active; };",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
        "struct S { @optional octet b; long c; string name; sequence<long> ids; \
         long grid[2][3]; map<long, string> table; };",
        "struct T { long end; long function; long global; };",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const string PROMPT = \"$HOME\";",
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
        "typedef long Vector; struct Coll { Vector a; sequence<long> b; };",
        "typedef long Nothing; struct Absent { Nothing a; @optional long b; };",
        "struct Node { sequence<Node> children; };",
        "struct B; struct A { sequence<B> bs; }; struct B { sequence<A> as; };",
        "struct Empty { };",
        "struct S2 { long a[1000]; };",
    ];
    for idl in sources {
        let generated = generate(idl);
        if let Err(why) = julia_loads(&generated) {
            panic!("{idl}\n\n{generated}\n\njulia said:\n{why}");
        }
    }
}

/// The whole corpus through the interpreter, one module per case.
///
/// `#[ignore]`, because it evaluates every case the column renders and
/// takes minutes. It is what proves the column rather than samples it, so
/// it is run with `--include-ignored` on a host that has Julia.
///
/// One `julia` process and not 378: start-up is about 0.3 s, which is two
/// minutes of it. Each case is `include`d into a module of its own, so a
/// type name two cases share is not reported as a redefinition — the
/// corpus has many, and a shared namespace would make this test about the
/// corpus's naming rather than about the generated Julia.
#[test]
#[ignore = "evaluates the whole corpus in one julia process; run with --include-ignored"]
fn every_corpus_case_that_renders_also_loads() {
    assert!(
        julia_available(),
        "the explicitly selected julia gate requires its compiler"
    );
    let dir = scratch("corpus");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    let mut written = Vec::new();
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = try_generate(&source, &path.display().to_string())
            .unwrap_or_else(|why| panic!("accepted corpus case {name} failed generation: {why}"));
        let case = dir.join(format!("{name}.jl"));
        std::fs::write(&case, text).expect("a generated file");
        written.push(case);
    }
    assert!(
        written.len() >= MUST_RENDER,
        "only {} cases were written; the gate says nothing below {MUST_RENDER}",
        written.len()
    );

    // A module per case, and every failure collected rather than the
    // first one raised: a run that stopped at the first would need as
    // many runs as there are failures to see them all.
    let driver = dir.join("zd_load_all.jl");
    std::fs::write(
        &driver,
        "failures = String[]\n\
         for (index, path) in enumerate(ARGS)\n\
         \x20   scope = Module(Symbol(\"ZdCase\", index))\n\
         \x20   try\n\
         \x20       Base.include(scope, path)\n\
         \x20   catch err\n\
         \x20       push!(failures, string(basename(path), \": \", sprint(showerror, err)))\n\
         \x20   end\n\
         end\n\
         for failure in failures\n\
         \x20   println(failure)\n\
         end\n\
         exit(isempty(failures) ? 0 : 1)\n",
    )
    .expect("the driver");

    let output = Command::new("julia")
        .arg("--startup-file=no")
        .arg("--color=no")
        .arg(&driver)
        .args(&written)
        .output()
        .expect("julia");
    let reported = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let ok = output.status.success();
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("{} corpus case(s) evaluated by Julia", written.len());
    assert!(
        ok,
        "{} of {} generated files did not load:\n{reported}{stderr}",
        reported.lines().count(),
        written.len()
    );
}
