// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Type expressions, scoped names and constant expressions.
//! Spec §2.3, §3.3.

use crate::diagnostic::DiagnosticCode;
use crate::parser::Parser;
use zerodds_idl_cst::{CstKind, TokenKind};

impl Parser<'_> {
    /// A type specification that may carry annotations of its own.
    ///
    /// Three positions in the grammar annotate a *use* of a type rather
    /// than a declaration: the element of a `sequence`, the key and the
    /// value of a `map`, and a union's discriminator. DDS-XTypes 1.3
    /// §7.3.1.2.1.12 writes them as
    /// `sequence<@try_construct(TRIM) String5, 10>` and
    /// `union U switch (@try_construct T)`, and §7.3.1.2.1.12.4 is an
    /// example built on the first.
    ///
    /// The annotations are siblings of the type specification, which is
    /// the shape `parse_annotated_definition` already uses for a
    /// declaration and its annotations. Nothing wraps them, so the
    /// positional indexing of a template argument list is unchanged and
    /// the token order — and with it the round trip — is the source's.
    pub(crate) fn parse_annotated_type_spec(&mut self) {
        while self.current() == TokenKind::At {
            self.parse_annotation();
        }
        self.parse_type_spec();
    }

    /// A type specification.
    pub(crate) fn parse_type_spec(&mut self) {
        match self.current() {
            TokenKind::Sequence => self.parse_sequence_type(),
            TokenKind::StringKw => self.parse_string_type(CstKind::StringType),
            TokenKind::Wstring => self.parse_string_type(CstKind::WStringType),
            TokenKind::Map => self.parse_map_type(),
            TokenKind::Fixed => self.parse_fixed_type(),
            TokenKind::Ident | TokenKind::ColonColon => {
                self.start(CstKind::NamedType);
                self.parse_scoped_name();
                self.finish();
            }
            kind if is_primitive_keyword(kind) => self.parse_primitive_type(),
            _ => {
                self.diagnose(
                    DiagnosticCode::UnexpectedToken,
                    "expected a type",
                    "OMG IDL 4.2 §7.4.1.4",
                );
                self.start(CstKind::Error);
                if !self.at_eof() {
                    self.bump();
                }
                self.finish();
            }
        }
    }

    /// A primitive type, which may be spelled in several keywords.
    ///
    /// The multi-word forms IDL 4.2 §7.4.1.4.2 defines, and only those:
    /// `unsigned short`, `unsigned long`, `unsigned long long`,
    /// `long long`, `long double`. Anything else is one keyword, and a
    /// loop that keeps eating type keywords would swallow the next
    /// declaration's type.
    fn parse_primitive_type(&mut self) {
        self.start(CstKind::PrimitiveType);
        match self.current() {
            TokenKind::Unsigned => {
                self.bump();
                if self.eat(TokenKind::Long) {
                    // `unsigned long long`
                    let _ = self.eat(TokenKind::Long);
                } else if !self.eat(TokenKind::Short) {
                    self.diagnose(
                        DiagnosticCode::MissingToken,
                        "expected `short` or `long` after `unsigned`",
                        "OMG IDL 4.2 §7.4.1.4.2",
                    );
                }
            }
            TokenKind::Long => {
                self.bump();
                // `long long` or `long double`; a bare `long` otherwise.
                if !self.eat(TokenKind::Long) {
                    let _ = self.eat(TokenKind::Double);
                }
            }
            _ => self.bump(),
        }
        self.finish();
    }

    fn parse_sequence_type(&mut self) {
        self.start(CstKind::SequenceType);
        self.bump(); // sequence
        if self.expect_type_arguments_open() {
            self.start(CstKind::TypeArguments);
            self.parse_annotated_type_spec();
            if self.eat(TokenKind::Comma) {
                self.parse_const_expr();
            }
            self.close_type_arguments();
            self.finish();
        }
        self.finish();
    }

    fn parse_string_type(&mut self, kind: CstKind) {
        self.start(kind);
        self.bump(); // string or wstring
        if self.current() == TokenKind::Less {
            self.start(CstKind::TypeArguments);
            self.bump();
            self.parse_const_expr();
            self.close_type_arguments();
            self.finish();
        }
        self.finish();
    }

    fn parse_map_type(&mut self) {
        self.start(CstKind::MapType);
        self.bump(); // map
        if self.expect_type_arguments_open() {
            self.start(CstKind::TypeArguments);
            self.parse_annotated_type_spec();
            self.expect(TokenKind::Comma, "`,` between the map key and value");
            self.parse_annotated_type_spec();
            if self.eat(TokenKind::Comma) {
                self.parse_const_expr();
            }
            self.close_type_arguments();
            self.finish();
        }
        self.finish();
    }

    fn parse_fixed_type(&mut self) {
        self.start(CstKind::FixedType);
        self.bump(); // fixed
        if self.current() == TokenKind::Less {
            self.start(CstKind::TypeArguments);
            self.bump();
            self.parse_const_expr();
            self.expect(TokenKind::Comma, "`,` between digits and scale");
            self.parse_const_expr();
            self.close_type_arguments();
            self.finish();
        }
        self.finish();
    }

    fn expect_type_arguments_open(&mut self) -> bool {
        if self.eat(TokenKind::Less) {
            return true;
        }
        self.diagnose(
            DiagnosticCode::MissingToken,
            "expected `<` opening the type arguments",
            "OMG IDL 4.2 §7.4.1.4.4",
        );
        false
    }

    /// Close a type argument list, splitting `>>` when it closes two.
    ///
    /// `sequence<sequence<long>>` ends in one `>>` token. Closing the
    /// inner list consumes the first half and leaves the second for the
    /// outer one; the split is visible in the tree as two `>` nodes over
    /// one source token, so the round trip still holds. Spec §2.3.
    pub(crate) fn close_type_arguments(&mut self) {
        if self.eat(TokenKind::Greater) {
            return;
        }
        if self.current() == TokenKind::GreaterGreater {
            self.split_shift_right();
            return;
        }
        self.diagnose(
            DiagnosticCode::MissingToken,
            "expected `>` closing the type arguments",
            "OMG IDL 4.2 §7.4.1.4.4",
        );
    }
}

