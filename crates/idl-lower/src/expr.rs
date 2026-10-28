// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Constant expressions, from the CST into the model. Spec §5.
//!
//! The expression tree is built as nodes rather than folded to a value:
//! a bound, an array dimension and a case label all keep the form they
//! were written in, and evaluation stays a query the model answers.
//! Spec §3.5 of the AST.

use crate::lower::Lowering;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use zerodds_ast::{AnyId, BinaryOp, ConstExpr, ConstExprNode, Literal, UnaryOp};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};
use zerodds_idl_cst::{CstKind, SyntaxNode, TokenKind, TokenValue, literal_value};
use zerodds_idl_parse::DiagnosticCode;

impl Lowering<'_> {
    /// Lower one expression subtree.
    ///
    /// Returns `None` only when the CST holds no expression at all — a
    /// malformed one still becomes a node, so the surrounding
    /// declaration keeps its shape and the diagnostic points at the
    /// expression rather than at the declaration.
    pub(crate) fn expr(&mut self, node: &Arc<SyntaxNode>) -> Option<Id<ConstExprNode>> {
        let expr = match node.kind() {
            CstKind::LiteralExpr => ConstExpr::Literal(self.literal(node)?),
            CstKind::NameExpr => self.name_expr(node)?,
            CstKind::ParenExpr => {
                // Parentheses group; the model records the grouping in
                // the source facet's byte range, not as a node kind.
                return self.first_expr_child(node);
            }
            CstKind::UnaryExpr => {
                let op = unary_op(node)?;
                let operand = self.first_expr_child(node)?;
                ConstExpr::Unary { op, operand }
            }
            CstKind::BinaryExpr => {
                let op = binary_op(node)?;
                let mut operands = self.expr_children(node);
                if operands.len() != 2 {
                    return None;
                }
                let rhs = operands.pop()?;
                let lhs = operands.pop()?;
                ConstExpr::Binary { op, lhs, rhs }
            }
            _ => return self.first_expr_child(node),
        };

        let span = node.span();
        let id = self.ast.add_expr(expr).ok()?;
        self.record_expr_origin(id, span);
        Some(id)
    }

    /// A name standing in for a value: a constant, or an enumerator.
    ///
    /// OMG IDL 4.2 §7.4.1.4.2: "An enum constant can only be defined
    /// using a scoped name for the enumerator. The scoped name is
    /// resolved using the normal scope resolution rules (see 7.5, Names
    /// and Scoping)." Its example, `module M { enum Size { small,
    /// medium, large }; }; const M::Size MYSIZE = M::medium;`, fixes
    /// both halves: an enumerator is reachable by its simple name from
    /// inside the enclosing scope, and by that scope's path from
    /// outside.
    ///
    /// §7.4.1.4.4.4.2 rule (54) makes a union's `<case_label>` a
    /// `<const_expr>`, and the same clause admits "an enumeration" as a
    /// `<switch_type_spec>`. Until a name here could reach an
    /// enumerator, no union over an enumeration lowered at all: the
    /// label was taken for a constant, looked up from the root, and
    /// reported `NotFound`.
    ///
    /// An enumerator is resolved on the spot rather than left pending,
    /// because it has to be declared before it is named — the
    /// discriminator type alone guarantees the enumeration is already in
    /// the model.
    ///
    /// A **constant** named by its simple name is resolved on the spot
    /// for the same reason and by the same rule. `Lowerer::scoped_name_of`
    /// marks every name it reads absolute, whether or not the source
    /// wrote a leading `::`, and a pending reference carries no scope —
    /// so a simple name left pending is later looked up from the root and
    /// reported `NotFound`. That is what happened to
    ///
    /// ```text
    /// module sensing { module common {
    ///   const uint32 SZ_SMALL = 256;
    ///   struct FrameHeader { sequence<BlobRef, SZ_SMALL> blobs; };
    /// }; };
    /// ```
    ///
    /// where the bound and the constant sit in one scope. §7.5's rule is
    /// that a simple name walks the enclosing scopes, and walking them
    /// here is what makes it true. A name that reaches neither an
    /// enumerator nor a constant keeps the pending `NameRef` it always
    /// had, so a genuine forward reference is unaffected.
    fn name_expr(&mut self, node: &Arc<SyntaxNode>) -> Option<ConstExpr> {
        let name = self.scoped_name_of(node)?;
        if let Some(enumerator) = self.lookup_enumerator(name.segments()) {
            return Some(ConstExpr::Literal(Literal::Enumerator(Ref::Resolved(
                enumerator,
            ))));
        }
        if let Some(constant) = self.lookup_constant(name.segments()) {
            return Some(ConstExpr::NameRef(Ref::Resolved(constant)));
        }
        Some(ConstExpr::NameRef(Ref::Pending(PendingRef::ScopedName(
            name,
        ))))
    }

    /// The enumerator a written name denotes, if it denotes one.
    ///
    /// A simple name walks the scope stack, which is what §7.5 calls for
    /// and what makes `case RED:` work inside the enumeration's own
    /// scope. A qualified name goes to the model's index, where
    /// `Ast::scoped_name_of` has put every enumerator under its
    /// enclosing module's path.
    fn lookup_enumerator(
        &self,
        segments: &[alloc::string::String],
    ) -> Option<Id<zerodds_ast::EnumeratorNode>> {
        match self.lookup_name(segments)? {
            AnyId::Enumerator(id, _) => Some(id),
            _ => None,
        }
    }

    /// The constant a written name denotes, if it denotes one already in
    /// the model. Same two rules as [`Self::lookup_enumerator`].
    fn lookup_constant(
        &self,
        segments: &[alloc::string::String],
    ) -> Option<Id<zerodds_ast::ConstNode>> {
        match self.lookup_name(segments)? {
            AnyId::Const(id, _) => Some(id),
            _ => None,
        }
    }

    /// §7.5 name resolution for a name written in an expression: a simple
    /// name walks the enclosing scopes, a qualified one goes to the index.
    fn lookup_name(&self, segments: &[alloc::string::String]) -> Option<AnyId> {
        match segments {
            [] => None,
            [single] => self.scopes.lookup(single),
            qualified => self
                .ast
                .find_by_scoped_name(&ScopedName::absolute(qualified.to_vec()))
                .ok(),
        }
    }

    /// The first expression among a node's children, lowered.
    pub(crate) fn first_expr_child(&mut self, node: &Arc<SyntaxNode>) -> Option<Id<ConstExprNode>> {
        let child = node
            .children()
            .into_iter()
            .find(|child| is_expr(child.kind()))?;
        self.expr(&child)
    }

    fn expr_children(&mut self, node: &Arc<SyntaxNode>) -> Vec<Id<ConstExprNode>> {
        node.children()
            .into_iter()
            .filter(|child| is_expr(child.kind()))
            .filter_map(|child| self.expr(&child))
            .collect()
    }

    /// The literal a `LiteralExpr` holds.
    fn literal(&mut self, node: &Arc<SyntaxNode>) -> Option<Literal> {
        let (span, token) = node
            .syntax_tokens()
            .into_iter()
            .find(|(_, token)| token.kind().is_literal())?;
        let text = token.text();
        let literal = match token.kind() {
            TokenKind::True => Literal::Boolean(true),
            TokenKind::False => Literal::Boolean(false),
            TokenKind::IntLiteral => {
                let TokenValue::Int { value, .. } = literal_value(TokenKind::IntLiteral, text)
                else {
                    // The lexer turns an out-of-range literal into an
                    // error token, so reaching here means the text is
                    // not the integer it was taken for.
                    self.diagnose_at(
                        DiagnosticCode::UnexpectedToken,
                        span,
                        "this integer literal has no value",
                        "OMG IDL 4.2 §7.2.6.1",
                    );
                    return None;
                };
                Literal::Integer(value)
            }
            TokenKind::FloatLiteral => split_float(text),
            TokenKind::FixedLiteral => split_fixed(text),
            TokenKind::CharLiteral | TokenKind::WCharLiteral => {
                let TokenValue::Char { value } = literal_value(token.kind(), text) else {
                    return None;
                };
                if token.kind() == TokenKind::WCharLiteral {
                    Literal::WChar(value)
                } else {
                    // A narrow character literal outside Unicode is an
                    // error rather than a substituted replacement.
                    let ch = char::from_u32(value)?;
                    Literal::Char(ch)
                }
            }
            TokenKind::StringLiteral | TokenKind::WStringLiteral => {
                // Every piece of the literal (§7.2.6.3): the parser keeps
                // each piece as its own token in this one node.
                let value =
                    crate::lower::joined_string_literals(node.syntax_tokens(), token.kind())
                        .into_iter()
                        .next()?;
                if token.kind() == TokenKind::WStringLiteral {
                    Literal::WString(value.chars().map(u32::from).collect())
                } else {
                    Literal::String(value)
                }
            }
            _ => return None,
        };
        Some(literal)
    }
}

