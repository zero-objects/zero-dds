// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Rust, through every layer of the rebuild.
//!
//! Parse, lower, adapt, apply, materialise, render. A test at any one
//! layer can pass while the chain is broken; this is the one that
//! cannot.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "support/resolved.rs"]
mod resolved;

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::cst::rust_items;
use zerodds_lang_rules::rule::rust;
use zerodds_lang_rules::{ModelGraph, apply};

/// IDL in, Rust out.
fn generate(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );

    let ast: Ast = lowered.ast;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply");
    let items = rust_items(&ast, &applied).expect("materialise");

    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    zerodds_lang_render::rust::file(&file)
}

#[test]
fn a_struct_of_primitives_becomes_a_rust_struct() {
    assert_eq!(
        generate("struct Pose { long x; double y; boolean active; };"),
        "\
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    pub x: i32,
    pub y: f64,
    pub active: bool,
}
"
    );
}

#[test]
fn every_idl_primitive_reaches_a_rust_type() {
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
        "pub a: bool,",
        "pub b: u8,",
        "pub c: char,",
        "pub d: char,",
        "pub e: i8,",
        "pub f: u8,",
        "pub g: i16,",
        "pub h: u16,",
        "pub i: i32,",
        "pub j: u32,",
        "pub k: i64,",
        "pub l: u64,",
        "pub m: f32,",
        "pub n: f64,",
    ] {
        assert!(
            generated.contains(expected),
            "missing {expected}\n{generated}"
        );
    }
}

#[test]
fn an_optional_member_becomes_an_option_and_the_others_do_not() {
    // DDS-XTypes 1.3 §7.2.2.4.4.4.7: "the member may or may not be
    // present in a given object of that type". A field of the member's
    // own type has nowhere to put the absence rule (19) and rule (20)
    // both mark, so the decoder could only refuse a sample the other
    // three columns hand back.
    assert_eq!(
        generate("struct S { @optional octet b; long c; };"),
        "\
#[derive(Debug, Clone, PartialEq)]
pub struct S {
    pub b: Option<u8>,
    pub c: i32,
}
"
    );
}

#[test]
fn a_member_named_after_a_rust_keyword_is_escaped() {
    // `r#type` rather than `type_`: the reader of the generated code
    // sees the field the IDL declared.
    let generated = generate("struct S { long type; long match; };");
    assert!(generated.contains("pub r#type: i32,"), "{generated}");
    assert!(generated.contains("pub r#match: i32,"), "{generated}");
}

#[test]
fn a_sequence_becomes_a_vec_and_a_string_a_string() {
    let generated = generate("struct Bag { sequence<long> ids; string name; };");
    assert!(generated.contains("pub ids: Vec<i32>,"), "{generated}");
    assert!(generated.contains("pub name: String,"), "{generated}");
}

#[test]
fn an_array_nests_outermost_dimension_first() {
    // `long grid[2][3]` is two rows of three, which in Rust is
    // `[[i32; 3]; 2]`. Reversing it silently transposes the data.
    let generated = generate("struct S { long grid[2][3]; };");
    assert!(
        generated.contains("pub grid: [[i32; 3]; 2],"),
        "{generated}"
    );
}

#[test]
fn a_member_of_a_named_type_refers_to_it_by_name() {
    let generated = generate(
        "struct Header { long id; };
         struct Body { Header head; };",
    );
    assert!(generated.contains("pub head: Header,"), "{generated}");
}

#[test]
fn a_typedef_resolves_to_what_it_aliases() {
    let generated = generate(
        "typedef long Distance;
         struct S { Distance d; };",
    );
    assert!(generated.contains("pub d: i32,"), "{generated}");
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    // Everything in the chain is deterministic: the model's identities,
    // the graph built from them, the order the items are walked in.
    let idl = "struct A { long x; }; struct B { long y; long z; };";
    assert_eq!(generate(idl), generate(idl));
}

#[test]
fn an_enum_becomes_a_rust_enum_with_its_numbering_written_out() {
    // Without discriminants a Rust enum renumbers silently when a
    // variant is inserted, and the wire format is the IDL's numbering,
    // not Rust's.
    assert_eq!(
        generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };"),
        "\
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    IDLE = 0,
    ACTIVE = 4,
    FAULT = 5,
}
"
    );
}

