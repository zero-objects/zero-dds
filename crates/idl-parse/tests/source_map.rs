// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Placing a range of the expanded text back in the file it came from.
//!
//! The parser and every stage after it see the preprocessor's output:
//! comments and directives gone, includes inlined, one space between
//! tokens. An offset into that text is an offset into no file. Read
//! against the unit, four lines of comment were enough to put a
//! diagnostic three lines early and two columns off.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_idl_cst::Span;
use zerodds_idl_parse::{Diagnostic, DiagnosticCode, MemoryLoader, Preprocessed, preprocess};

/// The expanded span of the `n`-th token whose text is `text`.
fn token(result: &Preprocessed, text: &str, n: usize) -> Span {
    result
        .tokens
        .iter()
        .filter(|token| token.span.slice(&result.text) == Some(text))
        .nth(n)
        .unwrap_or_else(|| panic!("no token `{text}` #{n}"))
        .span
}

fn slice(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

const UNIT: &str = "// one\n/* two\n   three */\n#define WIDTH 8\nmodule m {\n  struct S { long field[WIDTH]; };\n};\n";

#[test]
fn a_token_after_comments_maps_to_its_own_bytes() {
    let result = preprocess("main.idl", UNIT, &MemoryLoader::new());
    let (file, span) = result
        .map
        .original(token(&result, "field", 0))
        .expect("mapped");
    assert_eq!(file, zerodds_idl_parse::SourceMap::ROOT);
    assert_eq!(slice(UNIT, span), "field");
}

#[test]
fn a_range_over_several_tokens_runs_from_the_first_to_the_last() {
    let result = preprocess("main.idl", UNIT, &MemoryLoader::new());
    let from = token(&result, "struct", 0);
    let to = token(&result, "}", 0);
    let (_, span) = result
        .map
        .original(Span::new(from.start, to.end))
        .expect("mapped");
    assert_eq!(slice(UNIT, span), "struct S { long field[WIDTH]; }");
}

/// A macro's expansion is placed at the invocation, which is the text a
/// reader can find.
#[test]
fn an_expanded_token_maps_to_the_invocation() {
    let result = preprocess("main.idl", UNIT, &MemoryLoader::new());
    let (_, span) = result.map.original(token(&result, "8", 0)).expect("mapped");
    assert_eq!(slice(UNIT, span), "WIDTH");
}

#[test]
fn a_token_from_an_include_names_that_file() {
    let included = "// header\n\nmodule inc {\n  struct T { long t; };\n};\n";
    let loader = MemoryLoader::new().with("inc.idl", included);
    let unit = "// main\n#include \"inc.idl\"\nmodule m { struct U { long u; }; };\n";
    let result = preprocess("main.idl", unit, &loader);

    let mut diagnostic = Diagnostic::error(
        DiagnosticCode::UnexpectedToken,
        token(&result, "t", 0),
        String::from("in the include"),
        "test",
    )
    .with_label(token(&result, "u", 0), String::from("in the unit"));
    result.locate(&mut diagnostic);
    assert_eq!(diagnostic.file.as_deref(), Some("inc.idl"));
    assert_eq!(slice(included, diagnostic.primary), "t");
    // The label is in the unit, so its note says which file.
    let (span, note) = &diagnostic.labels[0];
    assert_eq!(slice(unit, *span), "u");
    assert_eq!(note, "main.idl: in the unit");

    let mut in_unit = Diagnostic::error(
        DiagnosticCode::UnexpectedToken,
        token(&result, "U", 0),
        String::new(),
        "test",
    );
    result.locate(&mut in_unit);
    assert_eq!(in_unit.file, None, "the unit is not named");
    assert_eq!(slice(unit, in_unit.primary), "U");
}

/// A preprocessor diagnostic raised inside an included file names it.
#[test]
fn a_directive_error_in_an_include_names_that_file() {
    let included = "\n#if 1 / 0\n#endif\n";
    let loader = MemoryLoader::new().with("bad.idl", included);
    let result = preprocess("main.idl", "#include \"bad.idl\"\n", &loader);
    let error = result
        .diagnostics
        .iter()
        .find(|d| d.message.contains("division by zero"))
        .expect("the division is reported");
    assert_eq!(error.file.as_deref(), Some("bad.idl"));
}

/// A range of the tree is a range of the expanded text only through
/// `Parse::source_span`. The tree holds the tokens without the space the
/// expanded text puts between two of them, and a pragma's text in front
/// of the token it leads where the expanded text has it behind; a node's
/// range read as an offset into the expanded text pointed at the
/// declaration before it.
#[test]
fn a_range_of_the_tree_is_moved_to_the_expanded_text() {
    let unit = "#pragma prefix \"p\"\nmodule m {\n  struct S { long field; };\n};\n";
    let result = preprocess("main.idl", unit, &MemoryLoader::new());
    let parse = zerodds_idl_parse::parse_preprocessed(&result);
    let mut seen = Vec::new();
    for node in parse.syntax().descendants() {
        for (span, token) in node.tokens() {
            if matches!(token.text(), "m" | "S" | "field") {
                let expanded = parse.source_span(span);
                assert_eq!(slice(&result.text, expanded), token.text(), "{span:?}");
                seen.push(token.text().to_string());
            }
        }
    }
    assert_eq!(seen, ["m", "S", "field"]);
}
