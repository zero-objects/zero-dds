//! Integration-Tests fuer den vollstaendigen End-to-End-Pfad
//! `&str → Tokenizer → Recognizer → build_cst → CstNode`.
//!
//! Komplementaer zu den Inline-Tests in `cst/build.rs` und `cst/walk.rs`,
//! die jeweils einen Layer fokussieren — diese Datei zeigt die Public-API
//! aus Endkonsumenten-Sicht.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::cst::{CstKind, build_cst, walk};
use zerodds_idl::engine::Recognizer;
use zerodds_idl::errors::Span;
use zerodds_idl::grammar::ProductionId;
use zerodds_idl::grammar::TokenKind;
use zerodds_idl::grammar::toy::TOY;
use zerodds_idl::lexer::Tokenizer;

/// Helper: fuehrt die ganze Pipeline aus und liefert den CST.
fn parse(source: &'static str) -> zerodds_idl::cst::CstNode<'static> {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream: zerodds_idl::lexer::TokenStream<'static> =
        tokenizer.tokenize(source).expect("tokenize must succeed");
    let result = Recognizer::new(&TOY).recognize(stream.tokens());

    // Detach: stream lebt nur in dieser Funktion, aber CstNode<'static> haelt
    // Slices, die in den 'static-Source-String zeigen — also OK.
    build_cst(&TOY, stream.tokens(), &result).expect("build must succeed")
}

// ---------------------------------------------------------------------------
// Strukturelle Akzeptanz-Tests
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_single_n_produces_three_internal_levels() {
    let cst = parse("n");
    assert_eq!(cst.production(), Some(ProductionId(0))); // E
    // E → T → F → Token. Tiefe: 3 (root=E, depth=3 nach walk::depth-Konvention)
    assert_eq!(walk::depth(&cst), 3);
    let tokens = walk::tokens_only(&cst);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_kind(), Some(TokenKind::Keyword("n")));
}

#[test]
fn end_to_end_addition_has_three_e_nodes_for_n_plus_n_plus_n() {
    // n + n + n → drei E-Knoten dank Linksrekursion: Top, Sub, Sub-Sub
    let cst = parse("n + n + n");
    let es = walk::find_by_production(&cst, ProductionId(0));
    assert_eq!(es.len(), 3);
}

#[test]
fn end_to_end_precedence_multiplication_binds_tighter() {
    // n + n * n  → E (plus) [E_left, "+", T (times)]
    let cst = parse("n + n * n");
    // Top-E ist plus-Alt
    let CstKind::Internal {
        alternative_index: top_alt,
        ..
    } = cst.kind
    else {
        panic!("Top muss Internal sein");
    };
    assert_eq!(top_alt, 0, "Top-E muss plus-Alt sein");
    // Rechtes Kind (T) muss times-Alt sein, weil "*" hoehere Praezedenz hat.
    let right_t = &cst.children[2];
    let CstKind::Internal {
        alternative_index: t_alt,
        ..
    } = right_t.kind
    else {
        panic!("Right child muss Internal sein");
    };
    assert_eq!(t_alt, 0, "Right T muss times-Alt sein");
}

#[test]
fn end_to_end_parens_override_precedence() {
    // (n + n) * n  — die Klammer macht das + vor *, statt umgekehrt.
    let cst = parse("(n + n) * n");
    // Top-E ist just_term, weil hoechster Operator ist *
    let CstKind::Internal {
        alternative_index: top_alt,
        ..
    } = cst.kind
    else {
        panic!();
    };
    assert_eq!(top_alt, 1, "Top-E muss just_term-Alt sein");
    // Inneres T muss times-Alt sein, dessen erstes F muss paren-Alt sein.
    let t_node = &cst.children[0];
    let CstKind::Internal {
        alternative_index: t_alt,
        ..
    } = t_node.kind
    else {
        panic!();
    };
    assert_eq!(t_alt, 0, "T muss times-Alt sein");
    let first_f = &t_node.children[0]; // T (times) Children: [T, "*", F]
    // first_f sollte ein T sein (Times-Alt), nicht direkt F. Lass mich klarer prüfen:
    // T (times) ::= T "*" F. Das erste Children ist T.
    assert_eq!(first_f.production(), Some(ProductionId(1)));
}

// ---------------------------------------------------------------------------
// Span-Propagation
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_spans_propagate_from_lexer_to_cst_leaves() {
    let cst = parse("n + n");
    let tokens = walk::tokens_only(&cst);
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].span, Span::new(0, 1));
    assert_eq!(tokens[1].span, Span::new(2, 3));
    assert_eq!(tokens[2].span, Span::new(4, 5));
}

#[test]
fn end_to_end_root_span_covers_full_input() {
    let cst = parse("n + n");
    assert_eq!(cst.span, Span::new(0, 5));
}

#[test]
fn end_to_end_token_text_slices_match_source_substrings() {
    let cst = parse("n + n");
    let tokens = walk::tokens_only(&cst);
    assert_eq!(tokens[0].text, "n");
    assert_eq!(tokens[1].text, "+");
    assert_eq!(tokens[2].text, "n");
}

// ---------------------------------------------------------------------------
// Walk-Helper auf realer Struktur
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_count_token_kinds_in_complex_expression() {
    let cst = parse("n + n * (n + n)");
    let plus_count = walk::find_by_token_kind(&cst, TokenKind::Punct("+")).len();
    let times_count = walk::find_by_token_kind(&cst, TokenKind::Punct("*")).len();
    let n_count = walk::find_by_token_kind(&cst, TokenKind::Keyword("n")).len();
    let lparen_count = walk::find_by_token_kind(&cst, TokenKind::Punct("(")).len();
    let rparen_count = walk::find_by_token_kind(&cst, TokenKind::Punct(")")).len();
    assert_eq!(plus_count, 2);
    assert_eq!(times_count, 1);
    assert_eq!(n_count, 4);
    assert_eq!(lparen_count, 1);
    assert_eq!(rparen_count, 1);
}

#[test]
fn end_to_end_depth_grows_with_nested_parentheses() {
    let depth_n = walk::depth(&parse("n"));
    let depth_paren_n = walk::depth(&parse("(n)"));
    let depth_nested = walk::depth(&parse("((n))"));
    assert!(depth_paren_n > depth_n);
    assert!(depth_nested > depth_paren_n);
}

// ---------------------------------------------------------------------------
// Reject-Pfade
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_invalid_input_rejected_at_recognizer_or_lexer() {
    // Reject-Pfad ueber den Lexer (unbekanntes Zeichen "@")
    let tokenizer = Tokenizer::for_grammar(&TOY);
    assert!(tokenizer.tokenize("n @ n").is_err());
}

#[test]
fn end_to_end_partial_expression_rejected_at_recognizer() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    // Lexer akzeptiert "n +" → 2 Tokens; Recognizer akzeptiert nicht.
    let stream = tokenizer.tokenize("n +").expect("lex must succeed");
    let result = Recognizer::new(&TOY).recognize(stream.tokens());
    assert!(!result.accepted);
    let cst = build_cst(&TOY, stream.tokens(), &result);
    assert!(cst.is_none(), "Reject muss kein CST liefern");
}
