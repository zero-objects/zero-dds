// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-idl-cst`. Safety classification: **SAFE**.
//!
//! The lossless concrete syntax tree for OMG IDL 4.2: tokens that carry
//! their trivia, and a green/red tree that reproduces its input byte for
//! byte — including for files that fail to parse, because error recovery
//! inserts nodes rather than dropping input.
//!
//! This crate holds the tree. The parser that builds it is
//! `zerodds-idl-parse`; the lowering that consumes it is
//! `zerodds-idl-lower`.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-idl-frontend-1.0.md` §1 (lexing), §2 (the tree).
//! - OMG IDL 4.2 §7.2 (lexical conventions), §7.4 (grammar).
//!
//! ## Layer position
//!
//! Layer 0 of the frontend — no dependencies.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

// The unit-test harness itself needs std; the crate does not.
#[cfg(test)]
extern crate std;

pub mod lex;
pub mod span;
pub mod token;
pub mod tree;

pub use lex::{Lexer, lex, literal_value};
pub use span::{LineColumn, Span};
pub use token::{
    KEYWORDS, Radix, Token, TokenKind, TokenValue, Trivia, TriviaKind, identifier_name,
    keyword_collision,
};
pub use tree::{
    Checkpoint, CstKind, GreenChild, GreenNode, GreenToken, SyntaxNode, SyntaxSpans, TreeBuilder,
};
