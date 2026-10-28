// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Constant expressions of the XML type representation.
//!
//! XTypes 1.3 §7.3.2.4 types the bound attributes (`stringMaxLength`,
//! `sequenceMaxLength`, `mapMaxLength`, `mapKeyStringMaxLength`) and
//! `arrayDimensions` as strings because "the values of these attributes may
//! be any constant expression as defined by the `<const_exp>` production in
//! the IDL grammar"; Annex A types them `xs:string` too. The same holds for
//! the `value` of a `<const>`, an `<enumerator>` and a `<caseDiscriminator>`.
//!
//! This module evaluates the integer part of `const_exp`: the literals
//! (decimal, `0x` hexadecimal, `0` octal), the names of `<const>`
//! declarations (scoped as in IDL), the unary operators `+ - ~`, the binary
//! operators `* / % + - << >> & ^ |` with IDL's precedence, and
//! parentheses. A string, character, floating-point or boolean literal is
//! not an integer expression and is an error that names it.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::errors::XmlError;

/// Deepest nesting of parentheses and of constants that refer to constants.
const MAX_DEPTH: usize = 32;

/// The `<const>` declarations of a `<types>` block, by scoped name
/// (`Module::NAME`, and `Struct::NAME` for a constant inside a struct).
#[derive(Debug, Default, Clone)]
pub(crate) struct ConstTable {
    entries: BTreeMap<String, ConstDef>,
    /// Names declared twice (in the same scope): referring to one is an
    /// error rather than a silent pick.
    duplicates: Vec<String>,
}

#[derive(Debug, Clone)]
struct ConstDef {
    /// The `type` attribute (`int32`, `string`, `nonBasic`, …).
    type_name: String,
    /// The `value` attribute, an expression for the integer types.
    value: String,
    /// The scope the declaration sits in; its own references resolve there.
    scope: String,
}

impl ConstTable {
    pub(crate) fn insert(&mut self, scope: &str, name: &str, type_name: &str, value: &str) {
        let scoped = qualify(scope, name);
        let def = ConstDef {
            type_name: type_name.to_string(),
            value: value.to_string(),
            scope: scope.to_string(),
        };
        if self.entries.insert(scoped.clone(), def).is_some() {
            self.duplicates.push(scoped);
        }
    }

    /// Evaluates `expr` written in `scope`.
    ///
    /// # Errors
    /// [`XmlError::ValueOutOfRange`] for a syntax error, an overflow, a
    /// division by zero or a literal that is no integer;
    /// [`XmlError::UnresolvedReference`] for a name that is no constant.
    pub(crate) fn eval(&self, expr: &str, scope: &str) -> Result<i64, XmlError> {
        self.eval_depth(expr, scope, 0)
    }

    /// Evaluates `expr` into the range of a length or dimension, `0` up to
    /// `u32::MAX`.
    pub(crate) fn eval_u32(&self, expr: &str, scope: &str) -> Result<u32, XmlError> {
        let v = self.eval(expr, scope)?;
        u32::try_from(v).map_err(|_| {
            XmlError::ValueOutOfRange(format!("`{}` is {v}, outside 0..=4294967295", expr.trim()))
        })
    }

    /// zerodds-lint: recursion-depth = [`MAX_DEPTH`] (constants that refer
    /// to constants; `depth` is checked on every entry).
    fn eval_depth(&self, expr: &str, scope: &str, depth: usize) -> Result<i64, XmlError> {
        if depth > MAX_DEPTH {
            return Err(XmlError::LimitExceeded(format!(
                "constant expression `{}` nests deeper than {MAX_DEPTH}",
                expr.trim()
            )));
        }
        let tokens = tokenize(expr)?;
        let mut p = Parser {
            table: self,
            scope,
            tokens: &tokens,
            pos: 0,
            depth,
            expr,
        };
        let v = p.expression(0)?;
        if p.pos != tokens.len() {
            return Err(XmlError::ValueOutOfRange(format!(
                "constant expression `{}`: unexpected `{}`",
                expr.trim(),
                tokens[p.pos].text()
            )));
        }
        Ok(v)
    }

