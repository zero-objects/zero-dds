// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the Python backend.
//!
//! What this proves is not that the octets are right — that is
//! `runtime/python`'s own tests against the ledger, and the corpus
//! sweep — but that the walk reaches the end for the shapes the corpus
//! has and that what comes out is Python rather than C with different
//! punctuation. Every defect the Java, C++ and TypeScript columns found
//! on their first day was of that second kind: a runtime name from
//! another language, a cast that is not a cast here, an operator that
//! groups differently.
//!
//! Python adds a fourth kind the other columns cannot have. Its block
//! structure *is* its indentation and an assignment is *not* an
//! expression, so a tree that would be untidy output anywhere else is
//! here a `SyntaxError` or, worse, a different program. Every test
//! below therefore ends by handing the text to `python3 -c
//! 'compile(...)'` where one is on the path — the only reader that can
//! tell the two apart.
// `print_stderr`: the one `eprintln!` says that CPython was not on the
// path and the syntax was therefore not checked. A test that quietly
// checks nothing is worse than one that says it did not run, and there
// is no other channel a passing test can say it on.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use zerodds_ast::Ast;
use zerodds_lang_rules::emit::python::Python;
use zerodds_lang_rules::{ModelGraph, apply};

/// One IDL source, as the Python codec for its root type.
fn codec(idl: &str, type_name: &str, writing: bool) -> String {
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
    let _applied =
        apply(&model, &zerodds_lang_rules::rule::python::catalogue()).expect("the rules apply");
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
        zerodds_lang_rules::emit::encode::codec_from::<Python>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end")
    } else {
        zerodds_lang_rules::emit::decode::decoder_from::<Python>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end")
    };
    let text = statements
        .iter()
        .map(zerodds_lang_render::python::statement)
        .collect::<Vec<_>>()
        .join("\n");
    parses(&text);
    text
}

fn encoder(idl: &str, type_name: &str) -> String {
    codec(idl, type_name, true)
}

fn decoder(idl: &str, type_name: &str) -> String {
    codec(idl, type_name, false)
}

/// Hand the text to CPython and ask whether it is a program.
///
/// `compile` and not `exec`: the file names the runtime and the
/// generated declarations, neither of which is importable here, and
/// what is in question is the syntax. An indent that is off by four or
/// an `=` that reached an expression is a `SyntaxError` with a line
/// number, and there is no other reader in this crate that would see
/// either.
///
/// Skipped, loudly, where no interpreter is on the path — a test that
/// silently checks nothing is worse than one that says it did not run.
fn parses(text: &str) {
    let Ok(output) = std::process::Command::new("python3")
        .arg("-c")
        .arg("import sys; compile(sys.stdin.read(), '<generated>', 'exec')")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(text.as_bytes())?;
            }
            child.wait_with_output()
        })
    else {
        eprintln!("no python3 on the path: the syntax of the generated module was not checked");
        return;
    };
    assert!(
        output.status.success(),
        "the generated module is not Python:\n{}\n--- source ---\n{text}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The decoder starts from the union's Table 9 default, whose `default:`
/// member is set. Every case field goes back to `None` before the branch
/// the discriminator selects is read, so a decoded sample has one live
/// case — measured before: a `Sel` read with `_d = 2` came back with
/// `n = 9` and `z = 0` (`idl-conformance/_interop` `mapenum`).
#[test]
fn a_union_decoder_leaves_only_the_selected_case_set() {
    let text = decoder(
        "@final union Sel switch (long) { case 1: long p; case 2: long n; default: octet z; };",
        "Sel",
    );
    for field in ["p", "n", "z"] {
        assert!(
            text.contains(&format!("v.{field} = None")),
            "case field `{field}` is not cleared:\n{text}"
        );
    }
    let cleared = text.find("v.z = None").expect("z is cleared");
    let first_branch = text.find("if (v._d").expect("the branch chain");
    assert!(
        cleared < first_branch,
        "the cases are cleared after the branch chain starts:\n{text}"
    );
    parses(&text);
}

#[test]
fn an_array_decoder_answers_with_the_tuples_its_member_declares() {
    // `adapter/python.rs` declares `long grid[2][3]` as a tuple of two
    // tuples; a list in its place is not `==` to the sample written.
    let text = decoder("@final struct G { long grid[2][3]; };", "G");
    let opened = text
        .find("v = list(v)")
        .expect("the default's outer row is made a list");
    let inner_opened = text
        .find("v[zd_i0] = list(v[zd_i0])")
        .expect("the default's inner rows are made lists");
    let read = text
        .find("v[zd_i0][zd_i1] = zd_dec_p_long")
        .expect("the elements are read into the rows");
    let inner_closed = text
        .find("v[zd_i0] = tuple(v[zd_i0])")
        .expect("the inner rows are tuples again");
    let closed = text
        .find("v = tuple(v)")
        .expect("the outer row is a tuple again");
    assert!(
        opened < inner_opened
            && inner_opened < read
            && read < inner_closed
            && inner_closed < closed,
        "the rows are not listed before and tupled after the elements:\n{text}"
    );
    assert!(
        text.contains("tuple[tuple[int, int, int], tuple[int, int, int]]"),
        "the decoder does not declare the member's tuple type:\n{text}"
    );
    parses(&text);
}

#[test]
fn a_structure_of_primitives_becomes_a_python_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in [
        "def zd_enc_",
        "w.write_i32",
        "w.write_f64",
        "w.write_bool",
        "v.x",
    ] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    // Nothing from another column's runtime, and nothing from another
    // column's syntax.
    for foreign in ["zdw_", "->x", "writeI32", "WriteI32", "{", "}", ";"] {
        assert!(!text.contains(foreign), "{foreign} reached Python:\n{text}");
    }
}

