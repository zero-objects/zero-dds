// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Earley-Parse-Engine.
//!
//! Die Engine liest Token-Streams und produziert Parse-Forests, basierend
//! auf einer Grammar aus [`crate::grammar`]. Implementierung verteilt sich
//! ueber mehrere Submodule:
//!
//! - [`state`] — Grunddatentypen [`EarleyItem`] und [`StateSet`] (Task 1.3).
//! - [`recognize`] — Scan/Predict/Complete-Algorithmus (Task 1.4).
//! - Parse-Forest-Konstruktion folgt in Task 2.4.
//!
//! Dieses Modul liefert ausserdem die Top-Level-Facade [`Engine`] (Task 1.5):
//! ein Wrapper um Grammar + [`Recognizer`], der beim Konstruieren die Grammar
//! validiert ([`crate::grammar::validate`]) und bei der Recognition die
//! Validation-Errors blockiert. Externe Konsumenten (z.B. `tools/idlc`)
//! arbeiten gegen diese Facade, nicht direkt gegen den Recognizer.
//!
//! Siehe RFC 0001 §5.2.

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

/// High-Level-Fehler beim Engine-Einsatz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// Die Grammar enthaelt mindestens einen `Severity::Error`-Befund
    /// (Invalid-Start oder Dangling-Reference). Recognition wird in diesem
    /// Fall ohne Versuch gestoppt — die Resultate waeren sonst irrefuehrend.
    InvalidGrammar(ValidationReport),
    /// Recognition lief ohne Engine-Fehler durch, aber die Grammar hat die
    /// Token-Sequenz nicht akzeptiert.
    NotAccepted {
        /// Anzahl der konsumierten Tokens — letzte Position, an der noch
        /// Items vorhanden waren (best-effort fuer Diagnostik).
        last_consumed: usize,
    },
}

/// Engine-Facade.
///
/// Konstruktion validiert die Grammar einmalig. Anschliessende
/// `recognize`-Aufrufe nutzen den persistierten Validation-Report, um
/// Aufrufe auf kaputte Grammatiken sofort abzulehnen, ohne den
/// Recognizer-Pfad zu durchlaufen.
#[derive(Debug, Clone)]
pub struct Engine<'g> {
    grammar: &'g Grammar,
    validation: ValidationReport,
    compiled: CompiledGrammar,
}

impl<'g> Engine<'g> {
    /// Konstruiert eine Engine fuer die gegebene Grammar.
    ///
    /// Validation und EBNF-Compile laufen einmal sofort. Bei
    /// `Severity::Error`-Befunden bleibt die Engine trotzdem
    /// konstruierbar; `recognize` lehnt dann jedoch jeden Aufruf
    /// mit `EngineError::InvalidGrammar` ab.
    ///
    /// Der Compile-Pass desugart `Symbol::Repeat`/`Symbol::Choice` zu
    /// rekursiven Hilfs-Productions; siehe [`crate::grammar::compile`].
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

    /// Zugriff auf die EBNF-desugarte Form der Grammar. Wird vom
    /// Recognizer benutzt; externe CST-Bauer brauchen sie ebenfalls
    /// (rufe [`build_cst`](crate::cst::build_cst) mit dieser Referenz).
    #[must_use]
    pub fn compiled_grammar(&self) -> &CompiledGrammar {
        &self.compiled
    }

    /// Zugriff auf den Validation-Report (Errors + Warnings).
    #[must_use]
    pub fn validation_report(&self) -> &ValidationReport {
        &self.validation
    }

    /// Die zugrundeliegende Grammar.
    #[must_use]
    pub fn grammar(&self) -> &'g Grammar {
        self.grammar
    }

    /// Recognition fuer eine Token-Sequenz.
    ///
    /// # Errors
    /// Liefert [`EngineError::InvalidGrammar`], wenn die Grammar bei der
    /// Konstruktion `Severity::Error`-Befunde hatte; oder
    /// [`EngineError::NotAccepted`], wenn die Recognition durchlief, aber
    /// die Grammar die Token-Sequenz ablehnte.
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

/// Convenience-Funktion: Engine bauen + Recognition in einem Schritt.
///
/// Nutzbar fuer einmalige Aufrufe in Tests oder im `tools/idlc`-CLI. Fuer
/// wiederholte Recognition auf derselben Grammar ist [`Engine::new`] +
/// mehrfaches [`Engine::recognize`] effizienter (Validation laeuft nur
/// einmal).
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

    /// Test-Helper: Token aus reinem TokenKind ohne Quellort.
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

    /// Kaputte Grammar: Start zeigt auf nicht existierende Production.
    const G_INVALID_START: Grammar = Grammar {
        name: "invalid_start",
        version: IdlVersion::V4_2,
        productions: &[],
        start: ProductionId(99),
        token_rules: &[],
    };

    /// Kaputte Grammar: Dangling-Nonterminal-Referenz.
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
        // Pointer-Vergleich ueber identitaet des Slices.
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
        // Report sollte nicht durch Recognize-Aufrufe modifiziert werden.
        assert!(engine.validation_report().is_empty());
    }
}
