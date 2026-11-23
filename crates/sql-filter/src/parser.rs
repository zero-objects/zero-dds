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
//! operand = literal | null | field | '%' DIGITS
//! field   = NAME { '.' NAME } { '[' (DIGITS | STRING) ']' [ '.' field ] }
//! ```

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::{CmpOp, Expr, Operand, Value};
use crate::lexer::{LexError, Token, tokenize_with};

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

/// The grammar an expression is parsed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Grammar {
    /// The filter expressions of DDS 1.4 Annex B with the additions of
    /// DDS-XTypes 1.3 §7.6.7 (subscripts, `#discriminator`, `null`) and
    /// `LIKE`, `!=`, `TRUE`, `FALSE`.
    #[default]
    Dcps14,
    /// DDS 1.2 Annex B, the syntax of DLRL queries and filters: a
    /// comparison has a FIELDNAME on at least one side, `BETWEEN` and
    /// `NOT BETWEEN` take a FIELDNAME and a Range of two Parameters, the
    /// RelOps are `=`, `>`, `>=`, `<`, `<=`, `<>`, a PARAMETER is `%0` to
    /// `%99`; `LIKE`, `!=`, the boolean literals, `null` and
    /// `#discriminator` are not part of it.
    Dlrl12,
}

/// Parse entry point. The entire input must be part of exactly one
/// expression — leftover strings are treated as errors.
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    parse_with(input, Grammar::Dcps14)
}

/// [`parse`] with the given [`Grammar`].
///
/// # Errors
/// [`ParseError`] when `input` is not one expression of `grammar`, and
/// when it nests deeper than 64 levels (`MAX_EXPR_DEPTH`).
pub fn parse_with(input: &str, grammar: Grammar) -> Result<Expr, ParseError> {
    let tokens = tokenize_with(input, grammar)?;
    let mut p = Parser { tokens, pos: 0 };
    let (expr, _) = p.parse_or(0)?;
    if p.pos != p.tokens.len() {
        return Err(ParseError(alloc::format!(
            "unexpected trailing token at position {}",
            p.pos
        )));
    }
    if grammar == Grammar::Dlrl12 {
        check_dlrl12(&expr)?;
    }
    Ok(expr)
}

/// The productions of DDS 1.2 Annex B B.1 that the shared parser
/// accepts more widely: ComparisonPredicate, BetweenPredicate, Range and
/// PARAMETER (B.2: "a natural number (zero included) smaller than 100").
///
/// zerodds-lint: recursion-depth 64 — one call per AND, OR and NOT of a
/// parsed expression; the parser refuses one that nests deeper than
/// `MAX_EXPR_DEPTH` (64).
fn check_dlrl12(expr: &Expr) -> Result<(), ParseError> {
    fn param(op: &Operand) -> Result<(), ParseError> {
        match op {
            Operand::Param(n) if *n >= 100 => Err(ParseError(alloc::format!(
                "PARAMETER %{n} is not smaller than 100 (DDS 1.2 Annex B)"
            ))),
            _ => Ok(()),
        }
    }
    match expr {
        Expr::And(a, b) | Expr::Or(a, b) => {
            check_dlrl12(a)?;
            check_dlrl12(b)
        }
        Expr::Not(inner) => check_dlrl12(inner),
        Expr::Cmp { lhs, rhs, .. } => {
            if !matches!(lhs, Operand::Field(_)) && !matches!(rhs, Operand::Field(_)) {
                return Err(ParseError(
                    "a ComparisonPredicate needs a FIELDNAME on one side (DDS 1.2 Annex B)".into(),
                ));
            }
            param(lhs)?;
            param(rhs)
        }
        Expr::Between {
            field, low, high, ..
        } => {
            if !matches!(field, Operand::Field(_)) {
                return Err(ParseError(
                    "a BetweenPredicate starts with a FIELDNAME (DDS 1.2 Annex B)".into(),
                ));
            }
            if matches!(low, Operand::Field(_)) || matches!(high, Operand::Field(_)) {
                return Err(ParseError(
                    "the Range of a BetweenPredicate is two Parameters (DDS 1.2 Annex B)".into(),
                ));
            }
            param(low)?;
            param(high)
        }
    }
}

