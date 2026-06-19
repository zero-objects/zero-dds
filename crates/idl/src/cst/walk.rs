// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Tree-traversal helpers for the CST.
//!
//! All helpers collect `Vec<&CstNode>` refs (eager). Lazy iterators
//! would be more idiomatic, but require stack state and cost clarity.
//! For IDL-typical tree sizes (a few hundred to a few thousand
//! nodes) the eager collection is irrelevant in practice.
//!
//! API families:
//!
//! - **Order traversal**: [`preorder`], [`postorder`]
//! - **Type filter**: [`tokens_only`], [`internals_only`]
//! - **Search helpers**: [`find_by_production`], [`find_first_by_production`],
//!   [`find_by_token_kind`], [`count_by_production`]
//! - **Structure metric**: [`depth`]
//!
//! See RFC 0001 §5.4.

use crate::grammar::{ProductionId, TokenKind};

use super::node::CstNode;

/// Pre-order Depth-First-Traversal: root → children left-to-right.
#[must_use]
pub fn preorder<'a, 'src>(root: &'a CstNode<'src>) -> Vec<&'a CstNode<'src>> {
    let mut out = Vec::new();
    collect_preorder(root, &mut out);
    out
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_preorder<'a, 'src>(node: &'a CstNode<'src>, out: &mut Vec<&'a CstNode<'src>>) {
    out.push(node);
    for child in &node.children {
        collect_preorder(child, out);
    }
}

/// Post-order Depth-First-Traversal: children left-to-right → root.
#[must_use]
pub fn postorder<'a, 'src>(root: &'a CstNode<'src>) -> Vec<&'a CstNode<'src>> {
    let mut out = Vec::new();
    collect_postorder(root, &mut out);
    out
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_postorder<'a, 'src>(node: &'a CstNode<'src>, out: &mut Vec<&'a CstNode<'src>>) {
    for child in &node.children {
        collect_postorder(child, out);
    }
    out.push(node);
}

/// Pre-order, filtered to token leaves.
#[must_use]
pub fn tokens_only<'a, 'src>(root: &'a CstNode<'src>) -> Vec<&'a CstNode<'src>> {
    preorder(root)
        .into_iter()
        .filter(|n| n.is_token())
        .collect()
}

/// Pre-order, filtered to internal nodes.
#[must_use]
pub fn internals_only<'a, 'src>(root: &'a CstNode<'src>) -> Vec<&'a CstNode<'src>> {
    preorder(root)
        .into_iter()
        .filter(|n| n.is_internal())
        .collect()
}

/// All internal nodes with the given production ID, in pre-order.
#[must_use]
pub fn find_by_production<'a, 'src>(
    root: &'a CstNode<'src>,
    production: ProductionId,
) -> Vec<&'a CstNode<'src>> {
    preorder(root)
        .into_iter()
        .filter(|n| n.production() == Some(production))
        .collect()
}

/// First internal node with the given production ID, in pre-order.
#[must_use]
pub fn find_first_by_production<'a, 'src>(
    root: &'a CstNode<'src>,
    production: ProductionId,
) -> Option<&'a CstNode<'src>> {
    preorder(root)
        .into_iter()
        .find(|n| n.production() == Some(production))
}

/// All token leaves with the given TokenKind, in pre-order.
#[must_use]
pub fn find_by_token_kind<'a, 'src>(
    root: &'a CstNode<'src>,
    kind: TokenKind,
) -> Vec<&'a CstNode<'src>> {
    preorder(root)
        .into_iter()
        .filter(|n| n.token_kind() == Some(kind))
        .collect()
}

/// Number of internal nodes with the given production ID.
#[must_use]
pub fn count_by_production(root: &CstNode<'_>, production: ProductionId) -> usize {
    find_by_production(root, production).len()
}

/// Maximum depth of the subtree. A single node has depth 0,
/// root + single child has depth 1, etc.
#[must_use]
pub fn depth(node: &CstNode<'_>) -> usize {
    if node.children.is_empty() {
        return 0;
    }
    1 + node.children.iter().map(depth).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::cst::CstNode;
    use crate::cst::build::build_cst;
    use crate::engine::Recognizer;
    use crate::errors::Span;
    use crate::grammar::toy::TOY;
    use crate::grammar::{ProductionId, TokenKind};
    use crate::lexer::Token;

    fn t(kind: TokenKind) -> Token<'static> {
        Token::synthetic(kind)
    }

    fn make_toy_cst<'src>(tokens: &[Token<'src>]) -> CstNode<'src> {
        let result = Recognizer::new(&TOY).recognize(tokens);
        build_cst(&TOY, tokens, &result).expect("must build")
    }

    // -----------------------------------------------------------------
    // Reihenfolgen
    // -----------------------------------------------------------------

    #[test]
    fn preorder_visits_root_first_then_children() {
        let mut root = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        root.push_child(CstNode::token(Token::synthetic(TokenKind::Keyword("a"))));
        let mut sub = CstNode::internal(ProductionId(1), 0, Span::SYNTHETIC);
        sub.push_child(CstNode::token(Token::synthetic(TokenKind::Keyword("b"))));
        root.push_child(sub);

        let order: Vec<_> = preorder(&root)
            .iter()
            .map(|n| {
                if let Some(p) = n.production() {
                    format!("I({})", p.0)
                } else if let Some(k) = n.token_kind() {
                    format!("T({k:?})")
                } else {
                    "?".to_string()
                }
            })
            .collect();
        assert_eq!(order[0], "I(0)");
        assert_eq!(order[1], "T(Keyword(\"a\"))");
        assert_eq!(order[2], "I(1)");
        assert_eq!(order[3], "T(Keyword(\"b\"))");
    }

    #[test]
    fn postorder_visits_children_first_then_root() {
        let mut root = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        let mut sub = CstNode::internal(ProductionId(1), 0, Span::SYNTHETIC);
        sub.push_child(CstNode::token(Token::synthetic(TokenKind::Keyword("a"))));
        root.push_child(sub);
        root.push_child(CstNode::token(Token::synthetic(TokenKind::Keyword("b"))));

        let order: Vec<_> = postorder(&root)
            .iter()
            .map(|n| {
                if let Some(p) = n.production() {
                    format!("I({})", p.0)
                } else if let Some(k) = n.token_kind() {
                    format!("T({k:?})")
                } else {
                    "?".to_string()
                }
            })
            .collect();
        // Expected: T(a), I(1), T(b), I(0)
        assert_eq!(
            order,
            vec![
                "T(Keyword(\"a\"))".to_string(),
                "I(1)".to_string(),
                "T(Keyword(\"b\"))".to_string(),
                "I(0)".to_string(),
            ]
        );
    }

    // -----------------------------------------------------------------
    // Typ-Filter
    // -----------------------------------------------------------------

    #[test]
    fn tokens_only_returns_only_leaves() {
        let cst = make_toy_cst(&[
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
        ]);
        let toks = tokens_only(&cst);
        assert_eq!(toks.len(), 3);
        assert!(toks.iter().all(|n| n.is_token()));
    }

    #[test]
    fn internals_only_returns_only_internals() {
        let cst = make_toy_cst(&[t(TokenKind::Keyword("n"))]);
        let internals = internals_only(&cst);
        // E → T → F, all three are internal.
        assert_eq!(internals.len(), 3);
        assert!(internals.iter().all(|n| n.is_internal()));
    }

    // -----------------------------------------------------------------
    // Production-Such-Helper
    // -----------------------------------------------------------------

    #[test]
    fn find_by_production_collects_all_matches_in_preorder() {
        // n + n + n  → 3 E nodes (top, sub, sub-sub) in the tree.
        let cst = make_toy_cst(&[
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
        ]);
        let es = find_by_production(&cst, ProductionId(0));
        assert_eq!(
            es.len(),
            3,
            "Expected 3 E nodes in n+n+n, found {}",
            es.len()
        );
    }

    #[test]
    fn find_first_by_production_returns_root_if_match() {
        let cst = make_toy_cst(&[t(TokenKind::Keyword("n"))]);
        let first_e = find_first_by_production(&cst, ProductionId(0));
        // The first E is the root itself.
        assert!(first_e.map(|n| std::ptr::eq(n, &cst)).unwrap_or(false));
    }

    #[test]
    fn find_first_by_production_none_for_missing() {
        let cst = make_toy_cst(&[t(TokenKind::Keyword("n"))]);
        // ProductionId(99) does not exist in TOY.
        assert!(find_first_by_production(&cst, ProductionId(99)).is_none());
    }

    #[test]
    fn count_by_production_matches_find_length() {
        let cst = make_toy_cst(&[
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("*")),
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("*")),
            t(TokenKind::Keyword("n")),
        ]);
        let t_nodes = find_by_production(&cst, ProductionId(1));
        assert_eq!(count_by_production(&cst, ProductionId(1)), t_nodes.len());
    }

    // -----------------------------------------------------------------
    // Token-Such-Helper
    // -----------------------------------------------------------------

    #[test]
    fn find_by_token_kind_collects_matching_tokens() {
        let cst = make_toy_cst(&[
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
            t(TokenKind::Punct("+")),
            t(TokenKind::Keyword("n")),
        ]);
        let pluses = find_by_token_kind(&cst, TokenKind::Punct("+"));
        assert_eq!(pluses.len(), 2);
        let ns = find_by_token_kind(&cst, TokenKind::Keyword("n"));
        assert_eq!(ns.len(), 3);
    }

    // -----------------------------------------------------------------
    // Depth
    // -----------------------------------------------------------------

    #[test]
    fn depth_of_single_token_is_zero() {
        let leaf = CstNode::token(Token::synthetic(TokenKind::Keyword("x")));
        assert_eq!(depth(&leaf), 0);
    }

    #[test]
    fn depth_of_root_with_single_child_is_one() {
        let mut root = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        root.push_child(CstNode::token(Token::synthetic(TokenKind::Keyword("x"))));
        assert_eq!(depth(&root), 1);
    }

    #[test]
    fn depth_of_toy_single_n_is_three() {
        // E → T → F → "n"  ⇒ depth 3
        let cst = make_toy_cst(&[t(TokenKind::Keyword("n"))]);
        assert_eq!(depth(&cst), 3);
    }

    #[test]
    fn depth_takes_max_of_children() {
        // Asymmetric tree: one long and one short branch.
        let mut root = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        // Short branch: one token
        root.push_child(CstNode::token(Token::synthetic(TokenKind::Keyword("a"))));
        // Long branch: three internal levels
        let mut deep = CstNode::internal(ProductionId(1), 0, Span::SYNTHETIC);
        let mut deeper = CstNode::internal(ProductionId(2), 0, Span::SYNTHETIC);
        deeper.push_child(CstNode::token(Token::synthetic(TokenKind::Keyword("b"))));
        deep.push_child(deeper);
        root.push_child(deep);
        // Root + Internal + Internal + Token ⇒ depth 3.
        assert_eq!(depth(&root), 3);
    }
}