    /// IDL name lookup from `scope` outwards; a leading `::` is absolute.
    fn lookup(&self, name: &str, scope: &str) -> Option<(&String, &ConstDef)> {
        if let Some(absolute) = name.strip_prefix("::") {
            return self.entries.get_key_value(absolute);
        }
        let mut scope = scope;
        loop {
            let candidate = qualify(scope, name);
            if let Some(found) = self.entries.get_key_value(&candidate) {
                return Some(found);
            }
            if scope.is_empty() {
                return None;
            }
            scope = scope.rsplit_once("::").map_or("", |(parent, _)| parent);
        }
    }
}

fn qualify(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{scope}::{name}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Number(i64),
    Name(String),
    Op(&'static str),
}

impl Token {
    fn text(&self) -> String {
        match self {
            Self::Number(n) => n.to_string(),
            Self::Name(n) => n.clone(),
            Self::Op(o) => (*o).to_string(),
        }
    }
}

fn tokenize(expr: &str) -> Result<Vec<Token>, XmlError> {
    let bad = |what: &str| {
        XmlError::ValueOutOfRange(format!("constant expression `{}`: {what}", expr.trim()))
    };
    let chars: Vec<char> = expr.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            let lit: String = chars[start..i].iter().collect();
            if i < chars.len() && chars[i] == '.' {
                return Err(bad("a floating-point literal is no integer expression"));
            }
            out.push(Token::Number(
                integer_literal(&lit)
                    .ok_or_else(|| bad(&format!("`{lit}` is no integer literal")))?,
            ));
        } else if c.is_ascii_alphabetic() || c == '_' || c == ':' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == ':')
            {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            if name.contains(":::") || name.ends_with(':') && !name.ends_with("::") || name == ":" {
                return Err(bad(&format!("`{name}` is no name")));
            }
            if name.ends_with("::") {
                return Err(bad(&format!("`{name}` is no name")));
            }
            out.push(Token::Name(name));
        } else if c == '"' || c == '\'' {
            return Err(bad(
                "a string or character literal is no integer expression",
            ));
        } else {
            let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let op = match two.as_str() {
                "<<" => Some("<<"),
                ">>" => Some(">>"),
                _ => None,
            };
            if let Some(op) = op {
                out.push(Token::Op(op));
                i += 2;
                continue;
            }
            let op = match c {
                '+' => "+",
                '-' => "-",
                '*' => "*",
                '/' => "/",
                '%' => "%",
                '&' => "&",
                '|' => "|",
                '^' => "^",
                '~' => "~",
                '(' => "(",
                ')' => ")",
                other => return Err(bad(&format!("unexpected `{other}`"))),
            };
            out.push(Token::Op(op));
            i += 1;
        }
    }
    if out.is_empty() {
        return Err(bad("empty"));
    }
    Ok(out)
}

/// An IDL integer literal: `0x…` hexadecimal, `0…` octal, else decimal.
fn integer_literal(lit: &str) -> Option<i64> {
    let digits = |s: &str, radix: u32| -> Option<i64> {
        if s.is_empty() {
            return None;
        }
        i64::from_str_radix(s, radix).ok()
    };
    if let Some(hex) = lit.strip_prefix("0x").or_else(|| lit.strip_prefix("0X")) {
        digits(hex, 16)
    } else if lit.len() > 1 && lit.starts_with('0') {
        digits(&lit[1..], 8)
    } else {
        digits(lit, 10)
    }
}

struct Parser<'a> {
    table: &'a ConstTable,
    scope: &'a str,
    tokens: &'a [Token],
    pos: usize,
    depth: usize,
    expr: &'a str,
}

/// Binary operators by IDL precedence, loosest first.
const LEVELS: &[&[&str]] = &[
    &["|"],
    &["^"],
    &["&"],
    &["<<", ">>"],
    &["+", "-"],
    &["*", "/", "%"],
];