/// The deepest an expression nests. Every AND, OR and NOT and every pair
/// of parentheses is one level, and no comparison sits more than this
/// many levels below the whole expression; a deeper one is refused with a
/// [`ParseError`]. The parser descends once per parenthesis and NOT, and
/// every walk over an [`Expr`] once per AND, OR and NOT: without a bound
/// the filter expression of a remote reader (the `ContentFilterProperty_t`
/// of its discovery data, DDSI-RTPS 2.5 §9.3.2.1) would exhaust the stack
/// of the participant that parses it. An AND or OR chain nests one level
/// per operator, `a = 1 OR b = 2 OR c = 3` two.
const MAX_EXPR_DEPTH: usize = 64;

/// One level below `level`, refused past [`MAX_EXPR_DEPTH`].
fn deeper(level: usize) -> Result<usize, ParseError> {
    let next = level + 1;
    if next > MAX_EXPR_DEPTH {
        return Err(ParseError(alloc::format!(
            "the expression nests deeper than {MAX_EXPR_DEPTH} levels"
        )));
    }
    Ok(next)
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

    // Each `parse_*` reads at `level` levels below the whole expression
    // and returns the expression with the levels it nests itself (0 for a
    // comparison); `level` plus those stays within `MAX_EXPR_DEPTH`.

    fn parse_or(&mut self, level: usize) -> Result<(Expr, usize), ParseError> {
        let (mut left, mut levels) = self.parse_and(level)?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.pos += 1;
            let (right, right_levels) = self.parse_and(level)?;
            levels = deeper(level + levels.max(right_levels))? - level;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok((left, levels))
    }

    fn parse_and(&mut self, level: usize) -> Result<(Expr, usize), ParseError> {
        let (mut left, mut levels) = self.parse_not(level)?;
        while matches!(self.peek(), Some(Token::And)) {
            self.pos += 1;
            let (right, right_levels) = self.parse_not(level)?;
            levels = deeper(level + levels.max(right_levels))? - level;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok((left, levels))
    }

    fn parse_not(&mut self, level: usize) -> Result<(Expr, usize), ParseError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.pos += 1;
            let (inner, levels) = self.parse_not(deeper(level)?)?;
            return Ok((Expr::Not(Box::new(inner)), levels + 1));
        }
        self.parse_atom(level)
    }

    fn parse_atom(&mut self, level: usize) -> Result<(Expr, usize), ParseError> {
        if matches!(self.peek(), Some(Token::LParen)) {
            self.pos += 1;
            let (inner, levels) = self.parse_or(deeper(level)?)?;
            if !matches!(self.peek(), Some(Token::RParen)) {
                return Err(ParseError("expected ')'".into()));
            }
            self.pos += 1;
            return Ok((inner, levels + 1));
        }
        Ok((self.parse_cmp()?, 0))
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
            Token::Null => Ok(Operand::Literal(Value::Null)),
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

    /// DDS 1.2 Annex B (DLRL): what the grammar refuses that the DDS 1.4
    /// grammar takes, and what both take.
    #[test]
    fn dlrl_b_grammar_dlrl12_refuses_what_annex_b_does_not_have() {
        for bad in [
            "a LIKE 'x%'",
            "a != 1",
            "1 = 2",
            "%0 < %1",
            "a = %100",
            "a BETWEEN b AND 3",
            "%0 BETWEEN 1 AND 2",
            "#discriminator = 1",
            "s = 'O''Brien'",
            // The key in brackets is the same STRING: no '' escape, no
            // new-line, ended by a right quote.
            "m['it''s'] = 1",
            "m['a\nb'] = 1",
            "m[`a`] = 1",
            "m['a] = 1",
        ] {
            assert!(parse_with(bad, Grammar::Dlrl12).is_err(), "{bad}");
        }
        for good in [
            "a <> 1",
            "%0 >= a",
            "a.b = c[2].d",
            "a NOT BETWEEN %0 AND %99",
            "NOT (a = 'x' OR b < -0x10) AND c = `y'",
            "m[-3] = 1.5e-2",
            "m[0x10] = 'z'",
            "TRUE = 1",
        ] {
            assert!(parse_with(good, Grammar::Dlrl12).is_ok(), "{good}");
        }
        // TRUE is a FIELDNAME there, a boolean literal in DDS 1.4.
        assert_eq!(
            parse_with("TRUE = 1", Grammar::Dlrl12).unwrap(),
            Expr::Cmp {
                lhs: Operand::Field("TRUE".into()),
                op: CmpOp::Eq,
                rhs: Operand::Literal(Value::Int(1)),
            }
        );
        assert_eq!(
            parse_with("m[-0x1F] = 1", Grammar::Dlrl12).unwrap(),
            Expr::Cmp {
                lhs: Operand::Field("m[-31]".into()),
                op: CmpOp::Eq,
                rhs: Operand::Literal(Value::Int(1)),
            }
        );
        // A key opened by a left quote, as a STRING outside the brackets.
        assert_eq!(
            parse_with("m[`k`'] = `v'", Grammar::Dlrl12).unwrap(),
            Expr::Cmp {
                lhs: Operand::Field("m['k`']".into()),
                op: CmpOp::Eq,
                rhs: Operand::Literal(Value::String("v".into())),
            }
        );
        assert_eq!(
            parse_with("m[ 'k' ].n = 1", Grammar::Dlrl12).unwrap(),
            Expr::Cmp {
                lhs: Operand::Field("m['k'].n".into()),
                op: CmpOp::Eq,
                rhs: Operand::Literal(Value::Int(1)),
            }
        );
        // The DDS 1.4 grammar is unchanged.
        assert!(parse("a LIKE 'x%' AND b != 1 AND s = 'O''Brien'").is_ok());
        assert!(parse("m[-3] = 1").is_err());
        assert!(parse("m['it''s'] = 1").is_ok());
    }

    /// An expression nests at most `MAX_EXPR_DEPTH` levels — parentheses,
    /// NOT, and AND/OR operators of a chain — and one deeper is a
    /// `ParseError`, not a stack overflow: a filter expression can come
    /// from a remote reader.
    #[test]
    fn parse_refuses_an_expression_nested_deeper_than_max_expr_depth() {
        let parens = |n: usize| alloc::format!("{}a = 1{}", "(".repeat(n), ")".repeat(n));
        assert!(parse(&parens(MAX_EXPR_DEPTH)).is_ok());
        assert!(parse(&parens(MAX_EXPR_DEPTH + 1)).is_err());
        assert!(parse(&parens(200_000)).is_err());

        let nots = |n: usize| alloc::format!("{}a = 1", "NOT ".repeat(n));
        assert!(parse(&nots(MAX_EXPR_DEPTH)).is_ok());
        assert!(parse(&nots(MAX_EXPR_DEPTH + 1)).is_err());
        assert!(parse(&nots(200_000)).is_err());

        // n comparisons with OR nest n - 1 levels.
        let chain = |n: usize, op: &str| alloc::vec!["a = 1"; n].join(op);
        for op in [" OR ", " AND "] {
            assert!(parse(&chain(MAX_EXPR_DEPTH + 1, op)).is_ok(), "{op}");
            assert!(parse(&chain(MAX_EXPR_DEPTH + 2, op)).is_err(), "{op}");
            assert!(parse(&chain(50_000, op)).is_err(), "{op}");
        }

        // The levels add up across parentheses, NOT and chains.
        let mixed = alloc::format!("NOT ({})", chain(MAX_EXPR_DEPTH - 1, " OR "));
        assert!(parse(&mixed).is_ok());
        let mixed = alloc::format!("NOT ({})", chain(MAX_EXPR_DEPTH, " OR "));
        assert!(parse(&mixed).is_err());
    }
}
