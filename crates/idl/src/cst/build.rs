// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CST-Konstruktion aus Recognizer-Output.
//!
//! Aus den Earley-State-Sets eines erfolgreichen [`recognize`-Laufs](crate::engine::Recognizer::recognize)
//! wird der Concrete Syntax Tree per **Backtracking-Search** rekonstruiert:
//!
//! 1. Im finalen State-Set `Sₙ` ein abgeschlossenes Item finden, das die
//!    Start-Production matched (Origin 0, Dot am Ende).
//! 2. Top-down Tree-Konstruktion durch die Alternative-Symbole:
//!    - **Terminal**: passender Token wird als Leaf eingefuegt.
//!    - **Nonterminal `B`**: alle in den State-Sets auffindbaren
//!      abgeschlossenen `B`-Items mit `origin == aktuelle Position`
//!      werden als Kandidaten ausprobiert. Der erste, der sich rekursiv
//!      bauen laesst **und** dessen Rest-Symbole zum Ziel-Endpunkt fuehren,
//!      gewinnt.
//!
//! Greedy-Longest-Match wird bewusst **nicht** verwendet: bei
//! linksrekursiven Grammars (E ::= E "+" T) wuerde der Greedy-Pfad das
//! Sub-E ueber das gesamte Rest-Input ziehen und nichts fuer die
//! verbleibenden Symbole `"+" T` lassen. Backtracking probiert kleinere
//! Sub-Spans und findet die korrekte Links-Assoziativitaet.
//!
//! Komplexitaet ohne Memoization: worst-case exponentiell wenn die
//! Grammar viele nullable Nonterminals hat (jedes davon multipliziert die
//! Position-Kandidaten fuer den naechsten Symbol-Match). Fuer
//! IDL-4.2-Grammar mit `<annotation_appl_seq>`-Hooks ueberall ist das
//! relevant.
//!
//! Mit der Memoization aus T6.0 (`(production, alternative, start, end)
//! -> Option<CstNode>`) laeuft die Reconstruction in O(productions ×
//! alternatives × n²) — polynomial. Cache lebt nur fuer einen
//! `build_cst`-Call und wird beim Return verworfen.
//!
//! Repeat- und Choice-Symbole werden hier nicht behandelt — analog zur
//! .  Desugaring-Pass folgt in einem spaeteren
//! Task.
//!
//! Siehe RFC 0001 §4.1 und §5.4.

use std::collections::HashMap;

use crate::engine::{EarleyItem, RecognitionResult, StateSet};
use crate::errors::Span;
use crate::grammar::{GrammarLike, ProductionId, Symbol};
use crate::lexer::Token;

use super::node::CstNode;

/// Cache-Key fuer [`Memo`]: `(production_id, alternative_index, start, end)`.
type MemoKey = (ProductionId, u32, u32, u32);

/// Memoization-Tabelle fuer `build_internal_node`. Verhindert
/// exponentielles Re-Exploration bei nullable-reichen Grammars (T6.0).
/// `Option`-Wert reflektiert "kein gueltiger Build" — Misserfolge werden
/// genauso gecacht wie Erfolge.
type Memo<'src> = HashMap<MemoKey, Option<CstNode<'src>>>;

/// Baut den CST aus einem Recognizer-Resultat und der zugehoerigen
/// Token-Sequenz.
///
/// Liefert `None`, wenn das Recognition-Ergebnis abgelehnt wurde
/// (`!result.accepted`) oder wenn die Reconstruction fehlschlaegt
/// (z.B. wegen Grammar-Konstrukten, die die aktuelle Engine nicht
/// behandelt — Repeat/Choice).
#[must_use]
pub fn build_cst<'src, G: GrammarLike + ?Sized>(
    grammar: &G,
    tokens: &[Token<'src>],
    result: &RecognitionResult,
) -> Option<CstNode<'src>> {
    if !result.accepted {
        return None;
    }
    let n = tokens.len();
    let final_set = result.state_sets.get(n)?;
    let final_item = find_accepting_item(final_set, grammar)?;
    let mut memo: Memo<'src> = HashMap::new();
    build_internal_node(
        grammar,
        tokens,
        &result.state_sets,
        final_item,
        0,
        n,
        &mut memo,
    )
}

/// Sucht im finalen State-Set ein abgeschlossenes Item der Start-Production
/// mit Origin 0.
fn find_accepting_item<G: GrammarLike + ?Sized>(set: &StateSet, grammar: &G) -> Option<EarleyItem> {
    set.items()
        .iter()
        .copied()
        .find(|it| it.production == grammar.start() && it.origin == 0 && it.is_complete(grammar))
}

