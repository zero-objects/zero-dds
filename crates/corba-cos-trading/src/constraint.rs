// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OMG Trading **constraint language** — tokenizer, recursive-descent parser,
//! and evaluator over a property map.
//!
//! Supported grammar (subset of the OMG Trading Constraint Language):
//!
//! ```text
//! expr       := or_expr
//! or_expr    := and_expr ('or' and_expr)*
//! and_expr   := not_expr ('and' not_expr)*
//! not_expr   := 'not' not_expr | comparison
//! comparison := primary (('==' | '!=' | '<' | '<=' | '>' | '>=') primary)?
//! primary    := '(' expr ')' | 'exist' ident | literal | ident
//! literal    := integer | float | "'" string "'" | TRUE | FALSE
//! ```
//!
//! Property references missing from the map make their comparisons `false`
//! (OMG semantics: a constraint over a missing property does not match).

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::property::Value;

/// Parser/lexer error of the constraint language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintError {
    /// Unexpected token at position.
    UnexpectedToken(String),
    /// Expression ended prematurely.
    UnexpectedEnd,
    /// Invalid numeric literal.
    BadNumber(String),
    /// String literal without a closing `'`.
    UnterminatedString,
    /// Unknown character.
    BadChar(char),
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    And,
    Or,
    Not,
    Exist,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Ident(String),
    Lit(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
enum Operand {
    Prop(String),
    Lit(Value),
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare(Operand, CmpOp, Operand),
    Exist(String),
    /// Bare property as a boolean (matches if it is `Bool(true)`).
    PropBool(String),
    Bool(bool),
}

/// A parsed constraint expression, evaluable over a property map.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    expr: Expr,
}

impl Constraint {
    /// Parses a constraint string. An empty string always matches (`TRUE`).
    ///
    /// # Errors
    /// [`ConstraintError`] on lexer/parser errors.
    pub fn parse(input: &str) -> Result<Self, ConstraintError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(Self {
                expr: Expr::Bool(true),
            });
        }
        let tokens = tokenize(trimmed)?;
        let mut p = Parser {
            tokens: &tokens,
            pos: 0,
        };
        let expr = p.parse_or()?;
        if p.pos != p.tokens.len() {
            return Err(ConstraintError::UnexpectedToken(alloc::format!(
                "{:?}",
                p.tokens[p.pos]
            )));
        }
        Ok(Self { expr })
    }

    /// Evaluates the constraint over `props`.
    #[must_use]
    pub fn eval(&self, props: &BTreeMap<String, Value>) -> bool {
        eval(&self.expr, props)
    }
}

/// zerodds-lint: recursion-depth 64 (Constraint-Expr-Tree; bounded by parsed TCL nesting)
fn eval(expr: &Expr, props: &BTreeMap<String, Value>) -> bool {
    match expr {
        Expr::And(a, b) => eval(a, props) && eval(b, props),
        Expr::Or(a, b) => eval(a, props) || eval(b, props),
        Expr::Not(a) => !eval(a, props),
        Expr::Bool(b) => *b,
        Expr::Exist(name) => props.contains_key(name),
        Expr::PropBool(name) => props.get(name) == Some(&Value::Bool(true)),
        Expr::Compare(l, op, r) => {
            let (Some(lv), Some(rv)) = (resolve(l, props), resolve(r, props)) else {
                return false; // missing property → comparison does not match
            };
            match lv.compare(rv) {
                Some(ord) => match op {
                    CmpOp::Eq => ord == Ordering::Equal,
                    CmpOp::Ne => ord != Ordering::Equal,
                    CmpOp::Lt => ord == Ordering::Less,
                    CmpOp::Le => ord != Ordering::Greater,
                    CmpOp::Gt => ord == Ordering::Greater,
                    CmpOp::Ge => ord != Ordering::Less,
                },
                None => false, // incompatible types
            }
        }
    }
}

fn resolve<'a>(op: &'a Operand, props: &'a BTreeMap<String, Value>) -> Option<&'a Value> {
    match op {
        Operand::Lit(v) => Some(v),
        Operand::Prop(name) => props.get(name),
    }
}