impl Parser<'_> {
    fn fail(&self, what: &str) -> XmlError {
        XmlError::ValueOutOfRange(format!(
            "constant expression `{}`: {what}",
            self.expr.trim()
        ))
    }

    fn peek_op(&self) -> Option<&'static str> {
        match self.tokens.get(self.pos) {
            Some(Token::Op(o)) => Some(o),
            _ => None,
        }
    }

    /// Binary expression at precedence `level` and tighter.
    ///
    /// zerodds-lint: recursion-depth = [`MAX_DEPTH`] levels of parentheses
    /// (checked in `primary`) times the six precedence levels.
    fn expression(&mut self, level: usize) -> Result<i64, XmlError> {
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.expression(level + 1)?;
        while let Some(op) = self.peek_op().filter(|o| LEVELS[level].contains(o)) {
            self.pos += 1;
            let right = self.expression(level + 1)?;
            left = self.apply(op, left, right)?;
        }
        Ok(left)
    }

    fn apply(&self, op: &str, l: i64, r: i64) -> Result<i64, XmlError> {
        let overflow = || self.fail("overflow");
        match op {
            "|" => Ok(l | r),
            "^" => Ok(l ^ r),
            "&" => Ok(l & r),
            "<<" => u32::try_from(r)
                .ok()
                .and_then(|s| l.checked_shl(s))
                .ok_or_else(overflow),
            ">>" => u32::try_from(r)
                .ok()
                .and_then(|s| l.checked_shr(s))
                .ok_or_else(overflow),
            "+" => l.checked_add(r).ok_or_else(overflow),
            "-" => l.checked_sub(r).ok_or_else(overflow),
            "*" => l.checked_mul(r).ok_or_else(overflow),
            "/" => {
                if r == 0 {
                    Err(self.fail("division by zero"))
                } else {
                    l.checked_div(r).ok_or_else(overflow)
                }
            }
            "%" => {
                if r == 0 {
                    Err(self.fail("division by zero"))
                } else {
                    l.checked_rem(r).ok_or_else(overflow)
                }
            }
            _ => Err(self.fail("unknown operator")),
        }
    }

    /// Unary `+ - ~` operators, counted (at most [`MAX_DEPTH`]) and applied
    /// innermost first.
    fn unary(&mut self) -> Result<i64, XmlError> {
        let mut ops: Vec<&'static str> = Vec::new();
        while let Some(op) = self.peek_op().filter(|o| matches!(*o, "+" | "-" | "~")) {
            self.pos += 1;
            ops.push(op);
            if ops.len() > MAX_DEPTH {
                return Err(self.fail("too many unary operators"));
            }
        }
        let mut v = self.primary()?;
        for op in ops.into_iter().rev() {
            v = match op {
                "-" => v.checked_neg().ok_or_else(|| self.fail("overflow"))?,
                "~" => !v,
                _ => v,
            };
        }
        Ok(v)
    }

    fn primary(&mut self) -> Result<i64, XmlError> {
        let Some(tok) = self.tokens.get(self.pos).cloned() else {
            return Err(self.fail("ends where an operand is expected"));
        };
        self.pos += 1;
        match tok {
            Token::Number(n) => Ok(n),
            Token::Op("(") => {
                // Parentheses nest through `expression`; the nesting is
                // counted on the shared depth.
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return Err(self.fail("parentheses nest too deeply"));
                }
                let v = self.expression(0)?;
                self.depth -= 1;
                if self.tokens.get(self.pos) == Some(&Token::Op(")")) {
                    self.pos += 1;
                    Ok(v)
                } else {
                    Err(self.fail("`)` missing"))
                }
            }
            Token::Name(name) => {
                // The constants of DDS-XML 1.0 §7.2.2.1, when no `<const>`
                // of that name is declared.
                if self.table.lookup(&name, self.scope).is_none() {
                    if let Some(v) = builtin(&name) {
                        return Ok(v);
                    }
                }
                let (scoped, def) = self.table.lookup(&name, self.scope).ok_or_else(|| {
                    XmlError::UnresolvedReference(format!(
                        "`{name}` in constant expression `{}` names no <const>",
                        self.expr.trim()
                    ))
                })?;
                if self.table.duplicates.contains(scoped) {
                    return Err(XmlError::UnresolvedReference(format!(
                        "<const> `{scoped}` is declared more than once"
                    )));
                }
                if !is_integer_type(&def.type_name) {
                    return Err(self.fail(&format!(
                        "`{name}` is a `{}` constant, not an integer",
                        def.type_name
                    )));
                }
                self.table
                    .eval_depth(&def.value, &def.scope, self.depth + 1)
            }
            Token::Op(o) => Err(self.fail(&format!("unexpected `{o}`"))),
        }
    }
}

/// The predefined constants of DDS-XML 1.0 §7.2.2.1.
fn builtin(name: &str) -> Option<i64> {
    Some(match name {
        "LENGTH_UNLIMITED" | "TIME_INVALID_SEC" => -1,
        "DURATION_INFINITE_SEC" | "DURATION_INFINITE_NSEC" => 0x7fff_ffff,
        "DURATION_ZERO_SEC" | "DURATION_ZERO_NSEC" => 0,
        "TIME_INVALID_NSEC" => 0xffff_ffff,
        _ => return None,
    })
}

