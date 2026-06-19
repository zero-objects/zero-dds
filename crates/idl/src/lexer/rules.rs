// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Token-rule extraction from a grammar.
//!
//! Instead of each grammar maintaining its lexer rules by hand, we derive them
//! automatically from the `Symbol::Terminal` occurrences. Advantage: no
//! duplication and no drift between grammar productions and lexer
//! configuration.
//!
//! The produced [`TokenRules`] container is sorted by **lexer priority**.
//! The actual tokenizer (Task 2.3) iterates over the rules
//! in this order and tries a longest match per source position.
//!
//! ## Priority ordering
//!
//! We split rules into two classes:
//!
//! - **Literals** ([`TokenKind::Keyword`], [`TokenKind::Punct`]) — exactly
//!   matching strings. Within this class: longer strings before
//!   shorter ones (`"::"` before `":"`, `"=="` before `"="`). This way
//!   multi-char operators win over their components.
//! - **Pattern-based** (identifiers, literals such as `IntegerLiteral`,
//!   `StringLiteral`, etc.) — hand-written match functions in the lexer.
//!   Order: identifiers after keywords (otherwise `struct` would be split
//!   into an ident), literals after punctuation.
//!
//! Within the same length, the sort is stable-alphabetical for
//! deterministic outputs.
//!
//! See RFC 0001 §5.2 (engine) and §4.1 (pipeline).

use crate::grammar::{Grammar, Symbol, TokenKind, TokenRule, TokenRuleId};

/// Container for extracted token rules, sorted by lexer priority.
#[derive(Debug, Clone, Default)]
pub struct TokenRules {
    rules: Vec<TokenRule>,
}

impl TokenRules {
    /// Constructs an empty container.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Extracts all token rules from the terminals of a grammar.
    ///
    /// Iterates over all productions and all symbol sequences
    /// (recursively through [`Symbol::Repeat`] and [`Symbol::Choice`]),
    /// collects unique [`TokenKind`]s and sorts them.
    #[must_use]
    pub fn from_grammar(grammar: &Grammar) -> Self {
        Self::from_productions(grammar.productions_iter())
    }

    /// Variant of [`from_grammar`] that works over any
    /// production sequence (e.g. a
    /// [`CompiledGrammar`](crate::grammar::compile::CompiledGrammar)
    /// after delta composition; T6.5).
    #[must_use]
    pub fn from_productions<'a, I>(productions: I) -> Self
    where
        I: IntoIterator<Item = &'a crate::grammar::Production>,
    {
        let mut kinds: Vec<TokenKind> = Vec::new();
        for production in productions {
            for alt in production.alternatives {
                collect_terminal_kinds(alt.symbols, &mut kinds);
            }
        }
        // Dedup while preserving order — comes before sorting,
        // so that the sort-stability behavior becomes deterministic.
        dedup_preserving_order(&mut kinds);
        sort_by_lexer_priority(&mut kinds);
        let rules = kinds
            .into_iter()
            .enumerate()
            .map(|(idx, kind)| TokenRule {
                id: TokenRuleId(idx as u32),
                kind,
                pattern: pattern_for(&kind),
            })
            .collect();
        Self { rules }
    }

    /// All rules in lexer-priority order.
    #[must_use]
    pub fn rules(&self) -> &[TokenRule] {
        &self.rules
    }

    /// Iterates over all rules in lexer-priority order.
    pub fn iter(&self) -> impl Iterator<Item = &TokenRule> {
        self.rules.iter()
    }

    /// Number of rules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// `true` if the container contains no rules.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Looks up a rule by its [`TokenKind`].
    #[must_use]
    pub fn by_kind(&self, kind: &TokenKind) -> Option<&TokenRule> {
        self.rules.iter().find(|r| r.kind == *kind)
    }

    /// All reserved keywords (keyword literals).
    pub fn keywords(&self) -> impl Iterator<Item = &str> {
        self.rules.iter().filter_map(|r| match r.kind {
            TokenKind::Keyword(s) => Some(s),
            _ => None,
        })
    }

