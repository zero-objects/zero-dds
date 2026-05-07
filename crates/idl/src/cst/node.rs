// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Concrete-Syntax-Tree-Knoten — untyped Baum-Struktur.
//!
//! Der CST ist die unmittelbare Baum-Repraesentation eines Parse-Ergebnisses.
//! Er folgt 1:1 der Grammar-Production-Struktur: jeder Internal-Node
//! traegt eine [`ProductionId`] und einen Alternative-Index, jeder
//! Token-Leaf einen [`TokenKind`]. Span-Information wird durchgereicht
//! aus dem Lexer-Output.
//!
//! Im Gegensatz zum typisierten AST (Woche 5, Task 5.1+) verliert der CST
//! keine Information aus dem Source — auch Whitespace/Kommentare koennten
//! kuenftig als Trivia-Children erfasst werden, falls Source-Preserving-
//! Rewrites gebraucht werden (Phase 1+).
//!
//! Modell-Inspiration: rust-analyzer / rowan (vereinfachte Variante:
//! `Vec<CstNode>` statt Arena-Index, kein Green-Tree-Sharing — Phase 0
//! priorisiert Klarheit ueber Performance).
//!
//! Siehe RFC 0001 §4.1 und §5.4 (CST vs. typed AST).

use core::fmt;

use crate::errors::Span;
use crate::grammar::{ProductionId, TokenKind};
use crate::lexer::Token;

/// Klassifikation eines [`CstNode`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CstKind {
    /// Internal Node — gehoert zu einer Grammar-Production und enthaelt
    /// Children entsprechend der gewaehlten Alternative.
    Internal {
        /// Welche Production diesen Node erzeugt hat.
        production: ProductionId,
        /// Index der Alternative innerhalb der Production. Zusammen mit
        /// `production` eindeutig — Builder braucht beides fuer Dispatch
        /// auf den richtigen AST-Builder (Woche 5).
        alternative_index: usize,
    },
    /// Token-Leaf — gescannt vom Lexer.
    Token(TokenKind),
    /// Error-Node — Platzhalter fuer Recovery (Phase 1+; in Phase 0 nicht
    /// vom Builder erzeugt, aber als Datentyp vorgesehen).
    Error,
}

/// Ein Knoten im Concrete Syntax Tree.
///
/// `'src` ist die Lifetime des urspruenglichen Source-Texts; `text` zeigt
/// bei Token-Leaves auf den jeweiligen Source-Slice. Internal-Nodes
/// koennen ein `text`-Slice tragen, das die volle Spanne ueberdeckt
/// (vom ersten bis zum letzten Token-Children-Span), oder `""` wenn das
/// nicht relevant ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CstNode<'src> {
    /// Klassifikation.
    pub kind: CstKind,
    /// Span im Source-Text, ueberdeckt alle Children.
    pub span: Span,
    /// Source-Slice. Bei Token-Leaves: der gescannte Token-Text. Bei
    /// Internal-Nodes optional der ueberdeckte Source-Bereich. Bei
    /// `Error` und synthetischen Nodes oft leer.
    pub text: &'src str,
    /// Kinder-Knoten in Reihenfolge des Parse-Pfads.
    pub children: Vec<CstNode<'src>>,
}

impl<'src> CstNode<'src> {
    /// Konstruiert einen Internal-Node ohne Children. Span und Text werden
    /// vom Builder aus den finalen Children abgeleitet.
    #[must_use]
    pub fn internal(production: ProductionId, alternative_index: usize, span: Span) -> Self {
        Self {
            kind: CstKind::Internal {
                production,
                alternative_index,
            },
            span,
            text: "",
            children: Vec::new(),
        }
    }

    /// Konstruiert einen Token-Leaf direkt aus einem Lexer-[`Token`].
    #[must_use]
    pub fn token(token: Token<'src>) -> Self {
        Self {
            kind: CstKind::Token(token.kind),
            span: token.span,
            text: token.text,
            children: Vec::new(),
        }
    }

    /// Konstruiert einen Error-Node — Platzhalter fuer Recovery in
    /// spaeteren Phasen.
    #[must_use]
    pub fn error(span: Span) -> Self {
        Self {
            kind: CstKind::Error,
            span,
            text: "",
            children: Vec::new(),
        }
    }

    /// `true`, wenn der Node ein Token-Leaf ist.
    #[must_use]
    pub fn is_token(&self) -> bool {
        matches!(self.kind, CstKind::Token(_))
    }

