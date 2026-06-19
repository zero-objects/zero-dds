// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Earley-Recognition: Scan, Predict, Complete.
//!
//! This stage decides **whether** a token sequence matches a grammar.
//! It produces the state-set sequence `S₀ … Sₙ`, from which
//! a later forest construction (Task 2.4) derives the concrete syntax tree.
//! Acceptance means: in `Sₙ` there exists a completed
//! item whose production is the start production of the grammar and
//! whose origin is `0`.
//!
//! Algorithm (classic, Aycock/Horspool 2002):
//!
//! ```text
//! Initialize S₀ with all items [Start → · α, 0] for each alternative
//! of the start production.
//!
//! for k = 0 .. n:
//!   repeat until Sₖ is fixed:
//!     for each item it in Sₖ:
//!       if it.is_complete:
//!         COMPLETE: for each item w in S_{it.origin} with
//!                   w.next_symbol == Nonterminal(it.production):
//!           insert w.advance() into Sₖ
//!       else if it.next_symbol == Nonterminal(B):
//!         PREDICT: for each alternative of B:
//!           insert [B → · γ, k] into Sₖ
//!       else if it.next_symbol == Terminal(t) and tokens[k] == t:
//!         (SCAN is handled below at the end of this k)
//!   SCAN: for each item it in Sₖ with next_symbol == Terminal(t) and
//!         tokens[k] == t: insert it.advance() into Sₖ₊₁.
//!
//! accepted if Sₙ contains an item with:
//!   production == grammar.start, dot at the end, origin == 0.
//! ```
//!
//! Repeat and choice symbols are **not handled directly** by the engine
//! recognizer. Instead, a compile pass runs before recognition
//! ([`crate::grammar::compile`]) that desugars EBNF constructs into recursive
//! helper productions. The [`crate::engine::Engine`] facade
//! calls this pass automatically in [`crate::engine::Engine::new`].
//! Direct recognition on a raw [`Grammar`] with repeat/choice
//! ignores the constructs and may reject valid input — therefore
//! always work via `Engine`/`parse`.

use crate::grammar::{Grammar, GrammarLike, ProductionId, Symbol, TokenKind};
use crate::lexer::Token;

use super::state::{EarleyItem, StateSet};

/// Result of a recognition run.
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// The state sets `S₀ … Sₙ`. `state_sets[0]` is the init set,
    /// `state_sets[n]` the final set after consuming all tokens.
    pub state_sets: Vec<StateSet>,
    /// `true` if the grammar accepts the token sequence.
    pub accepted: bool,
}

/// Recognizer frontend.
///
/// `G` is generic over [`crate::grammar::GrammarLike`], so that the
/// recognizer works both on raw [`Grammar`] constants and on
/// EBNF-desugared [`crate::grammar::compile::CompiledGrammar`] values.
#[derive(Debug, Clone, Copy)]
pub struct Recognizer<'g, G: GrammarLike + ?Sized = Grammar> {
    grammar: &'g G,
}

impl<'g, G: GrammarLike + ?Sized> Recognizer<'g, G> {
    /// Constructs a recognizer for the given grammar.
    #[must_use]
    pub const fn new(grammar: &'g G) -> Self {
        Self { grammar }
    }

