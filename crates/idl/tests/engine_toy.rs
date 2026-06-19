//! End-to-end integration tests for the Earley engine against the toy grammar
//! (`E ::= E "+" T | T;  T ::= T "*" F | F;  F ::= "n" | "(" E ")"`).
//!
//! Validates the full pipeline (grammar data model, validator, recognizer,
//! engine facade) via `zerodds_idl::engine::parse` as the public API.
//! If these tests are green, week-1 milestone M1 is reached:
//! "engine recognizes the toy grammar" (see
//! `.planning/wp-0.3-idl-parser/PLAN.md` week table).

use zerodds_idl::engine::{EngineError, parse};
use zerodds_idl::grammar::{TokenKind, toy::TOY};
use zerodds_idl::lexer::Token;

const N: Token<'static> = Token::synthetic(TokenKind::Keyword("n"));
const PLUS: Token<'static> = Token::synthetic(TokenKind::Punct("+"));
const TIMES: Token<'static> = Token::synthetic(TokenKind::Punct("*"));
const LPAREN: Token<'static> = Token::synthetic(TokenKind::Punct("("));
const RPAREN: Token<'static> = Token::synthetic(TokenKind::Punct(")"));

// ---------------------------------------------------------------------------
// Akzeptierende Eingaben
// ---------------------------------------------------------------------------

#[test]
fn accepts_single_number() {
    assert!(parse(&TOY, &[N]).is_ok());
}

#[test]
fn accepts_addition() {
    // n + n
    assert!(parse(&TOY, &[N, PLUS, N]).is_ok());
}

#[test]
fn accepts_multiplication() {
    // n * n
    assert!(parse(&TOY, &[N, TIMES, N]).is_ok());
}

#[test]
fn accepts_mixed_precedence() {
    // n + n * n  — precedence via the grammar (T binds tighter)
    assert!(parse(&TOY, &[N, PLUS, N, TIMES, N]).is_ok());
}

#[test]
fn accepts_left_associative_addition() {
    // n + n + n + n
    assert!(parse(&TOY, &[N, PLUS, N, PLUS, N, PLUS, N]).is_ok());
}

#[test]
fn accepts_left_associative_multiplication() {
    // n * n * n
    assert!(parse(&TOY, &[N, TIMES, N, TIMES, N]).is_ok());
}

#[test]
fn accepts_parenthesized_subexpression() {
    // ( n + n ) * n  — parentheses override precedence
    assert!(parse(&TOY, &[LPAREN, N, PLUS, N, RPAREN, TIMES, N]).is_ok());
}

#[test]
fn accepts_nested_parentheses() {
    // ( ( n ) )
    assert!(parse(&TOY, &[LPAREN, LPAREN, N, RPAREN, RPAREN]).is_ok());
}

#[test]
fn accepts_parenthesized_complex_expression() {
    // ( n + n * ( n + n ) ) * n
    assert!(
        parse(
            &TOY,
            &[
                LPAREN, N, PLUS, N, TIMES, LPAREN, N, PLUS, N, RPAREN, RPAREN, TIMES, N
            ],
        )
        .is_ok()
    );
}

// ---------------------------------------------------------------------------
// Ablehnende Eingaben
// ---------------------------------------------------------------------------

#[test]
fn rejects_empty_input() {
    let result = parse(&TOY, &[]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

#[test]
fn rejects_lone_operator() {
    let result = parse(&TOY, &[PLUS]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

#[test]
fn rejects_two_numbers_without_operator() {
    // n n  — no operator
    let result = parse(&TOY, &[N, N]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

#[test]
fn rejects_trailing_operator() {
    // n +
    let result = parse(&TOY, &[N, PLUS]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

#[test]
fn rejects_unmatched_open_paren() {
    // ( n
    let result = parse(&TOY, &[LPAREN, N]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

#[test]
fn rejects_unmatched_close_paren() {
    // n )
    let result = parse(&TOY, &[N, RPAREN]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

#[test]
fn rejects_double_operator() {
    // n + + n
    let result = parse(&TOY, &[N, PLUS, PLUS, N]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

#[test]
fn rejects_empty_parens() {
    // ( )
    let result = parse(&TOY, &[LPAREN, RPAREN]);
    assert!(matches!(result, Err(EngineError::NotAccepted { .. })));
}

// ---------------------------------------------------------------------------
// State-set invariant: n+1 sets for n tokens
// ---------------------------------------------------------------------------

#[test]
fn state_set_count_matches_token_count_plus_one() {
    let tokens = [N, PLUS, N, TIMES, N];
    let result = parse(&TOY, &tokens);
    assert!(matches!(
        result,
        Ok(ref r) if r.accepted && r.state_sets.len() == tokens.len() + 1
    ));
}

// ---------------------------------------------------------------------------
// NotAccepted carries last_consumed
// ---------------------------------------------------------------------------

#[test]
fn not_accepted_reports_last_consumed_count() {
    let tokens = [N, PLUS]; // incomplete
    let err = parse(&TOY, &tokens);
    assert!(matches!(
        err,
        Err(EngineError::NotAccepted { last_consumed: 2 })
    ));
}