    /// `true`, wenn der Node ein Internal-Node ist.
    #[must_use]
    pub fn is_internal(&self) -> bool {
        matches!(self.kind, CstKind::Internal { .. })
    }

    /// `true`, wenn der Node ein Error-Knoten ist.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self.kind, CstKind::Error)
    }

    /// [`ProductionId`] eines Internal-Nodes; sonst `None`.
    #[must_use]
    pub fn production(&self) -> Option<ProductionId> {
        match self.kind {
            CstKind::Internal { production, .. } => Some(production),
            _ => None,
        }
    }

    /// Index der gewaehlten Alternative; sonst `None`.
    #[must_use]
    pub fn alternative_index(&self) -> Option<usize> {
        match self.kind {
            CstKind::Internal {
                alternative_index, ..
            } => Some(alternative_index),
            _ => None,
        }
    }

    /// Token-Klassifikation eines Token-Leafs; sonst `None`.
    #[must_use]
    pub fn token_kind(&self) -> Option<TokenKind> {
        match self.kind {
            CstKind::Token(kind) => Some(kind),
            _ => None,
        }
    }

    /// Haengt einen Child-Node an. Span des Parents wird **nicht** automatisch
    /// erweitert — Builder muss Span explizit setzen, wenn vollstaendig.
    pub fn push_child(&mut self, child: CstNode<'src>) {
        self.children.push(child);
    }

    /// Aktualisiert den Span auf die Vereinigung aller Children-Spans.
    /// Nuetzlich nachdem alle Children eingefuegt wurden, falls der Builder
    /// die Position-Info nicht im Voraus hatte.
    pub fn recompute_span_from_children(&mut self) {
        if self.children.is_empty() {
            return;
        }
        let mut span = self.children[0].span;
        for child in &self.children[1..] {
            span = span.merge(child.span);
        }
        self.span = span;
    }

    /// Pre-order-Traversal: ruft `visitor` fuer self und dann rekursiv fuer
    /// jeden Child.
    pub fn walk_preorder<F: FnMut(&CstNode<'src>)>(&self, visitor: &mut F) {
        visitor(self);
        for child in &self.children {
            child.walk_preorder(visitor);
        }
    }

    /// Anzahl Knoten im Teilbaum (inkl. self).
    #[must_use]
    pub fn count_nodes(&self) -> usize {
        1 + self.children.iter().map(Self::count_nodes).sum::<usize>()
    }
}

