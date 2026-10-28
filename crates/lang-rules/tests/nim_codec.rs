// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the Nim backend.
//!
//! What this proves is not that the octets are right — that is the
//! matrix's, `tools/wireprobe/wireprobe-zerodds-nim` against the Rust
//! column's recorded octets — but that the walk reaches the end for the
//! shapes the corpus has and that what comes out is Nim rather than C
//! with different punctuation.
//!
//! Nim adds three of its own. A union's branches are `Option[T]` fields
//! — the marker for "this is the selected branch" — so both directions
//! route through `get` and `some(..)`; `@optional` is `Option[T]` and
//! `@external` is `ref T`, and the slot markers keep the four
//! combinations apart; and Nim resolves a module-scope name only after
//! its declaration, so the walk's forward declarations are real here
//! where D and Go drop them. Every shape below therefore ends in
//! `nim check` against the real `runtime/nim`, where a toolchain is on
//! the path.
// `print_stderr`: the one `eprintln!` says that no Nim compiler was on
// the path and the build was therefore not checked. A test that quietly
// checks nothing is worse than one that says it did not run.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use zerodds_ast::Ast;
use zerodds_lang_rules::default_nim::nim_defaulters_called_by;
use zerodds_lang_rules::emit::nim::Nim;
use zerodds_lang_rules::{ModelGraph, apply};

/// One IDL source, as the whole Nim module its codec needs.
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
        apply(&model, &zerodds_lang_rules::rule::nim::catalogue()).expect("the rules apply");
    let sections = zerodds_lang_rules::adapter::nim::nim_sections(&ast, &applied)
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
    let codec = if writing {
        zerodds_lang_rules::emit::encode::codec_from::<Nim>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end")
    } else {
        let decoders = zerodds_lang_rules::emit::decode::decoder_from::<Nim>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end");
        let mut all = nim_defaulters_called_by(&ast, &decoders).expect("Table 9 answers");
        all.extend(decoders);
        all
    };
    let file = zerodds_lang_cst::nim::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        // `std/options` unconditionally: a union's branches are
        // `Option[T]` whether or not any member is `@optional`, and an
        // unused import is a hint, not an error.
        imports: vec![
            zerodds_lang_cst::nim::Import::new("std/options"),
            zerodds_lang_cst::nim::Import::new("zdwire"),
        ],
        sections,
    };
    let mut out = zerodds_lang_render::nim::file(&file);
    out.push('\n');
    out.push_str(&zerodds_lang_render::nim::proc_decls(&codec));
    out
}

fn encoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, true)
}

/// The write walk's own answer for one source, refusal included.
fn try_encoder(
    idl: &str,
    type_name: &str,
) -> Result<Vec<zerodds_lang_cst::nim::ProcDecl>, zerodds_lang_rules::encode::EncodeError> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(parsed.diagnostics().is_empty(), "the source parses");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "the source lowers");
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    zerodds_lang_rules::emit::encode::codec_from::<Nim>(&ast, root, key, Default::default())
}

fn decoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, false)
}

/// Hand the module to `nim check` — the semantic pass with no code
/// generation — against the real `runtime/nim`.
fn compiles(text: &str) {
    let found = Command::new("nim")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !found {
        eprintln!("no Nim compiler on the path: the generated module was not checked");
        return;
    }
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/nim")
        .canonicalize()
        .expect("runtime/nim");
    // A counter beside the pid: the tests run as threads of one
    // process, and a directory keyed on the module's length alone
    // collided when two shapes came out the same size — one test's
    // cleanup raced the other's compile.
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-nim-codec-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("the build directory");
    let file = dir.join("zd_types.nim");
    std::fs::write(&file, text).expect("the module");
    let output = Command::new("nim")
        .args(["check", "--hints:off", "--warnings:off"])
        .arg(format!("--path:{}", runtime.display()))
        .arg(format!("--nimcache:{}", dir.join("nimcache").display()))
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("the Nim compiler runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the generated module is not Nim:\n{stderr}\n--- source ---\n{text}"
    );
}

