// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the Swift backend.
//!
//! What this proves is not that the octets are right — that is the
//! matrix's, `matrix/gen-zerodds-swift.sh`, against the Rust column's
//! recorded octets — but that the walk reaches the end for the shapes
//! the corpus has and that what comes out is Swift rather than C with
//! different punctuation.
//!
//! Swift adds four of its own. Every conversion is a call — `zdTo` —
//! because Swift has no implicit numeric conversion and none at all
//! between an integer and a raw enumeration; a union branch is a `T?`
//! whose value side force-unwraps and whose place side does not; a
//! sequence is read by appending, because a Swift array has no
//! uninitialised slot; and `@external` is the `ZdExternal` box, a
//! class, because a reference is Swift's only out-of-line storage.
//! Every shape below therefore ends in `swiftc -typecheck` against the
//! real `runtime/swift`, where a toolchain is on the path.
// `print_stderr`: the one `eprintln!` says that no Swift compiler was
// on the path and the build was therefore not checked. A test that
// quietly checks nothing is worse than one that says it did not run.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::Command;
use zerodds_ast::Ast;
use zerodds_lang_rules::default_swift::swift_defaulters_called_by;
use zerodds_lang_rules::emit::swift::Swift;
use zerodds_lang_rules::{ModelGraph, apply};

/// One IDL source, as the whole Swift file its codec needs.
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
        apply(&model, &zerodds_lang_rules::rule::swift::catalogue()).expect("the rules apply");
    let items = zerodds_lang_rules::adapter::swift::swift_items(&ast, &applied)
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
        zerodds_lang_rules::emit::encode::codec_from::<Swift>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end")
    } else {
        let decoders = zerodds_lang_rules::emit::decode::decoder_from::<Swift>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end");
        let mut all = swift_defaulters_called_by(&ast, &decoders).expect("Table 9 answers");
        all.extend(decoders);
        all
    };
    let mut file = zerodds_lang_cst::swift::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        imports: vec![zerodds_lang_cst::common::Ident::new("Foundation")],
        decls: items,
    };
    file.decls.extend(codec);
    zerodds_lang_render::swift::file(&file)
}

fn encoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, true)
}

fn decoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, false)
}

/// Whether a Swift compiler is on the path.
fn swift_compiler() -> Option<&'static str> {
    let found = Command::new("swiftc")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    found.then_some("swiftc")
}

/// Hand the file to the Swift compiler, against the real
/// `runtime/swift`, as the one module the probe also compiles.
fn compiles(text: &str) {
    let Some(compiler) = swift_compiler() else {
        eprintln!("no Swift compiler on the path: the generated file was not compiled");
        return;
    };
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/swift/Sources/ZdWire")
        .canonicalize()
        .expect("runtime/swift");
    let dir = std::env::temp_dir().join(format!(
        "zd-swift-codec-{}-{}",
        std::process::id(),
        text.len()
    ));
    std::fs::create_dir_all(&dir).expect("the build directory");
    let file = dir.join("zd_types.swift");
    std::fs::write(&file, text).expect("the module");
    let mut command = Command::new(compiler);
    command
        .arg("-swift-version")
        .arg("5")
        .arg("-suppress-warnings")
        .arg("-typecheck")
        .arg(&file);
    for entry in std::fs::read_dir(&runtime).expect("the runtime sources") {
        let path = entry.expect("a source").path();
        if path.extension().is_some_and(|e| e == "swift") {
            command.arg(path);
        }
    }
    let output = command
        .current_dir(&dir)
        .output()
        .expect("the Swift compiler runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the generated file is not Swift:\n{stderr}\n--- source ---\n{text}"
    );
}

#[test]
fn a_structure_of_primitives_becomes_a_swift_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in [
        "func zd_enc_",
        "w.writeI32",
        "w.writeF64",
        "w.writeBool",
        "v.x",
    ] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    for foreign in ["zdw_", "->  ", "write_i32", "cast("] {
        assert!(!text.contains(foreign), "{foreign} reached Swift:\n{text}");
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
fn an_optional_member_assigns_bare_and_clears_with_nil() {
    // The `T?` promotes on assignment, so the read needs no wrapper —
    // and the absent case is `nil`, the one state no written value can
    // take.
    let text = decoder("@final struct S { @optional long a; };", "S");
    assert!(
        text.contains("v.a = try zd_dec_p_long(r, "),
        "the optional member does not assign the decoded value:\n{text}"
    );
    assert!(
        text.contains("v.a = nil"),
        "the absent case does not clear the member:\n{text}"
    );
    compiles(&text);
}

#[test]
fn an_external_member_reads_through_the_box_and_writes_its_value() {
    let read = decoder(
        "@final struct Inner { long a; }; @final struct S { @external Inner i; };",
        "S",
    );
    assert!(
        read.contains("v.i = ZdExternal(try zd_dec_Inner(r, "),
        "the external member does not go through the box:\n{read}"
    );
    compiles(&read);
    let write = encoder(
        "@final struct Inner { long a; }; @final struct S { @external Inner i; };",
        "S",
    );
    assert!(
        write.contains("v.i.value"),
        "the external member's value is not taken on the way out:\n{write}"
    );
    compiles(&write);
}

/// An `@external` union branch is a `ZdExternal<T>?` and its codec
/// routes through the same two seams a structure member does: the box
/// constructor makes the storage on the way in, the unwrapped value
/// goes out on the way out.
#[test]
fn an_external_union_branch_reads_through_the_box_and_writes_its_value() {
    let idl = "@final struct Inner { long a; }; \
               @final union U switch (long) { case 1: @external Inner i; default: long other; }; \
               @final struct S { U pick; };";
    let read = decoder(idl, "S");
    assert!(
        read.contains("v.i = ZdExternal(try zd_dec_Inner(r, "),
        "the external branch does not go through the box:\n{read}"
    );
    compiles(&read);
    let write = encoder(idl, "S");
    assert!(
        write.contains("v.i!.value"),
        "the external branch's value is not taken on the way out:\n{write}"
    );
    compiles(&write);
}

#[test]
fn a_union_branch_is_a_place_bare_and_a_value_unwrapped() {
    let idl = "@final union U switch (long) { case 1: long one; default: string other; }; \
               @final struct S { U pick; };";
    let read = decoder(idl, "S");
    assert!(
        read.contains("v.one = ") && read.contains("v._d"),
        "the branch is not assigned bare:\n{read}"
    );
    compiles(&read);
    let write = encoder(idl, "S");
    assert!(
        write.contains("v.one!"),
        "the selected branch is not unwrapped on the way out:\n{write}"
    );
    compiles(&write);
}

#[test]
fn a_sequence_reads_by_appending() {
    let text = decoder("@final struct S { sequence<long> a; };", "S");
    assert!(
        text.contains("v.append(") || text.contains("a.append("),
        "the sequence does not append:\n{text}"
    );
    assert!(
        text.contains("reserveCapacity"),
        "the room is not reserved:\n{text}"
    );
    compiles(&text);
}

/// Every aggregate the corpus has, through both directions, to the end
/// of the walk, each typechecked against the real runtime.
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
