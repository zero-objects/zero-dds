// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Toy grammar of arithmetic expressions.
//!
//! Classic operator-precedence structure:
//!
//! ```text
//! E ::= E "+" T | T
//! T ::= T "*" F | F
//! F ::= "n" | "(" E ")"
//! ```
//!
//! This grammar is left-recursive (E and T) — Earley handles that correctly,
//! the validator reports the expected `LeftRecursion` warnings without errors.
//! It is the test grammar for M1: shows that the full T1.1–T1.5
//! pipeline (data model, validation, recognition, facade) holds up against a
//! realistic-looking use case.
//!
//! `n` stands in for "number"; in a real grammar this would be
//! a `TokenKind::IntegerLiteral`. We use `Keyword("n")` to
//! stay lexer-independent (the lexer comes in week 2).
//!
//! Usage:
//!
//! ```rust,ignore
//! use zerodds_idl::engine::parse;
//! use zerodds_idl::grammar::toy::TOY;
//! use zerodds_idl::grammar::TokenKind;
//!
//! let tokens = [
//!     TokenKind::Keyword("n"),
//!     TokenKind::Punct("+"),
//!     TokenKind::Keyword("n"),
//! ];
//! assert!(parse(&TOY, &tokens).is_ok());
//! ```

use super::{
    Alternative, Grammar, IdlVersion, Production, ProductionId, SpecRef, Symbol, TokenKind,
};

/// Spec anchor for the toy grammar — refers to this module, not to
/// an external specification.
const SR: SpecRef = SpecRef {
    doc: "ZeroDDS Toy Arith Grammar",
    section: "0.0",
};

/// `E ::= E "+" T | T`
const PROD_E: Production = Production {
    id: ProductionId(0),
    name: "expr",
    spec_ref: SR,
    alternatives: &[
        Alternative {
            name: Some("plus"),
            symbols: &[
                Symbol::Nonterminal(ProductionId(0)), // E
                Symbol::Terminal(TokenKind::Punct("+")),
                Symbol::Nonterminal(ProductionId(1)), // T
            ],
            note: None,
        },
        Alternative {
            name: Some("just_term"),
            symbols: &[Symbol::Nonterminal(ProductionId(1))],
            note: None,
        },
    ],
    ast_hint: None,
};

/// `T ::= T "*" F | F`
const PROD_T: Production = Production {
    id: ProductionId(1),
    name: "term",
    spec_ref: SR,
    alternatives: &[
        Alternative {
            name: Some("times"),
            symbols: &[
                Symbol::Nonterminal(ProductionId(1)), // T
                Symbol::Terminal(TokenKind::Punct("*")),
                Symbol::Nonterminal(ProductionId(2)), // F
            ],
            note: None,
        },
        Alternative {
            name: Some("just_factor"),
            symbols: &[Symbol::Nonterminal(ProductionId(2))],
            note: None,
        },
    ],
    ast_hint: None,
};

/// `F ::= "n" | "(" E ")"`
const PROD_F: Production = Production {
    id: ProductionId(2),
    name: "factor",
    spec_ref: SR,
    alternatives: &[
        Alternative {
            name: Some("number"),
            symbols: &[Symbol::Terminal(TokenKind::Keyword("n"))],
            note: None,
        },
        Alternative {
            name: Some("paren"),
            symbols: &[
                Symbol::Terminal(TokenKind::Punct("(")),
                Symbol::Nonterminal(ProductionId(0)), // E
                Symbol::Terminal(TokenKind::Punct(")")),
            ],
            note: None,
        },
    ],
    ast_hint: None,
};

/// The fully assembled toy grammar with start = E.
pub const TOY: Grammar = Grammar {
    name: "toy_arith",
    version: IdlVersion::V4_2,
    productions: &[PROD_E, PROD_T, PROD_F],
    start: ProductionId(0),
    token_rules: &[],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::validate::{Severity, ValidationIssue, validate};

    #[test]
    fn toy_grammar_has_no_errors_only_warnings() {
        let report = validate(&TOY);
        assert!(
            !report.has_errors(),
            "Errors: {:?}",
            report.errors().collect::<Vec<_>>()
        );
        assert!(report.warnings().all(|i| i.severity() == Severity::Warning));
    }

    #[test]
    fn toy_grammar_validator_reports_left_recursion() {
        let report = validate(&TOY);
        let lr_count = report
            .issues()
            .iter()
            .filter(|i| matches!(i, ValidationIssue::LeftRecursion { .. }))
            .count();
        assert!(
            lr_count >= 1,
            "The toy grammar is left-recursive (E and T) — at least one LR warning expected. Report: {:?}",
            report.issues()
        );
    }

    #[test]
    fn toy_grammar_validator_reports_first_first_conflicts() {
        // E ::= E "+" T | T and T ::= T "*" F | F: both alts have
        // FIRST = {n, (}. The validator should report that.
        let report = validate(&TOY);
        let ffc_count = report
            .issues()
            .iter()
            .filter(|i| matches!(i, ValidationIssue::FirstFirstConflict { .. }))
            .count();
        assert!(
            ffc_count >= 2,
            "expected ≥2 FirstFirstConflicts (E and T). Report: {:?}",
            report.issues()
        );
    }

    #[test]
    fn toy_grammar_starts_at_expression() {
        let start = TOY.start_production();
        assert!(start.is_some_and(|p| p.name == "expr"));
    }

    #[test]
    fn toy_grammar_has_three_productions() {
        assert_eq!(TOY.production_count(), 3);
    }

    #[test]
    fn alternative_names_are_set() {
        let production = TOY.production(ProductionId(0));
        assert_eq!(
            production.map(|p| p.alternatives.len()),
            Some(2),
            "E must have 2 alternatives"
        );
        assert_eq!(
            production.map(|p| p.alternatives[0].name),
            Some(Some("plus"))
        );
        assert_eq!(
            production.map(|p| p.alternatives[1].name),
            Some(Some("just_term"))
        );
    }
}