/// Baut einen Internal-Node fuer ein bestimmtes Earley-Item, das die
/// Tokens `[start..end]` ueberdeckt.
///
/// Mit Memoization: bei Cache-Hit wird das gespeicherte Ergebnis
/// (Erfolg oder Misserfolg) ohne erneute Backtracking-Suche
/// zurueckgegeben. Cache-Key: `(production_id, alternative_index,
/// start, end)`.
fn build_internal_node<'src, G: GrammarLike + ?Sized>(
    grammar: &G,
    tokens: &[Token<'src>],
    state_sets: &[StateSet],
    item: EarleyItem,
    start: usize,
    end: usize,
    memo: &mut Memo<'src>,
) -> Option<CstNode<'src>> {
    let key: MemoKey = (
        item.production,
        item.alternative_index as u32,
        start as u32,
        end as u32,
    );
    if let Some(cached) = memo.get(&key) {
        return cached.clone();
    }

    let result = build_internal_node_uncached(grammar, tokens, state_sets, item, start, end, memo);
    memo.insert(key, result.clone());
    result
}

fn build_internal_node_uncached<'src, G: GrammarLike + ?Sized>(
    grammar: &G,
    tokens: &[Token<'src>],
    state_sets: &[StateSet],
    item: EarleyItem,
    start: usize,
    end: usize,
    memo: &mut Memo<'src>,
) -> Option<CstNode<'src>> {
    let production = grammar.production(item.production)?;
    let alternative = production.alternatives.get(item.alternative_index)?;

    let mut children: Vec<CstNode<'src>> = Vec::new();
    let final_pos = try_match_symbols(
        grammar,
        tokens,
        state_sets,
        alternative.symbols,
        start,
        end,
        &mut children,
        memo,
    )?;
    if final_pos != end {
        return None;
    }

    let span = compute_span(tokens, start, end);
    let mut node = CstNode::internal(item.production, item.alternative_index, span);
    node.children = children;
    Some(node)
}

/// Versucht, die `symbols`-Sequenz beginnend bei Token-Position `start` zu
/// matchen, sodass die Konsumation an Position `target_end` endet.
///
/// Bei Erfolg sind die Children-Nodes in `accumulated` angehaengt und der
/// Returnwert ist `Some(target_end)`.
#[allow(clippy::too_many_arguments)] // Builder-Recursion mit Memo-State.
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn try_match_symbols<'src, G: GrammarLike + ?Sized>(
    grammar: &G,
    tokens: &[Token<'src>],
    state_sets: &[StateSet],
    symbols: &[Symbol],
    start: usize,
    target_end: usize,
    accumulated: &mut Vec<CstNode<'src>>,
    memo: &mut Memo<'src>,
) -> Option<usize> {
    if symbols.is_empty() {
        return if start == target_end {
            Some(start)
        } else {
            None
        };
    }

    let head = &symbols[0];
    let tail = &symbols[1..];

    match head {
        Symbol::Terminal(expected_kind) => {
            let token = tokens.get(start)?;
            if token.kind != *expected_kind {
                return None;
            }
            accumulated.push(CstNode::token(*token));
            match try_match_symbols(
                grammar,
                tokens,
                state_sets,
                tail,
                start + 1,
                target_end,
                accumulated,
                memo,
            ) {
                Some(end) => Some(end),
                None => {
                    accumulated.pop();
                    None
                }
            }
        }
        Symbol::Nonterminal(nonterminal) => {
            // Probiere alle abgeschlossenen B-Items mit Origin = start
            // ueber alle moeglichen Endpunkte k in [start, target_end].
            // Greedy-longest-first wuerde bei Linksrekursion fehlschlagen;
            // wir gehen kuerzeste-zuerst, damit linksrekursive E-Sub-Trees
            // nicht das ganze Rest-Input schlucken.
            //
            // k == start ist noetig fuer nullable Nonterminals (z.B.
            // empty `<annotation_appl_seq>`, `<definition_list>`,
            // `<member_list>`). Earley liefert in dem Fall ein
            // complete-Item mit origin == end == start, und die
            // empty-Alternative der Production matcht via
            // `symbols.is_empty() && start == target_end`.
            for k in start..=target_end {
                if let Some(set) = state_sets.get(k) {
                    for candidate in set.items() {
                        if candidate.production == *nonterminal
                            && candidate.origin == start
                            && candidate.is_complete(grammar)
                        {
                            if let Some(child) = build_internal_node(
                                grammar, tokens, state_sets, *candidate, start, k, memo,
                            ) {
                                accumulated.push(child);
                                if let Some(end) = try_match_symbols(
                                    grammar,
                                    tokens,
                                    state_sets,
                                    tail,
                                    k,
                                    target_end,
                                    accumulated,
                                    memo,
                                ) {
                                    return Some(end);
                                }
                                accumulated.pop();
                            }
                        }
                    }
                }
            }
            None
        }
        Symbol::Repeat(_, _) | Symbol::Choice(_) => {
            // Repeat/Choice werden nicht direkt
            // gehandelt. Die Reconstruction schlaegt hier fehl; eine
            // Desugaring-Pass-Erweiterung folgt in einem spaeteren Task.
            None
        }
    }
}