/// Whether a node kind is an expression form.
pub(crate) const fn is_expr(kind: CstKind) -> bool {
    matches!(
        kind,
        CstKind::LiteralExpr
            | CstKind::NameExpr
            | CstKind::UnaryExpr
            | CstKind::BinaryExpr
            | CstKind::ParenExpr
    )
}

fn unary_op(node: &Arc<SyntaxNode>) -> Option<UnaryOp> {
    node.syntax_tokens().into_iter().find_map(|(_, token)| {
        Some(match token.kind() {
            TokenKind::Plus => UnaryOp::Plus,
            TokenKind::Minus => UnaryOp::Minus,
            TokenKind::Tilde => UnaryOp::Not,
            _ => return None,
        })
    })
}

fn binary_op(node: &Arc<SyntaxNode>) -> Option<BinaryOp> {
    node.syntax_tokens().into_iter().find_map(|(_, token)| {
        Some(match token.kind() {
            TokenKind::Pipe => BinaryOp::Or,
            TokenKind::Caret => BinaryOp::Xor,
            TokenKind::Ampersand => BinaryOp::And,
            TokenKind::LessLess => BinaryOp::Shl,
            TokenKind::GreaterGreater => BinaryOp::Shr,
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Mod,
            _ => return None,
        })
    })
}

/// Split a float literal into mantissa and exponent, as written.
///
/// Kept as text rather than converted: `1.0` and `1.` and `1e0` are the
/// same value and different source, and a formatter has to reproduce the
/// one that was written. Spec §3.5 of the AST.
fn split_float(text: &str) -> Literal {
    match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => Literal::Float {
            mantissa: mantissa.to_string(),
            exponent: Some(exponent.to_string()),
        },
        None => Literal::Float {
            mantissa: text.to_string(),
            exponent: None,
        },
    }
}

/// Split a fixed-point literal into its digits and scale.
///
/// `123.45d` has digits `12345` and scale 2. IDL 4.2 §7.2.6.4.
fn split_fixed(text: &str) -> Literal {
    let body = text.strip_suffix(['d', 'D']).unwrap_or(text);
    match body.split_once('.') {
        Some((whole, fraction)) => {
            let mut digits = whole.to_string();
            digits.push_str(fraction);
            Literal::Fixed {
                digits,
                scale: u8::try_from(fraction.len()).unwrap_or(u8::MAX),
            }
        }
        None => Literal::Fixed {
            digits: body.to_string(),
            scale: 0,
        },
    }
}
