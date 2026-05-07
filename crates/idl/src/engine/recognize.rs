// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Earley-Recognition: Scan, Predict, Complete.
//!
//! Diese Stufe entscheidet, **ob** eine Token-Sequenz einer Grammar
//! entspricht. Sie produziert die State-Set-Sequenz `S₀ … Sₙ`, aus der
//! eine spaetere Forest-Construction (Task 2.4) den Concrete-Syntax-Tree
//! ableitet. Akzeptanz entspricht: in `Sₙ` existiert ein abgeschlossenes
//! Item, dessen Production die Start-Production der Grammar ist und
//! dessen Origin `0` ist.
//!
//! Algorithmus (klassisch, Aycock/Horspool 2002):
//!
//! ```text
//! Initialisiere S₀ mit allen Items [Start → · α, 0] fuer jede Alternative
//! der Start-Production.
//!
//! fuer k = 0 .. n:
//!   wiederhole bis Sₖ fix:
//!     fuer jedes Item it in Sₖ:
//!       wenn it.is_complete:
//!         COMPLETE: fuer jedes Item w in S_{it.origin} mit
//!                   w.next_symbol == Nonterminal(it.production):
//!           fuege w.advance() in Sₖ ein
//!       sonst wenn it.next_symbol == Nonterminal(B):
//!         PREDICT: fuer jede Alternative von B:
//!           fuege [B → · γ, k] in Sₖ ein
//!       sonst wenn it.next_symbol == Terminal(t) und tokens[k] == t:
//!         (SCAN wird unten am Ende dieses k abgehandelt)
//!   SCAN: fuer jedes Item it in Sₖ mit next_symbol == Terminal(t) und
//!         tokens[k] == t: fuege it.advance() in Sₖ₊₁ ein.
//!
//! akzeptiert wenn Sₙ ein Item enthaelt mit:
//!   production == grammar.start, dot am Ende, origin == 0.
//! ```
//!
//! Repeat- und Choice-Symbole werden vom Engine-Recognizer **nicht direkt**
//! behandelt. Stattdessen erfolgt vor der Recognition ein Compile-Pass
//! ([`crate::grammar::compile`]), der EBNF-Konstrukte zu rekursiven
//! Hilfs-Productions desugart. Die [`crate::engine::Engine`]-Facade
//! ruft diesen Pass automatisch in [`crate::engine::Engine::new`] auf.
//! Direkt-Recognition auf einer rohen [`Grammar`] mit Repeat/Choice
//! ignoriert die Konstrukte und kann valides Input ablehnen — daher
//! immer ueber `Engine`/`parse` arbeiten.

use crate::grammar::{Grammar, GrammarLike, ProductionId, Symbol, TokenKind};
use crate::lexer::Token;

use super::state::{EarleyItem, StateSet};

/// Ergebnis eines Recognition-Laufs.
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// Die State-Sets `S₀ … Sₙ`. `state_sets[0]` ist der Init-Set,
    /// `state_sets[n]` der Final-Set nach Konsum aller Tokens.
    pub state_sets: Vec<StateSet>,
    /// `true`, wenn die Grammar die Token-Sequenz akzeptiert.
    pub accepted: bool,
}

/// Recognizer-Frontend.
///
/// `G` ist generisch ueber [`crate::grammar::GrammarLike`], sodass der
/// Recognizer sowohl auf rohen [`Grammar`]-Konstanten als auch auf
/// EBNF-desugarten [`crate::grammar::compile::CompiledGrammar`]-Werten
/// arbeitet.
#[derive(Debug, Clone, Copy)]
pub struct Recognizer<'g, G: GrammarLike + ?Sized = Grammar> {
    grammar: &'g G,
}

impl<'g, G: GrammarLike + ?Sized> Recognizer<'g, G> {
    /// Konstruiert einen Recognizer fuer die gegebene Grammar.
    #[must_use]
    pub const fn new(grammar: &'g G) -> Self {
        Self { grammar }
    }