#[test]
fn a_typedef_becomes_a_type_alias_as_well_as_resolving_in_a_member() {
    let generated = generate("typedef long Distance; struct S { Distance d; };");
    assert!(
        generated.contains("pub type Distance = i32;"),
        "{generated}"
    );
    assert!(generated.contains("pub d: i32,"), "{generated}");
}

#[test]
fn a_construct_with_no_rule_produces_nothing_rather_than_something_wrong() {
    // A user-declared annotation has no Rust equivalent, and the
    // coverage table says so. Emitting an approximation would be worse
    // than emitting nothing.
    assert_eq!(generate("@annotation Sensitive { string reason; };"), "");
}

#[test]
fn the_declaration_order_of_the_source_is_the_order_of_the_output() {
    let generated = generate("struct First { long a; }; struct Second { long b; };");
    let first = generated.find("struct First").expect("First");
    let second = generated.find("struct Second").expect("Second");
    assert!(first < second, "the output reordered the declarations");
}

// -- every construct the Rust catalogue claims ---------------------------

#[test]
fn a_union_becomes_an_enum_with_one_variant_per_case() {
    // Rust's untagged union is unsafe, and the discriminator already is
    // the tag; an enum says the same thing safely.
    let generated = generate(
        "enum Mode { IDLE, ACTIVE };
         union Payload switch (Mode) {
           case IDLE: long idle;
           case ACTIVE: double active;
         };",
    );
    assert!(generated.contains("pub enum Payload {"), "{generated}");
    assert!(generated.contains("Idle(i32),"), "{generated}");
    assert!(generated.contains("Active(f64),"), "{generated}");
}

#[test]
fn a_bitmask_becomes_an_integer_and_one_constant_per_bit() {
    // An enum could not hold two bits at once, which is the point of a
    // bitmask; the constants are the positions written out.
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("pub type Flags = u16;"), "{generated}");
    assert!(
        generated.contains("pub const READY: u16 = 1 << 0;"),
        "{generated}"
    );
    assert!(
        generated.contains("pub const MOVING: u16 = 1 << 4;"),
        "{generated}"
    );
}

#[test]
fn a_bitset_becomes_a_struct_of_holder_typed_fields() {
    let generated = generate("bitset B { bitfield<3> low; bitfield<12> high; };");
    assert!(generated.contains("pub struct B {"), "{generated}");
    assert!(generated.contains("pub low: u8,"), "{generated}");
    assert!(generated.contains("pub high: u16,"), "{generated}");
}

#[test]
fn a_valuetype_becomes_a_struct_of_its_state() {
    let generated = generate("valuetype V { public long counter; private string secret; };");
    assert!(generated.contains("pub struct V {"), "{generated}");
    assert!(generated.contains("pub counter: i32,"), "{generated}");
    assert!(generated.contains("pub secret: String,"), "{generated}");
}

#[test]
fn a_constant_becomes_a_rust_constant_of_its_declared_type() {
    let generated = generate("const octet LIMIT = 8;");
    assert!(
        generated.contains("pub const LIMIT: u8 = 8;"),
        "{generated}"
    );
}

#[test]
fn an_exception_becomes_a_struct_of_its_members() {
    let generated = generate("exception Failure { long code; string reason; };");
    assert!(generated.contains("pub struct Failure {"), "{generated}");
    assert!(generated.contains("pub code: i32,"), "{generated}");
    assert!(generated.contains("pub reason: String,"), "{generated}");
}

#[test]
fn an_interface_becomes_a_trait_of_signatures() {
    let generated = generate(
        "struct Pose { long x; };
         interface Control {
           void go(in Pose target);
           long status();
         };",
    );
    assert!(generated.contains("pub trait Control {"), "{generated}");
    assert!(
        generated.contains("fn go(&self, target: Pose);"),
        "{generated}"
    );
    assert!(
        generated.contains("fn status(&self) -> i32;"),
        "{generated}"
    );
}

#[test]
fn a_native_declaration_becomes_an_opaque_alias() {
    // Never a guess at what the name might mean: the concrete type comes
    // from the facet a `#pragma` or an annotation set, and there is none
    // here.
    let generated = generate("native Handle;");
    assert!(generated.contains("pub type Handle = &u8;"), "{generated}");
}