// ---- Lexer -----------------------------------------------------------------

#[allow(clippy::too_many_lines)]
fn tokenize(s: &str) -> Result<Vec<Token>, ConstraintError> {
    let chars: Vec<char> = s.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Eq);
                    i += 2;
                } else {
                    return Err(ConstraintError::UnexpectedToken("=".to_string()));
                }
            }
            '!' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Ne);
                    i += 2;
                } else {
                    return Err(ConstraintError::UnexpectedToken("!".to_string()));
                }
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Le);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Ge);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '\'' => {
                let mut j = i + 1;
                let mut buf = String::new();
                while j < chars.len() && chars[j] != '\'' {
                    buf.push(chars[j]);
                    j += 1;
                }
                if j >= chars.len() {
                    return Err(ConstraintError::UnterminatedString);
                }
                tokens.push(Token::Lit(Value::Str(buf)));
                i = j + 1;
            }
            c if c.is_ascii_digit()
                || (c == '-' && matches!(chars.get(i + 1), Some(d) if d.is_ascii_digit())) =>
            {
                let start = i;
                i += 1;
                let mut is_float = false;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    if chars[i] == '.' {
                        is_float = true;
                    }
                    i += 1;
                }
                let num: String = chars[start..i].iter().collect();
                if is_float {
                    let f: f64 = num
                        .parse()
                        .map_err(|_| ConstraintError::BadNumber(num.clone()))?;
                    tokens.push(Token::Lit(Value::Float(f)));
                } else {
                    let n: i64 = num
                        .parse()
                        .map_err(|_| ConstraintError::BadNumber(num.clone()))?;
                    tokens.push(Token::Lit(Value::Int(n)));
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                tokens.push(keyword_or_ident(word));
            }
            other => return Err(ConstraintError::BadChar(other)),
        }
    }
    Ok(tokens)
}

fn keyword_or_ident(word: String) -> Token {
    match word.to_ascii_lowercase().as_str() {
        "and" => Token::And,
        "or" => Token::Or,
        "not" => Token::Not,
        "exist" => Token::Exist,
        "true" => Token::Lit(Value::Bool(true)),
        "false" => Token::Lit(Value::Bool(false)),
        _ => Token::Ident(word),
    }
}

