// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Lexer-Stufe der IDL-Pipeline.
//!
//! Die Lexer-Schicht zerlegt den Source-Text in eine Sequenz von [`Token`]s,
//! die der Recognizer (siehe [`crate::engine`]) konsumieren kann. Lexer-
//! Datentypen leben in [`token`]; die Token-Regel-Extraktion aus
//! Grammar-Daten und der eigentliche Tokenizer folgen in Tasks 2.2/2.3.
//!
//! Siehe RFC 0001 §4.1 (Pipeline) und §5.x (Lexer).

pub mod rules;
pub mod token;
pub mod tokenizer;

pub use rules::TokenRules;
pub use token::{Token, TokenStream};
pub use tokenizer::Tokenizer;