    /// All punctuation literals.
    pub fn punctuation(&self) -> impl Iterator<Item = &str> {
        self.rules.iter().filter_map(|r| match r.kind {
            TokenKind::Punct(s) => Some(s),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------
// Helper functions (private, but test-accessible via super::)
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 64 (parser/AST walk; bounded by IDL nesting)
fn collect_terminal_kinds(symbols: &[Symbol], out: &mut Vec<TokenKind>) {
    for sym in symbols {
        match sym {
            Symbol::Terminal(kind) => out.push(*kind),
            Symbol::Repeat(_, inner) => collect_terminal_kinds(inner, out),
            Symbol::Choice(branches) => {
                for branch in *branches {
                    collect_terminal_kinds(branch, out);
                }
            }
            Symbol::Nonterminal(_) => {}
        }
    }
}

fn dedup_preserving_order(kinds: &mut Vec<TokenKind>) {
    let mut seen: std::collections::HashSet<TokenKind> = std::collections::HashSet::new();
    kinds.retain(|k| seen.insert(*k));
}

/// Lexer-priority class. Lower values have higher priority
/// (tried first by the tokenizer).
fn priority_class(kind: &TokenKind) -> u8 {
    match kind {
        // Literals: longer strings before shorter ones — achieved via the
        // secondary sort by -len() within the same class.
        TokenKind::Punct(_) => 0,
        TokenKind::Keyword(_) => 1,
        // Pattern-based: after literals.
        TokenKind::BoolLiteral => 2,
        TokenKind::WideCharLiteral => 3,
        TokenKind::CharLiteral => 4,
        TokenKind::WideStringLiteral => 5,
        TokenKind::StringLiteral => 6,
        TokenKind::FixedPtLiteral => 7,
        TokenKind::FloatLiteral => 8,
        TokenKind::IntegerLiteral => 9,
        TokenKind::Ident => 10,
        // Synthetic — not produced by the lexer, to the end.
        TokenKind::EndOfInput => 255,
    }
}

/// Length of a literal (for the inner sort of Punct/Keyword).
/// Pattern-based tokens have length 0, because they are not literal-based.
fn literal_len(kind: &TokenKind) -> usize {
    match kind {
        TokenKind::Punct(s) | TokenKind::Keyword(s) => s.len(),
        _ => 0,
    }
}

/// Sort key: (priority_class, -literal_len, kind tie-break).
/// `kind` as a tie-break ensures alphabetical stability at equal
/// length (e.g. "+" vs "-").
fn sort_by_lexer_priority(kinds: &mut [TokenKind]) {
    kinds.sort_by(|a, b| {
        let cls_a = priority_class(a);
        let cls_b = priority_class(b);
        cls_a
            .cmp(&cls_b)
            .then_with(|| literal_len(b).cmp(&literal_len(a))) // -len: longer first
            .then_with(|| a.cmp(b))
    });
}

/// Returns the pattern label for a token rule. For literal tokens
/// (Keyword/Punct) the literal string itself; for pattern-based tokens
/// a symbolic name that the tokenizer (Task 2.3) maps to a
/// hand-written match function.
fn pattern_for(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Keyword(s) | TokenKind::Punct(s) => s,
        TokenKind::Ident => "ident",
        TokenKind::IntegerLiteral => "int_literal",
        TokenKind::FloatLiteral => "float_literal",
        TokenKind::StringLiteral => "string_literal",
        TokenKind::CharLiteral => "char_literal",
        TokenKind::BoolLiteral => "bool_literal",
        TokenKind::WideCharLiteral => "wide_char_literal",
        TokenKind::WideStringLiteral => "wide_string_literal",
        TokenKind::FixedPtLiteral => "fixed_pt_literal",
        TokenKind::EndOfInput => "eof",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::{
        Alternative, Grammar, IdlVersion, Production, ProductionId, RepeatKind, SpecRef,
    };

    const TS: SpecRef = SpecRef {
        doc: "TEST",
        section: "0.0",
    };

    const fn alt(symbols: &'static [Symbol]) -> Alternative {
        Alternative {
            name: None,
            symbols,
            note: None,
        }
    }

    const fn prod(id: u32, name: &'static str, alts: &'static [Alternative]) -> Production {
        Production {
            id: ProductionId(id),
            name,
            spec_ref: TS,
            alternatives: alts,
            ast_hint: None,
        }
    }

    fn grammar(productions: &'static [Production]) -> Grammar {
        Grammar {
            name: "test",
            version: IdlVersion::V4_2,
            productions,
            start: ProductionId(0),
            token_rules: &[],
        }
    }

    // -----------------------------------------------------------------
    // Extraktion
    // -----------------------------------------------------------------

    #[test]
    fn from_empty_grammar_yields_no_rules() {
        const PRODS: &[Production] = &[prod(0, "a", &[alt(&[])])];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        assert!(rules.is_empty());
        assert_eq!(rules.len(), 0);
    }

    #[test]
    fn extracts_single_keyword() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[Symbol::Terminal(TokenKind::Keyword("struct"))])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        assert_eq!(rules.len(), 1);
        assert_eq!(rules.rules()[0].kind, TokenKind::Keyword("struct"));
        assert_eq!(rules.rules()[0].pattern, "struct");
    }

    #[test]
    fn deduplicates_repeated_terminal_across_productions() {
        const PRODS: &[Production] = &[
            prod(
                0,
                "a",
                &[alt(&[Symbol::Terminal(TokenKind::Keyword("module"))])],
            ),
            prod(
                1,
                "b",
                &[alt(&[Symbol::Terminal(TokenKind::Keyword("module"))])],
            ),
        ];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn descends_into_repeat_and_choice() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Repeat(
                    RepeatKind::ZeroOrMore,
                    &[Symbol::Terminal(TokenKind::Keyword("foo"))],
                ),
                Symbol::Choice(&[
                    &[Symbol::Terminal(TokenKind::Keyword("bar"))],
                    &[Symbol::Terminal(TokenKind::Punct(";"))],
                ]),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        assert_eq!(rules.len(), 3);
        let kinds: Vec<TokenKind> = rules.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&TokenKind::Keyword("foo")));
        assert!(kinds.contains(&TokenKind::Keyword("bar")));
        assert!(kinds.contains(&TokenKind::Punct(";")));
    }

