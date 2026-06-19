// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Grammar data model for the grammar-driven IDL parser.
//!
//! Grammars are defined as `&'static` data (compile-time constants),
//! not as code. The parse engine (`crate::engine`) traverses the
//! grammar data to convert tokens into a concrete syntax tree.
//!
//! See RFC 0001 §5.1 for the design rationale.
//!
//! A static validator for grammar data lives in the submodule
//! [`validate`] — see there for the list of detected problems.
//!
//! ## Design invariants
//!
//! - No heap allocation for grammar data at runtime. Productions,
//!   alternatives and symbols are `&'static [...]` slices in the binary segment.
//! - Each [`Production`] carries a [`SpecRef`] — the exact section number
//!   in the underlying spec (OMG IDL 4.2 §7.x). This anchor is consumed by
//!   `tools/traceability` and is part of the audit evidence
//!   (`docs/architecture/04_safety_by_architecture.md §4`).
//! - [`ProductionId`] is a newtype wrapper around `u32` and is used to refer
//!   between productions (nonterminal references).

use core::fmt;

pub mod compile;
pub mod compose;
pub mod deltas;
pub mod idl42;
pub mod toy;
pub mod validate;

/// Version of the IDL spec a grammar adheres to.
///
/// Version deltas (Task 6.4, `grammar::deltas`) compose a base grammar
/// with version-specific deviations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdlVersion {
    /// Pre-OMG-4.0. Historically relevant for migration from older codebases.
    V3_5,
    /// First OMG 4.x edition.
    V4_0,
    /// Intermediate revision.
    V4_1,
    /// Current target standard for ZeroDDS. Default.
    V4_2,
}

impl Default for IdlVersion {
    fn default() -> Self {
        Self::V4_2
    }
}

/// Unique identifier for a production within a grammar.
///
/// Newtype around `u32`. Indices are stable within a grammar constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProductionId(pub u32);

impl ProductionId {
    /// The raw index as `usize` — for array lookups.
    #[inline]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Unique identifier for a token rule within a grammar.
///
/// Newtype around `u32`. Assigned when extracting the token rules from terminals
/// (Task 2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenRuleId(pub u32);

/// Reference to a spec section.
///
/// Example: `SpecRef { doc: "OMG IDL 4.2", section: "7.4.1.4.4.2" }` refers
/// to the `<struct_def>` production in the IDL 4.2 document.
///
/// `Display` renders as `"OMG IDL 4.2 §7.4.1.4.4.2"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpecRef {
    /// Human-readable document name (e.g. `"OMG IDL 4.2"`).
    pub doc: &'static str,
    /// Section path within the document (e.g. `"7.4.1.4.4.2"`).
    pub section: &'static str,
}

impl fmt::Display for SpecRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} §{}", self.doc, self.section)
    }
}

/// Classification of a terminal token.
///
/// Token level of the grammar: what the lexer recognizes from the source text.
/// Concrete lexer rules are extracted from the terminals of a grammar
/// (Task 2.2). The lexer set is expanded in week 2; for Task 1.1 the
/// basic categorization suffices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TokenKind {
    /// Reserved keyword (`struct`, `module`, `interface`, ...).
    Keyword(&'static str),
    /// Punctuation or operator (`{`, `;`, `::`, `<`, ...).
    Punct(&'static str),
    /// Identifier.
    Ident,
    /// Integer literal.
    IntegerLiteral,
    /// Floating-point literal.
    FloatLiteral,
    /// String literal.
    StringLiteral,
    /// Char literal.
    CharLiteral,
    /// Boolean literal (`TRUE`, `FALSE`).
    BoolLiteral,
    /// Wide-char literal (`L'x'`, IDL 4.2 §7.2.6.3).
    WideCharLiteral,
    /// Wide-string literal (`L"..."`, IDL 4.2 §7.2.6.5).
    WideStringLiteral,
    /// Fixed-point literal (e.g. `1.234d`, IDL 4.2 §7.2.6.6).
    FixedPtLiteral,
    /// Start or end of the input (used synthetically by the engine).
    EndOfInput,
}

/// Repetition within a production alternative.
///
/// Corresponds to the EBNF metasymbols:
/// - [`RepeatKind::ZeroOrMore`] — `{ X }*`
/// - [`RepeatKind::OneOrMore`] — `{ X }+`
/// - [`RepeatKind::Optional`] — `[ X ]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepeatKind {
    /// Zero or more repetitions.
    ZeroOrMore,
    /// One or more repetitions.
    OneOrMore,
    /// Optional — zero or one repetition.
    Optional,
}

