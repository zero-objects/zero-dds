// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL to Node (plain JavaScript), through every layer of the rebuild.
//!
//! The counterpart of `zig_end_to_end.rs` and `typescript_reader_generates.rs`,
//! shaped by [`crate::adapter::node`]'s own design rather than copied
//! from either: this column declares almost nothing (the module
//! documentation of `crates/lang-rules/src/adapter/node.rs` says why),
//! so "every corpus case renders" is a check on the materialiser's
//! evaluation and collision handling and not on how much text came out.
//! What proves this column is the codec: a curated set of shapes run
//! all the way to `function zd_enc_T`/`function zd_dec_T` pairs and
//! through the real `node --check` parser, exactly as
//! `the_generated_zig_compiles` runs its own set through `zig`.
//!
//! `print_stderr` is allowed here on purpose, as it is in every sibling
//! harness: a skipped `node` check and a corpus case the frontend
//! refused both have to be said out loud on a channel a passing test
//! does not use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/resolved.rs"]
mod resolved;

use std::process::Command;
use zerodds_ast::Ast;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_cst::node::escape_decl;
use zerodds_lang_rules::adapter::node::node_file;
use zerodds_lang_rules::default_node::node_defaulters_called_by;
use zerodds_lang_rules::emit::decode::decoder_from;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from};
use zerodds_lang_rules::emit::node::Node;

/// IDL in, the declarations `crates/lang-rules/src/adapter/node.rs`
/// produces out.
fn generate(idl: &str) -> String {
    try_generate(idl, "test.idl").unwrap_or_else(|why| panic!("{why}"))
}

/// The same, naming the layer that refused instead of panicking.
fn try_generate(idl: &str, origin: &str) -> Result<String, String> {
    let ast = model_of(idl, origin)?;
    let file = node_file(&ast).map_err(|error| format!("materialise: {error}"))?;
    Ok(zerodds_lang_render::node::file(&file))
}

/// Parse, lower and resolve one IDL source, naming the layer that
/// refused.
fn model_of(idl: &str, origin: &str) -> Result<Ast, String> {
    let ast = resolved::ast(idl, origin)?;
    Ok(ast)
}

/// The one type declared in a single-declaration source.
fn only_type(ast: &Ast) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .last()
        .map(|(id, _)| id)
        .expect("the source declares a type")
}

/// The full `.js` text a root type's write and read codecs render to,
/// alongside the declarations and Table 9 defaulters — everything
/// `node --check` needs to see one name resolve to another.
fn full_program(idl: &str) -> String {
    let ast = model_of(idl, "compile.idl").unwrap_or_else(|why| panic!("{why}"));
    let root = only_type(&ast);
    let key = LayoutKey::in_dialect(
        Dialect::Specification,
        Encoding::PlainCdr2,
        Endianness::Little,
        Purpose::Data,
    );
    let declarations = node_file(&ast).unwrap_or_else(|why| panic!("materialise: {why}"));
    let encoders = codec_from::<Node>(&ast, root, key, CodecOptions::default())
        .unwrap_or_else(|why| panic!("encode: {why}"));
    let decoders = decoder_from::<Node>(&ast, root, key, CodecOptions::default())
        .unwrap_or_else(|why| panic!("decode: {why}"));
    let mut defaulters = node_defaulters_called_by(&ast, &encoders)
        .unwrap_or_else(|why| panic!("defaulters (encode): {why}"));
    defaulters.extend(
        node_defaulters_called_by(&ast, &decoders)
            .unwrap_or_else(|why| panic!("defaulters (decode): {why}")),
    );

    let mut out = String::from("\"use strict\";\nconst zdwire = require(\"./zdwire.js\");\n\n");
    out.push_str(&zerodds_lang_render::node::file(&declarations));
    out.push('\n');
    for decl in &defaulters {
        out.push_str(&zerodds_lang_render::node::stat(decl));
        out.push('\n');
    }
    for decl in &encoders {
        out.push_str(&zerodds_lang_render::node::stat(decl));
        out.push('\n');
    }
    for decl in &decoders {
        out.push_str(&zerodds_lang_render::node::stat(decl));
        out.push('\n');
    }
    out
}

// -- the mapping -----------------------------------------------------------

#[test]
fn an_enumeration_is_a_frozen_object_of_its_names() {
    let generated = generate("enum Mode { IDLE, @value(4) ACTIVE, FAULT };");
    assert!(
        generated.contains("const Mode = Object.freeze({"),
        "{generated}"
    );
    assert!(generated.contains("IDLE: 0,"), "{generated}");
    assert!(generated.contains("ACTIVE: 4,"), "{generated}");
    assert!(generated.contains("FAULT: 5"), "{generated}");
}

