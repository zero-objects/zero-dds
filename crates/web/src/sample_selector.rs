// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-WEB Sample Selector BNF Parser — Spec §7.4.8.
//!
//! Implements §7.4.8 fully.
//!
//! Spec source: OMG DDS-WEB 1.0 §7.4.8 (pp. 51-53) — `DataReader::read`
//! with the `SampleSelector` BNF grammar:
//!
//! ```text
//! SampleSelector  ::= FilterExpression ( ',' MetadataExpression )?
//! FilterExpression ::= Term ( ('AND' | 'OR') Term )*
//! Term            ::= Atom | '(' FilterExpression ')'
//! Atom            ::= FieldRef CompOp Literal
//! MetadataExpression ::= 'sample_state=' SampleState
//!                      | 'view_state='   ViewState
//!                      | 'instance_state='InstanceState
//! ```
//!
//! The parser returns an AST [`SampleSelector`] that the caller can
//! translate into a DDS [`QueryCondition`]
//! (`crates/dcps/src/query_condition.rs`).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Top-level AST of the sample-selector expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleSelector {
    /// Filter expression (may be empty if metadata only).
    pub filter: Option<FilterExpression>,
    /// Metadata expressions (several joined with AND possible).
    pub metadata: Vec<MetadataExpression>,
}

/// Filter expression — boolean expression over field references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterExpression {
    /// Single comparison.
    Comparison {
        /// Field-Pfad (z.B. `position.x`).
        field: String,
        /// Comparison operator.
        op: CompareOp,
        /// Literal value (right-hand side).
        value: Literal,
    },
    /// Conjunction / disjunction.
    Boolean {
        /// Boolean operator.
        op: BoolOp,
        /// Left sub-expression.
        lhs: alloc::boxed::Box<FilterExpression>,
        /// Right sub-expression.
        rhs: alloc::boxed::Box<FilterExpression>,
    },
}

/// Comparison operator (Spec §7.4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    /// `=`
    Eq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// Boolean operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// `AND`
    And,
    /// `OR`
    Or,
}

/// Literal value on the right-hand side of a comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// Integer.
    Integer(i64),
    /// String (between `'...'` or `"..."`).
    Str(String),
    /// Boolean (`true` / `false`).
    Bool(bool),
}

/// Metadata expression from Spec §7.4.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataExpression {
    /// `sample_state=read|not_read|any`
    SampleState(SampleStateMatch),
    /// `view_state=new|not_new|any`
    ViewState(ViewStateMatch),
    /// `instance_state=alive|not_alive_disposed|not_alive_no_writers|any`
    InstanceState(InstanceStateMatch),
}

/// Spec §7.4.8 — sample-state match (DDS DCPS §2.2.2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleStateMatch {
    /// READ samples only.
    Read,
    /// NOT_READ samples only.
    NotRead,
    /// Both.
    Any,
}

/// Spec §7.4.8 — view-state match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewStateMatch {
    /// NEW only.
    New,
    /// NOT_NEW only.
    NotNew,
    /// Both.
    Any,
}

/// Spec §7.4.8 — instance-state match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStateMatch {
    /// ALIVE only.
    Alive,
    /// NOT_ALIVE_DISPOSED only.
    NotAliveDisposed,
    /// NOT_ALIVE_NO_WRITERS only.
    NotAliveNoWriters,
    /// All.
    Any,
}

/// Parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Unexpected token at position.
    Unexpected {
        /// Char-position in source.
        pos: usize,
        /// Description what was found.
        found: String,
    },
    /// EOF in the middle of an expression.
    UnexpectedEof,
    /// Unbalanced parenthesis.
    UnbalancedParen,
    /// Numeric literal not parsable.
    InvalidNumber(String),
    /// Unknown metadata key (e.g. `xyz_state`).
    UnknownMetadataKey(String),
    /// Unknown metadata value.
    UnknownMetadataValue(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unexpected { pos, found } => {
                write!(f, "unexpected token '{found}' at pos {pos}")
            }
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::UnbalancedParen => f.write_str("unbalanced parenthesis"),
            Self::InvalidNumber(s) => write!(f, "invalid number literal '{s}'"),
            Self::UnknownMetadataKey(s) => write!(f, "unknown metadata key '{s}'"),
            Self::UnknownMetadataValue(s) => write!(f, "unknown metadata value '{s}'"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

/// Parses a sample-selector string per Spec §7.4.8 BNF.
///
/// # Errors
/// `ParseError` on BNF violation.
pub fn parse_sample_selector(src: &str) -> Result<SampleSelector, ParseError> {
    let mut p = Parser::new(src);
    p.skip_whitespace();

    // Spec §7.4.8: a selector can start with a FilterExpression or with
    // a (optionally comma-separated) sequence of
    // MetadataExpressions.
    let filter = if p.peek_metadata_key().is_some() {
        None
    } else {
        Some(p.parse_filter_expression()?)
    };

    let mut metadata = Vec::new();
    loop {
        p.skip_whitespace();
        if p.is_eof() {
            break;
        }
        // Optional `,` between the filter and metadata, or between
        // multiple metadata expressions.
        let _ = p.consume_char(',');
        p.skip_whitespace();
        if p.peek_metadata_key().is_none() {
            break;
        }
        metadata.push(p.parse_metadata_expression()?);
    }

    if !p.is_eof() {
        return Err(ParseError::Unexpected {
            pos: p.pos,
            found: p.peek_token(),
        });
    }
    Ok(SampleSelector { filter, metadata })
}

// ============================================================================
//  Parser (recursive descent).
// ============================================================================

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) -> bool {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
        !self.is_eof()
    }

    fn consume_char(&mut self, c: char) -> bool {
        if self.peek() == Some(c as u8) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn peek_token(&self) -> String {
        let mut end = self.pos;
        while end < self.src.len() && !self.src[end].is_ascii_whitespace() && self.src[end] != b','
        {
            end += 1;
        }
        if end > self.pos {
            String::from_utf8_lossy(&self.src[self.pos..end]).into_owned()
        } else {
            "<eof>".to_string()
        }
    }

    fn parse_filter_expression(&mut self) -> Result<FilterExpression, ParseError> {
        let mut lhs = self.parse_term()?;
        loop {
            self.skip_whitespace();
            let saved = self.pos;
            let op = if self.consume_keyword("AND") {
                BoolOp::And
            } else if self.consume_keyword("OR") {
                BoolOp::Or
            } else {
                self.pos = saved;
                break;
            };
            self.skip_whitespace();
            let rhs = self.parse_term()?;
            lhs = FilterExpression::Boolean {
                op,
                lhs: alloc::boxed::Box::new(lhs),
                rhs: alloc::boxed::Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_term(&mut self) -> Result<FilterExpression, ParseError> {
        self.skip_whitespace();
        if self.consume_char('(') {
            let inner = self.parse_filter_expression()?;
            self.skip_whitespace();
            if !self.consume_char(')') {
                return Err(ParseError::UnbalancedParen);
            }
            return Ok(inner);
        }
        // Atom: FieldRef CompOp Literal
        let field = self.parse_identifier_path()?;
        self.skip_whitespace();
        let op = self.parse_compare_op()?;
        self.skip_whitespace();
        let value = self.parse_literal()?;
        Ok(FilterExpression::Comparison { field, op, value })
    }

    fn parse_identifier_path(&mut self) -> Result<String, ParseError> {
        self.skip_whitespace();
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(ParseError::Unexpected {
                pos: self.pos,
                found: self.peek_token(),
            });
        }
        Ok(String::from_utf8_lossy(&self.src[start..self.pos]).into_owned())
    }

    fn parse_compare_op(&mut self) -> Result<CompareOp, ParseError> {
        let op = match (self.peek(), self.src.get(self.pos + 1).copied()) {
            (Some(b'='), _) => {
                self.pos += 1;
                CompareOp::Eq
            }
            (Some(b'!'), Some(b'=')) => {
                self.pos += 2;
                CompareOp::NotEq
            }
            (Some(b'<'), Some(b'=')) => {
                self.pos += 2;
                CompareOp::Le
            }
            (Some(b'<'), _) => {
                self.pos += 1;
                CompareOp::Lt
            }
            (Some(b'>'), Some(b'=')) => {
                self.pos += 2;
                CompareOp::Ge
            }
            (Some(b'>'), _) => {
                self.pos += 1;
                CompareOp::Gt
            }
            _ => {
                return Err(ParseError::Unexpected {
                    pos: self.pos,
                    found: self.peek_token(),
                });
            }
        };
        Ok(op)
    }

    fn parse_literal(&mut self) -> Result<Literal, ParseError> {
        match self.peek() {
            Some(b'\'') | Some(b'"') => self.parse_string_literal(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number_literal(),
            Some(b) if b.is_ascii_alphabetic() => {
                // boolean
                let saved = self.pos;
                if self.consume_keyword("true") {
                    return Ok(Literal::Bool(true));
                }
                if self.consume_keyword("false") {
                    return Ok(Literal::Bool(false));
                }
                self.pos = saved;
                Err(ParseError::Unexpected {
                    pos: self.pos,
                    found: self.peek_token(),
                })
            }
            _ => Err(ParseError::Unexpected {
                pos: self.pos,
                found: self.peek_token(),
            }),
        }
    }

    fn parse_string_literal(&mut self) -> Result<Literal, ParseError> {
        let quote = self.peek().ok_or(ParseError::UnexpectedEof)?;
        self.pos += 1;
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b == quote {
                let s = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
                self.pos += 1;
                return Ok(Literal::Str(s));
            }
            self.pos += 1;
        }
        Err(ParseError::UnexpectedEof)
    }

    fn parse_number_literal(&mut self) -> Result<Literal, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let raw = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        raw.parse::<i64>()
            .map(Literal::Integer)
            .map_err(|_| ParseError::InvalidNumber(raw))
    }

    fn consume_keyword(&mut self, kw: &str) -> bool {
        let bytes = kw.as_bytes();
        if self.pos + bytes.len() > self.src.len() {
            return false;
        }
        for (i, b) in bytes.iter().enumerate() {
            let actual = self.src[self.pos + i];
            // Match case-insensitive for AND/OR; case-sensitive for true/false
            let matches = if kw.chars().all(|c| c.is_ascii_uppercase()) {
                actual.eq_ignore_ascii_case(b)
            } else {
                actual == *b
            };
            if !matches {
                return false;
            }
        }
        // Boundary check: next char must not be alphanumeric.
        if let Some(after) = self.src.get(self.pos + bytes.len()) {
            if after.is_ascii_alphanumeric() || *after == b'_' {
                return false;
            }
        }
        self.pos += bytes.len();
        true
    }

    fn peek_metadata_key(&self) -> Option<&'static str> {
        for key in ["sample_state", "view_state", "instance_state"] {
            let bytes = key.as_bytes();
            if self.pos + bytes.len() <= self.src.len()
                && &self.src[self.pos..self.pos + bytes.len()] == bytes
            {
                return Some(key);
            }
        }
        None
    }

    fn parse_metadata_expression(&mut self) -> Result<MetadataExpression, ParseError> {
        let key = self
            .peek_metadata_key()
            .ok_or_else(|| ParseError::UnknownMetadataKey(self.peek_token()))?;
        self.pos += key.len();
        self.skip_whitespace();
        if !self.consume_char('=') {
            return Err(ParseError::Unexpected {
                pos: self.pos,
                found: self.peek_token(),
            });
        }
        self.skip_whitespace();
        let val = self.parse_identifier_path()?;
        match key {
            "sample_state" => match val.as_str() {
                "read" => Ok(MetadataExpression::SampleState(SampleStateMatch::Read)),
                "not_read" => Ok(MetadataExpression::SampleState(SampleStateMatch::NotRead)),
                "any" => Ok(MetadataExpression::SampleState(SampleStateMatch::Any)),
                _ => Err(ParseError::UnknownMetadataValue(val)),
            },
            "view_state" => match val.as_str() {
                "new" => Ok(MetadataExpression::ViewState(ViewStateMatch::New)),
                "not_new" => Ok(MetadataExpression::ViewState(ViewStateMatch::NotNew)),
                "any" => Ok(MetadataExpression::ViewState(ViewStateMatch::Any)),
                _ => Err(ParseError::UnknownMetadataValue(val)),
            },
            "instance_state" => match val.as_str() {
                "alive" => Ok(MetadataExpression::InstanceState(InstanceStateMatch::Alive)),
                "not_alive_disposed" => Ok(MetadataExpression::InstanceState(
                    InstanceStateMatch::NotAliveDisposed,
                )),
                "not_alive_no_writers" => Ok(MetadataExpression::InstanceState(
                    InstanceStateMatch::NotAliveNoWriters,
                )),
                "any" => Ok(MetadataExpression::InstanceState(InstanceStateMatch::Any)),
                _ => Err(ParseError::UnknownMetadataValue(val)),
            },
            _ => Err(ParseError::UnknownMetadataKey(key.to_string())),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::unreachable,
    clippy::panic
)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_equality_filter() {
        let s = parse_sample_selector("speed = 42").expect("parse");
        assert!(s.filter.is_some());
        assert!(s.metadata.is_empty());
        if let FilterExpression::Comparison { field, op, value } = s.filter.unwrap() {
            assert_eq!(field, "speed");
            assert_eq!(op, CompareOp::Eq);
            assert_eq!(value, Literal::Integer(42));
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parses_inequality_with_string_literal() {
        let s = parse_sample_selector("name != 'sensor'").expect("parse");
        if let FilterExpression::Comparison { field, op, value } = s.filter.unwrap() {
            assert_eq!(field, "name");
            assert_eq!(op, CompareOp::NotEq);
            assert_eq!(value, Literal::Str("sensor".to_string()));
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parses_dotted_field_path() {
        let s = parse_sample_selector("position.x > 0").expect("parse");
        if let FilterExpression::Comparison { field, .. } = s.filter.unwrap() {
            assert_eq!(field, "position.x");
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parses_and_conjunction() {
        let s = parse_sample_selector("a > 1 AND b < 10").expect("parse");
        if let FilterExpression::Boolean { op, .. } = s.filter.unwrap() {
            assert_eq!(op, BoolOp::And);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parses_or_with_parenthesis() {
        let s = parse_sample_selector("(a = 1) OR (b = 2)").expect("parse");
        if let FilterExpression::Boolean { op, .. } = s.filter.unwrap() {
            assert_eq!(op, BoolOp::Or);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parses_metadata_only_expression() {
        let s = parse_sample_selector("sample_state=read").expect("parse");
        assert!(s.filter.is_none());
        assert_eq!(s.metadata.len(), 1);
        assert_eq!(
            s.metadata[0],
            MetadataExpression::SampleState(SampleStateMatch::Read)
        );
    }

    #[test]
    fn parses_filter_plus_metadata() {
        let s = parse_sample_selector("speed > 5, view_state=new").expect("parse");
        assert!(s.filter.is_some());
        assert_eq!(
            s.metadata,
            alloc::vec![MetadataExpression::ViewState(ViewStateMatch::New)]
        );
    }

    #[test]
    fn parses_all_three_metadata_kinds() {
        let s = parse_sample_selector("sample_state=any, view_state=any, instance_state=alive")
            .expect("parse");
        assert_eq!(s.metadata.len(), 3);
        assert!(matches!(
            s.metadata[2],
            MetadataExpression::InstanceState(InstanceStateMatch::Alive)
        ));
    }

    #[test]
    fn rejects_unknown_metadata_key() {
        let err = parse_sample_selector("xyz_state=read").expect_err("error");
        // "xyz_state" parses as identifier-path until '='; then comparison
        // tries to read literal 'read' which is invalid - that's still an
        // unexpected token at pos>0; either way it's an error.
        assert!(matches!(
            err,
            ParseError::UnknownMetadataKey(_)
                | ParseError::Unexpected { .. }
                | ParseError::UnknownMetadataValue(_)
        ));
    }

    #[test]
    fn rejects_unknown_metadata_value() {
        let err = parse_sample_selector("sample_state=xyz").expect_err("error");
        assert!(matches!(err, ParseError::UnknownMetadataValue(_)));
    }

    #[test]
    fn rejects_unbalanced_parenthesis() {
        let err = parse_sample_selector("(a = 1").expect_err("error");
        assert!(matches!(err, ParseError::UnbalancedParen));
    }

    #[test]
    fn rejects_trailing_garbage() {
        let err = parse_sample_selector("a = 1 garbage").expect_err("error");
        assert!(matches!(err, ParseError::Unexpected { .. }));
    }

    #[test]
    fn comparison_operators_full_coverage() {
        for (src, expected) in [
            ("a = 1", CompareOp::Eq),
            ("a != 1", CompareOp::NotEq),
            ("a < 1", CompareOp::Lt),
            ("a <= 1", CompareOp::Le),
            ("a > 1", CompareOp::Gt),
            ("a >= 1", CompareOp::Ge),
        ] {
            let s = parse_sample_selector(src).expect("parse");
            if let FilterExpression::Comparison { op, .. } = s.filter.unwrap() {
                assert_eq!(op, expected, "for src={src}");
            } else {
                unreachable!("expected Comparison for src={src}");
            }
        }
    }

    #[test]
    fn boolean_literal_supported() {
        let s = parse_sample_selector("active = true").expect("parse");
        if let FilterExpression::Comparison { value, .. } = s.filter.unwrap() {
            assert_eq!(value, Literal::Bool(true));
        } else {
            unreachable!();
        }
    }
}