impl fmt::Display for CstNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_indented(self, f, 0)
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn write_indented(node: &CstNode<'_>, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
    for _ in 0..depth {
        f.write_str("  ")?;
    }
    match &node.kind {
        CstKind::Internal {
            production,
            alternative_index,
        } => writeln!(
            f,
            "Internal(prod={}, alt={}) @ {}",
            production.0, alternative_index, node.span
        )?,
        CstKind::Token(kind) => writeln!(f, "Token({kind:?}) @ {} {:?}", node.span, node.text)?,
        CstKind::Error => writeln!(f, "Error @ {}", node.span)?,
    }
    for child in &node.children {
        write_indented(child, f, depth + 1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;

    fn token(kind: TokenKind, span: Span, text: &'static str) -> Token<'static> {
        Token::new(kind, span, text)
    }

    #[test]
    fn internal_node_carries_production_and_alternative() {
        let node = CstNode::internal(ProductionId(7), 2, Span::new(10, 20));
        assert!(node.is_internal());
        assert!(!node.is_token());
        assert!(!node.is_error());
        assert_eq!(node.production(), Some(ProductionId(7)));
        assert!(node.children.is_empty());
        assert_eq!(node.span, Span::new(10, 20));
    }

    #[test]
    fn token_node_carries_kind_span_and_text() {
        let t = token(TokenKind::Keyword("struct"), Span::new(0, 6), "struct");
        let node = CstNode::token(t);
        assert!(node.is_token());
        assert_eq!(node.token_kind(), Some(TokenKind::Keyword("struct")));
        assert_eq!(node.text, "struct");
        assert_eq!(node.span, Span::new(0, 6));
    }

    #[test]
    fn error_node_has_only_span() {
        let node = CstNode::error(Span::new(5, 8));
        assert!(node.is_error());
        assert!(!node.is_internal());
        assert!(!node.is_token());
        assert_eq!(node.span, Span::new(5, 8));
        assert!(node.children.is_empty());
    }

    #[test]
    fn production_returns_none_for_token_or_error() {
        let t = token(TokenKind::Ident, Span::new(0, 3), "foo");
        assert!(CstNode::token(t).production().is_none());
        assert!(CstNode::error(Span::SYNTHETIC).production().is_none());
    }

    #[test]
    fn token_kind_returns_none_for_internal_or_error() {
        let n = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        assert!(n.token_kind().is_none());
        assert!(CstNode::error(Span::SYNTHETIC).token_kind().is_none());
    }

    #[test]
    fn push_child_appends_in_order() {
        let mut parent = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        let a = CstNode::token(token(TokenKind::Keyword("a"), Span::new(0, 1), "a"));
        let b = CstNode::token(token(TokenKind::Keyword("b"), Span::new(2, 3), "b"));
        parent.push_child(a);
        parent.push_child(b);
        assert_eq!(parent.children.len(), 2);
        assert_eq!(parent.children[0].text, "a");
        assert_eq!(parent.children[1].text, "b");
    }

    #[test]
    fn recompute_span_from_children_takes_outer_bounds() {
        let mut parent = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        parent.push_child(CstNode::token(token(
            TokenKind::Ident,
            Span::new(5, 8),
            "foo",
        )));
        parent.push_child(CstNode::token(token(
            TokenKind::Punct(";"),
            Span::new(10, 11),
            ";",
        )));
        parent.recompute_span_from_children();
        assert_eq!(parent.span, Span::new(5, 11));
    }

    #[test]
    fn recompute_span_on_empty_children_keeps_existing_span() {
        let mut parent = CstNode::internal(ProductionId(0), 0, Span::new(3, 7));
        parent.recompute_span_from_children();
        assert_eq!(parent.span, Span::new(3, 7));
    }

    #[test]
    fn walk_preorder_visits_root_then_children_in_order() {
        let mut parent = CstNode::internal(ProductionId(0), 0, Span::new(0, 10));
        let mut sub = CstNode::internal(ProductionId(1), 0, Span::new(0, 5));
        sub.push_child(CstNode::token(token(
            TokenKind::Keyword("a"),
            Span::new(0, 1),
            "a",
        )));
        sub.push_child(CstNode::token(token(
            TokenKind::Keyword("b"),
            Span::new(2, 3),
            "b",
        )));
        parent.push_child(sub);
        parent.push_child(CstNode::token(token(
            TokenKind::Punct(";"),
            Span::new(9, 10),
            ";",
        )));

        let mut visited: Vec<String> = Vec::new();
        parent.walk_preorder(&mut |node| match &node.kind {
            CstKind::Internal { production, .. } => visited.push(format!("I({})", production.0)),
            CstKind::Token(kind) => visited.push(format!("T({kind:?})")),
            CstKind::Error => visited.push("E".to_string()),
        });
        assert_eq!(
            visited,
            vec![
                "I(0)".to_string(),
                "I(1)".to_string(),
                "T(Keyword(\"a\"))".to_string(),
                "T(Keyword(\"b\"))".to_string(),
                "T(Punct(\";\"))".to_string(),
            ]
        );
    }

    #[test]
    fn count_nodes_includes_self_and_descendants() {
        let mut parent = CstNode::internal(ProductionId(0), 0, Span::SYNTHETIC);
        parent.push_child(CstNode::token(token(
            TokenKind::Ident,
            Span::new(0, 3),
            "foo",
        )));
        let mut sub = CstNode::internal(ProductionId(1), 0, Span::SYNTHETIC);
        sub.push_child(CstNode::token(token(
            TokenKind::Punct(";"),
            Span::new(3, 4),
            ";",
        )));
        sub.push_child(CstNode::token(token(
            TokenKind::Punct("{"),
            Span::new(4, 5),
            "{",
        )));
        parent.push_child(sub);
        // 1 (parent) + 1 (Ident-Token) + 1 (sub) + 2 (sub's tokens) = 5
        assert_eq!(parent.count_nodes(), 5);
    }

    #[test]
    fn display_renders_indented_tree_with_kind_span_text() {
        let mut parent = CstNode::internal(ProductionId(0), 0, Span::new(0, 6));
        parent.push_child(CstNode::token(token(
            TokenKind::Keyword("struct"),
            Span::new(0, 6),
            "struct",
        )));
        let s = format!("{parent}");
        assert!(s.contains("Internal(prod=0, alt=0)"));
        assert!(s.contains("Token(Keyword(\"struct\"))"));
        assert!(s.contains("\"struct\""));
    }

    #[test]
    fn display_renders_error_node() {
        let n = CstNode::error(Span::new(2, 5));
        let s = format!("{n}");
        assert!(s.contains("Error"));
        assert!(s.contains("2..5"));
    }
}