    /// Runs Earley recognition for a token sequence.
    ///
    /// The engine builds `tokens.len() + 1` state sets. Per position `k`
    /// a fixpoint of predict + complete runs; scan mediates between
    /// `Sₖ` and `Sₖ₊₁`. The tokens' spans are not consumed directly here,
    /// but are available to the downstream stages (CST build, AST build,
    /// diagnostics).
    #[must_use]
    pub fn recognize(&self, tokens: &[Token<'_>]) -> RecognitionResult {
        let mut state_sets: Vec<StateSet> = (0..=tokens.len()).map(|_| StateSet::new()).collect();

        // Init S₀ with all alternatives of the start production.
        let start_id = self.grammar.start();
        if let Some(start) = self.grammar.production(start_id) {
            for (alt_idx, _) in start.alternatives.iter().enumerate() {
                state_sets[0].push(EarleyItem::new(start_id, alt_idx, 0));
            }
        }

        for k in 0..=tokens.len() {
            self.close_set_inner(&mut state_sets, k);
            if k < tokens.len() {
                self.scan(&mut state_sets, k, tokens[k].kind);
            }
        }

        let accepted = self.is_accepted(&state_sets, tokens.len());
        RecognitionResult {
            state_sets,
            accepted,
        }
    }

    /// Repeat predict + complete on Sₖ until the fixpoint.
    fn close_set_inner(&self, state_sets: &mut [StateSet], k: usize) {
        // Index-based loop, because `state_sets[k]` grows during the
        // iteration via predict/complete.
        let mut i = 0;
        while i < state_sets[k].items().len() {
            let item = state_sets[k].items()[i];
            if item.is_complete(self.grammar) {
                self.complete(state_sets, k, item);
            } else if let Some(symbol) = item.next_symbol(self.grammar) {
                match symbol {
                    Symbol::Nonterminal(b) => self.predict(state_sets, k, *b),
                    Symbol::Terminal(_) => {
                        // Scan candidate — handled at the set end by scan().
                    }
                    Symbol::Repeat(_, _) | Symbol::Choice(_) => {
                        // Repeat/choice are transformed into pure CFG later
                        // via the desugaring pass.
                        // Ignore here — recognition may thereby reject
                        // valid input, which is avoided in tests.
                    }
                }
            }
            i += 1;
        }
    }

    /// PREDICT: for an item `[A → α · B β, j]` in Sₖ, insert all alternatives of
    /// B as `[B → · γ, k]` into Sₖ.
    fn predict(&self, state_sets: &mut [StateSet], k: usize, nonterminal: ProductionId) {
        // coverage: justified — a dangling production is reported as an error by the validator
        // (grammar::validate::check_dangling_references);
        // here only a defensive fallback, unreachable in valid grammars.
        let Some(production) = self.grammar.production(nonterminal) else {
            return;
        };
        for (alt_idx, _) in production.alternatives.iter().enumerate() {
            let new_item = EarleyItem::new(nonterminal, alt_idx, k);
            state_sets[k].push(new_item);
        }
    }

    /// COMPLETE: for a completed item `[B → γ ·, j]` in Sₖ, advance all
    /// waiting items in Sⱼ of the form `[A → α · B β, i]`.
    fn complete(&self, state_sets: &mut [StateSet], k: usize, completed: EarleyItem) {
        let origin = completed.origin;
        // Snapshot of the items in S_origin, so we can hold a mut-borrow on S_k
        // without conflict.
        let waiting: Vec<EarleyItem> = state_sets[origin]
            .items()
            .iter()
            .copied()
            .filter(|it| {
                matches!(
                    it.next_symbol(self.grammar),
                    Some(Symbol::Nonterminal(b)) if *b == completed.production
                )
            })
            .collect();
        for it in waiting {
            state_sets[k].push(it.advance());
        }
    }

    /// SCAN: reads token `tokens[k]` and advances all Sₖ items waiting on
    /// this terminal into Sₖ₊₁.
    fn scan(&self, state_sets: &mut [StateSet], k: usize, token: TokenKind) {
        // Snapshot of the items to advance; the mut-borrow on S_{k+1} comes
        // afterwards.
        let advancing: Vec<EarleyItem> = state_sets[k]
            .items()
            .iter()
            .copied()
            .filter(|it| {
                matches!(
                    it.next_symbol(self.grammar),
                    Some(Symbol::Terminal(t)) if *t == token
                )
            })
            .collect();
        for it in advancing {
            state_sets[k + 1].push(it.advance());
        }
    }

    /// Acceptance check: `Sₙ` contains a completed item whose
    /// production is the start production and whose origin is `0`.
    fn is_accepted(&self, state_sets: &[StateSet], n: usize) -> bool {
        // coverage: justified — `recognize()` initializes state_sets with
        // length tokens.len()+1 and passes n=tokens.len(), so the index is
        // always in range. Defensive fallback for future API changes.
        let Some(final_set) = state_sets.get(n) else {
            return false;
        };
        final_set.items().iter().any(|it| {
            it.production == self.grammar.start() && it.origin == 0 && it.is_complete(self.grammar)
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::grammar::{Alternative, Grammar, IdlVersion, Production, SpecRef};

    const TS: SpecRef = SpecRef {
        doc: "TEST",
        section: "0.0",
    };

    /// Test helper: creates a synthetic token from a TokenKind.
    /// Makes recognizer tests independent of real source text.
    fn t(kind: TokenKind) -> Token<'static> {
        Token::synthetic(kind)
    }

    /// Helper constructor: a single production from index, name, alternative list.
    const fn prod(id: u32, name: &'static str, alts: &'static [Alternative]) -> Production {
        Production {
            id: ProductionId(id),
            name,
            spec_ref: TS,
            alternatives: alts,
            ast_hint: None,
        }
    }

    const fn alt(symbols: &'static [Symbol]) -> Alternative {
        Alternative {
            name: None,
            symbols,
            note: None,
        }
    }

    // -----------------------------------------------------------------
    // Test grammars (all const, in the binary segment).
    // -----------------------------------------------------------------

    /// `A ::= "x"`
    const G_SINGLE_TERMINAL: Grammar = Grammar {
        name: "single",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[alt(&[Symbol::Terminal(TokenKind::Keyword("x"))])],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    /// `A ::= "x" "y"`
    const G_SEQUENCE: Grammar = Grammar {
        name: "seq",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[alt(&[
                Symbol::Terminal(TokenKind::Keyword("x")),
                Symbol::Terminal(TokenKind::Keyword("y")),
            ])],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    /// `A ::= "x" | "y"`
    const G_ALTERNATIVES: Grammar = Grammar {
        name: "alts",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[
                alt(&[Symbol::Terminal(TokenKind::Keyword("x"))]),
                alt(&[Symbol::Terminal(TokenKind::Keyword("y"))]),
            ],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    /// `A ::= B "y"`, `B ::= "x"`
    const G_NESTED: Grammar = Grammar {
        name: "nested",
        version: IdlVersion::V4_2,
        productions: &[
            prod(
                0,
                "a",
                &[alt(&[
                    Symbol::Nonterminal(ProductionId(1)),
                    Symbol::Terminal(TokenKind::Keyword("y")),
                ])],
            ),
            prod(1, "b", &[alt(&[Symbol::Terminal(TokenKind::Keyword("x"))])]),
        ],
        start: ProductionId(0),
        token_rules: &[],
    };

    /// `A ::= A "+" "n" | "n"` — Linksrekursion.
    const G_LEFT_RECURSIVE: Grammar = Grammar {
        name: "left_rec",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[
                alt(&[
                    Symbol::Nonterminal(ProductionId(0)),
                    Symbol::Terminal(TokenKind::Punct("+")),
                    Symbol::Terminal(TokenKind::Keyword("n")),
                ]),
                alt(&[Symbol::Terminal(TokenKind::Keyword("n"))]),
            ],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    /// `A ::= "n" "+" A | "n"` — Rechtsrekursion.
    const G_RIGHT_RECURSIVE: Grammar = Grammar {
        name: "right_rec",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[
                alt(&[
                    Symbol::Terminal(TokenKind::Keyword("n")),
                    Symbol::Terminal(TokenKind::Punct("+")),
                    Symbol::Nonterminal(ProductionId(0)),
                ]),
                alt(&[Symbol::Terminal(TokenKind::Keyword("n"))]),
            ],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    /// `A ::= ε | "x"` — Epsilon-Alternative.
    const G_EPSILON: Grammar = Grammar {
        name: "epsilon",
        version: IdlVersion::V4_2,
        productions: &[prod(
            0,
            "a",
            &[
                alt(&[]), // epsilon
                alt(&[Symbol::Terminal(TokenKind::Keyword("x"))]),
            ],
        )],
        start: ProductionId(0),
        token_rules: &[],
    };

    // -----------------------------------------------------------------
    // Recognition-Tests.
    // -----------------------------------------------------------------

    #[test]
    fn recognize_single_terminal_input() {
        let r = Recognizer::new(&G_SINGLE_TERMINAL);
        let result = r.recognize(&[t(TokenKind::Keyword("x"))]);
        assert!(result.accepted);
        assert_eq!(result.state_sets.len(), 2);
    }

    #[test]
    fn recognize_two_terminals_in_sequence() {
        let r = Recognizer::new(&G_SEQUENCE);
        let result = r.recognize(&[t(TokenKind::Keyword("x")), t(TokenKind::Keyword("y"))]);
        assert!(result.accepted);
        assert_eq!(result.state_sets.len(), 3);
    }

    #[test]
    fn recognize_first_alternative() {
        let r = Recognizer::new(&G_ALTERNATIVES);
        assert!(r.recognize(&[t(TokenKind::Keyword("x"))]).accepted);
    }

    #[test]
    fn recognize_second_alternative() {
        let r = Recognizer::new(&G_ALTERNATIVES);
        assert!(r.recognize(&[t(TokenKind::Keyword("y"))]).accepted);
    }

    #[test]
    fn recognize_nonterminal_nesting() {
        let r = Recognizer::new(&G_NESTED);
        assert!(
            r.recognize(&[t(TokenKind::Keyword("x")), t(TokenKind::Keyword("y"))])
                .accepted
        );
    }

    #[test]
    fn recognize_left_recursive_grammar() {
        // "n + n + n" — three "n", two "+"
        let r = Recognizer::new(&G_LEFT_RECURSIVE);
        let tokens = [
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
        ];
        assert!(r.recognize(&tokens).accepted);
    }

    #[test]
    fn recognize_right_recursive_grammar() {
        let r = Recognizer::new(&G_RIGHT_RECURSIVE);
        let tokens = [
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
        ];
        assert!(r.recognize(&tokens).accepted);
    }

    #[test]
    fn recognize_epsilon_with_empty_input() {
        let r = Recognizer::new(&G_EPSILON);
        assert!(r.recognize(&[]).accepted);
    }

    #[test]
    fn recognize_epsilon_with_terminal_input() {
        let r = Recognizer::new(&G_EPSILON);
        assert!(r.recognize(&[t(TokenKind::Keyword("x"))]).accepted);
    }

    #[test]
    fn rejects_input_for_wrong_terminal() {
        let r = Recognizer::new(&G_SINGLE_TERMINAL);
        assert!(!r.recognize(&[t(TokenKind::Keyword("y"))]).accepted);
    }

    #[test]
    fn rejects_partial_input() {
        let r = Recognizer::new(&G_SEQUENCE);
        // Expects "x" "y", input only "x".
        assert!(!r.recognize(&[t(TokenKind::Keyword("x"))]).accepted);
    }

    #[test]
    fn rejects_extra_input_at_end() {
        let r = Recognizer::new(&G_SINGLE_TERMINAL);
        assert!(
            !r.recognize(&[t(TokenKind::Keyword("x")), t(TokenKind::Keyword("y"))])
                .accepted
        );
    }

    #[test]
    fn rejects_empty_input_when_grammar_requires_terminal() {
        let r = Recognizer::new(&G_SINGLE_TERMINAL);
        assert!(!r.recognize(&[]).accepted);
    }

    #[test]
    fn state_set_count_is_tokens_plus_one() {
        let r = Recognizer::new(&G_SEQUENCE);
        let result = r.recognize(&[
            t(TokenKind::Keyword("x")),
            t(TokenKind::Keyword("y")),
            t(TokenKind::Keyword("y")), // ueberschuessig
        ]);
        assert_eq!(result.state_sets.len(), 4);
        assert!(!result.accepted);
    }

    #[test]
    fn predict_populates_initial_set_with_alternatives() {
        // For G_ALTERNATIVES, S0 should contain two items —
        // both alternatives of the start production.
        let r = Recognizer::new(&G_ALTERNATIVES);
        let result = r.recognize(&[]);
        assert_eq!(result.state_sets[0].len(), 2);
        assert!(
            result.state_sets[0]
                .items()
                .iter()
                .all(|it| it.production == ProductionId(0) && it.origin == 0 && it.dot == 0)
        );
    }

    #[test]
    fn predict_descends_into_nonterminal() {
        // G_NESTED: A ::= B "y", B ::= "x". S0 should contain items for both A
        // (alt 0, dot 0) and B (alt 0, dot 0),
        // because predict triggers via the nonterminal B in A's RHS.
        let r = Recognizer::new(&G_NESTED);
        let result = r.recognize(&[]);
        let items = result.state_sets[0].items();
        assert!(items.iter().any(|it| it.production == ProductionId(0)));
        assert!(items.iter().any(|it| it.production == ProductionId(1)));
    }

    #[test]
    fn complete_advances_parent_item() {
        // G_NESTED after consuming "x": item [B -> "x" ., 0] in S1
        // must advance the waiting [A -> . B "y", 0] to [A -> B . "y", 0] in S1.
        let r = Recognizer::new(&G_NESTED);
        let result = r.recognize(&[t(TokenKind::Keyword("x"))]);
        let s1 = &result.state_sets[1];
        assert!(s1.items().iter().any(|it| it.production == ProductionId(0)
            && it.alternative_index == 0
            && it.dot == 1
            && it.origin == 0));
    }

    #[test]
    fn empty_grammar_accepts_only_empty_input() {
        const G_EMPTY_PROD: Grammar = Grammar {
            name: "empty_prod",
            version: IdlVersion::V4_2,
            productions: &[prod(0, "a", &[alt(&[])])],
            start: ProductionId(0),
            token_rules: &[],
        };
        let r = Recognizer::new(&G_EMPTY_PROD);
        assert!(r.recognize(&[]).accepted);
        assert!(!r.recognize(&[t(TokenKind::Keyword("x"))]).accepted);
    }

    #[test]
    fn repeat_and_choice_symbols_are_skipped_phase_zero() {
        // A ::= [ "x" ] | "y"   (Optional-Repeat in Alt 0, Terminal in Alt 1).
        // The engine ignores repeat/choice — so alt 0 does not contribute to
        // recognition. Input "y" accepted via alt 1.
        const G_REPEAT: Grammar = Grammar {
            name: "with_repeat",
            version: IdlVersion::V4_2,
            productions: &[prod(
                0,
                "a",
                &[
                    alt(&[Symbol::Repeat(
                        crate::grammar::RepeatKind::Optional,
                        &[Symbol::Terminal(TokenKind::Keyword("x"))],
                    )]),
                    alt(&[Symbol::Terminal(TokenKind::Keyword("y"))]),
                ],
            )],
            start: ProductionId(0),
            token_rules: &[],
        };
        let r = Recognizer::new(&G_REPEAT);
        // Alt 1 carries recognition: "y" is accepted.
        assert!(r.recognize(&[t(TokenKind::Keyword("y"))]).accepted);
        // Alt 0 is not handled: "x" is not accepted (would have been
        // accepted with correct repeat handling — .
        assert!(!r.recognize(&[t(TokenKind::Keyword("x"))]).accepted);
    }

    #[test]
    fn duplicate_predicts_do_not_explode_state_set() {
        // Regression: on direct left recursion, predict repeatedly
        // produces the same item — dedup in the StateSet must kick in.
        let r = Recognizer::new(&G_LEFT_RECURSIVE);
        let result = r.recognize(&[t(TokenKind::Keyword("n"))]);
        // S0 should contain only a limited number of distinct items,
        // not explode endlessly through re-predict.
        assert!(result.state_sets[0].len() < 10);
    }
}
