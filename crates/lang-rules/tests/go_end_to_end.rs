// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Go, through every layer of the rebuild.
//!
//! The counterpart of `end_to_end.rs`, which does the same for Rust, and
//! of `c_end_to_end.rs`, which does it for C.
//!
//! Two gates and not one:
//!
//! 1. **Every corpus case renders**, or names the layer that refused.
//!    A case that stopped rendering would otherwise be found by whichever
//!    probe ran next, which is a build directory away from the construct
//!    that caused it.
//! 2. **The generated Go compiles**, under `go build` with `go vet`
//!    behind it. A snapshot says the emitter produced the text it
//!    produced last time; it does not say the text is Go, that every
//!    name in it resolves, or that a field's type exists. `go` is
//!    skipped where it is not on the path and the skip says so on
//!    standard error — a test that went green without running it would
//!    be the failure it is meant to catch.
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
use zerodds_lang_rules::adapter::go::go_file;
use zerodds_lang_rules::rule::go;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Go out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &go::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let file = go_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::go::file(&file))
}

/// The generated text with runs of spaces collapsed to one.
///
/// The renderer aligns a struct's type and tag columns the way `gofmt`
/// does, so `X int32 `idl:"x"`` is written with padding whose width
/// depends on the widest field beside it. An assertion on the alignment
/// would be an assertion about the other members of the same struct;
/// `the_generated_go_is_gofmt_stable` is where the alignment itself is
/// checked, against `gofmt` rather than against a literal.
fn squeezed(text: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for ch in text.chars() {
        if ch == ' ' {
            if !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            in_run = false;
            out.push(ch);
        }
    }
    out
}

// -- the mapping ---------------------------------------------------------

