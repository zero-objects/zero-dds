// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CST construction from recognizer output.
//!
//! From the Earley state sets of a successful [`recognize` run](crate::engine::Recognizer::recognize)
//! the concrete syntax tree is reconstructed via a **backtracking search**:
//!
//! 1. In the final state set `Sₙ`, find a completed item that matches the
//!    start production (origin 0, dot at the end).
//! 2. Top-down tree construction through the alternative's symbols:
//!    - **Terminal**: the matching token is inserted as a leaf.
//!    - **Nonterminal `B`**: all completed `B` items findable in the
//!      state sets with `origin == current position`
//!      are tried as candidates. The first one that recursively
//!      builds **and** whose remaining symbols lead to the target endpoint
//!      wins.
//!
//! Greedy longest-match is deliberately **not** used: with
//! left-recursive grammars (E ::= E "+" T) the greedy path would pull the
//! sub-E over the entire remaining input and leave nothing for the
//! remaining symbols `"+" T`. Backtracking tries smaller
//! sub-spans and finds the correct left associativity.
//!
//! Complexity without memoization: worst-case exponential if the
//! grammar has many nullable nonterminals (each of which multiplies the
//! position candidates for the next symbol match). For the
//! IDL-4.2 grammar with `<annotation_appl_seq>` hooks everywhere this is
//! relevant.
//!
//! With the memoization from T6.0 (`(production, alternative, start, end)
//! -> Option<CstNode>`) the reconstruction runs in O(productions ×
//! alternatives × n²) — polynomial. The cache lives only for one
//! `build_cst` call and is discarded on return.
//!
//! Repeat and choice symbols are not handled here — analogous to the
//! .  A desugaring pass follows in a later
//! task.
//!
//! See RFC 0001 §4.1 and §5.4.

use std::collections::HashMap;

use crate::engine::{EarleyItem, RecognitionResult, StateSet};
use crate::errors::Span;
use crate::grammar::{GrammarLike, ProductionId, Symbol};
use crate::lexer::Token;

use super::node::CstNode;

/// Cache key for [`Memo`]: `(production_id, alternative_index, start, end)`.
type MemoKey = (ProductionId, u32, u32, u32);

/// Memoization table for `build_internal_node`. Prevents
/// exponential re-exploration on nullable-rich grammars (T6.0).
/// The `Option` value reflects "no valid build" — failures are
/// cached just like successes.
type Memo<'src> = HashMap<MemoKey, Option<CstNode<'src>>>;

/// Builds the CST from a recognizer result and the associated
/// token sequence.
///
/// Returns `None` if the recognition result was rejected
/// (`!result.accepted`) or if the reconstruction fails
/// (e.g. because of grammar constructs the current engine does not
/// handle — repeat/choice).
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

/// Searches the final state set for a completed item of the start production
/// with origin 0.
fn find_accepting_item<G: GrammarLike + ?Sized>(set: &StateSet, grammar: &G) -> Option<EarleyItem> {
    set.items()
        .iter()
        .copied()
        .find(|it| it.production == grammar.start() && it.origin == 0 && it.is_complete(grammar))
}

/// Builds an internal node for a specific Earley item that covers the
/// tokens `[start..end]`.
///
/// With memoization: on a cache hit, the stored result
/// (success or failure) is returned without a fresh backtracking
/// search. Cache key: `(production_id, alternative_index,
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

/// Tries to match the `symbols` sequence starting at token position `start`,
/// so that consumption ends at position `target_end`.
///
/// On success the children nodes are appended into `accumulated` and the
/// return value is `Some(target_end)`.
#[allow(clippy::too_many_arguments)] // Builder recursion with memo state.
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
            // Try all completed B items with origin = start
            // over all possible endpoints k in [start, target_end].
            // Greedy-longest-first would fail on left recursion;
            // we go shortest-first, so that left-recursive E sub-trees
            // do not swallow the entire remaining input.
            //
            // k == start is needed for nullable nonterminals (e.g.
            // empty `<annotation_appl_seq>`, `<definition_list>`,
            // `<member_list>`). In that case Earley returns a
            // complete item with origin == end == start, and the
            // empty alternative of the production matches via
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
            // Repeat/choice are not handled directly.
            // The reconstruction fails here; a
            // desugaring-pass extension follows in a later task.
            None
        }
    }
}

/// Computes the span over the tokens `[start..end]`.
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
    // Acceptance paths on the toy grammar
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
        // n + n → E (alt 0 = plus) with children [E, "+", T]
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
        // E (plus) has 3 children: E, +, T
        assert_eq!(cst.children.len(), 3);
        assert_eq!(cst.children[0].production(), Some(ProductionId(0))); // sub-E
        assert_eq!(cst.children[1].token_kind(), Some(TokenKind::Punct("+")));
        assert_eq!(cst.children[2].production(), Some(ProductionId(1))); // T
    }

    #[test]
    fn cst_for_multiplication_uses_times_alternative() {
        // n * n → E (just_term) → T (times) with children [T, "*", F]
        let cst = parse_to_cst(
            &TOY,
            &[
                t(TokenKind::Keyword("n")),
                t(TokenKind::Punct("*")),
                t(TokenKind::Keyword("n")),
            ],
        )
        .expect("must build");
        // Top-level E should use the just_term alt (alt_index 1).
        let CstKind::Internal {
            alternative_index, ..
        } = cst.kind
        else {
            panic!("expected Internal");
        };
        assert_eq!(alternative_index, 1, "E should use the just_term alt");
        // T should use the times alt.
        let t_node = &cst.children[0];
        let CstKind::Internal {
            alternative_index: t_alt,
            ..
        } = t_node.kind
        else {
            panic!("expected Internal");
        };
        assert_eq!(t_alt, 0, "T should use the times alt");
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
        assert_eq!(alternative_index, 0, "top E must be the plus alt");

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
            "sub-E must also be the plus alt (left associativity)"
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
        // Descend to the F node.
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
        assert_eq!(alternative_index, 1, "F must be the paren alt");
        assert_eq!(f_node.children.len(), 3);
        assert_eq!(f_node.children[0].token_kind(), Some(TokenKind::Punct("(")));
        assert_eq!(f_node.children[2].token_kind(), Some(TokenKind::Punct(")")));
    }

    // -----------------------------------------------------------------
    // Reject paths
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
        // Build tokens with real (instead of synthetic) spans.
        let tokens = vec![
            Token::new(TokenKind::Keyword("n"), Span::new(0, 1), "n"),
            Token::new(TokenKind::Punct("+"), Span::new(2, 3), "+"),
            Token::new(TokenKind::Keyword("n"), Span::new(4, 5), "n"),
        ];
        let result = Recognizer::new(&TOY).recognize(&tokens);
        let cst = build_cst(&TOY, &tokens, &result).expect("must build");

        // Collect all token-leaf spans via pre-order.
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
        // Top E should cover span 0..5.
        assert_eq!(cst.span, Span::new(0, 5));
    }

    // -----------------------------------------------------------------
    // E2E via the tokenizer (real lexer output → CST)
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
