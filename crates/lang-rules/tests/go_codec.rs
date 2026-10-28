// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the Go backend.
//!
//! What this proves is not that the octets are right — that is the
//! corpus sweep's, `tools/wireprobe/wireprobe-go-corpus`, against the
//! Rust column's recorded octets — but that the walk reaches the end
//! for the shapes the corpus has and that what comes out is Go rather
//! than C with different punctuation. Every defect the Java, C++ and
//! TypeScript columns found on their first day was of that second
//! kind: a runtime name from another language, a cast that is not a
//! cast here, an operator that groups differently.
//!
//! Go adds two of its own. An unused local and an unused import are
//! compile errors, not warnings, so a declaration the walk emits and
//! nothing reads is a program that does not build; and an untyped
//! constant converts where a typed value does not, so a mask the walk
//! complements has to carry its `uint64` before the `^` reaches it.
//! Every shape below therefore ends in `go build` where a toolchain is
//! on the path — the only reader that can tell the two apart.
// `print_stderr`: the one `eprintln!` says that `go` was not on the
// path and the build was therefore not checked. A test that quietly
// checks nothing is worse than one that says it did not run.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::Command;
use zerodds_ast::Ast;
use zerodds_lang_rules::default_go::go_defaulters_called_by;
use zerodds_lang_rules::emit::go::Go;
use zerodds_lang_rules::{ModelGraph, apply};

/// One IDL source, as the whole Go module its codec needs: the types,
/// Table 9's defaulters where the read direction is asked for, and the
/// codec itself.
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
        apply(&model, &zerodds_lang_rules::rule::go::catalogue()).expect("the rules apply");
    let items =
        zerodds_lang_rules::adapter::go::go_items(&ast, &applied).expect("the types materialise");
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
        zerodds_lang_rules::emit::encode::codec_from::<Go>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end")
    } else {
        let decoders = zerodds_lang_rules::emit::decode::decoder_from::<Go>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end");
        let mut all = go_defaulters_called_by(&ast, &decoders).expect("Table 9 answers");
        all.extend(decoders);
        all
    };
    let mut file = zerodds_lang_cst::go::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        package: zerodds_lang_cst::common::Ident::new("zdtypes"),
        imports: vec![zerodds_lang_cst::go::Import {
            path: "zdwire".to_string(),
            alias: None,
        }],
        decls: items,
    };
    file.decls.extend(codec);
    zerodds_lang_render::go::file(&file)
}

fn encoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, true)
}

fn decoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, false)
}

/// Whether `go` is on the path at all.
fn go_available() -> bool {
    Command::new("go")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Hand the module to `go build`, against the real `runtime/go`.
///
/// Skipped, loudly, where no toolchain is on the path — a test that
/// silently checks nothing is worse than one that says it did not run.
fn compiles(text: &str) {
    if !go_available() {
        eprintln!("no `go` on the path: the generated module was not compiled");
        return;
    }
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/go")
        .canonicalize()
        .expect("runtime/go");
    let dir =
        std::env::temp_dir().join(format!("zd-go-codec-{}-{}", std::process::id(), text.len()));
    std::fs::create_dir_all(&dir).expect("the build directory");
    std::fs::write(dir.join("zd_types.go"), text).expect("the module");
    std::fs::write(
        dir.join("go.mod"),
        format!(
            "module zdtypes\n\ngo 1.21\n\nrequire zdwire v0.0.0\n\nreplace zdwire => {}\n",
            runtime.display()
        ),
    )
    .expect("the module file");
    let output = Command::new("go")
        .arg("build")
        .arg("./...")
        .env("GOFLAGS", "-mod=mod")
        .current_dir(&dir)
        .output()
        .expect("go build runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the generated module is not Go:\n{stderr}\n--- source ---\n{text}"
    );
}

#[test]
fn a_structure_of_primitives_becomes_a_go_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in [
        // The published spelling: Go's visibility is the first letter
        // of the name, and `emit::go::exported_name` raises it so a
        // consumer of the generated package can call the codec.
        "func Zd_enc_",
        "w.WriteI32",
        "w.WriteF64",
        "w.WriteBool",
        "v.X",
    ] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    // Nothing from another column's runtime, and nothing from another
    // column's syntax.
    for foreign in ["zdw_", "->", "write_i32", "writeI32", "(void)"] {
        assert!(!text.contains(foreign), "{foreign} reached Go:\n{text}");
    }
    compiles(&text);
}

#[test]
fn a_sequence_counts_in_the_wire_width_and_loops() {
    let text = encoder("@final struct S { sequence<long> many; };", "S");
    assert!(
        text.contains("uint32(len(v.Many))") || text.contains("uint32(len(v))"),
        "the count is not the wire's width:\n{text}"
    );
    assert!(
        text.contains("for i := uint32(0);"),
        "the loop is missing:\n{text}"
    );
    compiles(&text);
}

#[test]
fn the_read_direction_opens_with_table_9_and_returns_the_sample() {
    let text = decoder("@final struct S { long a; string b; };", "S");
    assert!(
        text.contains("v := Zd_def_S()"),
        "the decoder does not start from Table 9:\n{text}"
    );
    assert!(
        text.contains("return v"),
        "the sample is not returned:\n{text}"
    );
    assert!(
        text.contains("func Zd_def_S() S {"),
        "the defaulter is missing:\n{text}"
    );
    compiles(&text);
}

#[test]
fn an_optional_member_reads_through_a_pointer() {
    let text = decoder("@final struct S { @optional long a; };", "S");
    assert!(
        text.contains("zdwire.Ptr(Zd_dec_p_long(r, "),
        "the optional member does not go through Ptr:\n{text}"
    );
    assert!(
        text.contains("v.A = nil"),
        "the absent case does not write nil:\n{text}"
    );
    compiles(&text);
}

#[test]
fn a_bitmask_mask_is_gos_own_and_typed() {
    let text = decoder(
        "@bit_bound(16) bitmask Flags { @position(0) A, @position(3) B }; \
         @final struct S { Flags set; };",
        "S",
    );
    assert!(
        text.contains("uint64(9)"),
        "the mask of the declared bits (A at 0, B at 3) is not Go's typed one:\n{text}"
    );
    assert!(!text.contains('~'), "a C complement reached Go:\n{text}");
    compiles(&text);
}

#[test]
fn a_mutable_aggregate_compiles_with_its_seen_flags() {
    // `zd_sent` and the seen-flags are the locals Go's "declared and
    // not used" error watches; the walk reads every one of them, and
    // this is the shape that proves it.
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

#[test]
fn a_union_compares_its_discriminator_as_a_number() {
    // A `wchar` discriminator is a `uint16` here, so a surrogate label
    // — refused as a string in the Python column — is an ordinary
    // integer.
    let text = encoder("@final union U switch (char) { case 'a': long one; };", "U");
    assert!(
        text.contains("v.D == uint8(97)"),
        "the label is not the code unit:\n{text}"
    );
    compiles(&text);
}

/// Every aggregate the corpus has, through both directions, to the end
/// of the walk. The write half of each is also compiled; the read half
/// of each is compiled with its defaulters.
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
        ("@final struct Node { sequence<Node> children; };", "Node"),
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