/// Element of an alternative.
///
/// Recursive enum: terminals (tokens), nonterminals (references to other
/// productions), repetitions and inline alternatives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    /// Terminal — a token produced by the lexer.
    Terminal(TokenKind),
    /// Nonterminal — a reference to another production.
    Nonterminal(ProductionId),
    /// Repetition of a subsequence.
    Repeat(RepeatKind, &'static [Symbol]),
    /// Inline alternatives — several branches in place.
    Choice(&'static [&'static [Symbol]]),
}

impl Symbol {
    /// `true` if the symbol is a terminal (i.e. a lexer token).
    #[inline]
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Terminal(_))
    }

    /// `true` if the symbol is a nonterminal (i.e. a reference to
    /// another production).
    #[inline]
    #[must_use]
    pub const fn is_nonterminal(self) -> bool {
        matches!(self, Self::Nonterminal(_))
    }
}

/// An alternative within a production.
///
/// Corresponds to a branch of an EBNF right-hand side, e.g. in
/// `<type_spec> ::= <simple_type_spec> | <template_type_spec>` the
/// two nonterminals are each an alternative.
#[derive(Debug, Clone, Copy)]
pub struct Alternative {
    /// Optional name of the alternative (e.g. `"prefixed"`, `"unqualified"`).
    /// Useful for AST-builder dispatch (Task 5.2) and as a diagnostic
    /// anchor in validation reports.
    pub name: Option<&'static str>,
    /// The sequence of symbols that form this alternative.
    pub symbols: &'static [Symbol],
    /// Optional review note (e.g. a hint about vendor specifics or
    /// ambiguities in the spec). Appears in the grammar validation report.
    pub note: Option<&'static str>,
}

/// Reference to a specific alternative of a production.
///
/// Used in validation reports to locate issues exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AltRef {
    /// Index of the alternative within `Production::alternatives`.
    pub index: usize,
    /// Copy of the optional name (see `Alternative::name`).
    pub name: Option<&'static str>,
}

/// Optional hint for the AST builder, which builder function to call for
/// this production. Details are specified in week 5
/// (Task 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AstHint {
    /// Trigger for the AST builder under this symbolic name.
    /// The builder dispatches on this name, not on the production ID.
    Named(&'static str),
}

/// A production — the left-hand side of an EBNF rule.
///
/// Example:
///
/// ```rust,ignore
/// const PROD_MODULE: Production = Production {
///     id: ProductionId(1),
///     name: "module",
///     spec_ref: SpecRef { doc: "OMG IDL 4.2", section: "7.4.1.3" },
///     alternatives: &[/* ... */],
///     ast_hint: Some(AstHint::Named("Module")),
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Production {
    /// Unique ID within the grammar.
    pub id: ProductionId,
    /// Human-readable name (corresponds to the EBNF nonterminal name).
    pub name: &'static str,
    /// Reference to the spec section this production stems from.
    pub spec_ref: SpecRef,
    /// The branches of the right-hand side.
    pub alternatives: &'static [Alternative],
    /// Optional builder hint.
    pub ast_hint: Option<AstHint>,
}

/// Token-match rule for the lexer.
///
/// For now only structure. The match logic is implemented in week 2 (Task 2.3).
#[derive(Debug, Clone, Copy)]
pub struct TokenRule {
    /// ID of the rule within the grammar.
    pub id: TokenRuleId,
    /// Which TokenKind is produced.
    pub kind: TokenKind,
    /// Match literal (for `Keyword` and `Punct`) or pattern name (for
    /// regex-like tokens such as identifiers). Pattern names are mapped by the lexer
    /// to a hand-written match function (Task 2.3).
    pub pattern: &'static str,
}

/// A grammar — the complete description of a language syntax.
///
/// Composed of productions (nonterminals) and a set of
/// token rules (terminals). The start production is referenced via
/// [`Grammar::start`].
#[derive(Debug, Clone, Copy)]
pub struct Grammar {
    /// Human-readable name (e.g. `"IDL 4.2"`).
    pub name: &'static str,
    /// IDL version the grammar is oriented to.
    pub version: IdlVersion,
    /// The production set. Index `i` corresponds to `ProductionId(i as u32)`.
    pub productions: &'static [Production],
    /// The start production — typically `<specification>` for IDL.
    pub start: ProductionId,
    /// Token rules for the lexer.
    pub token_rules: &'static [TokenRule],
}

/// Abstraction over [`Grammar`] and [`compile::CompiledGrammar`] —
/// a uniform lookup trait for the recognizer.
pub trait GrammarLike {
    /// Looks up a production by its ID.
    fn production(&self, id: ProductionId) -> Option<&Production>;
    /// Start production ID.
    fn start(&self) -> ProductionId;
    /// Slice over all productions (in ID order).
    fn productions_slice(&self) -> &[Production];
}

