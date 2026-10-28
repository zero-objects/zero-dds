// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to OCaml, through every layer of the rebuild.
//!
//! The counterpart of `go_end_to_end.rs`, which does the same for Go,
//! and of `c_end_to_end.rs`, which does it for C.
//!
//! Three gates and not two:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//! 2. **The generated OCaml compiles**, under `ocamlc -c`. A snapshot
//!    says the emitter produced the text it produced last time; it does
//!    not say the text is OCaml, that every name in it resolves, that a
//!    record has at least one field, or that a literal is in its type's
//!    range. `ocamlc` is skipped where it is not on the path and the
//!    skip says so on standard error.
//! 3. **A recursive type comes out as one `and`-joined group** and
//!    compiles. This is the case the C column refuses outright, and it
//!    is the reason the OCaml adapter computes strongly connected
//!    components rather than doing the C column's post-order walk.
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
use std::sync::atomic::{AtomicUsize, Ordering};
use zerodds_lang_rules::adapter::ocaml::ocaml_file;
use zerodds_lang_rules::rule::ocaml;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, OCaml out.
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
    let applied = apply(&model, &ocaml::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = ocaml_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::ocaml::file(&file))
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_a_record_with_a_lower_case_name() {
    // A type constructor and a record field are `lowercase-ident` in the
    // OCaml grammar, so `struct Pose` cannot be `type Pose`.
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(generated.contains("type pose = {"), "{generated}");
    assert!(generated.contains("  x : int32;"), "{generated}");
    assert!(generated.contains("  y : float;"), "{generated}");
    assert!(generated.contains("  active : bool;"), "{generated}");
    assert!(
        generated.contains("IDL: `Pose`."),
        "the IDL spelling was not recorded\n{generated}"
    );
}

#[test]
fn every_idl_primitive_reaches_an_ocaml_type() {
    let generated = generate(
        "struct P {
           boolean a; octet b; char c; wchar d;
           int8 e; uint8 f; short g; unsigned short h;
           long i; unsigned long j; long long k; unsigned long long l;
           float m; double n;
         };",
    );
    for expected in [
        "a : bool;",
        "b : int;",
        "c : int;",
        "d : int;",
        "e : int;",
        "f : int;",
        "g : int;",
        "h : int;",
        "i : int32;",
        // An `unsigned long` is an `int64` and not an `int32`: 4 000 000
        // 000 in an `int32` reads back as −294 967 296, and OCaml has no
        // unsigned integer to read it back correctly.
        "j : int64;",
        "k : int64;",
        // An `unsigned long long` has no OCaml integer at all — `int64`
        // reads the top half of the range back negative — so the member
        // holds the decimal.
        "l : string;",
        "m : float;",
        "n : float;",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn no_generated_type_is_platform_dependent_in_a_width_a_member_can_reach() {
    // `nativeint` is the platform's word width. `int` is 31 bits on a
    // 32-bit target and 63 on a 64-bit one, and is used only where the
    // IDL type needs at most 16, so no value a member can hold reaches
    // the difference.
    let generated = generate(
        "struct P { long a; unsigned long b; long long c; sequence<long> d; long e[2]; \
         octet f; unsigned short g; };",
    );
    assert!(
        !generated.contains("nativeint"),
        "nativeint reached the output\n{generated}"
    );
    for wide in ["a : int;", "b : int;", "c : int;"] {
        assert!(
            !generated.contains(wide),
            "{wide:?} put a 32-bit or wider IDL type in an OCaml `int`\n{generated}"
        );
    }
}

#[test]
fn a_long_double_is_a_string_and_never_a_float() {
    // An OCaml `float` is an IEEE 754 binary64 with 53 bits of
    // significand; an IDL `long double` is a binary128 with 113. Writing
    // one into a `float` loses sixty bits at the assignment, before any
    // codec runs.
    let generated = generate("struct S { long double big; };");
    assert!(generated.contains("big : string;"), "{generated}");
}

#[test]
fn an_optional_member_becomes_an_option_and_the_others_do_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. `None` is the one state no written
    // value can take.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(generated.contains("b : int option;"), "{generated}");
    assert!(generated.contains("c : int32;"), "{generated}");
    assert!(!generated.contains("c : int32 option;"), "{generated}");
}