    /// Fuehrt Earley-Recognition fuer eine Token-Sequenz aus.
    ///
    /// Die Engine baut `tokens.len() + 1` State-Sets auf. Pro Position `k`
    /// laeuft ein Fixpoint aus Predict + Complete; Scan vermittelt zwischen
    /// `Sₖ` und `Sₖ₊₁`. Spans der Tokens werden hier nicht direkt konsumiert,
    /// stehen aber den nachgelagerten Stufen (CST-Bau, AST-Bau,
    /// Diagnostiken) zur Verfuegung.
    #[must_use]
    pub fn recognize(&self, tokens: &[Token<'_>]) -> RecognitionResult {
        let mut state_sets: Vec<StateSet> = (0..=tokens.len()).map(|_| StateSet::new()).collect();

        // Init S₀ mit allen Alternativen der Start-Production.
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

    /// Wiederhole Predict + Complete auf Sₖ bis zum Fixpoint.
    fn close_set_inner(&self, state_sets: &mut [StateSet], k: usize) {
        // Index-basierte Schleife, weil `state_sets[k]` waehrend der
        // Iteration durch Predict/Complete waechst.
        let mut i = 0;
        while i < state_sets[k].items().len() {
            let item = state_sets[k].items()[i];
            if item.is_complete(self.grammar) {
                self.complete(state_sets, k, item);
            } else if let Some(symbol) = item.next_symbol(self.grammar) {
                match symbol {
                    Symbol::Nonterminal(b) => self.predict(state_sets, k, *b),
                    Symbol::Terminal(_) => {
                        // Scan-Kandidat — behandelt am Set-Ende durch scan().
                    }
                    Symbol::Repeat(_, _) | Symbol::Choice(_) => {
                        // Repeat/Choice werden via
                        // Desugaring-Pass spaeter zu reinem CFG umgeformt.
                        // Hier ignorieren — Recognition kann dadurch ein
                        // valides Input ablehnen, was in Tests vermieden wird.
                    }
                }
            }
            i += 1;
        }
    }

    /// PREDICT: fuer ein Item `[A → α · B β, j]` in Sₖ alle Alternativen von
    /// B als `[B → · γ, k]` in Sₖ einfuegen.
    fn predict(&self, state_sets: &mut [StateSet], k: usize, nonterminal: ProductionId) {
        // coverage: justified — Dangling-Production wird vom Validator
        // (grammar::validate::check_dangling_references) als Error gemeldet;
        // hier nur defensiver Fallback, in gueltigen Grammars unerreichbar.
        let Some(production) = self.grammar.production(nonterminal) else {
            return;
        };
        for (alt_idx, _) in production.alternatives.iter().enumerate() {
            let new_item = EarleyItem::new(nonterminal, alt_idx, k);
            state_sets[k].push(new_item);
        }
    }

    /// COMPLETE: fuer ein abgeschlossenes Item `[B → γ ·, j]` in Sₖ alle
    /// wartenden Items in Sⱼ vom Form `[A → α · B β, i]` advancen.
    fn complete(&self, state_sets: &mut [StateSet], k: usize, completed: EarleyItem) {
        let origin = completed.origin;
        // Snapshot der Items in S_origin, damit wir mut-borrow auf S_k
        // halten koennen ohne Konflikt.
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

    /// SCAN: liest Token `tokens[k]` und advanced alle Sₖ-Items, die auf
    /// dieses Terminal warten, in Sₖ₊₁.
    fn scan(&self, state_sets: &mut [StateSet], k: usize, token: TokenKind) {
        // Snapshot der zu advancenden Items; mut-borrow auf S_{k+1} kommt
        // anschliessend.
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

    /// Akzeptanz-Check: `Sₙ` enthaelt ein abgeschlossenes Item, dessen
    /// Production die Start-Production ist und dessen Origin `0` ist.
    fn is_accepted(&self, state_sets: &[StateSet], n: usize) -> bool {
        // coverage: justified — `recognize()` initialisiert state_sets mit
        // Laenge tokens.len()+1 und uebergibt n=tokens.len(), Index ist also
        // immer in-range. Defensiver Fallback fuer kuenftige API-Aenderungen.
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

    /// Test-Helper: erzeugt einen synthetischen Token aus einem TokenKind.
    /// Macht Recognizer-Tests unabhaengig von echtem Source-Text.
    fn t(kind: TokenKind) -> Token<'static> {
        Token::synthetic(kind)
    }

    /// Hilfs-Konstruktor: einzelne Production aus Index, Name, Alternative-Liste.
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
    // Test-Grammatiken (alle const, im Binary-Segment).
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
        // "n + n + n" — drei "n", zwei "+"
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
        // Erwartet "x" "y", input nur "x".
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
        // Bei G_ALTERNATIVES sollte S0 zwei Items enthalten —
        // beide Alternativen der Start-Production.
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
        // G_NESTED: A ::= B "y", B ::= "x". S0 sollte sowohl Items fuer A
        // (alt 0, dot 0) als auch fuer B (alt 0, dot 0) enthalten,
        // weil Predict ueber das Nonterminal B in A's RHS triggert.
        let r = Recognizer::new(&G_NESTED);
        let result = r.recognize(&[]);
        let items = result.state_sets[0].items();
        assert!(items.iter().any(|it| it.production == ProductionId(0)));
        assert!(items.iter().any(|it| it.production == ProductionId(1)));
    }

    #[test]
    fn complete_advances_parent_item() {
        // G_NESTED nach Konsum von "x": Item [B -> "x" ., 0] in S1
        // muss das wartende [A -> . B "y", 0] zu [A -> B . "y", 0] in S1
        // advancen.
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
        // Engine ignoriert Repeat/Choice — Alt 0 traegt also nicht zur
        // Recognition bei. Eingabe "y" akzeptiert via Alt 1.
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
        // Alt 1 traegt Recognition: "y" wird akzeptiert.
        assert!(r.recognize(&[t(TokenKind::Keyword("y"))]).accepted);
        // Alt 0 wird nicht behandelt: "x" wird nicht akzeptiert (waere mit
        // korrektem Repeat-Handling akzeptiert worden — .
        assert!(!r.recognize(&[t(TokenKind::Keyword("x"))]).accepted);
    }

    #[test]
    fn duplicate_predicts_do_not_explode_state_set() {
        // Regression: bei direkter Linksrekursion produziert Predict
        // wiederholt dasselbe Item — Dedup im StateSet muss greifen.
        let r = Recognizer::new(&G_LEFT_RECURSIVE);
        let result = r.recognize(&[t(TokenKind::Keyword("n"))]);
        // S0 sollte nur eine begrenzte Anzahl distinct items enthalten,
        // nicht endlos durch Re-Predict explodieren.
        assert!(result.state_sets[0].len() < 10);
    }
}
