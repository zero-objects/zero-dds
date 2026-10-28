// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value of an `#if` or `#elif`, ISO/IEC 14882:2003 §16.1.
//!
//! What reaches here has had `defined` answered and every macro
//! replaced. §16.1/4: "After all replacements due to macro expansion and
//! the defined unary operator have been performed, all remaining
//! identifiers and keywords, except for true and false, are replaced
//! with the pp-number 0, and then each preprocessing token is converted
//! into a token. The resulting tokens comprise the controlling constant
//! expression", evaluated in `long`/`unsigned long` (here 64 bits) with
//! the usual arithmetic conversions.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A value of the controlling expression: 64 bits, and whether the usual
/// arithmetic conversions made it unsigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Number {
    bits: u64,
    unsigned: bool,
}

impl Number {
    const fn signed(value: i64) -> Self {
        Self {
            bits: value.cast_unsigned(),
            unsigned: false,
        }
    }

    const fn truth(value: bool) -> Self {
        Self::signed(if value { 1 } else { 0 })
    }

    const fn is_true(self) -> bool {
        self.bits != 0
    }

    const fn as_signed(self) -> i64 {
        self.bits.cast_signed()
    }
}

/// The outcome of one condition: its truth, and the identifiers §16.1/4
/// replaced with 0, which the caller reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub value: bool,
    pub zeroed: Vec<String>,
}

/// Evaluate the tokens of an `#if` line.
///
/// # Errors
/// What makes the line no constant expression, in one sentence.
pub(crate) fn evaluate(tokens: &[String]) -> Result<Outcome, String> {
    if tokens.is_empty() {
        return Err("`#if` with no expression".to_string());
    }
    let mut parser = Parser {
        tokens,
        at: 0,
        zeroed: Vec::new(),
    };
    let value = parser.conditional(true)?;
    if let Some(extra) = tokens.get(parser.at) {
        return Err(format!("`{extra}` after the end of the expression"));
    }
    Ok(Outcome {
        value: value.is_true(),
        zeroed: parser.zeroed,
    })
}

struct Parser<'a> {
    tokens: &'a [String],
    at: usize,
    zeroed: Vec<String>,
}

/// zerodds-lint: recursion-depth 256 (parenthesis and unary nesting of
/// one `#if` line). Each descent consumes at least one token before it
/// recurses, and the line is bounded by the directive's length.
impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.at).map(String::as_str)
    }

    fn eat(&mut self, text: &str) -> bool {
        if self.peek() == Some(text) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// `a ? b : c`. `live` is false in an operand the operators around
    /// it do not evaluate, where a division by zero is no error.
    ///
    /// zerodds-lint: recursion-depth 256 (expression nesting of one
    /// `#if` line; each level consumes a token).
    fn conditional(&mut self, live: bool) -> Result<Number, String> {
        let test = self.binary(0, live)?;
        if !self.eat("?") {
            return Ok(test);
        }
        let taken = test.is_true();
        let yes = self.conditional(live && taken)?;
        if !self.eat(":") {
            return Err("`?` without its `:`".to_string());
        }
        let no = self.conditional(live && !taken)?;
        let unsigned = yes.unsigned || no.unsigned;
        let chosen = if taken { yes } else { no };
        Ok(Number {
            bits: chosen.bits,
            unsigned,
        })
    }

    /// Precedence climbing over the binary operators, `||` loosest.
    ///
    /// zerodds-lint: recursion-depth 256 (expression nesting of one
    /// `#if` line; each level consumes a token).
    fn binary(&mut self, level: usize, live: bool) -> Result<Number, String> {
        const LEVELS: [&[&str]; 10] = [
            &["||"],
            &["&&"],
            &["|"],
            &["^"],
            &["&"],
            &["==", "!="],
            &["<", ">", "<=", ">="],
            &["<<", ">>"],
            &["+", "-"],
            &["*", "/", "%"],
        ];
        let Some(operators) = LEVELS.get(level) else {
            return self.unary(live);
        };
        let mut left = self.binary(level + 1, live)?;
        while let Some(operator) = self.peek().filter(|token| operators.contains(token)) {
            let operator = operator.to_string();
            self.at += 1;
            // `||` and `&&` do not evaluate their right operand when the
            // left one decides, §5.14/§5.15.
            let right_live = match operator.as_str() {
                "||" => live && !left.is_true(),
                "&&" => live && left.is_true(),
                _ => live,
            };
            let right = self.binary(level + 1, right_live)?;
            left = apply(&operator, left, right, right_live)?;
        }
        Ok(left)
    }

    /// zerodds-lint: recursion-depth 256 (unary operator nesting of one
    /// `#if` line; each level consumes a token).
    fn unary(&mut self, live: bool) -> Result<Number, String> {
        let Some(token) = self.peek() else {
            return Err("the expression ends where an operand was expected".to_string());
        };
        match token {
            "+" | "-" | "~" | "!" => {
                let operator = token.to_string();
                self.at += 1;
                let operand = self.unary(live)?;
                Ok(match operator.as_str() {
                    "+" => operand,
                    "-" => Number {
                        bits: operand.bits.wrapping_neg(),
                        unsigned: operand.unsigned,
                    },
                    "~" => Number {
                        bits: !operand.bits,
                        unsigned: operand.unsigned,
                    },
                    _ => Number::truth(!operand.is_true()),
                })
            }
            "(" => {
                self.at += 1;
                let inner = self.conditional(live)?;
                if !self.eat(")") {
                    return Err("`(` without its `)`".to_string());
                }
                Ok(inner)
            }
            _ => {
                let text = token.to_string();
                self.at += 1;
                primary(&text, &mut self.zeroed)
            }
        }
    }
}

