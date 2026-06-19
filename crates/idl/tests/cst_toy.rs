//! Integration tests for the complete end-to-end path
//! `&str → Tokenizer → Recognizer → build_cst → CstNode`.
//!
//! Complementary to the inline tests in `cst/build.rs` and `cst/walk.rs`,
//! which each focus on one layer — this file shows the public API
//! from the end-consumer perspective.

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

/// Helper: runs the whole pipeline and returns the CST.
fn parse(source: &'static str) -> zerodds_idl::cst::CstNode<'static> {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream: zerodds_idl::lexer::TokenStream<'static> =
        tokenizer.tokenize(source).expect("tokenize must succeed");
    let result = Recognizer::new(&TOY).recognize(stream.tokens());

    // Detach: stream only lives in this function, but CstNode<'static> holds
    // slices pointing into the 'static source string — so that's OK.
    build_cst(&TOY, stream.tokens(), &result).expect("build must succeed")
}

// ---------------------------------------------------------------------------
// Structural acceptance tests
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_single_n_produces_three_internal_levels() {
    let cst = parse("n");
    assert_eq!(cst.production(), Some(ProductionId(0))); // E
    // E → T → F → Token. Depth: 3 (root=E, depth=3 per walk::depth convention)
    assert_eq!(walk::depth(&cst), 3);
    let tokens = walk::tokens_only(&cst);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].token_kind(), Some(TokenKind::Keyword("n")));
}

#[test]
fn end_to_end_addition_has_three_e_nodes_for_n_plus_n_plus_n() {
    // n + n + n → three E nodes thanks to left recursion: top, sub, sub-sub
    let cst = parse("n + n + n");
    let es = walk::find_by_production(&cst, ProductionId(0));
    assert_eq!(es.len(), 3);
}

#[test]
fn end_to_end_precedence_multiplication_binds_tighter() {
    // n + n * n  → E (plus) [E_left, "+", T (times)]
    let cst = parse("n + n * n");
    // Top E is the plus alt
    let CstKind::Internal {
        alternative_index: top_alt,
        ..
    } = cst.kind
    else {
        panic!("top must be Internal");
    };
    assert_eq!(top_alt, 0, "top E must be the plus alt");
    // The right child (T) must be the times alt, because "*" has higher precedence.
    let right_t = &cst.children[2];
    let CstKind::Internal {
        alternative_index: t_alt,
        ..
    } = right_t.kind
    else {
        panic!("right child must be Internal");
    };
    assert_eq!(t_alt, 0, "right T must be the times alt");
}

#[test]
fn end_to_end_parens_override_precedence() {
    // (n + n) * n  — the parens make the + bind before *, instead of the reverse.
    let cst = parse("(n + n) * n");
    // top E is just_term, because the highest operator is *
    let CstKind::Internal {
        alternative_index: top_alt,
        ..
    } = cst.kind
    else {
        panic!();
    };
    assert_eq!(top_alt, 1, "top E must be the just_term alt");
    // The inner T must be the times alt, whose first F must be the paren alt.
    let t_node = &cst.children[0];
    let CstKind::Internal {
        alternative_index: t_alt,
        ..
    } = t_node.kind
    else {
        panic!();
    };
    assert_eq!(t_alt, 0, "T must be the times alt");
    let first_f = &t_node.children[0]; // T (times) children: [T, "*", F]
    // first_f should be a T (times alt), not F directly. Let me check more clearly:
    // T (times) ::= T "*" F. The first child is T.
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
// Walk helpers on a real structure
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
// Reject paths
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_invalid_input_rejected_at_recognizer_or_lexer() {
    // Reject path via the lexer (unknown character "@")
    let tokenizer = Tokenizer::for_grammar(&TOY);
    assert!(tokenizer.tokenize("n @ n").is_err());
}

#[test]
fn end_to_end_partial_expression_rejected_at_recognizer() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    // The lexer accepts "n +" → 2 tokens; the recognizer does not.
    let stream = tokenizer.tokenize("n +").expect("lex must succeed");
    let result = Recognizer::new(&TOY).recognize(stream.tokens());
    assert!(!result.accepted);
    let cst = build_cst(&TOY, stream.tokens(), &result);
    assert!(cst.is_none(), "reject must not yield a CST");
}