#[test]
fn a_bitmask_declares_one_constant_per_bit_and_no_type_of_its_own() {
    let generated = generate("@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };");
    assert!(generated.contains("const READY = 1 << 0;"), "{generated}");
    assert!(generated.contains("const MOVING = 1 << 4;"), "{generated}");
    assert!(
        !generated.contains("Flags ="),
        "a bitmask's own binding is not this column's — a member holding one is a plain \
         number\n{generated}"
    );
}

#[test]
fn a_wide_bitmask_writes_its_bits_as_bigint_literals() {
    let generated = generate("@bit_bound(64) bitmask Wide { @position(63) TOP };");
    assert!(generated.contains("const TOP = 1n << 63n;"), "{generated}");
}

#[test]
fn a_struct_a_union_and_a_bitset_declare_nothing() {
    // The module documentation's central claim: every aggregate is a
    // plain object literal wherever the codec walk or the defaulter
    // builds one, and none of them needs a name at load time.
    let generated = generate(
        "struct S { long a; };
         enum M { A, B };
         union U switch (M) { case A: long a; };
         bitset Bits { bitfield<3> low; };
         valuetype V { public long a; };
         exception E { long code; };
         interface I { void ping(); };
         native N;",
    );
    for forbidden in [
        "const S",
        "const U",
        "const Bits",
        "const V",
        "const E",
        "const I",
        "const N =",
    ] {
        assert!(
            !generated.contains(forbidden),
            "{forbidden:?} reached the output\n{generated}"
        );
    }
}

#[test]
fn a_boolean_constant_writes_the_literal() {
    let generated = generate("const boolean ON = TRUE;");
    assert!(generated.contains("const ON = true;"), "{generated}");
}

#[test]
fn a_sixty_four_bit_constant_carries_the_bigint_suffix() {
    let generated = generate("const unsigned long long BIG = 18446744073709551615;");
    assert!(generated.contains("18446744073709551615n"), "{generated}");
}

#[test]
fn a_string_constant_is_a_plain_string_literal() {
    let generated = generate("const string TAG = \"abc\";");
    assert!(generated.contains("const TAG = \"abc\";"), "{generated}");
}

#[test]
fn a_reserved_word_used_as_a_top_level_declaration_is_escaped() {
    let generated = generate("const long class = 1; const long delete = 2;");
    assert!(generated.contains("const class_ = 1;"), "{generated}");
    assert!(generated.contains("const delete_ = 2;"), "{generated}");
    assert!(generated.contains("IDL: `class`."), "{generated}");
}

#[test]
fn no_predefined_type_name_is_reserved_here() {
    // Plain JavaScript has no predefined type names, unlike TypeScript,
    // which reserves both of these as binding names. `string` and
    // `number` themselves are IDL keywords and cannot be tested as IDL
    // identifiers at all.
    let generated = generate("const long bigint = 1; const long unknown = 2;");
    assert!(generated.contains("const bigint = 1;"), "{generated}");
    assert!(generated.contains("const unknown = 2;"), "{generated}");
}

#[test]
fn a_module_flattens_onto_the_enumerations_binding() {
    let generated = generate("module common { module util { enum Mode { A }; }; };");
    assert!(
        generated.contains("const common_util_Mode ="),
        "{generated}"
    );
}

#[test]
fn escape_decl_never_moves_a_plain_name() {
    for name in ["Pose", "pose", "x_min", "_d", "MODULE_ID"] {
        assert_eq!(escape_decl(name), name);
    }
}

#[test]
fn two_runs_over_one_source_produce_the_same_text() {
    let idl = "enum E { A, B }; const long N = 1;";
    assert_eq!(generate(idl), generate(idl));
}

// -- the corpus --------------------------------------------------------

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

/// How many corpus cases have to reach the materialiser for this gate to
/// mean anything: the same 350-of-378 the sibling columns hold
/// themselves to, minus nothing — this column refuses no construct the
/// frontend hands it, because it declares almost none of them.
const MUST_RENDER: usize = 350;

#[test]
fn every_corpus_case_reaches_the_materialiser_or_names_its_failure() {
    let cases = corpus();
    assert!(
        cases.len() > 300,
        "the corpus is not where it was: {} cases",
        cases.len()
    );
    let mut failures = Vec::new();
    let mut frontend = Vec::new();
    let mut rendered = 0_usize;
    for (name, path) in cases {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        match try_generate(&source, &path.display().to_string()) {
            Ok(_) => rendered += 1,
            Err(why) => {
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so Node never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        rendered >= MUST_RENDER,
        "only {rendered} of the corpus reached the materialiser; the gate says nothing below \
         {MUST_RENDER}"
    );
}

// -- the codec, through node --check ----------------------------------

/// Whether `node` is on the path at all.
fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// One generated program through `node --check`: the syntax and
/// identifier-resolution gate this column has instead of a compiler.
/// `node --check` parses without running, which is the JavaScript
/// column's version of `zig ast-check` and does not need
/// `runtime/node/zdwire.js` to be present on disk, because it never
/// resolves the `require`.
fn node_check(source: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-node-check-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("probe.js");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("node")
        .arg("--check")
        .arg(&file)
        .output()
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).into_owned())
}

