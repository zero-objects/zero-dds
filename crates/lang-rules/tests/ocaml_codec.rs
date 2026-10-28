// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the OCaml backend.
//!
//! What this proves is not that the octets are right — that is the
//! matrix's, `tools/wireprobe/wireprobe-zerodds-ocaml` against the Rust
//! column's recorded octets — but that the walk reaches the end for the
//! shapes the corpus has and that what comes out is OCaml rather than C
//! with different punctuation. OCaml adds hazards no earlier column
//! had: there are no statements, no `return`, no `break` and no
//! implicit integer conversion, so a body that renders at all can still
//! be one missing `Int64.of_int32` away from not compiling. Every shape
//! below therefore ends in `ocamlc` where a toolchain is on the path —
//! the only reader that can tell the two apart.
// `print_stderr`: the one `eprintln!` says that `ocamlc` was not on the
// path and the build was therefore not checked. A test that quietly
// checks nothing is worse than one that says it did not run.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::Command;
use zerodds_ast::Ast;
use zerodds_lang_rules::default_ocaml::ocaml_defaulters_called_by;
use zerodds_lang_rules::emit::ocaml::OCaml;
use zerodds_lang_rules::{ModelGraph, apply};

/// One IDL source, as the whole OCaml module its codec needs: the
/// types, Table 9's defaulters where the read direction is asked for,
/// and the codec itself.
fn module(idl: &str, type_name: &str, writing: bool) -> String {
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
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    let model = ModelGraph::of(&ast);
    let applied =
        apply(&model, &zerodds_lang_rules::rule::ocaml::catalogue()).expect("the rules apply");
    let file = zerodds_lang_rules::adapter::ocaml::ocaml_file(&ast, &applied)
        .expect("the types materialise");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    let mut out = zerodds_lang_render::ocaml::file(&file);
    if writing {
        let codec = zerodds_lang_rules::emit::encode::codec_from::<OCaml>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the write walk reaches the end");
        out.push_str(&zerodds_lang_render::ocaml::codec_chain(&codec));
    } else {
        let decoders = zerodds_lang_rules::emit::decode::decoder_from::<OCaml>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end");
        let defaulters = ocaml_defaulters_called_by(&ast, &decoders).expect("Table 9 answers");
        out.push_str(&zerodds_lang_render::ocaml::value_chain(&defaulters));
        out.push_str(&zerodds_lang_render::ocaml::codec_chain(&decoders));
    }
    out
}

fn encoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, true)
}

fn decoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, false)
}