#[test]
fn a_member_named_after_an_ocaml_keyword_is_escaped_and_keeps_its_idl_name() {
    let generated = generate("struct S { long type; long method; long match; };");
    assert!(generated.contains("type_ : int32;"), "{generated}");
    assert!(generated.contains("method_ : int32;"), "{generated}");
    assert!(generated.contains("match_ : int32;"), "{generated}");
    assert!(generated.contains("IDL: `type`."), "{generated}");
}

#[test]
fn a_member_named_after_a_predefined_type_is_escaped_too() {
    // OCaml permits the shadowing. A generated file cannot afford it:
    // every field type in it is one of these, and an earlier
    // `type option = …` changes what `int32 option` means later.
    let generated = generate("struct S { long option; long array; };");
    assert!(generated.contains("option_ : int32;"), "{generated}");
    assert!(generated.contains("array_ : int32;"), "{generated}");
}

#[test]
fn a_bitmask_bit_folds_its_whole_name_and_not_only_the_first_letter() {
    // `READY` folded one letter would be `rEADY`, which parses and is
    // unreadable. The whole-name fold is safe because IDL 4.2 §7.2.3.1
    // puts every identifier in one namespace and compares them
    // case-insensitively, so two names differing only in case are
    // already a collision the frontend reports.
    let generated = generate("bitmask Flags { READY, MOVING };");
    assert!(generated.contains("let ready : flags = 1l"), "{generated}");
    assert!(generated.contains("let moving : flags = 2l"), "{generated}");
}

#[test]
fn the_case_fold_adds_no_collision_the_frontend_does_not_already_report() {
    // The transform does not invert — `Pose` and `pose` both give
    // `pose` — but the pair cannot occur in one scope: it is the
    // frontend that refuses it, at the layer that can name both IDL
    // declarations.
    let error = try_generate("struct S { long Value; long value; };", "case.idl")
        .expect_err("the case collision was not reported");
    assert!(
        error.starts_with("parse:") || error.starts_with("lower:"),
        "{error}"
    );
}