fn is_integer_type(t: &str) -> bool {
    matches!(
        t,
        "byte"
            | "octet"
            | "int8"
            | "uint8"
            | "int16"
            | "uint16"
            | "int32"
            | "uint32"
            | "int64"
            | "uint64"
            | "short"
            | "ushort"
            | "long"
            | "ulong"
            | "longlong"
            | "ulonglong"
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn table() -> ConstTable {
        let mut t = ConstTable::default();
        t.insert("", "MAX", "int32", "128");
        t.insert("", "DOUBLE", "int32", "MAX * 2");
        t.insert("M", "LOCAL", "uint32", "0x10");
        t.insert("M::N", "DEEP", "int32", "LOCAL + 1");
        t.insert("", "NAME", "string", "\"x\"");
        t.insert("", "LOOP_A", "int32", "LOOP_B");
        t.insert("", "LOOP_B", "int32", "LOOP_A");
        t
    }

    #[test]
    fn literals_and_precedence_follow_idl() {
        let t = table();
        assert_eq!(t.eval("6 * 3", "").unwrap(), 18);
        assert_eq!(t.eval("2 + 3 * 4", "").unwrap(), 14);
        assert_eq!(t.eval("(2 + 3) * 4", "").unwrap(), 20);
        assert_eq!(t.eval("0x1F", "").unwrap(), 31);
        assert_eq!(t.eval("010", "").unwrap(), 8);
        assert_eq!(t.eval("1 << 4 | 1", "").unwrap(), 17);
        assert_eq!(t.eval("7 & 3 ^ 1", "").unwrap(), 2);
        assert_eq!(t.eval("-(3 - 5)", "").unwrap(), 2);
        assert_eq!(t.eval("~0", "").unwrap(), -1);
        assert_eq!(t.eval("17 % 5 / 2", "").unwrap(), 1);
        assert_eq!(t.eval("-1", "").unwrap(), -1);
        // The constants of DDS-XML 1.0 §7.2.2.1 need no <const>.
        assert_eq!(t.eval("LENGTH_UNLIMITED", "").unwrap(), -1);
        assert_eq!(
            t.eval("DURATION_INFINITE_SEC + 0", "").unwrap(),
            0x7fff_ffff
        );
        assert_eq!(t.eval("TIME_INVALID_NSEC", "").unwrap(), 0xffff_ffff);
    }

    #[test]
    fn constants_resolve_like_idl_names() {
        let t = table();
        assert_eq!(t.eval("MAX", "").unwrap(), 128);
        assert_eq!(t.eval("DOUBLE + 1", "").unwrap(), 257);
        assert_eq!(t.eval("LOCAL", "M").unwrap(), 16);
        assert_eq!(t.eval("M::LOCAL", "").unwrap(), 16);
        assert_eq!(t.eval("::M::LOCAL", "M::N").unwrap(), 16);
        // From inside M::N, DEEP sees LOCAL of the enclosing M and MAX of
        // the global scope.
        assert_eq!(t.eval("DEEP + MAX", "M::N").unwrap(), 145);
        assert_eq!(t.eval_u32("MAX", "").unwrap(), 128);
    }

    #[test]
    fn errors_name_the_expression() {
        let t = table();
        for bad in [
            "",
            "1 +",
            "(1",
            "1 )",
            "1 / 0",
            "1 % 0",
            "MISSING",
            "NAME",
            "\"s\"",
            "'c'",
            "1.5",
            "0xZZ",
            "09",
            "9223372036854775807 + 1",
            "1 << 64",
            "LOOP_A",
            "#",
        ] {
            assert!(t.eval(bad, "").is_err(), "`{bad}` must not evaluate");
        }
        assert!(t.eval_u32("0 - 1", "").is_err());
        assert!(t.eval_u32("4294967296", "").is_err());
        let deep = "(".repeat(40) + "1" + &")".repeat(40);
        assert!(matches!(
            t.eval(&deep, ""),
            Err(XmlError::ValueOutOfRange(_))
        ));
    }

    #[test]
    fn a_name_declared_twice_is_not_picked() {
        let mut t = ConstTable::default();
        t.insert("", "X", "int32", "1");
        t.insert("", "X", "int32", "2");
        assert!(matches!(
            t.eval("X", ""),
            Err(XmlError::UnresolvedReference(_))
        ));
    }
}