/// Whether `ocamlc` is on the path at all.
fn ocaml_available() -> bool {
    Command::new("ocamlc")
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// One build directory per call.
///
/// The name was the process id and the module's length, and two
/// `#[test]` functions run as two threads of one process: a pair of
/// modules of equal length shared a directory, and whichever finished
/// first removed it under the other's `ocamlc` — which then reported
/// `I/O error: ./zdwire.cmi….tmp: No such file or directory` and the
/// failure read as a defect in the generated OCaml. A counter is unique
/// within the process and the process id separates two runs.
static BUILDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Hand the module to `ocamlc`, against the real `runtime/ocaml`.
///
/// Skipped, loudly, where no toolchain is on the path — a test that
/// silently checks nothing is worse than one that says it did not run.
/// `-w -a` because generated code is compiled without warnings, the
/// same choice the probe driver makes; the *errors* are the gate.
fn compiles(text: &str) {
    if !ocaml_available() {
        eprintln!("no `ocamlc` on the path: the generated module was not compiled");
        return;
    }
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/ocaml/zdwire.ml")
        .canonicalize()
        .expect("runtime/ocaml");
    let dir = std::env::temp_dir().join(format!(
        "zd-ocaml-codec-{}-{}",
        std::process::id(),
        BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("the build directory");
    std::fs::copy(&runtime, dir.join("zdwire.ml")).expect("the runtime copy");
    std::fs::write(dir.join("zd_types.ml"), text).expect("the module");
    let output = Command::new("ocamlc")
        .args(["-w", "-a", "-c", "zdwire.ml", "zd_types.ml"])
        .current_dir(&dir)
        .output()
        .expect("ocamlc runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the generated module is not OCaml:\n{stderr}\n--- source ---\n{text}"
    );
}

#[test]
fn a_structure_of_primitives_becomes_an_ocaml_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in [
        "let rec zd_enc_",
        "Zdwire.write_i32 w",
        "Zdwire.write_f64 w",
        "Zdwire.write_bool w",
        "v.x",
    ] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    // Nothing from another column's runtime, and nothing from another
    // column's syntax.
    for foreign in ["zdw_", "->", "WriteI32", "(void)", "v.X"] {
        assert!(!text.contains(foreign), "{foreign} reached OCaml:\n{text}");
    }
    compiles(&text);
}

#[test]
fn the_read_direction_opens_with_table_9_and_answers_the_cell() {
    let text = decoder("@final struct S { long a; string b; };", "S");
    assert!(
        text.contains("let v = ref (zd_def_s ()) in"),
        "the decoder does not start from Table 9:\n{text}"
    );
    assert!(text.contains("!v"), "the sample is not answered:\n{text}");
    assert!(
        text.contains("zd_def_s () : s ="),
        "the defaulter is missing:\n{text}"
    );
    compiles(&text);
}

#[test]
fn a_member_store_is_a_functional_record_update() {
    let text = decoder("@final struct S { long a; };", "S");
    assert!(
        text.contains("v := { !v with a ="),
        "the member store is not a functional update:\n{text}"
    );
    compiles(&text);
}

#[test]
fn an_optional_member_reads_into_some_and_absents_into_none() {
    let text = decoder("@final struct S { @optional long a; };", "S");
    assert!(
        text.contains("a = Some"),
        "the optional member does not wrap in Some:\n{text}"
    );
    assert!(
        text.contains("a = None"),
        "the absent case does not write None:\n{text}"
    );
    compiles(&text);
}

#[test]
fn an_enumeration_crosses_through_its_conversions() {
    let text = encoder(
        "enum Colour { RED, GREEN, BLUE }; @final struct S { Colour tint; };",
        "S",
    );
    assert!(
        text.contains("int32_of_colour"),
        "the enumeration's number does not go through its conversion:\n{text}"
    );
    compiles(&text);
}

/// A bitmask is a *pattern* and an enumeration holder is a *number*,
/// and OCaml spells both widenings `int64`.
///
/// Table 33 gives a bitmask of at most 32 bits an `int32` holder, so a
/// bound of 32 with bit 31 set is negative there and `Int64.of_int32`
/// alone would carry the sign into bits 32..63 — past the bound the
/// runtime measures against, which then refuses a value inside it. The
/// enumeration holder is signed by §7.4.3.5.3 rule (5) and must keep
/// its sign, so the two cannot share one conversion.
#[test]
fn a_bitmask_widens_by_its_pattern_and_an_enumeration_by_its_sign() {
    let mask = encoder(
        "@bit_bound(32) bitmask Flags { @position(0) LOW, @position(31) HIGH }; \
         @final struct S { Flags set; };",
        "S",
    );
    assert!(
        mask.contains("Zdwire.write_bitmask w 32 (Int64.logand (Int64.of_int32"),
        "the bitmask's widening does not keep the pattern:\n{mask}"
    );
    compiles(&mask);

    let holder = encoder(
        "@bit_bound(32) enum Colour { RED, GREEN, BLUE }; @final struct S { Colour tint; };",
        "S",
    );
    assert!(
        holder.contains("Zdwire.write_enum w 32 (Int64.of_int32 (int32_of_colour"),
        "the enumeration holder's widening does not keep the sign:\n{holder}"
    );
    compiles(&holder);
}

#[test]
fn a_mutable_aggregate_compiles_with_its_seen_flags() {
    let text = decoder(
        "@mutable struct S { @id(1) long a; @id(2) string b; };",
        "S",
    );
    assert!(
        text.contains("zd_seen0"),
        "the seen-flags are missing:\n{text}"
    );
    compiles(&text);
}

/// Every aggregate the corpus has, through both directions, to the end
/// of the walk; each half is compiled against the real runtime.
#[test]
fn the_shapes_the_corpus_has_reach_the_end_of_both_walks() {
    for (idl, root) in [
        ("@final struct S { long a; };", "S"),
        ("@appendable struct S { long a; string b; };", "S"),
        (
            "@mutable struct S { @id(1) long a; @id(2) sequence<double> b; };",
            "S",
        ),
        ("@final struct S { long a[3]; };", "S"),
        ("@final struct S { long g[2][3]; };", "S"),
        ("@final struct S { sequence<string> a; };", "S"),
        ("@final struct S { map<string, long> a; };", "S"),
        ("@final struct S { map<boolean, long> a; };", "S"),
        (
            "@final struct S { @optional long a; @optional string b; };",
            "S",
        ),
        (
            "enum Colour { RED, GREEN, BLUE }; @final struct S { Colour tint; };",
            "S",
        ),
        (
            "@bit_bound(16) bitmask Flags { @position(0) A, @position(3) B }; \
             @final struct S { Flags set; };",
            "S",
        ),
        (
            "@bit_bound(32) bitmask Wide { @position(0) A, @position(31) TOP }; \
             @final struct S { Wide set; };",
            "S",
        ),
        (
            "@bit_bound(64) bitmask Widest { @position(0) A, @position(63) TOP }; \
             @final struct S { Widest set; };",
            "S",
        ),
        (
            "@final struct S { map<long, map<long, long> > nested; };",
            "S",
        ),
        (
            "bitset Packed { bitfield<3> low; bitfield<5> high; }; \
          @final struct S { Packed p; };",
            "S",
        ),
        ("@final struct S { wchar c; wstring w; };", "S"),
        ("@final struct S { long double d; };", "S"),
        ("@final struct S { fixed<5,2> f; };", "S"),
        (
            "@final struct Inner { long a; }; @final struct S { Inner i; };",
            "S",
        ),
        (
            "@final union U switch (long) { case 1: long one; default: string other; }; \
             @final struct S { U pick; };",
            "S",
        ),
        (
            "enum Colour { RED, GREEN, BLUE }; \
             @final union U switch (Colour) { case RED: long one; case GREEN: case BLUE: \
             double two; }; @final struct S { U pick; };",
            "S",
        ),
        (
            "module a { module b { @final struct S { long x; }; }; };",
            "S",
        ),
        ("@final struct Node { sequence<Node> children; };", "Node"),
        (
            "typedef long Distance; @final struct S { Distance d; };",
            "S",
        ),
        ("@final struct S { unsigned long long big; };", "S"),
    ] {
        let written = encoder(idl, root);
        assert!(!written.is_empty(), "no encoder for {idl}");
        compiles(&written);
        let read = decoder(idl, root);
        assert!(!read.is_empty(), "no decoder for {idl}");
        compiles(&read);
    }
}

/// A type the model does not hold gets no defaulter, and says so.
///
/// `value_of` used to answer `Ok(None)` for a handle `Ast::type_`
/// refuses, and `ocaml_defaulters_over` reads that `None` as "this type
/// has no OCaml data form" and skips it. The two together made a stale
/// handle indistinguishable from a `native`: the chain came back
/// without the defaulter and without a word, and a decoder that opens
/// with `let v = ref (zd_def_T ()) in` then names a function nothing
/// binds. `default_lua`, `default_elixir` and `default_julia` already
/// refused it; this holds the OCaml column to the same answer.
#[test]
fn a_type_the_model_does_not_hold_refuses_rather_than_going_missing() {
    let parsed = zerodds_idl_parse::parse("@final struct S { long x; };");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");

    let stale: zerodds_ast_core::Id<zerodds_ast::TypeNode> = zerodds_ast_core::Id::first_at(9_999);
    assert!(
        ast.type_(stale).is_err(),
        "the probe handle has to be one the model does not hold"
    );

    match zerodds_lang_rules::default_ocaml::ocaml_defaulters_over(&ast, &[stale]) {
        Err(zerodds_lang_rules::encode::EncodeError::NoWriter { kind, .. }) => {
            assert_eq!(kind, "a type the model does not hold");
        }
        other => panic!("a stale type handle was not refused: {other:?}"),
    }
}
