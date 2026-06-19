// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Recursive-descent parser with precedence climbing.
//!
//! Grammar (EBNF, not all concrete literals):
//!
//! ```text
//! expr    = or_expr
//! or_expr = and_expr { OR and_expr }
//! and_expr= not_expr { AND not_expr }
//! not_expr= [ NOT ] atom
//! atom    = '(' expr ')' | cmp
//! cmp     = operand cmp_op operand
//! operand = literal | field | '%' DIGITS
//! ```

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::{CmpOp, Expr, Operand, Value};
use crate::lexer::{LexError, Token, tokenize};

/// Parse error with a human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "parse error: {}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        Self(e.0)
    }
}

/// Parse entry point. The entire input must be part of exactly one
/// expression — leftover strings are treated as errors.
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let tokens = tokenize(input)?;
    let mut p = Parser { tokens, pos: 0 };
    let expr = p.parse_or()?;
    if p.pos != p.tokens.len() {
        return Err(ParseError(alloc::format!(
            "unexpected trailing token at position {}",
            p.pos
        )));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_n(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset)
    }

    fn consume(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.pos += 1;
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.pos += 1;
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.pos += 1;
            let inner = self.parse_not()?;
            return Ok(Expr::Not(Box::new(inner)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek(), Some(Token::LParen)) {
            self.pos += 1;
            let inner = self.parse_or()?;
            if !matches!(self.peek(), Some(Token::RParen)) {
                return Err(ParseError("expected ')'".into()));
            }
            self.pos += 1;
            return Ok(inner);
        }
        self.parse_cmp()
    }

    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.parse_operand()?;
        // BETWEEN-Predicate: `field [NOT] BETWEEN low AND high`
        // (Spec §B.2.1 BetweenPredicate).
        let mut negated = false;
        if matches!(self.peek(), Some(Token::Not)) && matches!(self.peek_n(1), Some(Token::Between))
        {
            self.pos += 1;
            negated = true;
        }
        if matches!(self.peek(), Some(Token::Between)) {
            self.pos += 1;
            let low = self.parse_operand()?;
            if !matches!(self.consume(), Some(Token::And)) {
                return Err(ParseError("expected AND after BETWEEN lower bound".into()));
            }
            let high = self.parse_operand()?;
            return Ok(Expr::Between {
                field: lhs,
                low,
                high,
                negated,
            });
        }
        if negated {
            return Err(ParseError("NOT must be followed by BETWEEN".into()));
        }
        let op = match self.consume() {
            Some(Token::Eq) => CmpOp::Eq,
            Some(Token::Neq) => CmpOp::Neq,
            Some(Token::Lt) => CmpOp::Lt,
            Some(Token::Le) => CmpOp::Le,
            Some(Token::Gt) => CmpOp::Gt,
            Some(Token::Ge) => CmpOp::Ge,
            Some(Token::Like) => CmpOp::Like,
            other => {
                return Err(ParseError(alloc::format!(
                    "expected comparison op, got {other:?}"
                )));
            }
        };
        let rhs = self.parse_operand()?;
        Ok(Expr::Cmp { lhs, op, rhs })
    }

    fn parse_operand(&mut self) -> Result<Operand, ParseError> {
        let tok = self
            .consume()
            .ok_or_else(|| ParseError("unexpected end".into()))?;
        match tok {
            Token::StrLit(s) => Ok(Operand::Literal(Value::String(s))),
            Token::IntLit(n) => Ok(Operand::Literal(Value::Int(n))),
            Token::FloatLit(f) => Ok(Operand::Literal(Value::Float(f))),
            Token::BoolLit(b) => Ok(Operand::Literal(Value::Bool(b))),
            Token::Ident(name) => Ok(Operand::Field(name)),
            Token::Param(i) => Ok(Operand::Param(i)),
            other => Err(ParseError(alloc::format!(
                "expected operand, got {other:?}"
            ))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_eq() {
        let e = parse("color = 'RED'").expect("parse");
        match e {
            Expr::Cmp { lhs, op, rhs } => {
                assert_eq!(lhs, Operand::Field("color".into()));
                assert_eq!(op, CmpOp::Eq);
                assert_eq!(rhs, Operand::Literal(Value::String("RED".into())));
            }
            _ => panic!("expected Cmp"),
        }
    }

    #[test]
    fn parse_and_or_precedence() {
        // AND binds more tightly than OR.
        let e = parse("a = 1 OR b = 2 AND c = 3").expect("parse");
        match e {
            Expr::Or(_, rhs) => {
                assert!(matches!(*rhs, Expr::And(_, _)));
            }
            _ => panic!("expected Or at top level"),
        }
    }

    #[test]
    fn parse_parenthesized() {
        let e = parse("(a = 1 OR b = 2) AND c = 3").expect("parse");
        match e {
            Expr::And(lhs, _) => {
                assert!(matches!(*lhs, Expr::Or(_, _)));
            }
            _ => panic!("expected And at top level"),
        }
    }

    #[test]
    fn parse_not() {
        let e = parse("NOT a = 1").expect("parse");
        assert!(matches!(e, Expr::Not(_)));
    }

    #[test]
    fn parse_param() {
        let e = parse("x > %3").expect("parse");
        match e {
            Expr::Cmp {
                rhs: Operand::Param(3),
                ..
            } => {}
            _ => panic!("expected param operand"),
        }
    }

    #[test]
    fn parse_rejects_trailing_garbage() {
        assert!(parse("a = 1 garbage").is_err());
    }

    #[test]
    fn parse_between_predicate() {
        let e = parse("x BETWEEN 1 AND 10").expect("parse");
        match e {
            Expr::Between {
                field,
                low,
                high,
                negated,
            } => {
                assert_eq!(field, Operand::Field("x".into()));
                assert_eq!(low, Operand::Literal(Value::Int(1)));
                assert_eq!(high, Operand::Literal(Value::Int(10)));
                assert!(!negated);
            }
            _ => panic!("expected Between"),
        }
    }

    #[test]
    fn parse_not_between_predicate() {
        let e = parse("x NOT BETWEEN 1 AND 10").expect("parse");
        match e {
            Expr::Between { negated, .. } => assert!(negated),
            _ => panic!("expected negated Between"),
        }
    }

    #[test]
    fn parse_between_with_params() {
        let e = parse("ts BETWEEN %0 AND %1").expect("parse");
        let params = e.collect_param_indices();
        assert_eq!(params, alloc::vec![0, 1]);
    }

    #[test]
    fn parse_rejects_unbalanced_parens() {
        assert!(parse("(a = 1").is_err());
    }

    #[test]
    fn parse_node_count() {
        let e = parse("a = 1 AND b = 2 OR c = 3").expect("parse");
        // 3 comparisons + 1 AND + 1 OR = 5.
        assert_eq!(e.node_count(), 5);
    }

    #[test]
    fn parse_collects_param_indices() {
        let e = parse("a = %0 AND b = %2 OR c = %1").expect("parse");
        let mut idx = e.collect_param_indices();
        idx.sort_unstable();
        assert_eq!(idx, vec![0, 1, 2]);
    }
}