static SERIAL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Every shape below, through `node --check`.
///
/// One source per case: two of these declare an `S` and a `T`, and a
/// script may declare one function of a name only once.
#[test]
fn the_generated_node_passes_node_check() {
    if !node_available() {
        eprintln!(
            "SKIPPED: `node` is not on the path, so the generated JavaScript was not checked."
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
        "struct T { long class; long delete; long typeof; long new; };",
        "enum Mode { IDLE, @value(4) ACTIVE, FAULT };
         union Payload switch (Mode) { case IDLE: long idle; case ACTIVE: double active; };",
        "@bit_bound(16) bitmask Flags { READY, @position(4) MOVING };
         struct Holder { Flags f; };",
        "bitset Bits { bitfield<3> low; bitfield<12> high; };",
        "valuetype V { public long counter; private string secret; };",
        "struct Node1 { @optional @external Node1 next; long value; };",
        "struct Outer { sequence<sequence<long>> nested; };",
    ];
    for idl in sources {
        let generated = full_program(idl);
        if let Err(why) = node_check(&generated) {
            panic!("{idl}\n\n{generated}\n\n{why}");
        }
    }
}

/// A `map` under `--map-entries ascending-key` puts its entries on the
/// wire in ascending key order, for both key types that go through
/// `strcmp`.
///
/// The check runs the generated encoder rather than reading its text:
/// the defect this test pins was a spelling that parsed, passed
/// `node --check` and computed the opposite sign, because
/// `true - false` is `1` and the walk reads `strcmp(..) < 0`. Only the
/// order the entries actually leave in tells the two apart, so a stub
/// writer records the keys and the assertion is on that list.
#[test]
fn a_sorted_map_puts_its_string_keys_on_the_wire_ascending() {
    if !node_available() {
        eprintln!("SKIPPED: `node` is not on the path, so the map key order was not run.");
        return;
    }
    for (idl, encoder) in [
        (
            "@final struct S { map<string, long> m; };",
            "zd_enc_map_string_long",
        ),
        (
            "@final struct S { map<wstring, long> m; };",
            "zd_enc_map_wstring_long",
        ),
    ] {
        let ast = model_of(idl, "order.idl").unwrap_or_else(|why| panic!("{why}"));
        let root = only_type(&ast);
        let mut key = LayoutKey::in_dialect(
            Dialect::Specification,
            Encoding::PlainCdr2,
            Endianness::Little,
            Purpose::Data,
        );
        key.map_entries = zerodds_ast_layout::MapEntryOrder::AscendingKey;
        let encoders = codec_from::<Node>(&ast, root, key, CodecOptions::default())
            .unwrap_or_else(|why| panic!("encode: {why}"));

        // Enough of `runtime/node/zdwire.js` for the encoder to run:
        // the two names a `wstring` writer reaches for, and a writer
        // that keeps the keys instead of bytes.
        let mut source = String::from(
            "\"use strict\";\n\
             const zdwire = {\n\
             \x20 WstringLengthPolicy: { Specification: 0 },\n\
             \x20 WstringUnitOrder: { Stream: 0 },\n\
             };\n\
             const seen = [];\n\
             const w = {\n\
             \x20 writeDheader() { return 0; },\n\
             \x20 finishDheader(_d) {},\n\
             \x20 writeU32(_v) {},\n\
             \x20 writeI32(_v) {},\n\
             \x20 writeString(v) { seen.push(v); },\n\
             \x20 writeWstringUtf8(v, _p, _o) { seen.push(v); },\n\
             };\n",
        );
        for decl in &encoders {
            source.push_str(&zerodds_lang_render::node::stat(decl));
            source.push('\n');
        }
        source.push_str(encoder);
        source.push_str(
            "(w, [\n\
             \x20 { key: \"c\", value: 3 },\n\
             \x20 { key: \"a\", value: 1 },\n\
             \x20 { key: \"b\", value: 2 },\n\
             ]);\n\
             console.log(seen.join(\",\"));\n",
        );

        let order = node_run(&source).unwrap_or_else(|why| panic!("{source}\n\n{why}"));
        assert_eq!(
            order.trim(),
            "a,b,c",
            "{idl}: the entries did not leave in ascending key order:\n{source}"
        );
    }
}

/// One generated program through `node`, returning what it printed.
fn node_run(source: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!(
        "zd-node-run-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("probe.js");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("node")
        .arg(&file)
        .output()
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(String::from_utf8_lossy(&output.stderr).into_owned())
}