/// One operand: a number, a character, `true`, `false`, or an
/// identifier §16.1/4 replaces with 0.
fn primary(text: &str, zeroed: &mut Vec<String>) -> Result<Number, String> {
    match text {
        "true" => return Ok(Number::truth(true)),
        "false" => return Ok(Number::truth(false)),
        _ => {}
    }
    let first = text.as_bytes().first().copied().unwrap_or(0);
    if first.is_ascii_digit() {
        return integer(text);
    }
    if first == b'\'' || text.starts_with("L'") {
        return character(text);
    }
    if super::pptoken::is_identifier(text) {
        zeroed.push(text.to_string());
        return Ok(Number::signed(0));
    }
    Err(format!(
        "`{text}` is not an operand of a constant expression"
    ))
}

/// An integer literal, §2.13.1: decimal, octal or hexadecimal, with `u`
/// and `l` suffixes in any order.
fn integer(text: &str) -> Result<Number, String> {
    let digits = text.trim_end_matches(['u', 'U', 'l', 'L']);
    let suffix = &text[digits.len()..];
    let unsigned_suffix = suffix.contains(['u', 'U']);
    let (radix, body) = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, hex)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (8, &digits[1..])
    } else {
        (10, digits)
    };
    let value = u64::from_str_radix(body, radix)
        .map_err(|_| format!("`{text}` is not an integer a constant expression can hold"))?;
    Ok(Number {
        bits: value,
        // A literal past `long`'s range is `unsigned long`, §2.13.1/2.
        unsigned: unsigned_suffix || value > i64::MAX.cast_unsigned(),
    })
}

/// A character literal, §2.13.2: its value is the character's code.
fn character(text: &str) -> Result<Number, String> {
    let inner = text
        .trim_start_matches('L')
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .ok_or_else(|| format!("`{text}` is not a character literal"))?;
    let mut chars = inner.chars();
    let value = match chars.next() {
        Some('\\') => {
            let escape: String = chars.collect();
            escape_value(&escape).ok_or_else(|| format!("`{text}` has no value"))?
        }
        Some(only) if chars.next().is_none() => u64::from(u32::from(only)),
        _ => return Err(format!("`{text}` is not one character")),
    };
    Ok(Number {
        bits: value,
        unsigned: false,
    })
}

fn escape_value(escape: &str) -> Option<u64> {
    let simple = match escape {
        "n" => Some(10),
        "t" => Some(9),
        "v" => Some(11),
        "b" => Some(8),
        "r" => Some(13),
        "f" => Some(12),
        "a" => Some(7),
        "\\" => Some(u64::from(b'\\')),
        "?" => Some(u64::from(b'?')),
        "'" => Some(u64::from(b'\'')),
        "\"" => Some(u64::from(b'"')),
        _ => None,
    };
    if simple.is_some() {
        return simple;
    }
    if let Some(hex) = escape.strip_prefix('x') {
        return u64::from_str_radix(hex, 16).ok();
    }
    if !escape.is_empty() && escape.len() <= 3 && escape.bytes().all(|b| (b'0'..=b'7').contains(&b))
    {
        return u64::from_str_radix(escape, 8).ok();
    }
    None
}