#[test]
fn a_sequence_counts_and_loops() {
    let text = encoder("@final struct S { sequence<long> many; };", "S");
    assert!(text.contains("len(v)"), "the count is missing:\n{text}");
    assert!(
        text.contains("for i in range("),
        "the loop is missing:\n{text}"
    );
}

/// `for` is the only loop the walk has for a counted run, and Python
/// spells its bound with `range`.
#[test]
fn an_array_indexes_once_per_dimension() {
    let text = encoder("@final struct S { long grid[2][3]; };", "S");
    assert!(
        text.contains("for i0 in range(2)") && text.contains("for i1 in range(3)"),
        "the two dimensions are missing:\n{text}"
    );
    assert!(
        text.contains("[i0][i1]"),
        "the element is not indexed twice:\n{text}"
    );
}

/// The three operations whose Python signature is not in the walk's
/// order are written with their arguments named, so a transposition is
/// a `TypeError` and not a plausible octet.
#[test]
fn the_bit_bounded_kinds_name_their_arguments() {
    let text = encoder(
        "@bit_bound(8) enum Colour { RED, GREEN }; @final struct S { Colour c; };",
        "S",
    );
    assert!(
        text.contains("bit_bound=8") && text.contains("value="),
        "write_enum is not written with named arguments:\n{text}"
    );
}

/// `&&` and `||` are `and` and `or`; `&` and `|` are not, and the walk
/// uses both.
#[test]
fn the_short_circuiting_operators_are_words_and_the_bitwise_ones_are_not() {
    let text = decoder("@mutable struct S { @id(1) long a; @id(2) long b; };", "S");
    assert!(
        !text.contains("&&") && !text.contains("||"),
        "a C boolean operator reached Python:\n{text}"
    );
    assert!(text.contains(" and "), "the loop guard is missing:\n{text}");
}

/// `false` is a name Python does not bind. Twelve sites in the decode
/// walk reach `literal::<T>("false")`, and every one of them is a
/// `@mutable` member's seen-flag or a header's `end` test.
#[test]
fn the_walks_raw_boolean_tokens_are_written_in_pythons_spelling() {
    let text = decoder(
        "@mutable struct S { @id(1) long a; @id(2) string b; };",
        "S",
    );
    assert!(
        text.contains("= False"),
        "the seen-flags are not initialised:\n{text}"
    );
    for foreign in ["= false", "= true", " false", " true"] {
        assert!(
            !text.contains(foreign),
            "{foreign} reached Python, where it names nothing:\n{text}"
        );
    }
}

/// A decoder answers with the sample; there is no out-parameter to fill.
#[test]
fn a_decoder_returns_the_sample_it_built() {
    let text = decoder("@final struct S { long a; };", "S");
    assert!(
        text.contains("def zd_dec_S(r: Reader, zd_tc: TryConstruct)"),
        "the decoder's signature is not the Python one:\n{text}"
    );
    assert!(
        text.contains("v = zd_def_S()") && text.contains("return v"),
        "the sample is not started from Table 9 and answered with:\n{text}"
    );
}

