// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the Lua backend.
//!
//! What this proves is not that the octets are right — that is the
//! probe's measurement against the Rust column's recorded bytes — but
//! that the walk reaches the end for the shapes the corpus has and that
//! what comes out is Lua rather than C with different punctuation. Lua
//! adds a kind of failure no other column has: nothing between the
//! generated chunk and the interpreter checks anything, so every test
//! ends by handing the text to `lua`'s own compiler (`load`, which
//! compiles without running) where an interpreter is on the path — the
//! only reader that can tell a program from a near-program.
// `print_stderr`: the one `eprintln!` says that no interpreter was on
// the path and the syntax was therefore not checked. A test that
// quietly checks nothing is worse than one that says it did not run.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use zerodds_ast::Ast;
use zerodds_lang_rules::emit::lua::Lua;
use zerodds_lang_rules::{ModelGraph, apply};

fn model(idl: &str) -> Ast {
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
    ast
}

/// One IDL source, as the Lua codec for its root type.
fn codec(idl: &str, type_name: &str, writing: bool) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let _applied =
        apply(&graph, &zerodds_lang_rules::rule::lua::catalogue()).expect("the rules apply");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    let statements = if writing {
        zerodds_lang_rules::emit::encode::codec_from::<Lua>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end")
    } else {
        let decoders = zerodds_lang_rules::emit::decode::decoder_from::<Lua>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end");
        // A decoder opens with `local v = ZD.zd_def_T()`, so the
        // defaulters come with it — the same order the probe generator
        // writes.
        let mut all = zerodds_lang_rules::default_lua::lua_defaulters_called_by(&ast, &decoders)
            .expect("Table 9 answers");
        all.extend(decoders);
        all
    };
    let text = statements
        .iter()
        .map(zerodds_lang_render::lua::stat)
        .collect::<Vec<_>>()
        .join("\n");
    compiles(&text);
    text
}

fn encoder(idl: &str, type_name: &str) -> String {
    codec(idl, type_name, true)
}

fn decoder(idl: &str, type_name: &str) -> String {
    codec(idl, type_name, false)
}

/// Hand the text to the interpreter's compiler and ask whether it is a
/// chunk. `load` and not execution: the chunk names `ZD`, `M` and
/// `zdw`, none of which is bound here, and what is in question is the
/// syntax. `ZD` is bound as an empty table first because a
/// `function ZD.name()` statement is syntax whatever `ZD` holds.
fn compiles(text: &str) {
    let program = format!("local ZD = {{}}\nlocal M = {{}}\nlocal zdw = {{}}\n{text}");
    let Ok(output) = std::process::Command::new("lua")
        .arg("-e")
        .arg("local source = io.read('a'); assert(load(source, '=generated'))")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(program.as_bytes())?;
            }
            child.wait_with_output()
        })
    else {
        eprintln!("no lua on the path: the syntax of the generated chunk was not checked");
        return;
    };
    assert!(
        output.status.success(),
        "the generated chunk is not Lua:\n{}\n--- source ---\n{program}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_structure_of_primitives_becomes_a_lua_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in [
        "function ZD.zd_enc_",
        "zdw.write_i32(w, ",
        "zdw.write_f64(w, ",
        "zdw.write_bool(w, ",
        "v.x",
    ] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    // Nothing from another column's runtime, and nothing from another
    // column's syntax.
    for foreign in ["zdw_", "->x", "writeI32", "WriteI32", ";"] {
        assert!(!text.contains(foreign), "{foreign} reached Lua:\n{text}");
    }
}

#[test]
fn a_sequence_counts_zero_based_and_subscripts_one_based() {
    let text = encoder("@final struct S { sequence<long> many; };", "S");
    assert!(text.contains("#v"), "the count is missing:\n{text}");
    assert!(
        text.contains("for i = 0, #v - 1 do"),
        "the loop is not zero-based with an inclusive bound:\n{text}"
    );
    assert!(
        text.contains("[(i) + 1]"),
        "the subscript does not add one:\n{text}"
    );
}

#[test]
fn an_array_indexes_once_per_dimension() {
    let text = encoder("@final struct S { long grid[2][3]; };", "S");
    assert!(
        text.contains("for i0 = 0, 2 - 1 do") && text.contains("for i1 = 0, 3 - 1 do"),
        "the two dimensions are missing:\n{text}"
    );
    assert!(
        text.contains("[(i0) + 1][(i1) + 1]"),
        "the element is not indexed twice:\n{text}"
    );
}

#[test]
fn the_short_circuiting_operators_are_words_and_inequality_is_tilde() {
    let text = decoder("@mutable struct S { @id(1) long a; @id(2) long b; };", "S");
    assert!(
        !text.contains("&&") && !text.contains("||") && !text.contains("!="),
        "a C operator reached Lua:\n{text}"
    );
    assert!(text.contains(" and "), "the loop guard is missing:\n{text}");
    assert!(text.contains("~="), "no inequality was written:\n{text}");
}

#[test]
fn the_walks_raw_boolean_tokens_survive_as_booleans() {
    let text = decoder(
        "@mutable struct S { @id(1) long a; @id(2) string b; };",
        "S",
    );
    assert!(
        text.contains("= false"),
        "the seen-flags are not initialised:\n{text}"
    );
}

#[test]
fn a_decoder_returns_the_sample_it_built() {
    let text = decoder("@final struct S { long a; };", "S");
    assert!(
        text.contains("function ZD.zd_dec_S(r, zd_tc)"),
        "the decoder's signature is not the Lua one:\n{text}"
    );
    assert!(
        text.contains("local v = ZD.zd_def_S()") && text.contains("return v"),
        "the sample is not started from Table 9 and answered with:\n{text}"
    );
}