/// One binary operation under the usual arithmetic conversions.
fn apply(operator: &str, left: Number, right: Number, live: bool) -> Result<Number, String> {
    let unsigned = left.unsigned || right.unsigned;
    let arithmetic = |bits: u64| Number { bits, unsigned };
    let compare = |ordering: core::cmp::Ordering| {
        if unsigned {
            left.bits.cmp(&right.bits) == ordering
        } else {
            left.as_signed().cmp(&right.as_signed()) == ordering
        }
    };
    use core::cmp::Ordering::{Greater, Less};
    Ok(match operator {
        "||" => Number::truth(left.is_true() || right.is_true()),
        "&&" => Number::truth(left.is_true() && right.is_true()),
        "|" => arithmetic(left.bits | right.bits),
        "^" => arithmetic(left.bits ^ right.bits),
        "&" => arithmetic(left.bits & right.bits),
        "==" => Number::truth(left.bits == right.bits),
        "!=" => Number::truth(left.bits != right.bits),
        "<" => Number::truth(compare(Less)),
        ">" => Number::truth(compare(Greater)),
        "<=" => Number::truth(!compare(Greater)),
        ">=" => Number::truth(!compare(Less)),
        // The shift count is the right operand's value; past the width it
        // is undefined in C++, and here it is reported.
        "<<" | ">>" => {
            let count = right.bits;
            if count >= 64 {
                if !live {
                    return Ok(Number::signed(0));
                }
                return Err(format!(
                    "a shift by {count} past the 64 bits of the expression"
                ));
            }
            let count = u32::try_from(count).unwrap_or(0);
            if operator == "<<" {
                Number {
                    bits: left.bits.wrapping_shl(count),
                    unsigned: left.unsigned,
                }
            } else if left.unsigned {
                Number {
                    bits: left.bits.wrapping_shr(count),
                    unsigned: true,
                }
            } else {
                Number::signed(left.as_signed().wrapping_shr(count))
            }
        }
        "+" => arithmetic(left.bits.wrapping_add(right.bits)),
        "-" => arithmetic(left.bits.wrapping_sub(right.bits)),
        "*" => arithmetic(left.bits.wrapping_mul(right.bits)),
        "/" | "%" => {
            if right.bits == 0 {
                if !live {
                    return Ok(Number::signed(0));
                }
                return Err("a division by zero".to_string());
            }
            let bits = if unsigned {
                if operator == "/" {
                    left.bits / right.bits
                } else {
                    left.bits % right.bits
                }
            } else if operator == "/" {
                left.as_signed()
                    .wrapping_div(right.as_signed())
                    .cast_unsigned()
            } else {
                left.as_signed()
                    .wrapping_rem(right.as_signed())
                    .cast_unsigned()
            };
            arithmetic(bits)
        }
        other => {
            return Err(format!(
                "`{other}` is not an operator of a constant expression"
            ));
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn value(line: &str) -> bool {
        let tokens: Vec<String> = super::super::pptoken::tokenize(line)
            .unwrap()
            .into_iter()
            .map(|piece| piece.text)
            .collect();
        evaluate(&tokens).unwrap().value
    }

    #[test]
    fn arithmetic_and_comparison() {
        assert!(value("2 * 3 > 5"));
        assert!(!value("2 * 3 > 6"));
        assert!(value("(1 + 2) * 3 == 9"));
        assert!(value("10 / 3 == 3 && 10 % 3 == 1"));
        assert!(value("1 << 4 == 16"));
        assert!(value("-1 < 0"));
        assert!(value("0x10 == 16 && 010 == 8"));
    }

    #[test]
    fn the_usual_arithmetic_conversions_make_a_comparison_unsigned() {
        // §5/9: -1 converts to the largest unsigned value.
        assert!(!value("-1 < 0u"));
        assert!(value("-1 > 0u"));
    }

    #[test]
    fn logic_short_circuits_and_spares_a_dead_division() {
        assert!(value("1 || 1 / 0"));
        assert!(!value("0 && 1 / 0"));
        assert!(value("0 ? 1 / 0 : 1"));
        assert!(evaluate(&["1".into(), "/".into(), "0".into()]).is_err());
    }

    #[test]
    fn identifiers_are_zero_and_true_is_one() {
        let outcome = evaluate(&["UNDEFINED".to_string()]).unwrap();
        assert!(!outcome.value);
        assert_eq!(outcome.zeroed, ["UNDEFINED"]);
        assert!(value("true && !false"));
    }

    #[test]
    fn characters_have_their_codes() {
        assert!(value("'A' == 65"));
        assert!(value("'\\n' == 10 && '\\x41' == 65 && '\\101' == 65"));
    }

    #[test]
    fn a_malformed_expression_is_an_error() {
        for line in ["", "1 +", "(1", "1 ? 2", "1 2", "1.5"] {
            let tokens: Vec<String> = super::super::pptoken::tokenize(line)
                .unwrap()
                .into_iter()
                .map(|piece| piece.text)
                .collect();
            assert!(evaluate(&tokens).is_err(), "{line:?}");
        }
    }
}
