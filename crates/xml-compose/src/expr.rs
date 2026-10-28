// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `<const_exp>` of the IDL grammar, as an XML attribute writes it.
//!
//! DDS-XTypes 1.3 §7.3.2.1.3: "the string specified in the value
//! attribute [...] shall have the same syntax as the <const_exp>
//! production in the IDL grammar [IDL]". §7.3.2.4 says the same of
//! every collection bound (`stringMaxLength="2 + MY_CONSTANT"`,
//! `sequenceMaxLength="6 * 3"`, `arrayDimensions="2, MY_CONSTANT + 3"`).
//!
//! This is IDL 4.2 §7.4.1.4.3's expression grammar, with the precedence
//! of §7.4.1.4.2:
//!
//! ```text
//! const_exp   ::= or_expr
//! or_expr     ::= xor_expr  { "|"  xor_expr }
//! xor_expr    ::= and_expr  { "^"  and_expr }
//! and_expr    ::= shift_expr { "&" shift_expr }
//! shift_expr  ::= add_expr  { ("<<" | ">>") add_expr }
//! add_expr    ::= mult_expr { ("+" | "-") mult_expr }
//! mult_expr   ::= unary_expr { ("*" | "/" | "%") unary_expr }
//! unary_expr  ::= ("-" | "+" | "~") primary_expr | primary_expr
//! primary_expr::= scoped_name | literal | "(" const_exp ")"
//! ```
//!
//! The parser only reads the text. It builds a small tree
//! ([`Expression`]) and the lowering turns that into the model's
//! expression nodes, resolving the names against the scopes of the
//! document; nothing here knows a model.

use zerodds_ast::{BinaryOp, UnaryOp};

/// A parsed `<const_exp>`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Expression {
    /// An integer literal in its evaluated form.
    Integer(i128),
    /// A floating-point literal, as written.
    Float {
        /// The mantissa, including a trailing point.
        mantissa: String,
        /// The exponent as written, without the `e`.
        exponent: Option<String>,
    },
    /// A string literal with its escapes resolved.
    String(String),
    /// A character literal.
    Char(char),
    /// `TRUE` or `FALSE`.
    Boolean(bool),
    /// A scoped name.
    Name {
        /// Whether it starts with `::`.
        absolute: bool,
        /// Its segments.
        segments: Vec<String>,
    },
    /// A unary operator.
    Unary(UnaryOp, Box<Expression>),
    /// A binary operator.
    Binary(BinaryOp, Box<Expression>, Box<Expression>),
}

/// Parse one `<const_exp>`.
///
/// # Errors
/// A sentence naming what was met where the grammar wanted something
/// else, and at which character of the text.
pub(crate) fn parse(text: &str) -> Result<Expression, String> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        at: 0,
        depth: 0,
    };
    let expression = parser.or_expr()?;
    parser.skip_space();
    match parser.peek() {
        None => Ok(expression),
        Some(found) => Err(format!(
            "`{found}` at character {} follows a complete expression",
            parser.at + 1
        )),
    }
}

/// Parse a comma-delimited list of `<const_exp>`, as `arrayDimensions`
/// writes one: "Whitespace is allowed around each bound and is not
/// significant" (§7.3.2.4.1).
///
/// The comma is the list's separator and no operator of the grammar, so
/// the split is at every comma that is not inside parentheses or a
/// literal.
///
/// # Errors
/// As [`parse`], for the first bound that does not parse, and for a list
/// with nothing in it.
pub(crate) fn parse_list(text: &str) -> Result<Vec<Expression>, String> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut quote: Option<char> = None;
    let mut start = 0;
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        match quote {
            Some(open) => {
                if c == '\\' {
                    index += 1;
                } else if c == open {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    parts.push(chars[start..index].iter().collect::<String>());
                    start = index + 1;
                }
                _ => {}
            },
        }
        index += 1;
    }
    parts.push(chars[start..].iter().collect::<String>());
    if parts.iter().all(|part| part.trim().is_empty()) {
        return Err(String::from("the list names no bound"));
    }
    parts
        .iter()
        .filter(|part| !part.trim().is_empty())
        .map(|part| parse(part))
        .collect()
}

/// The deepest nesting of parentheses and unary operators read.
///
/// A document is input from elsewhere; a parser that recurses once per
/// `(` ends a process with a stack overflow on a few thousand of them.
const MAX_DEPTH: usize = 64;