#[test]
fn two_declarations_that_flatten_onto_one_name_are_refused_by_name() {
    // What the fold does not cause, the flattening does: `a::B_C` and
    // `A_B_C` both give `a_b_c`. OCaml would report `Multiple definition
    // of the type name a_b_c` and name neither IDL declaration.
    let error = try_generate(
        "module a { struct B_C { long x; }; }; struct A_B_C { long y; };",
        "collide.idl",
    )
    .expect_err("the flattening collision was not reported");
    assert!(error.contains("a_b_c"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_type_name_and_a_value_name_do_not_collide_because_ocaml_keeps_them_apart() {
    // `type a_limit = int32` and `let a_limit = 8l` coexist and neither
    // hides the other. A column with one flat name list would refuse a
    // pair the compiler accepts. The two IDL names have to sit in
    // different scopes to reach one OCaml name at all: IDL 4.2 §7.2.3.1
    // forbids `Limit` beside `LIMIT` in one scope.
    let generated = generate("module a { typedef long Limit; }; const long A_LIMIT = 8;");
    assert!(generated.contains("type a_limit = int32"), "{generated}");
    assert!(
        generated.contains("let a_limit : int32 = 8l"),
        "{generated}"
    );
}

#[test]
fn a_sequence_becomes_an_array_and_never_a_list() {
    // A `sequence<T>` goes on the wire as a count followed by the
    // elements (DDS-XTypes 1.3 §7.4.3.5.3). `List.length` is O(n), so an
    // encoder over a list would walk the whole structure to write its
    // first four octets.
    let generated = generate("struct Bag { sequence<long> ids; string name; };");
    assert!(generated.contains("ids : int32 array;"), "{generated}");
    assert!(generated.contains("name : string;"), "{generated}");
    assert!(
        !generated.contains("int32 list"),
        "a list reached the output\n{generated}"
    );
}

#[test]
fn an_array_loses_its_bound_to_the_comment_because_ocaml_has_no_fixed_size_array() {
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("grid : int32 array array;"),
        "{generated}"
    );
    assert!(
        generated.contains("Fixed size in the IDL: [2][3]."),
        "the bound was not recorded\n{generated}"
    );
}

#[test]
fn a_map_becomes_an_array_of_entry_records_and_never_a_hashtbl() {
    // A `map<K,V>` is an ordered sequence of pairs on the wire
    // (DDS-XTypes 1.3 §7.4.1.1.4). `Hashtbl` has no order at all, so one
    // value would give a different octet stream from one run to the
    // next.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        generated.contains("table : zd_map_entry_long_string array;"),
        "{generated}"
    );
    assert!(
        generated.contains("type zd_map_entry_long_string = {"),
        "{generated}"
    );
    assert!(generated.contains("  key : int32;"), "{generated}");
    assert!(generated.contains("  value : string;"), "{generated}");
    assert!(
        !generated.contains("Hashtbl"),
        "a Hashtbl reached the output\n{generated}"
    );
    // The entry has to be declared before the record that names it:
    // OCaml resolves a type name against what precedes it.
    let entry = generated
        .find("type zd_map_entry_long_string")
        .expect("entry");
    let user = generated.find("type s =").expect("s");
    assert!(
        entry < user,
        "the entry is declared after its user\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_an_abbreviation_and_is_declared_before_its_user() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type", which
    // is exactly what an OCaml abbreviation is.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(generated.contains("type distance = int32"), "{generated}");
    assert!(generated.contains("d : distance;"), "{generated}");
    let alias = generated.find("type distance").expect("distance");
    let user = generated.find("type s =").expect("s");
    assert!(alias < user, "{generated}");
}

#[test]
fn an_enum_becomes_a_variant_and_a_pair_of_conversions() {
    // An OCaml variant constructor carries no number, so the numbering
    // DDS-XTypes 1.3 §7.4.3.5.3 puts on the wire is in the conversions
    // or it is lost. `@value` admits gaps, which is why neither
    // conversion may be generated from a position.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(
        generated.contains("type mode =\n  | IDLE\n  | ACTIVE\n  | FAULT\n"),
        "{generated}"
    );
    assert!(
        generated.contains(
            "let int32_of_mode = function\n  | IDLE -> 0l\n  | ACTIVE -> 4l\n  | FAULT -> 5l\n"
        ),
        "{generated}"
    );
    assert!(
        generated.contains(
            "let mode_of_int32 = function\n  | 0l -> IDLE\n  | 4l -> ACTIVE\n  | 5l -> FAULT\n"
        ),
        "{generated}"
    );
    assert!(
        generated.contains("| value -> invalid_arg (Printf.sprintf \"mode_of_int32: %ld\" value)"),
        "the inverse has no answer for a value that names no enumerator\n{generated}"
    );
}

#[test]
fn a_union_becomes_a_record_and_never_a_variant() {
    // The load-bearing decision of this column. An IDL discriminator may
    // hold a value that selects no branch (IDL 4.2 §7.4.1.4.4.1), and an
    // OCaml variant has no constructor for that state.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("type payload = {"), "{generated}");
    assert!(generated.contains("  _d : mode;"), "{generated}");
    assert!(generated.contains("  idle : int32 option;"), "{generated}");
    assert!(
        generated.contains("  active : float option;"),
        "{generated}"
    );
    assert!(
        !generated.contains("type payload =\n  |"),
        "the union became a variant\n{generated}"
    );
}

#[test]
fn a_bitmask_becomes_an_integer_abbreviation_and_one_binding_per_bit() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("type flags = int32"), "{generated}");
    assert!(generated.contains("let ready : flags = 1l"), "{generated}");
    assert!(
        generated.contains("let moving : flags = 16l"),
        "{generated}"
    );
}

