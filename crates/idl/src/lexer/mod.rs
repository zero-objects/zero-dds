// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lexer stage of the IDL pipeline.
//!
//! The lexer layer splits the source text into a sequence of [`Token`]s
//! that the recognizer (see [`crate::engine`]) can consume. Lexer
//! data types live in [`token`]; the token-rule extraction from
//! grammar data and the actual tokenizer follow in Tasks 2.2/2.3.
//!
//! See RFC 0001 §4.1 (pipeline) and §5.x (lexer).

pub mod rules;
pub mod token;
pub mod tokenizer;

pub use rules::TokenRules;
pub use token::{Token, TokenStream};
pub use tokenizer::Tokenizer;