#[test]
fn every_construct_the_table_claims_for_rust_produces_output() {
    // The table says a rule covers it; this says the rule reaches text.
    // A table green on rules that emit nothing would be worse than a
    // table that admitted the gap.
    let sources = [
        ("struct S { long a; };", "pub struct S"),
        ("enum E { A };", "pub enum E"),
        ("typedef long T;", "pub type T"),
        (
            "enum M { A }; union U switch (M) { case A: long a; };",
            "pub enum U",
        ),
        ("bitmask B { X };", "pub type B"),
        ("bitset T { bitfield<3> a; };", "pub struct T"),
        ("valuetype V { public long a; };", "pub struct V"),
        ("const long N = 1;", "pub const N"),
        ("exception E2 { long code; };", "pub struct E2"),
        ("interface I { void go(); };", "pub trait I"),
        ("native N2;", "pub type N2"),
    ];
    for (idl, expected) in sources {
        let generated = generate(idl);
        assert!(
            generated.contains(expected),
            "{idl:?} produced no {expected:?}:\n{generated}"
        );
    }
}

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

/// The same, but naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    // As `idlc` and `compose` do, and every sibling gate: a scoped name
    // across modules is resolved here, not by `lower`. Without it a union
    // whose discriminator is `::k::Kind` reached the Rust column with the
    // discriminator unresolved.
    let ast = resolved::ast(idl, origin)?;
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).map_err(|error| format!("apply: {error}"))?;
    let items = rust_items(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    Ok(zerodds_lang_render::rust::file(&file))
}

/// The whole corpus, through every layer, with nothing skipped.
///
/// The five other language columns each carry this test in their own
/// adapter; the Rust column is the one every other column in the ledger
/// is measured against, and it had no such gate — a case that stopped
/// rendering would have been found by whichever probe ran next.
///
/// `ZD_RUST_OUT` names a directory to write the modules into, the same
/// switch the other columns spell `ZD_PYTHON_OUT` and `ZD_TS_OUT`.
/// `tools/wireprobe/collect-generated` uses it to fill the Rust column
/// of the side-by-side view.
#[test]
fn every_corpus_case_renders_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let out = std::env::var("ZD_RUST_OUT").ok();
    let mut failures = Vec::new();
    for (name, path) in cases {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        match try_generate(&source, &path.display().to_string()) {
            Ok(text) => {
                if let Some(dir) = &out {
                    let case = std::path::Path::new(dir).join(&name);
                    std::fs::create_dir_all(&case).expect("the output directory");
                    std::fs::write(case.join("zd_types.rs"), text).expect("the output file");
                }
            }
            Err(why) => failures.push(format!("{name}: {why}")),
        }
    }
    let failures = without_the_front_end_refusals(failures);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The corpus cases the IDL front end refuses, so that no column ever
/// sees a model for them.
///
/// `resolve-module-named-as-type` is `module common { struct common
/// { … }; }`. OMG IDL 4.2 §7.5.2: "The name of a module, structure,
/// union, map, interface, value type, event type, exception or home may
/// not be redefined within the immediate scope of the module, …", with
/// `module M { typedef short M; }` printed as the clause's own
/// `// Error` example. The case is in the corpus deliberately — it
/// measures which vendors refuse the file and which resolve it anyway —
/// and `tools/idlc/tests/corpus.rs` carries it as an `Intended`
/// diagnostic for that reason.
///
/// The lib-side walks share `zerodds_lang_rules::adapter`'s copy of this
/// list; an integration test cannot see a `#[cfg(test)]` item, so it is
/// written out here.
const REFUSED_BY_THE_FRONT_END: &[&str] = &["resolve-module-named-as-type"];

/// A walk's failure lines, minus the ones [`REFUSED_BY_THE_FRONT_END`]
/// accounts for.
///
/// Checked in both directions: a line that is not on the list stays a
/// failure, and a listed case that produced no line at all is reported
/// rather than passing silently. An exemption nothing checks is an
/// exemption that outlives its cause.
fn without_the_front_end_refusals(failures: Vec<String>) -> Vec<String> {
    let mut rest = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for line in failures {
        match REFUSED_BY_THE_FRONT_END
            .iter()
            .find(|case| line.starts_with(&format!("{case}: ")))
        {
            Some(case) => seen.push(case),
            None => rest.push(line),
        }
    }
    for case in REFUSED_BY_THE_FRONT_END {
        assert!(
            seen.contains(case),
            "`{case}` is listed as refused by the IDL front end and this walk rendered it \
             instead; the list has outlived its cause"
        );
    }
    rest
}
