// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Earley-Parse-Engine.
//!
//! The engine reads token streams and produces parse forests, based
//! on a grammar from [`crate::grammar`]. The implementation is split
//! across several submodules:
//!
//! - [`state`] — base data types [`EarleyItem`] and [`StateSet`] (Task 1.3).
//! - [`recognize`] — scan/predict/complete algorithm (Task 1.4).
//! - Parse-forest construction follows in Task 2.4.
//!
//! This module also provides the top-level facade [`Engine`] (Task 1.5):
//! a wrapper around grammar + [`Recognizer`] that validates the grammar
//! on construction ([`crate::grammar::validate`]) and blocks the
//! validation errors during recognition. External consumers (e.g. `tools/idlc`)
//! work against this facade, not directly against the recognizer.
//!
//! See RFC 0001 §5.2.

pub mod recognize;
pub mod state;

pub use recognize::{RecognitionResult, Recognizer};
pub use state::{EarleyItem, StateSet};

use crate::grammar::{
    Grammar,
    compile::{CompiledGrammar, compile},
    validate::{ValidationReport, validate},
};
use crate::lexer::Token;

/// High-level error when using the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// The grammar contains at least one `Severity::Error` finding
    /// (invalid-start or dangling-reference). Recognition is stopped in this
    /// case without an attempt — the results would otherwise be misleading.
    InvalidGrammar(ValidationReport),
    /// Recognition ran through without an engine error, but the grammar did
    /// not accept the token sequence.
    NotAccepted {
        /// Number of consumed tokens — the last position at which items
        /// were still present (best-effort for diagnostics).
        last_consumed: usize,
    },
}

/// Engine facade.
///
/// Construction validates the grammar once. Subsequent
/// `recognize` calls use the persisted validation report to
/// immediately reject calls on broken grammars, without going through the
/// recognizer path.
#[derive(Debug, Clone)]
pub struct Engine<'g> {
    grammar: &'g Grammar,
    validation: ValidationReport,
    compiled: CompiledGrammar,
}

impl<'g> Engine<'g> {
    /// Constructs an engine for the given grammar.
    ///
    /// Validation and EBNF compile run once immediately. On
    /// `Severity::Error` findings the engine is still
    /// constructible; `recognize` then rejects every call
    /// with `EngineError::InvalidGrammar`.
    ///
    /// The compile pass desugars `Symbol::Repeat`/`Symbol::Choice` into
    /// recursive helper productions; see [`crate::grammar::compile`].
    #[must_use]
    pub fn new(grammar: &'g Grammar) -> Self {
        let validation = validate(grammar);
        let compiled = compile(grammar);
        Self {
            grammar,
            validation,
            compiled,
        }
    }

    /// Access to the EBNF-desugared form of the grammar. Used by the
    /// recognizer; external CST builders need it too
    /// (call [`build_cst`](crate::cst::build_cst) with this reference).
    #[must_use]
    pub fn compiled_grammar(&self) -> &CompiledGrammar {
        &self.compiled
    }

    /// Access to the validation report (errors + warnings).
    #[must_use]
    pub fn validation_report(&self) -> &ValidationReport {
        &self.validation
    }

    /// The underlying grammar.
    #[must_use]
    pub fn grammar(&self) -> &'g Grammar {
        self.grammar
    }

    /// Recognition for a token sequence.
    ///
    /// # Errors
    /// Returns [`EngineError::InvalidGrammar`] if the grammar had
    /// `Severity::Error` findings at construction; or
    /// [`EngineError::NotAccepted`] if recognition ran through but
    /// the grammar rejected the token sequence.
    pub fn recognize(&self, tokens: &[Token<'_>]) -> Result<RecognitionResult, EngineError> {
        if self.validation.has_errors() {
            return Err(EngineError::InvalidGrammar(self.validation.clone()));
        }
        let result = Recognizer::new(&self.compiled).recognize(tokens);
        if result.accepted {
            Ok(result)
        } else {
            Err(EngineError::NotAccepted {
                last_consumed: tokens.len(),
            })
        }
    }
}

