//! Integration-Tests fuer den Tokenizer gegen die Toy-Grammar.
//!
//! Validiert die Public-API der Lexer-Schicht (`zerodds_idl::lexer`) mit echten
//! Source-Strings statt synthetischen Tokens. Komplementaer zu den Inline-
//! Unit-Tests in `lexer/tokenizer.rs`, die mit Test-internem Grammar-
//! Setup arbeiten — diese Datei zeigt den Endkonsumenten-Pfad.

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

use zerodds_idl::errors::Span;
use zerodds_idl::grammar::TokenKind;
use zerodds_idl::grammar::toy::TOY;
use zerodds_idl::lexer::Tokenizer;

// ---------------------------------------------------------------------------
// Akzeptierende Lexer-Laeufe
// ---------------------------------------------------------------------------

#[test]
fn tokenizer_for_toy_extracts_five_rules() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    // Toy-Grammar Terminals: "n" (Keyword), "+" "*" "(" ")" (Punct).
    assert_eq!(tokenizer.rules().len(), 5);
}

#[test]
fn tokenize_single_n() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream = tokenizer.tokenize("n").expect("must succeed");
    assert_eq!(stream.len(), 1);
    assert_eq!(stream.tokens()[0].kind, TokenKind::Keyword("n"));
    assert_eq!(stream.tokens()[0].text, "n");
    assert_eq!(stream.tokens()[0].span, Span::new(0, 1));
}

#[test]
fn tokenize_addition_with_whitespace() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream = tokenizer.tokenize("n + n").expect("must succeed");
    assert_eq!(stream.len(), 3);
    assert_eq!(stream.tokens()[0].span, Span::new(0, 1));
    assert_eq!(stream.tokens()[1].span, Span::new(2, 3));
    assert_eq!(stream.tokens()[2].span, Span::new(4, 5));
}

#[test]
fn tokenize_complex_expression_keeps_correct_spans() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let src = "n + n * (n + n)";
    let stream = tokenizer.tokenize(src).expect("must succeed");
    // 9 Tokens: n + n * ( n + n )
    assert_eq!(stream.len(), 9);
    // Erstes Token bei 0..1, letztes ")" bei 14..15.
    assert_eq!(stream.tokens()[0].span, Span::new(0, 1));
    assert_eq!(stream.tokens()[stream.len() - 1].span, Span::new(14, 15));
    // Konkrete Token-Sequenz pruefen.
    let kinds: Vec<TokenKind> = stream.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Keyword("n"),
            TokenKind::Punct("+"),
            TokenKind::Keyword("n"),
            TokenKind::Punct("*"),
            TokenKind::Punct("("),
            TokenKind::Keyword("n"),
            TokenKind::Punct("+"),
            TokenKind::Keyword("n"),
            TokenKind::Punct(")"),
        ]
    );
}

#[test]
fn tokenize_multiline_input_handles_newlines_as_whitespace() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let src = "n\n+\nn";
    let stream = tokenizer.tokenize(src).expect("must succeed");
    assert_eq!(stream.len(), 3);
    // "n" 0..1, "+" 2..3, "n" 4..5
    assert_eq!(stream.tokens()[0].span, Span::new(0, 1));
    assert_eq!(stream.tokens()[1].span, Span::new(2, 3));
    assert_eq!(stream.tokens()[2].span, Span::new(4, 5));
}

#[test]
fn tokenize_tabs_and_mixed_whitespace() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream = tokenizer
        .tokenize("\t  n\t+\t\tn  \r\n")
        .expect("must succeed");
    assert_eq!(stream.len(), 3);
    let kinds: Vec<TokenKind> = stream.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Keyword("n"),
            TokenKind::Punct("+"),
            TokenKind::Keyword("n"),
        ]
    );
}

// ---------------------------------------------------------------------------
// Lexer-Errors
// ---------------------------------------------------------------------------

#[test]
fn tokenize_unknown_character_returns_lexer_error_with_span() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let result = tokenizer.tokenize("n @ n");
    assert!(result.is_err());
    if let Err(err) = result {
        // Position 2 = das @-Zeichen
        assert_eq!(err.span(), Span::point(2));
    }
}

#[test]
fn tokenize_identifier_not_in_keywords_fails_for_toy() {
    // Toy-Grammar hat nur "n" als Identifier-Klassifikation. "foo" ist kein
    // Keyword, und Toy hat kein Ident in der Grammar — der Tokenizer
    // erzeugt einen Ident-Token, aber der waere fuer den Recognizer
    // nicht akzeptabel. Hier nur Lexer-Pfad: Tokenizer produziert ihn,
    // Recognizer wuerde ihn ablehnen.
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream = tokenizer.tokenize("foo").expect("Lexer akzeptiert Ident");
    assert_eq!(stream.len(), 1);
    assert_eq!(stream.tokens()[0].kind, TokenKind::Ident);
    assert_eq!(stream.tokens()[0].text, "foo");
}

// ---------------------------------------------------------------------------
// TokenStream-API
// ---------------------------------------------------------------------------

#[test]
fn token_stream_iter_and_get() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream = tokenizer.tokenize("n + n").expect("must succeed");

    let collected: Vec<TokenKind> = stream.iter().map(|t| t.kind).collect();
    assert_eq!(collected.len(), 3);

    // get() out-of-bounds returns None
    assert!(stream.get(99).is_none());
    // get(0) returns first token
    assert_eq!(stream.get(0).map(|t| t.kind), Some(TokenKind::Keyword("n")));
}

#[test]
fn token_stream_kinds_returns_classification_only() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let stream = tokenizer.tokenize("n + n").expect("must succeed");
    let kinds = stream.kinds();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Keyword("n"),
            TokenKind::Punct("+"),
            TokenKind::Keyword("n"),
        ]
    );
}

#[test]
fn token_text_slice_points_into_source() {
    let tokenizer = Tokenizer::for_grammar(&TOY);
    let src = String::from("n + n");
    let stream = tokenizer.tokenize(&src).expect("must succeed");
    // Pointer-Identitaet: Token::text muss in src liegen, nicht eine Kopie sein.
    let first_token = stream.tokens()[0];
    assert!(std::ptr::eq(first_token.text.as_ptr(), src.as_ptr()));
}