#[test]
fn a_map_entry_is_a_key_value_table() {
    let text = decoder("@final struct S { map<long, long> m; };", "S");
    assert!(
        text.contains(".key") && text.contains(".value"),
        "the halves are not the adapter's keys:\n{text}"
    );
}

#[test]
fn an_optional_member_is_the_key_being_there() {
    let text = encoder("@final struct S { @optional long a; };", "S");
    assert!(
        text.contains("v.a ~= nil"),
        "the presence test is missing:\n{text}"
    );
}

#[test]
fn a_union_selects_on_its_discriminator() {
    let text = encoder(
        "@final union U switch (long) { case 1: long one; default: string other; };",
        "U",
    );
    assert!(
        text.contains("v._d"),
        "the discriminator is missing:\n{text}"
    );
    assert!(
        text.contains("v.one") && text.contains("v.other"),
        "the cases are not plain keys:\n{text}"
    );
}

#[test]
fn a_boolean_discriminators_label_is_a_boolean_and_a_chars_stays_the_code_point() {
    let text = encoder(
        "@final union U switch (boolean) { case TRUE: long one; };",
        "U",
    );
    assert!(
        text.contains("v._d == true"),
        "the boolean label is still a number:\n{text}"
    );
    let text = encoder("@final union U switch (char) { case 'a': long one; };", "U");
    assert!(
        text.contains("v._d == 97"),
        "the char label is not the code point the member holds:\n{text}"
    );
}

#[test]
fn an_enumerations_wire_value_is_read_out_of_the_descriptor() {
    let text = encoder(
        "enum Colour { RED, GREEN, BLUE }; @final struct S { Colour tint; };",
        "S",
    );
    assert!(
        text.contains("M.Colour.values.RED") || text.contains("zdw.write_enum"),
        "the enumeration codec is missing:\n{text}"
    );
}

#[test]
fn a_keyword_member_keeps_its_name_in_the_bracket_form() {
    let text = encoder("@final struct S { long until; };", "S");
    assert!(
        text.contains("v[\"until\"]"),
        "the keyword member is not reached by its own name:\n{text}"
    );
    assert!(
        !text.contains("v.until"),
        "a keyword after a dot does not parse:\n{text}"
    );
}

#[test]
fn a_sorted_maps_char_key_wraps_and_its_u64_key_is_unsigned() {
    let text = codec_with_ascending_keys("@final struct S { map<char, long> m; };", "S");
    assert!(
        text.contains("% 256"),
        "the signed-char reading is missing:\n{text}"
    );
    assert!(
        !text.contains("signed char") && !text.contains("strcmp"),
        "a C name reached Lua:\n{text}"
    );
    let text =
        codec_with_ascending_keys("@final struct S { map<unsigned long long, long> m; };", "S");
    assert!(
        text.contains("math.ult("),
        "the unsigned comparison is missing:\n{text}"
    );
    let text = codec_with_ascending_keys("@final struct S { map<boolean, long> m; };", "S");
    assert!(
        text.contains("not ("),
        "the boolean order is missing:\n{text}"
    );
    let text = codec_with_ascending_keys("@final struct S { map<string, long> m; };", "S");
    assert!(
        text.contains("zdw.strcmp("),
        "the byte-wise comparison is missing:\n{text}"
    );
}

/// The write walk under `--map-entries ascending-key`.
fn codec_with_ascending_keys(idl: &str, type_name: &str) -> String {
    let ast = model(idl);
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let mut key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    key.map_entries = zerodds_ast_layout::MapEntryOrder::AscendingKey;
    let statements =
        zerodds_lang_rules::emit::encode::codec_from::<Lua>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end");
    let text = statements
        .iter()
        .map(zerodds_lang_render::lua::stat)
        .collect::<Vec<_>>()
        .join("\n");
    compiles(&text);
    text
}

/// Every aggregate the corpus has, through both directions, to the end
/// of the walk and past the interpreter's compiler.
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
        ("@final struct S { wchar c; wstring w; };", "S"),
        ("@final struct S { long double d; };", "S"),
        (
            "@final struct Inner { long a; }; @final struct S { Inner i; };",
            "S",
        ),
        ("@final struct S {};", "S"),
        (
            "module a { module b { @final struct S { long x; }; }; };",
            "S",
        ),
        ("@final struct S { fixed<5,2> f; };", "S"),
        (
            "bitset Packed { bitfield<3> low; bitfield<5> high; }; \
             @final struct S { Packed p; };",
            "S",
        ),
    ] {
        let written = encoder(idl, root);
        assert!(!written.is_empty(), "no encoder for {idl}");
        let read = decoder(idl, root);
        assert!(!read.is_empty(), "no decoder for {idl}");
    }
}

/// A `@mutable` member dispatch renders flat, as `elseif`, so a wide
/// aggregate does not nest toward the parser's own depth limit.
#[test]
fn a_mutable_member_chain_renders_flat() {
    let members: String = (0..120)
        .map(|index| format!("@id({}) long m{index}; ", index + 1))
        .collect();
    let text = decoder(&format!("@mutable struct S {{ {members} }};"), "S");
    assert!(
        text.contains("elseif "),
        "the chain is nested rather than flat:\n{text}"
    );
    let deepest = text
        .lines()
        .map(|line| line.len() - line.trim_start().len())
        .max()
        .unwrap_or(0);
    assert!(
        deepest < 60,
        "the deepest line is indented by {deepest} spaces; the chain nests once per member"
    );
}