    // -----------------------------------------------------------------
    // Prioritaets-Sortierung
    // -----------------------------------------------------------------

    #[test]
    fn punct_comes_before_keyword() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Keyword("if")),
                Symbol::Terminal(TokenKind::Punct(";")),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        let kinds: Vec<TokenKind> = rules.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec![TokenKind::Punct(";"), TokenKind::Keyword("if")]);
    }

    #[test]
    fn longer_punct_comes_before_shorter() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Punct(":")),
                Symbol::Terminal(TokenKind::Punct("::")),
                Symbol::Terminal(TokenKind::Punct("=")),
                Symbol::Terminal(TokenKind::Punct("==")),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        let punct: Vec<&str> = rules.punctuation().collect();
        // Longer first: "::" and "==" before ":" and "="
        assert_eq!(
            punct[0].len(),
            2,
            "first Punct must be 2 characters long, was: {punct:?}"
        );
        assert_eq!(punct[1].len(), 2);
        assert_eq!(punct[2].len(), 1);
        assert_eq!(punct[3].len(), 1);
    }

    #[test]
    fn keyword_comes_before_ident() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Ident),
                Symbol::Terminal(TokenKind::Keyword("struct")),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        let kinds: Vec<TokenKind> = rules.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec![TokenKind::Keyword("struct"), TokenKind::Ident]);
    }

    #[test]
    fn pattern_based_kinds_sorted_after_literals() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Ident),
                Symbol::Terminal(TokenKind::IntegerLiteral),
                Symbol::Terminal(TokenKind::Keyword("true")),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        let kinds: Vec<TokenKind> = rules.iter().map(|r| r.kind).collect();
        // Keyword (literal) before IntegerLiteral and Ident (pattern-based).
        assert_eq!(kinds[0], TokenKind::Keyword("true"));
        assert!(kinds.contains(&TokenKind::IntegerLiteral));
        assert!(kinds.contains(&TokenKind::Ident));
    }

    // -----------------------------------------------------------------
    // Helper-Accessoren
    // -----------------------------------------------------------------

    #[test]
    fn by_kind_finds_existing_rule() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[Symbol::Terminal(TokenKind::Punct("{"))])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        assert!(rules.by_kind(&TokenKind::Punct("{")).is_some());
        assert!(rules.by_kind(&TokenKind::Punct("}")).is_none());
    }

    #[test]
    fn keywords_iterator_yields_only_keyword_strings() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Keyword("struct")),
                Symbol::Terminal(TokenKind::Keyword("interface")),
                Symbol::Terminal(TokenKind::Punct(";")),
                Symbol::Terminal(TokenKind::Ident),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        let mut kws: Vec<&str> = rules.keywords().collect();
        kws.sort_unstable();
        assert_eq!(kws, vec!["interface", "struct"]);
    }

    #[test]
    fn pattern_for_known_kinds_is_stable() {
        // Pattern strings are part of the lexer-tokenizer interface (Task
        // 2.3); on change, adjust the tokenizer mapping.
        assert_eq!(pattern_for(&TokenKind::Ident), "ident");
        assert_eq!(pattern_for(&TokenKind::IntegerLiteral), "int_literal");
        assert_eq!(pattern_for(&TokenKind::FloatLiteral), "float_literal");
        assert_eq!(pattern_for(&TokenKind::StringLiteral), "string_literal");
        assert_eq!(pattern_for(&TokenKind::CharLiteral), "char_literal");
        assert_eq!(pattern_for(&TokenKind::BoolLiteral), "bool_literal");
        assert_eq!(
            pattern_for(&TokenKind::WideCharLiteral),
            "wide_char_literal"
        );
        assert_eq!(
            pattern_for(&TokenKind::WideStringLiteral),
            "wide_string_literal"
        );
        assert_eq!(pattern_for(&TokenKind::FixedPtLiteral), "fixed_pt_literal");
        assert_eq!(pattern_for(&TokenKind::EndOfInput), "eof");
        assert_eq!(pattern_for(&TokenKind::Punct("::")), "::");
        assert_eq!(pattern_for(&TokenKind::Keyword("struct")), "struct");
    }

    #[test]
    fn empty_constructor_yields_empty_container() {
        let rules = TokenRules::new();
        assert!(rules.is_empty());
        assert_eq!(rules.len(), 0);
        assert!(rules.rules().is_empty());
    }

    #[test]
    fn priority_class_distinguishes_all_pattern_kinds() {
        // Grammar with all literal classes — pattern_class and literal_len
        // are run for every variant, so that the sort logic
        // covers all match arms.
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::BoolLiteral),
                Symbol::Terminal(TokenKind::CharLiteral),
                Symbol::Terminal(TokenKind::StringLiteral),
                Symbol::Terminal(TokenKind::IntegerLiteral),
                Symbol::Terminal(TokenKind::FloatLiteral),
                Symbol::Terminal(TokenKind::WideCharLiteral),
                Symbol::Terminal(TokenKind::WideStringLiteral),
                Symbol::Terminal(TokenKind::FixedPtLiteral),
                Symbol::Terminal(TokenKind::EndOfInput),
                Symbol::Terminal(TokenKind::Ident),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        assert_eq!(rules.len(), 10);
        let kinds: Vec<TokenKind> = rules.iter().map(|r| r.kind).collect();
        // Order per priority_class:
        assert_eq!(kinds[0], TokenKind::BoolLiteral);
        assert_eq!(kinds[1], TokenKind::WideCharLiteral);
        assert_eq!(kinds[2], TokenKind::CharLiteral);
        assert_eq!(kinds[3], TokenKind::WideStringLiteral);
        assert_eq!(kinds[4], TokenKind::StringLiteral);
        assert_eq!(kinds[5], TokenKind::FixedPtLiteral);
        assert_eq!(kinds[6], TokenKind::FloatLiteral);
        assert_eq!(kinds[7], TokenKind::IntegerLiteral);
        assert_eq!(kinds[8], TokenKind::Ident);
        assert_eq!(kinds[9], TokenKind::EndOfInput);
    }

    // -----------------------------------------------------------------
    // E2E with the toy grammar
    // -----------------------------------------------------------------

    #[test]
    fn extracts_toy_grammar_rules_with_correct_count_and_priority() {
        use crate::grammar::toy::TOY;
        let rules = TokenRules::from_grammar(&TOY);
        // Toy-Grammar hat Terminals: "n" (Keyword), "+" "*" "(" ")" (Punct).
        assert_eq!(rules.len(), 5);
        let punct: Vec<&str> = rules.punctuation().collect();
        let kws: Vec<&str> = rules.keywords().collect();
        assert_eq!(punct.len(), 4);
        assert_eq!(kws, vec!["n"]);
        // Punct comes before Keyword (priority).
        let kinds: Vec<TokenKind> = rules.iter().map(|r| r.kind).collect();
        let first_keyword_pos = kinds
            .iter()
            .position(|k| matches!(k, TokenKind::Keyword(_)));
        let first_punct_pos = kinds.iter().position(|k| matches!(k, TokenKind::Punct(_)));
        assert!(first_punct_pos < first_keyword_pos);
    }

    #[test]
    fn token_rule_ids_are_assigned_sequentially() {
        const PRODS: &[Production] = &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Punct(";")),
                Symbol::Terminal(TokenKind::Keyword("if")),
            ])],
        )];
        let rules = TokenRules::from_grammar(&grammar(PRODS));
        assert_eq!(rules.rules()[0].id, TokenRuleId(0));
        assert_eq!(rules.rules()[1].id, TokenRuleId(1));
    }
}