/// Convenience function: build engine + recognition in one step.
///
/// Usable for one-off calls in tests or in the `tools/idlc` CLI. For
/// repeated recognition on the same grammar, [`Engine::new`] +
/// multiple [`Engine::recognize`] is more efficient (validation runs only
/// once).
///
/// # Errors
/// Wie [`Engine::recognize`].
pub fn parse(grammar: &Grammar, tokens: &[Token<'_>]) -> Result<RecognitionResult, EngineError> {
    Engine::new(grammar).recognize(tokens)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::grammar::{
        Alternative, IdlVersion, Production, ProductionId, SpecRef, Symbol, TokenKind,
    };

    /// Test helper: token from a pure TokenKind without a source location.
    fn t(kind: TokenKind) -> Token<'static> {
        Token::synthetic(kind)
    }

    const TS: SpecRef = SpecRef {
        doc: "TEST",
        section: "0.0",
    };

    /// Saubere Grammar: A ::= "x"
    const G_OK: Grammar = Grammar {
        name: "ok",
        version: IdlVersion::V4_2,
        productions: &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: TS,
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Terminal(TokenKind::Keyword("x"))],
                note: None,
            }],
            ast_hint: None,
        }],
        start: ProductionId(0),
        token_rules: &[],
    };

    /// Broken grammar: start points to a non-existent production.
    const G_INVALID_START: Grammar = Grammar {
        name: "invalid_start",
        version: IdlVersion::V4_2,
        productions: &[],
        start: ProductionId(99),
        token_rules: &[],
    };

    /// Broken grammar: dangling nonterminal reference.
    const G_DANGLING: Grammar = Grammar {
        name: "dangling",
        version: IdlVersion::V4_2,
        productions: &[Production {
            id: ProductionId(0),
            name: "a",
            spec_ref: TS,
            alternatives: &[Alternative {
                name: None,
                symbols: &[Symbol::Nonterminal(ProductionId(99))],
                note: None,
            }],
            ast_hint: None,
        }],
        start: ProductionId(0),
        token_rules: &[],
    };

    #[test]
    fn engine_new_runs_validation_eagerly() {
        let engine = Engine::new(&G_OK);
        assert!(engine.validation_report().is_empty());
    }

    #[test]
    fn engine_new_captures_errors_on_broken_grammar() {
        let engine = Engine::new(&G_INVALID_START);
        assert!(engine.validation_report().has_errors());
    }

    #[test]
    fn engine_grammar_accessor_returns_same_reference() {
        let engine = Engine::new(&G_OK);
        // Pointer comparison via slice identity.
        assert!(std::ptr::eq(engine.grammar(), &G_OK));
    }

    #[test]
    fn engine_recognize_succeeds_on_valid_grammar_and_input() {
        let engine = Engine::new(&G_OK);
        let result = engine.recognize(&[t(TokenKind::Keyword("x"))]);
        assert!(matches!(result, Ok(r) if r.accepted));
    }

    #[test]
    fn engine_recognize_returns_invalid_grammar_for_invalid_start() {
        let engine = Engine::new(&G_INVALID_START);
        let result = engine.recognize(&[]);
        assert!(matches!(result, Err(EngineError::InvalidGrammar(_))));
    }

    #[test]
    fn engine_recognize_returns_invalid_grammar_for_dangling_reference() {
        let engine = Engine::new(&G_DANGLING);
        let result = engine.recognize(&[]);
        assert!(matches!(result, Err(EngineError::InvalidGrammar(_))));
    }

    #[test]
    fn engine_recognize_returns_not_accepted_for_wrong_input() {
        let engine = Engine::new(&G_OK);
        let result = engine.recognize(&[t(TokenKind::Keyword("y"))]);
        assert!(matches!(
            result,
            Err(EngineError::NotAccepted { last_consumed: 1 })
        ));
    }

    #[test]
    fn engine_recognize_returns_not_accepted_for_empty_input_when_grammar_requires_terminal() {
        let engine = Engine::new(&G_OK);
        let result = engine.recognize(&[]);
        assert!(matches!(
            result,
            Err(EngineError::NotAccepted { last_consumed: 0 })
        ));
    }

    #[test]
    fn parse_convenience_function_succeeds_on_valid_input() {
        let result = parse(&G_OK, &[t(TokenKind::Keyword("x"))]);
        assert!(matches!(result, Ok(r) if r.accepted));
    }

    #[test]
    fn parse_convenience_function_propagates_invalid_grammar_error() {
        let result = parse(&G_INVALID_START, &[]);
        assert!(matches!(result, Err(EngineError::InvalidGrammar(_))));
    }

    #[test]
    fn engine_validation_report_persists_across_recognize_calls() {
        let engine = Engine::new(&G_OK);
        let _first = engine.recognize(&[t(TokenKind::Keyword("x"))]);
        let _second = engine.recognize(&[t(TokenKind::Keyword("x"))]);
        // The report should not be modified by recognize calls.
        assert!(engine.validation_report().is_empty());
    }
}