#[test]
fn a_structure_of_primitives_becomes_a_nim_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in [
        "proc zd_enc_",
        "w.writeI32",
        "w.writeF64",
        "w.writeBool",
        "v.x",
    ] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    for foreign in ["zdw_", "->", "write_i32(", "WriteI32", "func ", "(void)"] {
        assert!(!text.contains(foreign), "{foreign} reached Nim:\n{text}");
    }
    compiles(&text);
}

#[test]
fn the_read_direction_opens_with_table_9_and_returns_the_sample() {
    let text = decoder("@final struct S { long a; string b; };", "S");
    assert!(
        text.contains("var v = zd_def_S()"),
        "the decoder does not start from Table 9:\n{text}"
    );
    assert!(
        text.contains("return v"),
        "the sample is not returned:\n{text}"
    );
    compiles(&text);
}

#[test]
fn a_forward_declaration_precedes_every_body() {
    // Nim resolves a module-scope name only after its declaration, so
    // the bodiless signature has to come first — the same shape the C
    // column emits, and the reason `routine` answers `Some` for a body
    // of `None` here.
    //
    // The `*` is Nim's export marker and is written on both halves: it
    // is what makes the codec reachable from a module that imports this
    // one, and `crates/lang-rules/tests/every_column_publishes_its_codec.rs`
    // holds that for all twenty-one columns at once.
    let text = decoder("@final struct S { long a; };", "S");
    let forward = text
        .find("proc zd_dec_S*(r: zdwire.Reader, zd_tc: zdwire.TryConstruct): S\n")
        .expect("the forward declaration is present");
    let body = text
        .find("proc zd_dec_S*(r: zdwire.Reader, zd_tc: zdwire.TryConstruct): S =")
        .expect("the body is present");
    assert!(forward < body, "the forward declaration follows the body");
    compiles(&text);
}

#[test]
fn an_optional_member_reads_through_some_and_resets_on_absence() {
    let text = decoder("@final struct S { @optional long a; };", "S");
    assert!(
        text.contains("v.a = some(zd_dec_p_long(r, "),
        "the optional member does not go through some(..):\n{text}"
    );
    assert!(
        text.contains("reset(v.a)"),
        "the absent case does not reset:\n{text}"
    );
    compiles(&text);
}

#[test]
fn an_external_member_reads_through_zd_ref_and_writes_the_dereference() {
    let read = decoder(
        "@final struct Inner { long a; }; @final struct S { @external Inner i; };",
        "S",
    );
    assert!(
        read.contains("v.i = zdwire.zdRef(zd_dec_Inner(r, "),
        "the external member does not go through zdRef:\n{read}"
    );
    compiles(&read);
    let write = encoder(
        "@final struct Inner { long a; }; @final struct S { @external Inner i; };",
        "S",
    );
    assert!(
        write.contains("v.i[]"),
        "the external member is not dereferenced on the way out:\n{write}"
    );
    compiles(&write);
}

/// An `@external` union branch is `Option[ref T]` — the branch marker
/// outside the reference — and its codec routes through both wrappers:
/// `some(zdwire.zdRef(..))` makes the storage on the way in, `.get[]`
/// hands the value out on the way out.
#[test]
fn an_external_union_branch_reads_through_both_wrappers() {
    let idl = "@final struct Inner { long a; }; \
               @final union U switch (long) { case 1: @external Inner i; default: long other; }; \
               @final struct S { U pick; };";
    let read = decoder(idl, "S");
    assert!(
        read.contains("v.i = some(zdwire.zdRef(zd_dec_Inner(r, "),
        "the external branch does not go through some(zdRef(..)):\n{read}"
    );
    compiles(&read);
    let write = encoder(idl, "S");
    assert!(
        write.contains("v.i.get[]"),
        "the external branch is not unwrapped on the way out:\n{write}"
    );
    compiles(&write);
}