struct Parser {
    chars: Vec<char>,
    at: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.at + offset).copied()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_space();
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// `op` is eaten only when it is not the first half of a longer
    /// operator: `<` is no operator of the grammar, `<<` is.
    fn eat_pair(&mut self, first: char, second: char) -> bool {
        self.skip_space();
        if self.peek() == Some(first) && self.peek_at(1) == Some(second) {
            self.at += 2;
            true
        } else {
            false
        }
    }

    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(format!(
                "the expression nests deeper than {MAX_DEPTH} levels"
            ));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn or_expr(&mut self) -> Result<Expression, String> {
        let mut left = self.xor_expr()?;
        loop {
            self.skip_space();
            if self.peek() == Some('|') {
                self.at += 1;
                let right = self.xor_expr()?;
                left = Expression::Binary(BinaryOp::Or, Box::new(left), Box::new(right));
            } else {
                return Ok(left);
            }
        }
    }

    fn xor_expr(&mut self) -> Result<Expression, String> {
        let mut left = self.and_expr()?;
        while self.eat('^') {
            let right = self.and_expr()?;
            left = Expression::Binary(BinaryOp::Xor, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and_expr(&mut self) -> Result<Expression, String> {
        let mut left = self.shift_expr()?;
        while self.eat('&') {
            let right = self.shift_expr()?;
            left = Expression::Binary(BinaryOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn shift_expr(&mut self) -> Result<Expression, String> {
        let mut left = self.add_expr()?;
        loop {
            let op = if self.eat_pair('<', '<') {
                BinaryOp::Shl
            } else if self.eat_pair('>', '>') {
                BinaryOp::Shr
            } else {
                return Ok(left);
            };
            let right = self.add_expr()?;
            left = Expression::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn add_expr(&mut self) -> Result<Expression, String> {
        let mut left = self.mult_expr()?;
        loop {
            let op = if self.eat('+') {
                BinaryOp::Add
            } else if self.eat('-') {
                BinaryOp::Sub
            } else {
                return Ok(left);
            };
            let right = self.mult_expr()?;
            left = Expression::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn mult_expr(&mut self) -> Result<Expression, String> {
        let mut left = self.unary_expr()?;
        loop {
            let op = if self.eat('*') {
                BinaryOp::Mul
            } else if self.eat('/') {
                BinaryOp::Div
            } else if self.eat('%') {
                BinaryOp::Mod
            } else {
                return Ok(left);
            };
            let right = self.unary_expr()?;
            left = Expression::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn unary_expr(&mut self) -> Result<Expression, String> {
        self.skip_space();
        let op = match self.peek() {
            Some('-') => Some(UnaryOp::Minus),
            Some('+') => Some(UnaryOp::Plus),
            Some('~') => Some(UnaryOp::Not),
            _ => None,
        };
        let Some(op) = op else {
            return self.primary_expr();
        };
        self.at += 1;
        self.enter()?;
        let operand = self.primary_expr();
        self.leave();
        Ok(Expression::Unary(op, Box::new(operand?)))
    }

    fn primary_expr(&mut self) -> Result<Expression, String> {
        self.skip_space();
        match self.peek() {
            None => Err(String::from("the expression ends where an operand is due")),
            Some('(') => {
                self.at += 1;
                self.enter()?;
                let inner = self.or_expr();
                self.leave();
                let inner = inner?;
                if self.eat(')') {
                    Ok(inner)
                } else {
                    Err(format!(
                        "`)` is due at character {}, which closes the `(`",
                        self.at + 1
                    ))
                }
            }
            Some('"') => self.string_literal(),
            Some('\'') => self.char_literal(),
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            Some(c) if c.is_alphabetic() || c == '_' || c == ':' => self.name(),
            Some(c) => Err(format!(
                "`{c}` at character {} starts no operand",
                self.at + 1
            )),
        }
    }

    fn name(&mut self) -> Result<Expression, String> {
        let absolute = self.peek() == Some(':');
        if absolute {
            if self.peek_at(1) != Some(':') {
                return Err(format!(
                    "a single `:` at character {} is no part of a name",
                    self.at + 1
                ));
            }
            self.at += 2;
        }
        let mut segments = Vec::new();
        loop {
            let start = self.at;
            while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                self.at += 1;
            }
            if self.at == start {
                return Err(format!(
                    "a name segment is due at character {}",
                    self.at + 1
                ));
            }
            segments.push(self.chars[start..self.at].iter().collect::<String>());
            if self.peek() == Some(':') && self.peek_at(1) == Some(':') {
                self.at += 2;
            } else {
                break;
            }
        }
        if !absolute && segments.len() == 1 {
            match segments[0].as_str() {
                "TRUE" | "true" => return Ok(Expression::Boolean(true)),
                "FALSE" | "false" => return Ok(Expression::Boolean(false)),
                _ => {}
            }
        }
        Ok(Expression::Name { absolute, segments })
    }

    /// An integer (decimal, `0x` hexadecimal, `0` octal) or a floating
    /// literal. A fixed-point literal (`1.5d`) is not read: the model's
    /// bounds and constants of this loader are integers, strings and
    /// floats.
    fn number(&mut self) -> Result<Expression, String> {
        let start = self.at;
        let hexadecimal = self.peek() == Some('0') && matches!(self.peek_at(1), Some('x' | 'X'));
        if hexadecimal {
            self.at += 2;
            let digits = self.at;
            while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                self.at += 1;
            }
            if self.at == digits {
                return Err(format!(
                    "a hexadecimal digit is due at character {}",
                    self.at + 1
                ));
            }
            let text: String = self.chars[digits..self.at].iter().collect();
            return i128::from_str_radix(&text, 16)
                .map(Expression::Integer)
                .map_err(|_| format!("`0x{text}` does not fit the range of an integer constant"));
        }
        let mut is_float = false;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        if self.peek() == Some('.') {
            is_float = true;
            self.at += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.at += 1;
            }
        }
        let mantissa: String = self.chars[start..self.at].iter().collect();
        let mut exponent = None;
        if matches!(self.peek(), Some('e' | 'E')) {
            let before = self.at;
            self.at += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.at += 1;
            }
            let digits = self.at;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.at += 1;
            }
            if self.at == digits {
                self.at = before;
                return Err(format!(
                    "an exponent digit is due at character {}",
                    digits + 1
                ));
            }
            is_float = true;
            exponent = Some(self.chars[before + 1..self.at].iter().collect::<String>());
        }
        if matches!(self.peek(), Some('d' | 'D')) {
            return Err(format!(
                "the fixed-point literal at character {} is not read",
                start + 1
            ));
        }
        if is_float {
            return Ok(Expression::Float { mantissa, exponent });
        }
        // IDL 4.2 §7.4.1.4.3: a leading zero makes an integer octal.
        let radix = if mantissa.len() > 1 && mantissa.starts_with('0') {
            8
        } else {
            10
        };
        i128::from_str_radix(&mantissa, radix)
            .map(Expression::Integer)
            .map_err(|_| format!("`{mantissa}` is not an integer constant"))
    }

    fn string_literal(&mut self) -> Result<Expression, String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(String::from("a string literal is not closed")),
                Some('"') => {
                    self.at += 1;
                    return Ok(Expression::String(out));
                }
                Some('\\') => {
                    self.at += 1;
                    out.push(self.escape()?);
                }
                Some(c) => {
                    self.at += 1;
                    out.push(c);
                }
            }
        }
    }

    fn char_literal(&mut self) -> Result<Expression, String> {
        self.at += 1;
        let c = match self.peek() {
            None => return Err(String::from("a character literal is not closed")),
            Some('\\') => {
                self.at += 1;
                self.escape()?
            }
            Some(c) => {
                self.at += 1;
                c
            }
        };
        if self.peek() == Some('\'') {
            self.at += 1;
            Ok(Expression::Char(c))
        } else {
            Err(format!(
                "`'` is due at character {}, which closes the character literal",
                self.at + 1
            ))
        }
    }

    /// One escape after its backslash (IDL 4.2 §7.2.6.3, Table 7-9).
    fn escape(&mut self) -> Result<char, String> {
        let Some(c) = self.peek() else {
            return Err(String::from("a backslash ends the literal"));
        };
        self.at += 1;
        Ok(match c {
            'n' => '\n',
            't' => '\t',
            'v' => '\u{0b}',
            'b' => '\u{08}',
            'r' => '\r',
            'f' => '\u{0c}',
            'a' => '\u{07}',
            '\\' => '\\',
            '?' => '?',
            '\'' => '\'',
            '"' => '"',
            'x' => {
                let start = self.at;
                while self.at - start < 2 && self.peek().is_some_and(|d| d.is_ascii_hexdigit()) {
                    self.at += 1;
                }
                let text: String = self.chars[start..self.at].iter().collect();
                u32::from_str_radix(&text, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| String::from("`\\x` is followed by no hexadecimal digit"))?
            }
            d if ('0'..='7').contains(&d) => {
                let start = self.at - 1;
                while self.at - start < 3 && self.peek().is_some_and(|o| ('0'..='7').contains(&o)) {
                    self.at += 1;
                }
                let text: String = self.chars[start..self.at].iter().collect();
                u32::from_str_radix(&text, 8)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| String::from("an octal escape names no character"))?
            }
            other => return Err(format!("`\\{other}` is no escape sequence")),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn int(value: i128) -> Box<Expression> {
        Box::new(Expression::Integer(value))
    }

    #[test]
    fn the_examples_of_the_specification_parse() {
        assert_eq!(
            parse("2 + 3").unwrap(),
            Expression::Binary(BinaryOp::Add, int(2), int(3))
        );
        assert_eq!(
            parse("6 * 3").unwrap(),
            Expression::Binary(BinaryOp::Mul, int(6), int(3))
        );
        assert_eq!(
            parse("2 + MY_CONSTANT").unwrap(),
            Expression::Binary(
                BinaryOp::Add,
                int(2),
                Box::new(Expression::Name {
                    absolute: false,
                    segments: vec![String::from("MY_CONSTANT")]
                })
            )
        );
        assert_eq!(
            parse("-1").unwrap(),
            Expression::Unary(UnaryOp::Minus, int(1))
        );
    }

    #[test]
    fn the_precedence_is_that_of_idl() {
        // `1 + 2 * 3` is `1 + (2 * 3)`; `1 | 2 ^ 3 & 4` is `1 | (2 ^ (3 & 4))`;
        // `1 << 2 + 1` is `1 << (2 + 1)`.
        assert_eq!(
            parse("1 + 2 * 3").unwrap(),
            Expression::Binary(
                BinaryOp::Add,
                int(1),
                Box::new(Expression::Binary(BinaryOp::Mul, int(2), int(3)))
            )
        );
        assert_eq!(
            parse("1 | 2 ^ 3 & 4").unwrap(),
            Expression::Binary(
                BinaryOp::Or,
                int(1),
                Box::new(Expression::Binary(
                    BinaryOp::Xor,
                    int(2),
                    Box::new(Expression::Binary(BinaryOp::And, int(3), int(4)))
                ))
            )
        );
        assert_eq!(
            parse("1 << 2 + 1").unwrap(),
            Expression::Binary(
                BinaryOp::Shl,
                int(1),
                Box::new(Expression::Binary(BinaryOp::Add, int(2), int(1)))
            )
        );
        assert_eq!(
            parse("(1 + 2) * 3").unwrap(),
            Expression::Binary(
                BinaryOp::Mul,
                Box::new(Expression::Binary(BinaryOp::Add, int(1), int(2))),
                int(3)
            )
        );
    }

    #[test]
    fn literals_are_read_as_idl_reads_them() {
        assert_eq!(parse("0x1F").unwrap(), Expression::Integer(31));
        assert_eq!(parse("010").unwrap(), Expression::Integer(8));
        assert_eq!(parse("0").unwrap(), Expression::Integer(0));
        assert_eq!(
            parse("1.5e3").unwrap(),
            Expression::Float {
                mantissa: String::from("1.5"),
                exponent: Some(String::from("3"))
            }
        );
        assert_eq!(
            parse("\"a\\n\\x41\"").unwrap(),
            Expression::String(String::from("a\nA"))
        );
        assert_eq!(parse("'x'").unwrap(), Expression::Char('x'));
        assert_eq!(parse("TRUE").unwrap(), Expression::Boolean(true));
        assert_eq!(
            parse("::a::B").unwrap(),
            Expression::Name {
                absolute: true,
                segments: vec![String::from("a"), String::from("B")]
            }
        );
    }

    #[test]
    fn a_list_splits_at_the_commas_outside_parentheses_and_literals() {
        let list = parse_list(" 2, MY_CONSTANT + 3 ").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(parse_list(" 6,2, 3 ").unwrap().len(), 3);
        assert!(
            parse_list("(1,2)").is_err(),
            "a comma inside parentheses is no separator"
        );
        assert!(parse_list("  ").is_err());
    }

    #[test]
    fn what_is_not_an_expression_is_refused_with_a_position() {
        for bad in [
            "", "2 +", "(1", "1 2", "1 + * 2", "0x", "\"open", "1.5d", "@",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        let nested = "(".repeat(1000);
        assert!(parse(&nested).is_err(), "the nesting is bounded");
    }
}