/// A `map` entry is a two-slot list while it is being read and a tuple
/// once both halves are there, because a tuple cannot be written into.
#[test]
fn a_map_entry_is_written_into_as_a_list_and_answered_as_a_tuple() {
    let text = decoder("@final struct S { map<long, long> m; };", "S");
    assert!(
        text.contains("[None, None]"),
        "the entry slots are not mutable:\n{text}"
    );
    assert!(
        text.contains("tuple("),
        "the entries are never frozen:\n{text}"
    );
    assert!(
        text.contains("[0]") && text.contains("[1]"),
        "the halves are not reached by index:\n{text}"
    );
}

/// An `@optional` member is `None` when it is absent, and `is not None`
/// is what asks — not `!=`, which a dataclass's `__eq__` answers.
#[test]
fn an_optional_member_is_tested_against_none_by_identity() {
    let text = encoder("@final struct S { @optional long a; };", "S");
    assert!(
        text.contains("is not None"),
        "the presence test is missing:\n{text}"
    );
    assert!(
        !text.contains("!= None"),
        "the presence test calls __eq__:\n{text}"
    );
}

/// A union is one dataclass with `_d` and a field per case, so a case
/// member is a plain attribute and needs no narrowing.
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
        "the cases are not plain attributes:\n{text}"
    );
}

/// A `char` discriminator holds a character and the label is a code
/// point; comparing the two as they arrive would select nothing.
#[test]
fn a_character_label_is_written_as_the_character() {
    let text = encoder("@final union U switch (char) { case 'a': long one; };", "U");
    assert!(
        text.contains("\"a\""),
        "the label is still a code point:\n{text}"
    );
}

/// Every aggregate the corpus has, through both directions, to the end
/// of the walk and past CPython's parser.
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
            "@bit_bound(16) bitmask Flags { @position(0) A, @position(3) B }; @final struct S { Flags set; };",
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
    ] {
        let written = encoder(idl, root);
        assert!(!written.is_empty(), "no encoder for {idl}");
        let read = decoder(idl, root);
        assert!(!read.is_empty(), "no decoder for {idl}");
    }
}

/// A `map` sorted by its keys reaches two things the walk spells in C:
/// `strcmp` for a string key, and a `(signed char)` cast for a `char`
/// one. Neither is a name Python binds, and `map-key-char` and
/// `map-key-string` in the corpus reach them.
#[test]
fn a_sorted_map_key_names_no_c_function() {
    for (idl, wanted) in [
        ("@final struct S { map<char, long> m; };", "((ord("),
        ("@final struct S { map<string, long> m; };", "> "),
    ] {
        let text = codec_with_ascending_keys(idl, "S");
        assert!(
            !text.contains("strcmp") && !text.contains("signed char"),
            "a C name reached Python:\n{text}"
        );
        assert!(text.contains(wanted), "{wanted} is missing:\n{text}");
    }
}

/// The write walk under `--map-entries ascending-key`, which is the only
/// setting that orders a `map`'s entries by key at all.
fn codec_with_ascending_keys(idl: &str, type_name: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let ast: Ast = lowered.ast;
    let model = ModelGraph::of(&ast);
    let _applied =
        apply(&model, &zerodds_lang_rules::rule::python::catalogue()).expect("the rules apply");
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
        zerodds_lang_rules::emit::encode::codec_from::<Python>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end");
    let text = statements
        .iter()
        .map(zerodds_lang_render::python::statement)
        .collect::<Vec<_>>()
        .join("\n");
    parses(&text);
    text
}

/// A `@mutable` member chain is flat, and it has to be.
///
/// The walk builds one `If` per member, each in the previous one's
/// `else`. Rendered as nested suites that is one indent per member, and
/// CPython's tokenizer stops at 100 levels with an `IndentationError` —
/// a `SyntaxError`, so the module does not import at all rather than one
/// member failing to decode. The corpus does not reach it: its largest
/// case declares twenty members. So this is the bound and not a recorded
/// failure, and a hundred and twenty members is an IDL file somebody
/// writes rather than a case recorded here.
#[test]
fn a_mutable_member_chain_does_not_indent_once_per_member() {
    let members: String = (0..120)
        .map(|index| format!("@id({}) long m{index}; ", index + 1))
        .collect();
    let text = decoder(&format!("@mutable struct S {{ {members} }};"), "S");
    assert!(
        text.contains("elif "),
        "the chain is nested rather than flat:\n{text}"
    );
    let deepest = text
        .lines()
        .map(|line| line.len() - line.trim_start().len())
        .max()
        .unwrap_or(0);
    assert!(
        deepest < 100,
        "the deepest line is indented by {deepest} spaces, and CPython stops at 100 \
         levels of indentation"
    );
}