#[test]
fn a_struct_of_primitives_becomes_a_go_struct() {
    let generated = generate("struct Pose { long x; double y; boolean active; };");
    assert!(
        squeezed(&generated).contains("type Pose struct {"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("X int32 `idl:\"x\"`"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("Y float64 `idl:\"y\"`"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("Active bool `idl:\"active\"`"),
        "{generated}"
    );
}

#[test]
fn every_idl_primitive_reaches_a_go_type() {
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
        "A bool",
        "B uint8",
        "C uint8",
        "D uint16",
        "E int8",
        "F uint8",
        "G int16",
        "H uint16",
        "I int32",
        "J uint32",
        "K int64",
        "L uint64",
        "M float32",
        "N float64",
    ] {
        assert!(
            squeezed(&generated).contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn no_generated_type_has_an_implementation_defined_width() {
    // `int`, `uint` and `uintptr` are 32 or 64 bits depending on the
    // target (Go spec, "Numeric types"). A member declared with one
    // would change size between a build for amd64 and one for a 32-bit
    // ARM, for a wire format that fixes every width.
    let generated = generate(
        "struct P { long a; unsigned long b; long long c; sequence<long> d; long e[2]; };",
    );
    for forbidden in [" int\n", " uint\n", " uintptr\n", "[]int\n", "[2]int\n"] {
        assert!(
            !squeezed(&generated).contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn a_long_double_is_a_string_and_never_a_float64() {
    // A Go `float64` has 53 bits of significand; an IDL `long double` is
    // a binary128 with 113. Writing one into a `float64` loses sixty
    // bits at the assignment, before any codec runs, with nothing to say
    // it happened. The member holds the decimal instead.
    let generated = generate("struct S { long double big; };");
    assert!(squeezed(&generated).contains("Big string"), "{generated}");
}

#[test]
fn an_optional_member_becomes_a_pointer_and_the_others_do_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7. Go's zero value is
    // indistinguishable from a written zero, so a `long` that arrived as
    // 0 and one that did not arrive at all would be the same value;
    // `nil` is the one state no written value can take.
    let generated = generate("struct S { @optional octet b; long c; };");
    assert!(squeezed(&generated).contains("B *uint8"), "{generated}");
    assert!(squeezed(&generated).contains("C int32"), "{generated}");
    assert!(!squeezed(&generated).contains("C *int32"), "{generated}");
}

#[test]
fn a_member_named_after_a_go_keyword_is_escaped_and_keeps_its_idl_name() {
    // The escape runs before the export, so `type` becomes `Type_` and
    // not `Type`: exporting first would leave the escape nothing to do,
    // and `type` and `Type` would reach one Go name by two routes.
    let generated = generate("struct S { long type; long range; };");
    assert!(
        squeezed(&generated).contains("Type_ int32 `idl:\"type\"`"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("Range_ int32 `idl:\"range\"`"),
        "{generated}"
    );
}

#[test]
fn a_member_named_after_a_predeclared_identifier_is_escaped_too() {
    // Go permits the shadowing and the file compiles until a sibling
    // declaration wants the original. Every field type in a generated
    // file is one of these, and `len` and `cap` are functions a codec
    // written beside the declaration calls.
    let generated = generate("struct S { long len; long cap; };");
    assert!(
        squeezed(&generated).contains("Len_ int32 `idl:\"len\"`"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("Cap_ int32 `idl:\"cap\"`"),
        "{generated}"
    );
}

#[test]
fn two_members_that_would_reach_one_go_name_are_refused_by_name() {
    // Not the case-only pair — IDL 4.2 §7.2.3.1 already forbids `value`
    // beside `Value` in one scope, and the frontend reports it. What is
    // left is the pair the keyword escape makes: `type` escapes to
    // `type_` and exports to `Type_`, and a member already spelled
    // `type_` exports to the same name. Go would accept neither
    // declaration twice, but a column that emitted both and let the
    // compiler find it would name a Go identifier and not the two IDL
    // members behind it.
    let error = try_generate("struct S { long type; long type_; };", "collide.idl")
        .expect_err("the collision was not reported");
    assert!(error.contains("Type_"), "{error}");
    assert!(error.contains("unreachable"), "{error}");
}

#[test]
fn a_sequence_becomes_a_slice_and_a_string_a_string() {
    let generated = generate("struct Bag { sequence<long> ids; string name; };");
    assert!(squeezed(&generated).contains("Ids []int32"), "{generated}");
    assert!(squeezed(&generated).contains("Name string"), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first() {
    // `long grid[2][3]` is two rows of three, which in Go is
    // `[2][3]int32`. Reversing it silently transposes the data.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        squeezed(&generated).contains("Grid [2][3]int32"),
        "{generated}"
    );
}

#[test]
fn a_map_becomes_a_slice_of_entries_and_never_a_go_map() {
    // Go randomises map iteration order on purpose (Go spec, "For
    // statements with range clause"), and a `map<K,V>` goes on the wire
    // as a sequence of pairs (DDS-XTypes 1.3 §7.4.1.1.4). A Go `map`
    // would give a different octet stream on every run of one program
    // over one value.
    let generated = generate("struct S { map<long, string> table; };");
    assert!(
        squeezed(&generated).contains("Table []ZdMapEntry_Long_String"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("type ZdMapEntry_Long_String struct {"),
        "{generated}"
    );
    assert!(
        !squeezed(&generated).contains("map[int32]string"),
        "a Go map reached the output\n{generated}"
    );
}

#[test]
fn a_typedef_becomes_an_alias_and_not_a_defined_type() {
    // IDL 4.2 §7.4.1.4.3: an alias "does not define a new type".
    // `type Distance int32` would, and a value of it would then need an
    // explicit conversion to reach an `int32`.
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(
        squeezed(&generated).contains("type Distance = int32"),
        "{generated}"
    );
    assert!(squeezed(&generated).contains("D Distance"), "{generated}");
}

#[test]
fn an_enum_becomes_a_defined_type_with_its_numbering_written_out() {
    // Not an `iota` block: `iota` numbers consecutively from zero and
    // `@value` admits gaps, so an `iota` block would renumber the wire.
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(
        squeezed(&generated).contains("type Mode int32"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("const IDLE Mode = 0"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("const ACTIVE Mode = 4"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("const FAULT Mode = 5"),
        "{generated}"
    );
    assert!(
        !squeezed(&generated).contains("iota"),
        "an iota block reached the output\n{generated}"
    );
}

#[test]
fn a_union_becomes_a_struct_with_a_discriminator_and_one_field_per_branch() {
    // Go has no overlaid storage. The branches sit side by side and `D`
    // says which one carries the value; only that branch is written
    // (DDS-XTypes 1.3 §7.4.1.1.5.2).
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(
        squeezed(&generated).contains("type Payload struct {"),
        "{generated}"
    );
    assert!(squeezed(&generated).contains("D Mode"), "{generated}");
    assert!(squeezed(&generated).contains("Idle int32"), "{generated}");
    assert!(
        squeezed(&generated).contains("Active float64"),
        "{generated}"
    );
}

#[test]
fn a_bitmask_becomes_an_unsigned_type_and_one_constant_per_bit() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(
        squeezed(&generated).contains("type Flags uint16"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("const READY Flags = 1 << 0"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("const MOVING Flags = 1 << 4"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_struct_of_holder_typed_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(
        squeezed(&generated).contains("type B struct {"),
        "{generated}"
    );
    assert!(squeezed(&generated).contains("Low uint8"), "{generated}");
    assert!(squeezed(&generated).contains("High uint16"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_struct_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(
        squeezed(&generated).contains("type V struct {"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("Counter int32"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("Secret string"),
        "{generated}"
    );
}

#[test]
fn a_constant_becomes_a_go_constant_of_its_declared_type() {
    let generated = generate("const octet LIMIT = 8;");
    assert!(
        squeezed(&generated).contains("const LIMIT uint8 = 8"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_struct_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(
        squeezed(&generated).contains("type Failure struct {"),
        "{generated}"
    );
    assert!(squeezed(&generated).contains("Code int32"), "{generated}");
    assert!(
        squeezed(&generated).contains("Reason string"),
        "{generated}"
    );
}

#[test]
fn an_interface_becomes_a_go_interface_of_signatures() {
    // Covered here and not in the C column: a Go interface is a method
    // set with names, which is what an IDL interface declares.
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(
        squeezed(&generated).contains("type Control interface {"),
        "{generated}"
    );
    // `go` is a Go keyword, so the operation escapes before it exports.
    // Every method also returns an `error` besides its own result — Go's
    // own idiom for a call that can fail, which every DDS-RPC operation
    // can.
    assert!(
        squeezed(&generated).contains("Go_(target Pose) error"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("Status() (int32, error)"),
        "{generated}"
    );
}

#[test]
fn a_native_declaration_becomes_an_opaque_alias() {
    let generated = generate("native Handle;");
    assert!(
        squeezed(&generated).contains("type Handle = any"),
        "{generated}"
    );
}

#[test]
fn a_module_flattens_onto_the_name_because_a_go_package_is_a_directory() {
    let generated = generate("module common { module util { struct Location { long x; }; }; };");
    assert!(
        squeezed(&generated).contains("type Common_Util_Location struct {"),
        "{generated}"
    );
    assert!(
        squeezed(&generated).contains("IDL: `Location`."),
        "the IDL name was not recorded\n{generated}"
    );
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Go equivalent, and the coverage
    // table says so. What is left is the package clause and the file
    // comment.
    let generated = generate("@annotation Sensitive { string reason; };");
    assert!(
        squeezed(&generated).contains("package zdtypes"),
        "{generated}"
    );
    assert!(!squeezed(&generated).contains("type "), "{generated}");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("type First").expect("First");
    let second = generated.find("type Second").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn every_construct_the_table_claims_for_go_produces_output() {
    // The table says a rule covers it; this says the rule reaches text.
    let sources = [
        ("struct S { long a; };", "type S struct"),
        ("enum E { A };", "type E int32"),
        ("typedef long T;", "type T = int32"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "type U struct",
        ),
        ("bitmask B { X };", "type B uint32"),
        ("bitset T { bitfield<3> a; };", "type T struct"),
        ("valuetype V { public long a; };", "type V struct"),
        ("const long N = 1;", "const N int32 = 1"),
        ("exception E2 { long code; };", "type E2 struct"),
        ("interface I { void go(); };", "type I interface"),
        ("native N2;", "type N2 = any"),
    ];
    for (idl, expected) in sources {
        let generated = generate(idl);
        assert!(
            squeezed(&generated).contains(expected),
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

/// How many corpus cases have to reach Go for this gate to mean
/// anything.
///
/// A source that does not parse or does not lower says nothing about a
/// target language — the Rust column's own corpus gate reports the same
/// ones — so those are counted and printed rather than failed on here.
/// What would make this test vacuous is a frontend that refused
/// everything, and this is the number that stops it: the corpus is 378
/// cases and the column has to carry substantially all of them.
const MUST_RENDER: usize = 350;

/// The names the Go column itself refuses, each with the reason.
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
    let out = std::env::var("ZD_GO_OUT").ok();
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
                    std::fs::write(case.join("zd_types.go"), text).expect("the output file");
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
            "{} corpus case(s) the frontend refuses, so Go never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached Go; the gate says nothing below {MUST_RENDER}"
    );
}

// -- the formatter -------------------------------------------------------

/// One generated file through `gofmt`, which returns it reformatted.
fn gofmt(source: &str) -> Result<String, String> {
    let dir =
        std::env::temp_dir().join(format!("zd-go-fmt-{}-{}", std::process::id(), source.len()));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("zd_types.go");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("gofmt")
        .arg(&file)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// The whole corpus, byte-identical to what `gofmt` would write.
///
/// Not a style preference. `gofmt` is not optional in Go — every reader
/// of a generated file runs it, and a file that changes under it is a
/// diff nobody asked for, on a file nobody edited. The renderer
/// therefore aligns a struct's type and tag columns itself
/// (`zerodds_lang_render::go::render_fields`), and this is what says it
/// got the alignment right rather than merely plausible.
///
/// `#[ignore]`d with the other whole-corpus gates: it is one `gofmt`
/// invocation per case.
#[test]
#[ignore = "one `gofmt` per corpus case; run with --include-ignored"]
fn the_generated_go_is_gofmt_stable() {
    Command::new("gofmt")
        .arg("--help")
        .output()
        .expect("the selected gate requires gofmt");
    let mut differ = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let text = try_generate(&source, &path.display().to_string())
            .unwrap_or_else(|why| panic!("accepted corpus case {name} failed generation: {why}"));
        checked += 1;
        match gofmt(&text) {
            Ok(formatted) if formatted == text => {}
            Ok(_) => differ.push(name),
            Err(why) => differ.push(format!("{name}: gofmt refused: {why}")),
        }
    }
    assert!(
        differ.is_empty(),
        "{} of {checked} generated files change under gofmt:\n{differ:#?}",
        differ.len()
    );
    eprintln!("{checked} corpus case(s) checked by gofmt");
    assert!(
        checked >= MUST_RENDER,
        "too few corpus cases reached the selected gate: {checked}"
    );
}

// -- the compiler --------------------------------------------------------

/// One generated file through `go build`.
///
/// `go vet` is not run separately: `go build` resolves every name and
/// checks every type, which is what this test is about, and `vet`'s own
/// checks are about calls and formats that a types-only file has none
/// of.
fn go_compiles(source: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-go-compile-{}-{}",
        std::process::id(),
        source.len()
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    std::fs::write(dir.join("zd_types.go"), source).map_err(|error| error.to_string())?;
    // A module file, because `go build` outside one refuses before it
    // reads a single declaration.
    std::fs::write(dir.join("go.mod"), "module zdtypes\n\ngo 1.21\n")
        .map_err(|error| error.to_string())?;
    let output = Command::new("go")
        .arg("build")
        .arg("./...")
        .current_dir(&dir)
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

/// Whether `go` is on the path at all.
fn go_available() -> bool {
    Command::new("go")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Every shape above, compiled.
///
/// One test and not one per shape, because a `go build` costs a second
/// and the sources concatenate into one package cleanly — every
/// declaration in them has a name of its own.
#[test]
fn the_generated_go_compiles() {
    if !go_available() {
        // Loud, on the channel a passing test does not use. A green test
        // that never ran the compiler is exactly the failure this test
        // exists to catch.
        eprintln!(
            "SKIPPED: `go` is not on the path, so the generated Go was not compiled. \
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
        "struct T { long type; long range; long len; };",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "const octet LIMIT = 8; const string TAG = \"abc\";",
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
    ];
    for idl in sources {
        let generated = generate(idl);
        if let Err(why) = go_compiles(&generated) {
            panic!("{idl}\n\n{generated}\n\ngo build said:\n{why}");
        }
    }
}

/// The whole corpus through `go build`, one package per case.
///
/// `#[ignore]`, because it is 378 invocations of a compiler and takes
/// minutes. It is what proves the column rather than samples it, so it
/// is run with `--include-ignored` on a host that has Go and its result
/// is what `docs/language-columns-status.md` records.
#[test]
#[ignore = "378 `go build` invocations; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    assert!(
        go_available(),
        "the explicitly selected go gate requires its compiler"
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
        if let Err(why) = go_compiles(&text) {
            failures.push(format!("{name}: {why}"));
        }
    }
    eprintln!("{checked} corpus case(s) reached go build");
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