#[test]
fn a_sixty_four_bit_bitmask_takes_an_int64_and_its_top_bit_is_negative() {
    // OCaml has no unsigned integer, so bit 63 of a 64-bit mask is
    // −9 223 372 036 854 775 808. The alternative is a literal outside
    // the type's range, which `ocamlc` refuses outright.
    let generated = generate("@bit_bound(64) bitmask Wide { @position(63) TOP };");
    assert!(generated.contains("type wide = int64"), "{generated}");
    assert!(
        generated.contains("let top : wide = -9223372036854775808L"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_record_of_holder_typed_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("type b = {"), "{generated}");
    assert!(generated.contains("low : int;"), "{generated}");
    assert!(generated.contains("high : int;"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_record_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("type v = {"), "{generated}");
    assert!(generated.contains("counter : int32;"), "{generated}");
    assert!(generated.contains("secret : string;"), "{generated}");
}

#[test]
fn an_aggregate_with_no_members_becomes_unit_because_an_empty_record_does_not_parse() {
    // `type t = { }` is a syntax error — an OCaml record has at least
    // one field. `unit` is the type with exactly one value and no data,
    // which is what an IDL aggregate with no members is. The C column
    // refuses the case; OCaml has an answer C does not.
    let generated = generate("module m { @appendable struct S { }; };");
    assert!(generated.contains("type m_s = unit"), "{generated}");
    assert!(
        !generated.contains("= { }") && !generated.contains("= {\n}"),
        "an empty record reached the output\n{generated}"
    );
}

#[test]
fn a_constant_becomes_a_let_of_its_declared_type() {
    let generated = generate("const octet LIMIT = 8;");
    assert!(generated.contains("let limit : int = 8"), "{generated}");
}

#[test]
fn a_long_constant_carries_the_suffix_its_type_needs() {
    // `let n : int32 = 1` does not type-check: OCaml has no implicit
    // conversion between `int` and `int32`.
    let generated = generate("const long N = 1; const long long M = 2;");
    assert!(generated.contains("let n : int32 = 1l"), "{generated}");
    assert!(generated.contains("let m : int64 = 2L"), "{generated}");
}

#[test]
fn a_constant_of_an_enumeration_type_is_written_as_the_constructor() {
    // `let m : mode = 4l` does not type-check — a variant is not an
    // integer — and `mode_of_int32 4l` would be a function call run when
    // the module loads, where the constructor is the same value checked
    // at compile time.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE }; const Mode START = ACTIVE;");
    assert!(
        generated.contains("let start : mode = ACTIVE"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_record_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("type failure = {"), "{generated}");
    assert!(generated.contains("code : int32;"), "{generated}");
    assert!(generated.contains("reason : string;"), "{generated}");
}

#[test]
fn an_interface_becomes_a_class_type_of_method_signatures() {
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(
        generated.contains("class type control = object"),
        "{generated}"
    );
    assert!(
        generated.contains("method go : pose -> unit"),
        "{generated}"
    );
    // A nullary method is a value of its result type; adding `unit ->`
    // would change the signature for no gain.
    assert!(generated.contains("method status : int32"), "{generated}");
    assert!(generated.contains("\nend\n"), "{generated}");
}

#[test]
fn a_native_declaration_becomes_an_opaque_abbreviation_and_not_an_abstract_type() {
    // An abstract type declared in a structure has no value at all, so a
    // record with a member of one could never be built. `Obj.t` says
    // nothing about what is held, which is what `native` says, and
    // leaves the member constructible.
    let generated = generate("native Handle;");
    assert!(generated.contains("type handle = Obj.t"), "{generated}");
}

#[test]
fn a_module_flattens_onto_the_name_because_a_recursive_group_cannot_cross_one() {
    // OCaml does have nested modules and they would be the better
    // spelling. What rules them out is that a `type a = … and b = …`
    // group is one declaration inside one structure, and IDL puts no
    // restriction on where a type cycle may close.
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        generated.contains("type common_util_location = {"),
        "{generated}"
    );
    assert!(
        !generated.contains("module Common"),
        "a nested module reached the output\n{generated}"
    );
    assert!(generated.contains("IDL: `Location`."), "{generated}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(
        !generated.contains("\ntype "),
        "a declaration reached the output\n{generated}"
    );
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    // The order of a group's members and the order of the groups
    // themselves both come out of a graph walk, so this says the walk is
    // deterministic and not merely that the emitter is.
    let idl = "struct A { long x; map<long, string> t; }; \
               struct B { long y; sequence<A> as; }; \
               struct C { long z; sequence<B> bs; sequence<A> as; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_table_claims_for_ocaml_produces_output() {
    let sources = [
        ("struct S { long a; };", "type s = {"),
        ("enum E { A };", "type e =\n  | A"),
        ("typedef long T;", "type t = int32"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "type u = {",
        ),
        ("bitmask B { X };", "type b = int32"),
        ("bitset T { bitfield<3> a; };", "type t = {"),
        ("valuetype V { public long a; };", "type v = {"),
        ("const long N = 1;", "let n : int32 = 1l"),
        ("exception E2 { long code; };", "type e2 = {"),
        ("interface I { void go(); };", "class type i = object"),
        ("native N2;", "type n2 = Obj.t"),
    ];
    for (idl, expected) in sources {
        let generated = generate(idl);
        assert!(
            generated.contains(expected),
            "{idl:?} produced no {expected:?}:\n{generated}"
        );
    }
}

// -- the recursive group -------------------------------------------------

#[test]
fn a_self_recursive_type_is_one_declaration_that_names_itself() {
    let generated = generate("module m { struct Node { long value; sequence<Node> children; }; };");
    assert!(generated.contains("type m_node = {"), "{generated}");
    assert!(
        generated.contains("children : m_node array;"),
        "{generated}"
    );
    assert!(
        generated.contains("A recursive group."),
        "the group was not marked as one\n{generated}"
    );
}

#[test]
fn two_mutually_recursive_types_come_out_as_one_and_joined_group() {
    // The case the C column refuses. OCaml resolves a type name against
    // what precedes it and against its own group, so there is no
    // declaration order that works and the two have to be one `type …
    // and …`.
    let generated = generate(
        "module m {
           struct B;
           struct A { long value; sequence<B> bs; };
           struct B { long weight; sequence<A> as; };
         };",
    );
    assert!(generated.contains("bs : m_b array;"), "{generated}");
    // `as` is an OCaml keyword.
    assert!(generated.contains("as_ : m_a array;"), "{generated}");
    // One `type` and one `and`: the two are one declaration. Which of
    // them the `type` keyword lands on is the model's declaration order,
    // and the model's order here is `B` first, because `struct B;`
    // forward-declares it before `A` is written.
    assert_eq!(
        generated.matches("\ntype m_").count(),
        1,
        "the group was split into two declarations\n{generated}"
    );
    assert_eq!(
        generated.matches("\nand m_").count(),
        1,
        "the second member of the group is not `and`-joined\n{generated}"
    );
}

#[test]
fn a_recursive_group_compiles() {
    let Some(compiler) = ocaml_compiler() else {
        eprintln!(
            "SKIPPED: no OCaml compiler on the path, so the recursive group was not checked."
        );
        return;
    };
    for idl in [
        "module m { struct Node { long value; sequence<Node> children; }; };",
        "module m { struct B; struct A { long value; sequence<B> bs; }; \
         struct B { long weight; sequence<A> as; }; };",
        "module m { struct Node { long value; @optional @external Node next; }; };",
        "module m { struct Node { long value; map<long, Node> children; }; };",
        "module m { struct Node; union Branch switch (long) { case 0: long leaf; \
         default: sequence<Node> nodes; }; struct Node { long value; Branch link; }; };",
    ] {
        let generated = generate(idl);
        if let Err(why) = ocaml_compiles(&compiler, &generated) {
            panic!("{idl}\n\n{generated}\n\n{compiler} said:\n{why}");
        }
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

/// How many corpus cases have to reach OCaml for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it.
const MUST_RENDER: usize = 350;

/// The names the OCaml column itself refuses, each with the reason.
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
    let out = std::env::var("ZD_OCAML_OUT").ok();
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
                    std::fs::write(case.join("zd_types.ml"), text).expect("the output file");
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
            "{} corpus case(s) the frontend refuses, so OCaml never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached OCaml; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the compiler --------------------------------------------------------

/// A counter, so two temporary directories never collide.
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The OCaml byte-code compiler, by whichever name is on the path.
///
/// `ocamlfind ocamlc` is the usual invocation where packages are
/// involved; nothing generated here uses a package outside the standard
/// library, so plain `ocamlc` is enough and is tried first.
fn ocaml_compiler() -> Option<String> {
    for candidate in ["ocamlc", "ocamlc.opt"] {
        let available = Command::new(candidate)
            .arg("-version")
            .output()
            .is_ok_and(|output| output.status.success());
        if available {
            return Some(candidate.to_string());
        }
    }
    None
}

/// One generated file through the compiler.
///
/// `-c`, so it type-checks the structure and stops before linking; the
/// `.cmi` and `.cmo` land beside the source, which is why the source is
/// written into a directory of its own.
fn ocaml_compiles(compiler: &str, source: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-ocaml-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("zd_types.ml");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new(compiler)
        .arg("-c")
        .arg("zd_types.ml")
        .current_dir(&dir)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        Err(text)
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Every shape above, compiled.
///
/// One `ocamlc` per source and not one for all of them: two sources
/// declaring `type s` would collide in one structure, and the point of
/// this test is the shapes rather than their coexistence.
#[test]
fn the_generated_ocaml_compiles() {
    let Some(compiler) = ocaml_compiler() else {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: no `ocamlc` on the path, so the generated OCaml was not compiled. \
             Set it up or run this on a host that has it."
        );
        return;
    };
    // The command is checked before it is trusted. A `ocamlc` invocation
    // that reported success for a file with an unbound type constructor
    // would make every assertion below vacuous, and the failure would
    // arrive as "the column compiles" against a build nobody ran.
    let broken = "type bad = { x : zd_no_such_type; }\n";
    assert!(
        ocaml_compiles(&compiler, broken).is_err(),
        "the compiler check accepted a file with an unbound type constructor"
    );

    let sources = [
        "struct Pose { long x; double y; boolean active; };",
        "struct P { boolean a; octet b; char c; wchar d; int8 e; uint8 f; short g; \
         unsigned short h; long i; unsigned long j; long long k; unsigned long long l; \
         float m; double n; long double o; };",
        "struct S { @optional octet b; long c; string name; sequence<long> ids; \
         long grid[2][3]; map<long, string> table; };",
        "struct T { long type; long method; long option; long array; long match; };",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "@bit_bound(64) bitmask Wide { @position(63) TOP };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "const octet LIMIT = 8; const string TAG = \"abc\"; const long N = 1; \
         const long long M = 2; const double D = 1.5; const boolean B = TRUE;",
        "enum Mode2 { IDLE, @value(4) ACTIVE }; const Mode2 START = ACTIVE;",
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
        "module m { @appendable struct S { }; };",
        "module m { struct Node { long value; sequence<Node> children; }; };",
        // Two enumerations declaring the same constructor name. The
        // second shadows the first from its own declaration onwards,
        // which is why each conversion is emitted beside its own type.
        "module a { enum E { X, Y }; }; module b { enum F { X, Z }; };",
    ];
    for idl in sources {
        let generated = generate(idl);
        if let Err(why) = ocaml_compiles(&compiler, &generated) {
            panic!("{idl}\n\n{generated}\n\n{compiler} said:\n{why}");
        }
    }
}

/// The whole corpus through the compiler, one structure per case.
///
/// `#[ignore]`, because it is one compiler invocation per corpus case
/// and takes minutes. It is what proves the column rather than samples
/// it, so it is run with `--include-ignored` on a host that has OCaml.
#[test]
#[ignore = "one `ocamlc` invocation per corpus case; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    let compiler = ocaml_compiler().expect("the explicitly selected compiler must be installed");
    let mut failures = Vec::new();
    let mut compiled = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = try_generate(&source, &path.display().to_string())
            .unwrap_or_else(|why| panic!("accepted corpus case {name} failed generation: {why}"));
        match ocaml_compiles(&compiler, &text) {
            Ok(()) => compiled += 1,
            Err(why) => failures.push(format!("{name}: {why}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of the corpus did not compile:\n{failures:#?}",
        failures.len()
    );
    assert!(
        compiled >= MUST_RENDER,
        "only {compiled} corpus cases compiled; the gate says nothing below {MUST_RENDER}"
    );
    eprintln!("{compiled} corpus case(s) compiled with OCaml");
}