impl GrammarLike for Grammar {
    fn production(&self, id: ProductionId) -> Option<&Production> {
        // Productions are not guaranteed to be in ID order in the slice
        // (the insertion order in IDL_42.productions can deviate from the
        // numeric ID order, e.g. ID 100 is inserted after
        // ID 116). Linear scan for `id`.
        self.productions.iter().find(|p| p.id == id)
    }
    fn start(&self) -> ProductionId {
        self.start
    }
    fn productions_slice(&self) -> &[Production] {
        self.productions
    }
}

impl Grammar {
    /// Looks up a production by its ID.
    ///
    /// Returns `None` if the ID does not exist.
    #[must_use]
    pub fn production(&self, id: ProductionId) -> Option<&Production> {
        self.productions.iter().find(|p| p.id == id)
    }

    /// Returns the start production.
    ///
    /// # Errors
    /// Returns `None` if `self.start` refers to a non-existent
    /// production — in that case there is a grammar construction error,
    /// which is detected by [`crate::grammar::validate`] (Task 1.2).
    #[must_use]
    pub fn start_production(&self) -> Option<&Production> {
        self.production(self.start)
    }

    /// Number of productions.
    #[inline]
    #[must_use]
    pub fn production_count(&self) -> usize {
        self.productions.len()
    }

    /// Iterates over all productions.
    pub fn productions_iter(&self) -> impl Iterator<Item = &Production> {
        self.productions.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal grammar for tests: a single nonterminal branch that
    /// accepts two terminals (`module <Ident>`). Not a complete
    /// IDL grammar, just test data.
    const PROD_DUMMY_MODULE: Production = Production {
        id: ProductionId(0),
        name: "dummy_module",
        spec_ref: SpecRef {
            doc: "TEST",
            section: "0.0",
        },
        alternatives: &[Alternative {
            name: None,
            symbols: &[
                Symbol::Terminal(TokenKind::Keyword("module")),
                Symbol::Terminal(TokenKind::Ident),
            ],
            note: None,
        }],
        ast_hint: None,
    };

    const DUMMY_GRAMMAR: Grammar = Grammar {
        name: "dummy",
        version: IdlVersion::V4_2,
        productions: &[PROD_DUMMY_MODULE],
        start: ProductionId(0),
        token_rules: &[],
    };

    #[test]
    fn default_idl_version_is_v4_2() {
        assert_eq!(IdlVersion::default(), IdlVersion::V4_2);
    }

    #[test]
    fn production_id_converts_to_usize() {
        assert_eq!(ProductionId(42).as_usize(), 42);
    }

    #[test]
    fn spec_ref_displays_with_paragraph_sign() {
        let sref = SpecRef {
            doc: "OMG IDL 4.2",
            section: "7.4.1.4.4.2",
        };
        assert_eq!(format!("{sref}"), "OMG IDL 4.2 §7.4.1.4.4.2");
    }

    #[test]
    fn symbol_classifies_terminals_and_nonterminals() {
        let term = Symbol::Terminal(TokenKind::Ident);
        let nonterm = Symbol::Nonterminal(ProductionId(0));
        let rep = Symbol::Repeat(RepeatKind::ZeroOrMore, &[]);

        assert!(term.is_terminal());
        assert!(!term.is_nonterminal());

        assert!(nonterm.is_nonterminal());
        assert!(!nonterm.is_terminal());

        assert!(!rep.is_terminal());
        assert!(!rep.is_nonterminal());
    }

    #[test]
    fn grammar_looks_up_production_by_id() {
        let prod = DUMMY_GRAMMAR.production(ProductionId(0));
        assert!(prod.is_some());
        assert_eq!(prod.map(|p| p.name), Some("dummy_module"));
    }

    #[test]
    fn grammar_returns_none_for_out_of_range_production() {
        assert!(DUMMY_GRAMMAR.production(ProductionId(99)).is_none());
    }

    #[test]
    fn grammar_resolves_start_production() {
        let start = DUMMY_GRAMMAR.start_production();
        assert!(start.is_some());
        assert_eq!(start.map(|p| p.name), Some("dummy_module"));
    }

    #[test]
    fn grammar_with_invalid_start_returns_none() {
        const BROKEN: Grammar = Grammar {
            name: "broken",
            version: IdlVersion::V4_2,
            productions: &[],
            start: ProductionId(0),
            token_rules: &[],
        };
        assert!(BROKEN.start_production().is_none());
    }

    #[test]
    fn grammar_counts_and_iterates_productions() {
        assert_eq!(DUMMY_GRAMMAR.production_count(), 1);
        let names: Vec<&str> = DUMMY_GRAMMAR.productions_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["dummy_module"]);
    }
}