// ---- Parser ----------------------------------------------------------------

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_or(&mut self) -> Result<Expr, ConstraintError> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.pos += 1;
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ConstraintError> {
        let mut left = self.parse_not()?;
        while self.peek() == Some(&Token::And) {
            self.pos += 1;
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, ConstraintError> {
        if self.peek() == Some(&Token::Not) {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.parse_not()?)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ConstraintError> {
        // 'exist <ident>' is a standalone boolean primary.
        if self.peek() == Some(&Token::Exist) {
            self.pos += 1;
            return match self.advance() {
                Some(Token::Ident(name)) => Ok(Expr::Exist(name.clone())),
                Some(t) => Err(ConstraintError::UnexpectedToken(alloc::format!("{t:?}"))),
                None => Err(ConstraintError::UnexpectedEnd),
            };
        }
        // '(' expr ')'
        if self.peek() == Some(&Token::LParen) {
            self.pos += 1;
            let inner = self.parse_or()?;
            match self.advance() {
                Some(Token::RParen) => return Ok(inner),
                _ => return Err(ConstraintError::UnexpectedToken(")".to_string())),
            }
        }
        let left = self.parse_operand()?;
        if let Some(op) = self.peek_cmp() {
            self.pos += 1;
            let right = self.parse_operand()?;
            return Ok(Expr::Compare(left, op, right));
        }
        // A single boolean literal (`TRUE`) or a bare boolean
        // property (`Available`) as an expression.
        match left {
            Operand::Lit(Value::Bool(b)) => Ok(Expr::Bool(b)),
            Operand::Prop(name) => Ok(Expr::PropBool(name)),
            other => Err(ConstraintError::UnexpectedToken(alloc::format!(
                "{other:?}"
            ))),
        }
    }

    fn peek_cmp(&self) -> Option<CmpOp> {
        match self.peek() {
            Some(Token::Eq) => Some(CmpOp::Eq),
            Some(Token::Ne) => Some(CmpOp::Ne),
            Some(Token::Lt) => Some(CmpOp::Lt),
            Some(Token::Le) => Some(CmpOp::Le),
            Some(Token::Gt) => Some(CmpOp::Gt),
            Some(Token::Ge) => Some(CmpOp::Ge),
            _ => None,
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, ConstraintError> {
        match self.advance() {
            Some(Token::Ident(name)) => Ok(Operand::Prop(name.clone())),
            Some(Token::Lit(v)) => Ok(Operand::Lit(v.clone())),
            Some(t) => Err(ConstraintError::UnexpectedToken(alloc::format!("{t:?}"))),
            None => Err(ConstraintError::UnexpectedEnd),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]
mod tests {
    use super::*;

    fn props() -> BTreeMap<String, Value> {
        let mut m = BTreeMap::new();
        m.insert("Speed".to_string(), Value::Int(150));
        m.insert("Color".to_string(), Value::Str("red".to_string()));
        m.insert("Price".to_string(), Value::Float(9.99));
        m.insert("Available".to_string(), Value::Bool(true));
        m
    }

    fn matches(c: &str) -> bool {
        Constraint::parse(c).unwrap().eval(&props())
    }

    #[test]
    fn numeric_comparisons() {
        assert!(matches("Speed > 100"));
        assert!(!matches("Speed > 200"));
        assert!(matches("Speed >= 150"));
        assert!(matches("Speed <= 150"));
        assert!(matches("Speed == 150"));
        assert!(matches("Speed != 151"));
    }

    #[test]
    fn float_and_mixed_numeric() {
        assert!(matches("Price < 10"));
        assert!(matches("Price > 9"));
        assert!(matches("Price == 9.99"));
    }

    #[test]
    fn string_and_bool() {
        assert!(matches("Color == 'red'"));
        assert!(!matches("Color == 'blue'"));
        assert!(matches("Available == TRUE"));
        assert!(matches("Available"));
    }

    #[test]
    fn boolean_logic_and_precedence() {
        assert!(matches("Speed > 100 and Color == 'red'"));
        assert!(!matches("Speed > 100 and Color == 'blue'"));
        assert!(matches("Speed > 200 or Color == 'red'"));
        assert!(matches("not Speed > 200"));
        // 'and' binds tighter than 'or':
        assert!(matches("Color == 'blue' or Speed > 100 and Available"));
    }

    #[test]
    fn parentheses_override_precedence() {
        assert!(!matches("(Color == 'blue' or Speed > 100) and Speed > 200"));
        assert!(matches("(Color == 'blue' or Speed > 100) and Available"));
    }

    #[test]
    fn exist_and_missing_property() {
        assert!(matches("exist Speed"));
        assert!(!matches("exist Weight"));
        // A comparison over a missing property does not match.
        assert!(!matches("Weight > 5"));
        assert!(!matches("Weight == 0"));
    }

    #[test]
    fn empty_constraint_matches_all() {
        assert!(matches(""));
        assert!(matches("   "));
    }

    #[test]
    fn negative_numbers() {
        let mut m = BTreeMap::new();
        m.insert("Temp".to_string(), Value::Int(-5));
        let c = Constraint::parse("Temp < 0").unwrap();
        assert!(c.eval(&m));
        let c2 = Constraint::parse("Temp == -5").unwrap();
        assert!(c2.eval(&m));
    }

    #[test]
    fn parse_errors() {
        assert!(Constraint::parse("Speed >").is_err()); // missing operand
        assert!(Constraint::parse("Speed = 5").is_err()); // '=' instead of '=='
        assert!(Constraint::parse("Color == 'red").is_err()); // unterminated
        assert!(Constraint::parse("(Speed > 1").is_err()); // missing ')'
    }

    #[test]
    fn incompatible_types_do_not_match() {
        // Comparing Color (Str) numerically → no match, no panic.
        assert!(!matches("Color > 5"));
    }
}