/// Whether a keyword can begin a primitive type.
const fn is_primitive_keyword(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Boolean
            | TokenKind::Char
            | TokenKind::Wchar
            | TokenKind::Octet
            | TokenKind::Short
            | TokenKind::Long
            | TokenKind::Float
            | TokenKind::Double
            | TokenKind::Unsigned
            | TokenKind::Int8
            | TokenKind::Int16
            | TokenKind::Int32
            | TokenKind::Int64
            | TokenKind::Uint8
            | TokenKind::Uint16
            | TokenKind::Uint32
            | TokenKind::Uint64
            | TokenKind::Any
            | TokenKind::Object
            | TokenKind::Valuebase
    )
}

impl Parser<'_> {
    /// A scoped name, possibly rooted at `::`.
    pub(crate) fn parse_scoped_name(&mut self) {
        self.start(CstKind::ScopedName);
        // A leading `::` is the global scope, never an operator.
        let _ = self.eat(TokenKind::ColonColon);
        if !self.eat(TokenKind::Ident) {
            self.diagnose(
                DiagnosticCode::MissingToken,
                "expected a name",
                "OMG IDL 4.2 §7.4.1.2",
            );
            self.finish();
            return;
        }
        while self.current() == TokenKind::ColonColon {
            self.bump();
            if !self.eat(TokenKind::Ident) {
                self.diagnose(
                    DiagnosticCode::MissingToken,
                    "expected a name after `::`",
                    "OMG IDL 4.2 §7.4.1.2",
                );
                break;
            }
        }
        self.finish();
    }

    /// A constant expression, at the loosest precedence.
    pub(crate) fn parse_const_expr(&mut self) {
        self.parse_binary_expr(0);
    }

    /// Precedence-climbing over the IDL 4.2 ladder.
    ///
    /// Loosest to tightest: `|`, `^`, `&`, shifts, additive,
    /// multiplicative, unary. IDL 4.2 §7.4.1.4.2.
    fn parse_binary_expr(&mut self, min_level: u8) {
        let start = self.checkpoint();
        self.parse_unary_expr();
        loop {
            let Some(level) = binary_level(self.current()) else {
                return;
            };
            if level < min_level {
                return;
            }
            // In an expression `>>` is a shift and is NOT split — the
            // split belongs to type argument lists only. Spec §3.3.
            self.bump();
            // Left-associative: the right side binds one level tighter,
            // so `1 - 2 - 3` groups as `(1 - 2) - 3`.
            self.parse_binary_expr(level + 1);
            // The left operand was already emitted, so the node is
            // wrapped around it retroactively.
            self.start_at(start, CstKind::BinaryExpr);
            self.finish();
        }
    }

    fn parse_unary_expr(&mut self) {
        match self.current() {
            TokenKind::Minus | TokenKind::Plus | TokenKind::Tilde => {
                self.start(CstKind::UnaryExpr);
                self.bump();
                self.parse_unary_expr();
                self.finish();
            }
            TokenKind::LParen => {
                self.start(CstKind::ParenExpr);
                self.bump();
                self.parse_const_expr();
                self.expect(TokenKind::RParen, "`)` closing the expression");
                self.finish();
            }
            TokenKind::Ident | TokenKind::ColonColon => {
                self.start(CstKind::NameExpr);
                self.parse_scoped_name();
                self.finish();
            }
            kind if kind.is_literal() => {
                self.start(CstKind::LiteralExpr);
                self.bump();
                if matches!(kind, TokenKind::StringLiteral | TokenKind::WStringLiteral) {
                    self.eat_adjacent_string_literals(kind);
                }
                self.finish();
            }
            _ => {
                self.diagnose(
                    DiagnosticCode::UnexpectedToken,
                    "expected a constant expression",
                    "OMG IDL 4.2 §7.4.1.4.2",
                );
                self.start(CstKind::Error);
                if !self.at_eof() {
                    self.bump();
                }
                self.finish();
            }
        }
    }
}

/// The precedence of a binary operator, tighter being higher.
const fn binary_level(kind: TokenKind) -> Option<u8> {
    Some(match kind {
        TokenKind::Pipe => 1,
        TokenKind::Caret => 2,
        TokenKind::Ampersand => 3,
        TokenKind::LessLess | TokenKind::GreaterGreater => 4,
        TokenKind::Plus | TokenKind::Minus => 5,
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => 6,
        _ => return None,
    })
}