/// Berechnet die Span ueber die Tokens `[start..end]`.
fn compute_span(tokens: &[Token<'_>], start: usize, end: usize) -> Span {
    if start >= end || tokens.is_empty() {
        return Span::SYNTHETIC;
    }
    let first = tokens.get(start).map(|t| t.span).unwrap_or(Span::SYNTHETIC);
    let last = tokens
        .get(end.saturating_sub(1))
        .map(|t| t.span)
        .unwrap_or(first);
    first.merge(last)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::cst::CstKind;
    use crate::engine::Recognizer;
    use crate::grammar::Grammar;
    use crate::grammar::ProductionId;
    use crate::grammar::TokenKind;
    use crate::grammar::toy::TOY;

    fn t(kind: TokenKind) -> Token<'static> {
        Token::synthetic(kind)
    }

    fn parse_to_cst<'src>(grammar: &Grammar, tokens: &[Token<'src>]) -> Option<CstNode<'src>> {
        let result = Recognizer::new(grammar).recognize(tokens);
        build_cst(grammar, tokens, &result)
    }

    // -----------------------------------------------------------------
    // Akzeptanz-Pfade auf Toy-Grammar
    // -----------------------------------------------------------------

    #[test]
    fn cst_for_single_n_is_three_levels_deep() {
        // n → E → T → F → "n"
        let cst = parse_to_cst(&TOY, &[t(TokenKind::Keyword("n"))]).expect("must build");
        assert!(cst.is_internal());
        assert_eq!(cst.production(), Some(ProductionId(0))); // E
        // E → T (just_term) → F (number) → Token("n")
        let mut depth = 0;
        let mut current = &cst;
        while let Some(child) = current.children.first() {
            depth += 1;
            current = child;
            if current.is_token() {
                break;
            }
        }
        assert!(depth >= 3, "Expected depth >=3 (E→T→F→Token), got {depth}");
        assert_eq!(current.token_kind(), Some(TokenKind::Keyword("n")));
    }

    #[test]
    fn cst_for_addition_has_e_plus_t_structure() {
        // n + n → E (alt 0 = plus) mit Children [E, "+", T]
        let cst = parse_to_cst(
            &TOY,
            &[
                t(TokenKind::Keyword("n")),
                t(TokenKind::Punct("+")),
                t(TokenKind::Keyword("n")),
            ],
        )
        .expect("must build");
        assert_eq!(cst.production(), Some(ProductionId(0))); // E
        // E (plus) hat 3 Children: E, +, T
        assert_eq!(cst.children.len(), 3);
        assert_eq!(cst.children[0].production(), Some(ProductionId(0))); // sub-E
        assert_eq!(cst.children[1].token_kind(), Some(TokenKind::Punct("+")));
        assert_eq!(cst.children[2].production(), Some(ProductionId(1))); // T
    }

    #[test]
    fn cst_for_multiplication_uses_times_alternative() {
        // n * n → E (just_term) → T (times) mit Children [T, "*", F]
        let cst = parse_to_cst(
            &TOY,
            &[
                t(TokenKind::Keyword("n")),
                t(TokenKind::Punct("*")),
                t(TokenKind::Keyword("n")),
            ],
        )
        .expect("must build");
        // Top-Level E sollte just_term-Alt nutzen (alt_index 1).
        let CstKind::Internal {
            alternative_index, ..
        } = cst.kind
        else {
            panic!("expected Internal");
        };
        assert_eq!(alternative_index, 1, "E sollte just_term-Alt nutzen");
        // T sollte times-Alt nutzen.
        let t_node = &cst.children[0];
        let CstKind::Internal {
            alternative_index: t_alt,
            ..
        } = t_node.kind
        else {
            panic!("expected Internal");
        };
        assert_eq!(t_alt, 0, "T sollte times-Alt nutzen");
        assert_eq!(t_node.children.len(), 3);
    }

    #[test]
    fn cst_left_associative_addition() {
        // n + n + n  ⇒  (n + n) + n
        // Top-E (plus): [Sub-E (plus), "+", T]
        // Sub-E (plus): [Sub-Sub-E (just_term), "+", T]
        let cst = parse_to_cst(
            &TOY,
            &[
                t(TokenKind::Keyword("n")),
                t(TokenKind::Punct("+")),
                t(TokenKind::Keyword("n")),
                t(TokenKind::Punct("+")),
                t(TokenKind::Keyword("n")),
            ],
        )
        .expect("must build");

        // Top-E: alt=plus
        let CstKind::Internal {
            alternative_index, ..
        } = cst.kind
        else {
            panic!("expected Internal");
        };
        assert_eq!(alternative_index, 0, "Top-E muss plus-Alt sein");

        // Sub-E (Top-E.children[0]): alt=plus (linksrekursiv)
        let sub_e = &cst.children[0];
        let CstKind::Internal {
            alternative_index: sub_alt,
            ..
        } = sub_e.kind
        else {
            panic!("expected Internal");
        };
        assert_eq!(
            sub_alt, 0,
            "Sub-E muss auch plus-Alt sein (Linksassoziativitaet)"
        );
    }

    #[test]
    fn cst_parens_wrap_inner_expression() {
        // (n) → E → T → F (paren) → ["(", E, ")"]
        let cst = parse_to_cst(
            &TOY,
            &[
                t(TokenKind::Punct("(")),
                t(TokenKind::Keyword("n")),
                t(TokenKind::Punct(")")),
            ],
        )
        .expect("must build");
        // Steige hinab zu F-Knoten.
        let f_node = cst
            .children
            .first()
            .and_then(|t| t.children.first())
            .expect("E→T→F path");
        assert_eq!(f_node.production(), Some(ProductionId(2))); // F
        let CstKind::Internal {
            alternative_index, ..
        } = f_node.kind
        else {
            panic!("expected Internal");
        };
        assert_eq!(alternative_index, 1, "F muss paren-Alt sein");
        assert_eq!(f_node.children.len(), 3);
        assert_eq!(f_node.children[0].token_kind(), Some(TokenKind::Punct("(")));
        assert_eq!(f_node.children[2].token_kind(), Some(TokenKind::Punct(")")));
    }

    // -----------------------------------------------------------------
    // Reject-Pfade
    // -----------------------------------------------------------------

    #[test]
    fn no_cst_for_rejected_input() {
        let cst = parse_to_cst(&TOY, &[t(TokenKind::Punct("+"))]);
        assert!(cst.is_none());
    }

    #[test]
    fn no_cst_for_empty_input_when_grammar_requires_terminal() {
        let cst = parse_to_cst(&TOY, &[]);
        assert!(cst.is_none());
    }

    // -----------------------------------------------------------------
    // Token-Span-Propagation
    // -----------------------------------------------------------------

    #[test]
    fn token_leaves_carry_their_lexer_span() {
        // Tokens mit echten (statt synthetischen) Spans bauen.
        let tokens = vec![
            Token::new(TokenKind::Keyword("n"), Span::new(0, 1), "n"),
            Token::new(TokenKind::Punct("+"), Span::new(2, 3), "+"),
            Token::new(TokenKind::Keyword("n"), Span::new(4, 5), "n"),
        ];
        let result = Recognizer::new(&TOY).recognize(&tokens);
        let cst = build_cst(&TOY, &tokens, &result).expect("must build");

        // Sammle alle Token-Leaf-Spans via Pre-order.
        let mut spans: Vec<Span> = Vec::new();
        cst.walk_preorder(&mut |n| {
            if n.is_token() {
                spans.push(n.span);
            }
        });
        assert_eq!(
            spans,
            vec![Span::new(0, 1), Span::new(2, 3), Span::new(4, 5)]
        );
    }

    #[test]
    fn internal_node_span_covers_its_token_range() {
        let tokens = vec![
            Token::new(TokenKind::Keyword("n"), Span::new(0, 1), "n"),
            Token::new(TokenKind::Punct("+"), Span::new(2, 3), "+"),
            Token::new(TokenKind::Keyword("n"), Span::new(4, 5), "n"),
        ];
        let result = Recognizer::new(&TOY).recognize(&tokens);
        let cst = build_cst(&TOY, &tokens, &result).expect("must build");
        // Top-E sollte Span 0..5 ueberdecken.
        assert_eq!(cst.span, Span::new(0, 5));
    }

    // -----------------------------------------------------------------
    // E2E ueber Tokenizer (echter Lexer-Output → CST)
    // -----------------------------------------------------------------

    #[test]
    fn end_to_end_lexer_to_cst_for_arithmetic_expression() {
        use crate::lexer::Tokenizer;

        let tokenizer = Tokenizer::for_grammar(&TOY);
        let stream = tokenizer
            .tokenize("n + n * (n + n)")
            .expect("tokenize must succeed");
        let result = Recognizer::new(&TOY).recognize(stream.tokens());
        assert!(result.accepted);

        let cst = build_cst(&TOY, stream.tokens(), &result).expect("build must succeed");
        assert_eq!(cst.production(), Some(ProductionId(0)));
        // Knotenanzahl > 10 (Top-E + sub-Es + sub-Ts + sub-Fs + Token-Leaves)
        assert!(cst.count_nodes() > 10);
    }
}