#[test]
fn a_union_marks_its_selected_branch_with_some() {
    let text = decoder(
        "@final union U switch (long) { case 1: long one; default: string other; }; \
         @final struct S { U pick; };",
        "S",
    );
    assert!(
        text.contains("v.one = some(") && text.contains("v.d"),
        "the branch is not assigned through some(..):\n{text}"
    );
    compiles(&text);
    let write = encoder(
        "@final union U switch (long) { case 1: long one; default: string other; }; \
         @final struct S { U pick; };",
        "S",
    );
    assert!(
        write.contains("v.one.get"),
        "the selected branch is not read through get:\n{write}"
    );
    compiles(&write);
}

#[test]
fn the_operators_are_nims_and_not_cs() {
    // A bitmask decoder complements a 64-bit mask and masks the read:
    // Nim spells that `and`/`not`, and C's `&`/`~` would not parse.
    let text = decoder(
        "@bit_bound(16) bitmask Flags { @position(0) A, @position(3) B }; \
         @final struct S { Flags set; };",
        "S",
    );
    assert!(
        text.contains(" and ") && text.contains("not "),
        "the bitwise operators are not Nim's:\n{text}"
    );
    for foreign in [" && ", " || ", " ~", " & ", " | "] {
        assert!(
            !text.contains(foreign),
            "C's {foreign} reached Nim:\n{text}"
        );
    }
    compiles(&text);
}

/// Every aggregate the corpus has, through both directions, to the end
/// of the walk, each checked against the real runtime.
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
            "module a { module b { @final struct S { long x; }; }; };",
            "S",
        ),
        (
            "typedef long Distance; @final struct S { Distance d; };",
            "S",
        ),
    ] {
        let written = encoder(idl, root);
        assert!(!written.is_empty(), "no encoder for {idl}");
        compiles(&written);
        let read = decoder(idl, root);
        assert!(!read.is_empty(), "no decoder for {idl}");
        compiles(&read);
    }
}

/// A self-reference behind `@optional @external` is carried, exactly as
/// the D column's test records: the `@optional` member is what
/// terminates a value, so the planner admits the cycle and the
/// generated codec recurses on the value rather than on the type.
#[test]
fn the_recursive_external_shape_is_carried() {
    let idl = "@final struct Node { @optional @external Node next; long v; };";
    let written = encoder(idl, "Node");
    assert!(!written.is_empty(), "no encoder for the recursive shape");
    compiles(&written);
    let read = decoder(idl, "Node");
    assert!(!read.is_empty(), "no decoder for the recursive shape");
    compiles(&read);
}

/// The same shape without `@optional` has no terminator on the cycle
/// and is still refused by name.
#[test]
fn a_cycle_with_no_terminator_is_refused_by_name() {
    let idl = "@final struct Node { @external Node next; long v; };";
    let error = try_encoder(idl, "Node").expect_err("the planner refuses this shape");
    let text = format!("{error:?}");
    assert!(
        text.contains("closes a type cycle with nothing on it that terminates a value"),
        "the refusal changed its reason: {text}"
    );
}

/// A type handle the model will not hand over is refused by name, not
/// answered with "this type has no Nim data form".
///
/// `body_of` says `Ok(None)` for a construct Nim cannot hold, and
/// `nim_defaulters_over` reads that as a reason to emit no defaulter and
/// walk on. A stale handle used to take the same door: the defaulter the
/// closure asked for was simply not in the batch, while the decoder that
/// calls it by name still was, and the omission surfaced — if at all —
/// as an undeclared identifier from a compiler further down the chain.
#[test]
fn a_type_the_model_does_not_hold_refuses_rather_than_going_missing() {
    let mut ast = one_struct_model();
    let stale = take_and_remove(&mut ast);
    let error = zerodds_lang_rules::default_nim::nim_defaulters_over(&ast, &[stale])
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
