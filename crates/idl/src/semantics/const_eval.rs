// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Constant evaluator for OMG IDL 4.2 (C4.6 §1.1).
//!
//! Converts a parsed [`ConstExpr`] into a typed
//! [`ConstValue`]. Implements the type-promotion rules from
//! spec §7.4.1.4.3 + §7.4.4 (integer + float + mixed) as well as
//! range checks (octet 0..255, short i16, etc.), char/wide-char literals
//! with escape sequences, string concatenation, boolean (`TRUE`/`FALSE`),
//! enum resolution and const-expression evaluation with correct
//! operator precedence (which is already represented by the AST binary tree).
//!
//! # Phase-1 scope
//!
//! - Integer promotion (signed + unsigned, 8/16/32/64).
//! - Float promotion (float, double).
//! - LongDouble: minimal stub (16-byte raw representation).
//! - Fixed-point: decimal-string-based; precision tracking minimal.
//! - String-/WString concatenation (caller obligation: adjacent literals
//!   are merged via [`concat_strings`]).
//! - Char + wide-char with an escape decoder (`\n`, `\t`, `\\`, `\'`, `\"`,
//!   `\xHH`, `\uHHHH`, `\ooo`).
//! - Boolean: recognizes `TRUE`/`FALSE` identifiers as well as LiteralKind::Boolean.
//! - Enum resolution: `MyEnum::RED` resolved via [`SymbolTable`].
//! - Bitwise (`|`, `^`, `&`) + shift (`<<`, `>>`) on integers.
//! - Operator precedence is given by the AST.
//!
//! # Out of scope
//! - LongDouble Arithmetic — blockiert auf Rust-Stable f128
//!   (Tracking: rust-lang/rust#116909).

use crate::ast::{BinaryOp, ConstExpr, Literal, LiteralKind, ScopedName, UnaryOp};
use crate::errors::Span;
use num_bigint::BigInt;
use num_traits::Zero;

/// Typed value of a const-expression result.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    /// `boolean`.
    Bool(bool),
    /// `octet` (0..255).
    Octet(u8),
    /// `int8` — signed 8-bit (-128..127). Spec §7.4.13.4.4.
    Int8(i8),
    /// `uint8` — unsigned 8-bit (0..255). Spec §7.4.13.4.4.
    UInt8(u8),
    /// `short`.
    Short(i16),
    /// `unsigned short`.
    UShort(u16),
    /// `long`.
    Long(i32),
    /// `unsigned long`.
    ULong(u32),
    /// `long long`.
    LongLong(i64),
    /// `unsigned long long`.
    ULongLong(u64),
    /// `float`.
    Float(f32),
    /// `double`.
    Double(f64),
    /// `long double` — raw bytes (Spec §7.4.1 Tab.7-3).
    LongDouble([u8; 16]),
    /// `fixed` — as a decimal string with `digits`/`scale`.
    Fixed {
        /// Decimal representation.
        digits: String,
        /// Number of decimal places.
        scale: u32,
    },
    /// `char` (single byte / ASCII codepoint).
    Char(u8),
    /// `string` (UTF-8).
    String(String),
    /// `wchar` (UCS codepoint).
    WChar(u32),
    /// `wstring` (Sequenz aus UCS-Codepoints).
    WString(Vec<u32>),
    /// Enum value, resolved. `value` is the numeric index.
    Enum {
        /// Vollqualifizierter Enum-Type-Name.
        type_name: String,
        /// Numeric value (discriminant).
        value: i32,
    },
}

impl ConstValue {
    /// Truncated `i64` view, if integer-like. Floats and strings
    /// return `None`.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        Some(match self {
            Self::Bool(b) => i64::from(*b),
            Self::Octet(v) => i64::from(*v),
            Self::Int8(v) => i64::from(*v),
            Self::UInt8(v) => i64::from(*v),
            Self::Short(v) => i64::from(*v),
            Self::UShort(v) => i64::from(*v),
            Self::Long(v) => i64::from(*v),
            Self::ULong(v) => i64::from(*v),
            Self::LongLong(v) => *v,
            Self::ULongLong(v) => {
                if *v > i64::MAX as u64 {
                    return None;
                }
                *v as i64
            }
            Self::Char(c) => i64::from(*c),
            Self::WChar(c) => i64::from(*c),
            Self::Enum { value, .. } => i64::from(*value),
            _ => return None,
        })
    }

    /// `true` if the value has an `f64` conversion.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        Some(match self {
            Self::Float(f) => f64::from(*f),
            Self::Double(d) => *d,
            other => other.as_i64().map(|v| v as f64)?,
        })
    }
}

/// Error during const evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// Integer literal lies outside the expected range
    /// (e.g. `octet = 256`).
    OutOfRange {
        /// Target semantics, e.g. `"octet"`, `"short"`, etc.
        kind: &'static str,
        /// Raw representation from the source.
        raw: String,
        /// Source location.
        span: Span,
    },
    /// Division by zero.
    DivisionByZero {
        /// Source location of the division operator.
        span: Span,
    },
    /// Modulo by zero.
    ModuloByZero {
        /// Source location of the modulo operator.
        span: Span,
    },
    /// Shift with a nonsensical argument (negative, too large).
    InvalidShift {
        /// Description of the violation.
        message: String,
        /// Source location.
        span: Span,
    },
    /// The operator application does not match the operand types.
    TypeMismatch {
        /// Operator designator.
        operator: String,
        /// Description of the mismatch.
        message: String,
        /// Source location.
        span: Span,
    },
    /// The literal could not be parsed (e.g. a faulty char escape).
    InvalidLiteral {
        /// Description.
        message: String,
        /// Source location.
        span: Span,
    },
    /// Scoped name not in the symbol table.
    UnresolvedName {
        /// Full path.
        name: String,
        /// Source location.
        span: Span,
    },
    /// §7.2.6.2.1 / §7.2.6.3 — cross-assign between wide and
    /// narrow char/string. Example: `const char X = L'X';` is a
    /// spec violation.
    CrossAssignWideNarrow {
        /// Declared const type (`"char"`/`"wchar"`/`"string"`/`"wstring"`).
        declared: &'static str,
        /// Actual literal class.
        actual: &'static str,
        /// Source location of the const decl.
        span: Span,
    },
}

/// Symbol-table entry for enum/const resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum Symbol {
    /// Enum member: `(EnumName, Index)`.
    EnumValue {
        /// Fully-qualified enum type name.
        type_name: String,
        /// Index in the enumerator list.
        value: i32,
    },
    /// `const T NAME = expr;`.
    Const(ConstValue),
}

/// Very simple symbol table for const-eval (map onto full path). The
/// full resolver lives in [`crate::semantics::resolver`].
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    entries: std::collections::HashMap<String, Symbol>,
}

impl SymbolTable {
    /// Empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a symbol with full path (`Module::Sub::ENUM_VALUE`).
    pub fn insert(&mut self, full_name: impl Into<String>, sym: Symbol) {
        self.entries.insert(full_name.into(), sym);
    }

    /// Lookup of a full-path designation.
    #[must_use]
    pub fn get(&self, full_name: &str) -> Option<&Symbol> {
        self.entries.get(full_name)
    }
}

/// Eval a [`ConstExpr`] to a [`ConstValue`].
///
/// # Errors
/// See [`EvalError`].
///
/// §7.2.6.2.1 / §7.2.6.3 — checks whether an evaluated const value
/// is type-compatible with the declared const type. In particular,
/// wide-↔-narrow cross-assigns (`char`/`wchar`, `string`/
/// `wstring`) are reported as compile-time errors.
///
/// Other type matches (integer promotion, float mix etc.) are NOT
/// covered here — the eval pass only produces the
/// matching ConstValue anyway, and the cross-assign spec obligation applies only to
/// char/string types.
///
/// # Errors
/// `EvalError::CrossAssignWideNarrow` on mismatch.
pub fn check_const_decl_type_match(
    declared: &crate::ast::ConstType,
    value: &ConstValue,
    span: Span,
) -> Result<(), EvalError> {
    use crate::ast::ConstType;
    match (declared, value) {
        (ConstType::Char, ConstValue::WChar(_)) => Err(EvalError::CrossAssignWideNarrow {
            declared: "char",
            actual: "wchar",
            span,
        }),
        (ConstType::WideChar, ConstValue::Char(_)) => Err(EvalError::CrossAssignWideNarrow {
            declared: "wchar",
            actual: "char",
            span,
        }),
        (ConstType::String { wide: false }, ConstValue::WString(_)) => {
            Err(EvalError::CrossAssignWideNarrow {
                declared: "string",
                actual: "wstring",
                span,
            })
        }
        (ConstType::String { wide: true }, ConstValue::String(_)) => {
            Err(EvalError::CrossAssignWideNarrow {
                declared: "wstring",
                actual: "string",
                span,
            })
        }
        _ => Ok(()),
    }
}

/// zerodds-lint: recursion-depth 64
pub fn evaluate(expr: &ConstExpr, syms: &SymbolTable) -> Result<ConstValue, EvalError> {
    match expr {
        ConstExpr::Literal(l) => eval_literal(l),
        ConstExpr::Scoped(s) => eval_scoped(s, syms),
        ConstExpr::Unary { op, operand, span } => {
            let v = evaluate(operand, syms)?;
            apply_unary(*op, &v, *span)
        }
        ConstExpr::Binary { op, lhs, rhs, span } => {
            let l = evaluate(lhs, syms)?;
            let r = evaluate(rhs, syms)?;
            apply_binary(*op, &l, &r, *span)
        }
    }
}

fn eval_literal(l: &Literal) -> Result<ConstValue, EvalError> {
    match l.kind {
        LiteralKind::Boolean => Ok(ConstValue::Bool(l.raw == "TRUE" || l.raw == "true")),
        LiteralKind::Integer => parse_integer(&l.raw, l.span),
        LiteralKind::Floating => parse_floating(&l.raw, l.span),
        LiteralKind::Fixed => Ok(parse_fixed(&l.raw)),
        LiteralKind::Char => decode_char(&l.raw, l.span).map(ConstValue::Char),
        LiteralKind::WideChar => decode_wide_char(&l.raw, l.span).map(ConstValue::WChar),
        LiteralKind::String => decode_string(&l.raw, l.span).map(ConstValue::String),
        LiteralKind::WideString => decode_wide_string(&l.raw, l.span).map(ConstValue::WString),
    }
}

fn eval_scoped(s: &ScopedName, syms: &SymbolTable) -> Result<ConstValue, EvalError> {
    let name = scoped_full_name(s);
    // Special case: `TRUE`/`FALSE` as an unqualified scoped name (not
    // tokenized as a boolean literal).
    if !s.absolute && s.parts.len() == 1 {
        match s.parts[0].text.as_str() {
            "TRUE" | "true" => return Ok(ConstValue::Bool(true)),
            "FALSE" | "false" => return Ok(ConstValue::Bool(false)),
            _ => {}
        }
    }
    match syms.get(&name) {
        Some(Symbol::Const(v)) => Ok(v.clone()),
        Some(Symbol::EnumValue { type_name, value }) => Ok(ConstValue::Enum {
            type_name: type_name.clone(),
            value: *value,
        }),
        None => Err(EvalError::UnresolvedName { name, span: s.span }),
    }
}

fn scoped_full_name(s: &ScopedName) -> String {
    let mut out = if s.absolute {
        String::from("::")
    } else {
        String::new()
    };
    for (i, p) in s.parts.iter().enumerate() {
        if i > 0 {
            out.push_str("::");
        }
        out.push_str(&p.text);
    }
    out
}

// --------------------------------------------------------------------
// Integer parsing with promotion
// --------------------------------------------------------------------

fn parse_integer(raw: &str, span: Span) -> Result<ConstValue, EvalError> {
    // Suffixe aus C-Style: U, L, LL.
    let (digits, _suffix) = split_int_suffix(raw);
    let (radix, body) = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16u32, rest)
    } else if digits.starts_with('0')
        && digits.len() > 1
        && digits.chars().all(|c| c.is_ascii_digit())
    {
        (8u32, &digits[1..])
    } else {
        (10u32, digits)
    };
    let body = body.replace('_', "");
    // Use i128 for range decisions.
    let val = i128::from_str_radix(&body, radix).map_err(|_| EvalError::InvalidLiteral {
        message: format!("integer parse failed: {raw}"),
        span,
    })?;
    promote_int(val, span)
}

fn split_int_suffix(raw: &str) -> (&str, &str) {
    let bytes = raw.as_bytes();
    let mut end = bytes.len();
    while end > 0 {
        let c = bytes[end - 1];
        if c == b'u' || c == b'U' || c == b'l' || c == b'L' {
            end -= 1;
        } else {
            break;
        }
    }
    (&raw[..end], &raw[end..])
}

/// Spec §7.4.4: kleinster passender Integer-Type.
fn promote_int(v: i128, span: Span) -> Result<ConstValue, EvalError> {
    if (i32::MIN as i128..=i32::MAX as i128).contains(&v) {
        Ok(ConstValue::Long(v as i32))
    } else if (0..=u32::MAX as i128).contains(&v) {
        Ok(ConstValue::ULong(v as u32))
    } else if (i64::MIN as i128..=i64::MAX as i128).contains(&v) {
        Ok(ConstValue::LongLong(v as i64))
    } else if (0..=u64::MAX as i128).contains(&v) {
        Ok(ConstValue::ULongLong(v as u64))
    } else {
        Err(EvalError::OutOfRange {
            kind: "integer",
            raw: v.to_string(),
            span,
        })
    }
}

// --------------------------------------------------------------------
// Floating
// --------------------------------------------------------------------

fn parse_floating(raw: &str, span: Span) -> Result<ConstValue, EvalError> {
    // Suffixes: f / F (float), d / D (double), l / L (long double — falls
    // back to double with stub bytes).
    let trimmed = raw.trim_end_matches(['f', 'F', 'd', 'D', 'l', 'L']);
    let val: f64 = trimmed.parse().map_err(|_| EvalError::InvalidLiteral {
        message: format!("float parse failed: {raw}"),
        span,
    })?;
    if let Some(suffix) = raw.bytes().last() {
        if suffix == b'f' || suffix == b'F' {
            return Ok(ConstValue::Float(val as f32));
        }
        if suffix == b'l' || suffix == b'L' {
            // LongDouble: speichere f64 in 16-byte buffer.
            let mut buf = [0u8; 16];
            buf[..8].copy_from_slice(&val.to_le_bytes());
            return Ok(ConstValue::LongDouble(buf));
        }
    }
    Ok(ConstValue::Double(val))
}

fn parse_fixed(raw: &str) -> ConstValue {
    let trimmed = raw.trim_end_matches(['d', 'D']);
    let scale = trimmed
        .find('.')
        .map(|p| (trimmed.len() - p - 1) as u32)
        .unwrap_or(0);
    ConstValue::Fixed {
        digits: trimmed.replace('.', ""),
        scale,
    }
}

/// §7.2.6.5 Tab 7-11 — fixed-point arithmetic with BigInt backing.
///
/// Scaling rules (Spec Tab 7-11):
/// - `+`/`-`: scale_result = max(scale_l, scale_r); both operands
///   normalized to this scale.
/// - `*`: scale_result = scale_l + scale_r.
/// - `/`: scale_result = max(0, scale_l - scale_r) — simplified
///   compared to the spec's `sinf` (arbitrary precision quotient); the full
///   sinf semantics is a phase-3 item.
///
/// 31-Digit-Cap (Spec §7.4.1.4.3, S. 31): "If an individual computation
/// between a pair of fixed-point literals actually generates more than
/// 31 significant digits, then a 31-digit result is retained as
/// follows: fixed<d,s> => fixed<31, 31-d+s>. Leading and trailing zeros
/// shall not be considered significant. The omitted digits shall be
/// discarded; rounding shall not be performed."
///
/// 62-digit precision (Spec §7.4.1.4.3, p. 31): "All intermediate
/// computations shall be performed using double precision (i.e., 62
/// digits) arithmetic." With `num_bigint::BigInt` backing we reach
/// arbitrary precision; the 31-digit-cap logic truncates the result at the
/// end. This satisfies the spec constraint — no i128 overflow.
fn apply_binary_fixed(
    op: BinaryOp,
    l: &ConstValue,
    r: &ConstValue,
    span: Span,
) -> Result<ConstValue, EvalError> {
    let (lhs_digits, lhs_scale) = fixed_components(l, span)?;
    let (rhs_digits, rhs_scale) = fixed_components(r, span)?;
    let lhs_int = parse_fixed_digits_big(&lhs_digits, span)?;
    let rhs_int = parse_fixed_digits_big(&rhs_digits, span)?;
    match op {
        BinaryOp::Add | BinaryOp::Sub => {
            let target_scale = lhs_scale.max(rhs_scale);
            let lhs_n = scale_up_big(&lhs_int, target_scale - lhs_scale);
            let rhs_n = scale_up_big(&rhs_int, target_scale - rhs_scale);
            let v = if matches!(op, BinaryOp::Add) {
                lhs_n + rhs_n
            } else {
                lhs_n - rhs_n
            };
            let (digits, scale) = cap_to_31_digits(&v, target_scale);
            Ok(ConstValue::Fixed { digits, scale })
        }
        BinaryOp::Mul => {
            let v = &lhs_int * &rhs_int;
            let (digits, scale) = cap_to_31_digits(&v, lhs_scale + rhs_scale);
            Ok(ConstValue::Fixed { digits, scale })
        }
        BinaryOp::Div => {
            if rhs_int.is_zero() {
                return Err(EvalError::DivisionByZero { span });
            }
            // Spec §7.4.1.4.3 + Tab. 7-11: the quotient result type is
            // `fixed<(d1-s1+s2)+sinf, sinf>` — arbitrary precision.
            // We approximate `sinf` with additional places, so that
            // the 31-digit cap is fully usable at the end: lhs_scale +
            // 31 additional decimal places minus rhs_scale.
            let extra_scale: u32 = 31;
            let lhs_scaled = scale_up_big(&lhs_int, extra_scale);
            let v = &lhs_scaled / &rhs_int;
            // Resulting scale: lhs_scale + extra_scale - rhs_scale.
            let combined_scale = lhs_scale
                .saturating_add(extra_scale)
                .saturating_sub(rhs_scale);
            let (digits, scale) = cap_to_31_digits(&v, combined_scale);
            Ok(ConstValue::Fixed { digits, scale })
        }
        other => Err(EvalError::TypeMismatch {
            operator: format!("{other:?}"),
            message: "operator not defined on fixed-point".to_string(),
            span,
        }),
    }
}

/// Spec §7.4.1.4.3, p. 31 — 31-digit cap with truncation (no
/// rounding). Returns `(digit_string, adjusted_scale)`.
///
/// If the significant digits of `v` are <= 31: unchanged.
/// Otherwise the low-order `(actual_digits - 31)` places are
/// stripped and the scale reduced accordingly: `scale_new =
/// scale_old - (actual_digits - 31)`. Spec: "The omitted digits shall
/// be discarded; rounding shall not be performed."
fn cap_to_31_digits(v: &BigInt, scale: u32) -> (String, u32) {
    let raw = v.to_string();
    let abs_str = raw.strip_prefix('-').unwrap_or(&raw);
    let digit_count = abs_str.len();
    if digit_count <= 31 {
        return (raw, scale);
    }
    let drop = digit_count - 31;
    let mut truncated = raw.clone();
    truncated.truncate(raw.len() - drop);
    let new_scale = scale.saturating_sub(drop as u32);
    (truncated, new_scale)
}

fn fixed_components(v: &ConstValue, span: Span) -> Result<(String, u32), EvalError> {
    match v {
        ConstValue::Fixed { digits, scale } => Ok((digits.clone(), *scale)),
        // Integer is promoted to fixed with scale=0.
        ConstValue::Long(n) => Ok((n.to_string(), 0)),
        ConstValue::ULong(n) => Ok((n.to_string(), 0)),
        ConstValue::LongLong(n) => Ok((n.to_string(), 0)),
        ConstValue::ULongLong(n) => Ok((n.to_string(), 0)),
        other => Err(EvalError::TypeMismatch {
            operator: "fixed-arith".to_string(),
            message: format!("non-numeric operand {other:?}"),
            span,
        }),
    }
}

fn parse_fixed_digits_big(digits: &str, span: Span) -> Result<BigInt, EvalError> {
    digits
        .parse::<BigInt>()
        .map_err(|_| EvalError::InvalidLiteral {
            message: format!("fixed digits parse failed: {digits}"),
            span,
        })
}

fn scale_up_big(v: &BigInt, by: u32) -> BigInt {
    if by == 0 {
        return v.clone();
    }
    // 10^by — BigInt-Pow ist verlustfrei.
    let multiplier = BigInt::from(10u32).pow(by);
    v * multiplier
}

#[allow(dead_code)]
fn scale_up(v: i128, by: u32, span: Span) -> Result<i128, EvalError> {
    let mut out = v;
    for _ in 0..by {
        out = out.checked_mul(10).ok_or(EvalError::OutOfRange {
            kind: "fixed",
            raw: v.to_string(),
            span,
        })?;
    }
    Ok(out)
}

#[allow(dead_code)]
fn format_fixed_digits(v: i128) -> String {
    v.to_string()
}

// --------------------------------------------------------------------
// Char + String
// --------------------------------------------------------------------

fn decode_char(raw: &str, span: Span) -> Result<u8, EvalError> {
    let inner = strip_quotes(raw, '\'').ok_or_else(|| EvalError::InvalidLiteral {
        message: format!("char literal missing single quotes: {raw}"),
        span,
    })?;
    let bytes = decode_escapes(inner, span, false)?;
    if bytes.len() != 1 {
        return Err(EvalError::InvalidLiteral {
            message: format!(
                "char literal must encode exactly one byte, got {}",
                bytes.len()
            ),
            span,
        });
    }
    let v = bytes[0];
    u8::try_from(v).map_err(|_| EvalError::OutOfRange {
        kind: "char",
        raw: raw.to_string(),
        span,
    })
}

fn decode_wide_char(raw: &str, span: Span) -> Result<u32, EvalError> {
    // wide char: `L'x'`.
    let body = raw.strip_prefix('L').unwrap_or(raw);
    let inner = strip_quotes(body, '\'').ok_or_else(|| EvalError::InvalidLiteral {
        message: format!("wchar literal missing single quotes: {raw}"),
        span,
    })?;
    let codepoints = decode_escapes(inner, span, true)?;
    if codepoints.len() != 1 {
        return Err(EvalError::InvalidLiteral {
            message: format!(
                "wchar literal must encode exactly one codepoint, got {}",
                codepoints.len()
            ),
            span,
        });
    }
    Ok(codepoints[0])
}

fn decode_string(raw: &str, span: Span) -> Result<String, EvalError> {
    let inner = strip_quotes(raw, '"').ok_or_else(|| EvalError::InvalidLiteral {
        message: format!("string literal missing double quotes: {raw}"),
        span,
    })?;
    let bytes = decode_escapes(inner, span, false)?;
    // §7.2.6.3: NUL is forbidden in the string body (the string is
    // null-terminated by the reader, a NUL in the body would lead to
    // premature termination).
    if bytes.contains(&0) {
        return Err(EvalError::InvalidLiteral {
            message: "string literal must not contain '\\0' (NUL)".to_string(),
            span,
        });
    }
    let s = bytes
        .into_iter()
        .map(|c| {
            u8::try_from(c).map_err(|_| EvalError::OutOfRange {
                kind: "string-byte",
                raw: raw.to_string(),
                span,
            })
        })
        .collect::<Result<Vec<u8>, EvalError>>()?;
    String::from_utf8(s).map_err(|e| EvalError::InvalidLiteral {
        message: format!("invalid utf-8 in string literal: {e}"),
        span,
    })
}

fn decode_wide_string(raw: &str, span: Span) -> Result<Vec<u32>, EvalError> {
    let body = raw.strip_prefix('L').unwrap_or(raw);
    let inner = strip_quotes(body, '"').ok_or_else(|| EvalError::InvalidLiteral {
        message: format!("wstring literal missing double quotes: {raw}"),
        span,
    })?;
    let codepoints = decode_escapes(inner, span, true)?;
    // §7.2.6.3 (analogous): NUL in the wstring body is forbidden too.
    if codepoints.contains(&0) {
        return Err(EvalError::InvalidLiteral {
            message: "wstring literal must not contain '\\0' (NUL)".to_string(),
            span,
        });
    }
    Ok(codepoints)
}

fn strip_quotes(s: &str, q: char) -> Option<&str> {
    s.strip_prefix(q).and_then(|t| t.strip_suffix(q))
}

fn decode_escapes(input: &str, span: Span, allow_wide: bool) -> Result<Vec<u32>, EvalError> {
    let mut out = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c as u32);
            continue;
        }
        let esc = chars.next().ok_or_else(|| EvalError::InvalidLiteral {
            message: "trailing backslash in literal".to_string(),
            span,
        })?;
        match esc {
            'n' => out.push(0x0A),
            't' => out.push(0x09),
            'v' => out.push(0x0B),
            'b' => out.push(0x08),
            'r' => out.push(0x0D),
            'f' => out.push(0x0C),
            'a' => out.push(0x07),
            '\\' => out.push(b'\\' as u32),
            '?' => out.push(b'?' as u32),
            '\'' => out.push(b'\'' as u32),
            '"' => out.push(b'"' as u32),
            '0'..='7' => {
                // Octal: bis zu 3 Ziffern.
                let mut val = (esc as u32) - ('0' as u32);
                for _ in 0..2 {
                    match chars.peek() {
                        Some(d) if ('0'..='7').contains(d) => {
                            val = val * 8 + ((*d as u32) - ('0' as u32));
                            chars.next();
                        }
                        _ => break,
                    }
                }
                out.push(val);
            }
            'x' => {
                let mut val = 0u32;
                let mut count = 0;
                while count < 2 {
                    match chars.peek() {
                        Some(d) if d.is_ascii_hexdigit() => {
                            val = val * 16 + d.to_digit(16).unwrap_or(0);
                            chars.next();
                            count += 1;
                        }
                        _ => break,
                    }
                }
                if count == 0 {
                    return Err(EvalError::InvalidLiteral {
                        message: "\\x with no hex digits".to_string(),
                        span,
                    });
                }
                out.push(val);
            }
            'u' => {
                if !allow_wide {
                    return Err(EvalError::InvalidLiteral {
                        message: "\\u escape only valid in wide literals".to_string(),
                        span,
                    });
                }
                // Spec §7.2.6.2.2: "The escape \uhhhh consists of a
                // backslash followed by the character u, followed by one,
                // two, three, or four hexadecimal digits."
                let mut val = 0u32;
                let mut count = 0;
                while count < 4 {
                    match chars.peek() {
                        Some(d) if d.is_ascii_hexdigit() => {
                            val = val * 16 + d.to_digit(16).unwrap_or(0);
                            chars.next();
                            count += 1;
                        }
                        _ => break,
                    }
                }
                if count == 0 {
                    return Err(EvalError::InvalidLiteral {
                        message: "\\u expects at least one hex digit".to_string(),
                        span,
                    });
                }
                out.push(val);
            }
            other => {
                return Err(EvalError::InvalidLiteral {
                    message: format!("unknown escape sequence \\{other}"),
                    span,
                });
            }
        }
    }
    Ok(out)
}

/// Spec §7.2.6.3: adjacent string literals are concatenated.
///
/// Helper API for the AST builder: takes a list of raw literals
/// (as they come from the lexer) and returns the concatenated form.
///
/// # Errors
/// As [`decode_string`].
pub fn concat_strings(literals: &[Literal]) -> Result<String, EvalError> {
    let mut out = String::new();
    for l in literals {
        if !matches!(l.kind, LiteralKind::String) {
            return Err(EvalError::TypeMismatch {
                operator: "concat".to_string(),
                message: "non-string literal in string concatenation".to_string(),
                span: l.span,
            });
        }
        out.push_str(&decode_string(&l.raw, l.span)?);
    }
    Ok(out)
}

/// Like [`concat_strings`], but for wide strings.
///
/// # Errors
/// As [`decode_wide_string`].
pub fn concat_wstrings(literals: &[Literal]) -> Result<Vec<u32>, EvalError> {
    let mut out = Vec::new();
    for l in literals {
        if !matches!(l.kind, LiteralKind::WideString) {
            return Err(EvalError::TypeMismatch {
                operator: "concat".to_string(),
                message: "non-wstring literal in wstring concatenation".to_string(),
                span: l.span,
            });
        }
        out.extend(decode_wide_string(&l.raw, l.span)?);
    }
    Ok(out)
}

// --------------------------------------------------------------------
// Operatoren
// --------------------------------------------------------------------

fn apply_unary(op: UnaryOp, v: &ConstValue, span: Span) -> Result<ConstValue, EvalError> {
    match op {
        UnaryOp::Plus => Ok(v.clone()),
        UnaryOp::Minus => negate(v, span),
        UnaryOp::BitNot => bitnot(v, span),
    }
}

fn negate(v: &ConstValue, span: Span) -> Result<ConstValue, EvalError> {
    match v {
        ConstValue::Long(x) => Ok(ConstValue::Long(x.wrapping_neg())),
        ConstValue::ULong(x) => Ok(ConstValue::Long(-(*x as i64) as i32)),
        ConstValue::LongLong(x) => Ok(ConstValue::LongLong(x.wrapping_neg())),
        ConstValue::ULongLong(x) => {
            if *x > i64::MAX as u64 + 1 {
                Err(EvalError::OutOfRange {
                    kind: "long long",
                    raw: x.to_string(),
                    span,
                })
            } else {
                Ok(ConstValue::LongLong(0i64.wrapping_sub(*x as i64)))
            }
        }
        ConstValue::Short(x) => Ok(ConstValue::Short(x.wrapping_neg())),
        ConstValue::UShort(x) => Ok(ConstValue::Long(-(i32::from(*x)))),
        ConstValue::Octet(x) => Ok(ConstValue::Long(-(i32::from(*x)))),
        ConstValue::Float(x) => Ok(ConstValue::Float(-x)),
        ConstValue::Double(x) => Ok(ConstValue::Double(-x)),
        other => Err(EvalError::TypeMismatch {
            operator: "unary -".to_string(),
            message: format!("cannot negate {other:?}"),
            span,
        }),
    }
}

fn bitnot(v: &ConstValue, span: Span) -> Result<ConstValue, EvalError> {
    Ok(match v {
        ConstValue::Long(x) => ConstValue::Long(!*x),
        ConstValue::ULong(x) => ConstValue::ULong(!*x),
        ConstValue::LongLong(x) => ConstValue::LongLong(!*x),
        ConstValue::ULongLong(x) => ConstValue::ULongLong(!*x),
        ConstValue::Short(x) => ConstValue::Short(!*x),
        ConstValue::UShort(x) => ConstValue::UShort(!*x),
        ConstValue::Octet(x) => ConstValue::Octet(!*x),
        other => {
            return Err(EvalError::TypeMismatch {
                operator: "~".to_string(),
                message: format!("cannot bitwise-negate {other:?}"),
                span,
            });
        }
    })
}

fn apply_binary(
    op: BinaryOp,
    l: &ConstValue,
    r: &ConstValue,
    span: Span,
) -> Result<ConstValue, EvalError> {
    // §7.2.6.5 Tab 7-11 — fixed-point path: if either of the two is fixed.
    // The scaling rules follow the spec; intermediate result i128
    // (~38 decimal digits precision; the full 62-digit precision of the spec
    // requires BigDecimal and is phase-2 material).
    if matches!(l, ConstValue::Fixed { .. }) || matches!(r, ConstValue::Fixed { .. }) {
        return apply_binary_fixed(op, l, r, span);
    }
    // Float path: only if BOTH operands are float.
    // Spec §7.4.1.4.3: "An infix operator may combine two integer
    // types, floating point types or fixed point types, but not
    // mixtures of these." So `int + float` is a TypeMismatch,
    // not an implicit promote.
    let l_is_float = matches!(
        l,
        ConstValue::Float(_) | ConstValue::Double(_) | ConstValue::LongDouble(_)
    );
    let r_is_float = matches!(
        r,
        ConstValue::Float(_) | ConstValue::Double(_) | ConstValue::LongDouble(_)
    );
    if l_is_float != r_is_float {
        return Err(EvalError::TypeMismatch {
            operator: format!("{op:?}"),
            message: format!(
                "infix operator may not mix integer and floating-point types: lhs={l:?} rhs={r:?}"
            ),
            span,
        });
    }
    if l_is_float && r_is_float {
        let lhs = l.as_f64().ok_or_else(|| EvalError::TypeMismatch {
            operator: format!("{op:?}"),
            message: format!("non-numeric lhs {l:?}"),
            span,
        })?;
        let rhs = r.as_f64().ok_or_else(|| EvalError::TypeMismatch {
            operator: format!("{op:?}"),
            message: format!("non-numeric rhs {r:?}"),
            span,
        })?;
        let v = match op {
            BinaryOp::Add => lhs + rhs,
            BinaryOp::Sub => lhs - rhs,
            BinaryOp::Mul => lhs * rhs,
            BinaryOp::Div => {
                if rhs == 0.0 {
                    return Err(EvalError::DivisionByZero { span });
                }
                lhs / rhs
            }
            other => {
                return Err(EvalError::TypeMismatch {
                    operator: format!("{other:?}"),
                    message: "operator not defined on floats".to_string(),
                    span,
                });
            }
        };
        return Ok(promote_float(l, r, v));
    }

    // Integer path.
    let lhs = l.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: format!("{op:?}"),
        message: format!("non-integer lhs {l:?}"),
        span,
    })?;
    let rhs = r.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: format!("{op:?}"),
        message: format!("non-integer rhs {r:?}"),
        span,
    })?;
    let v: i128 = match op {
        BinaryOp::Add => i128::from(lhs) + i128::from(rhs),
        BinaryOp::Sub => i128::from(lhs) - i128::from(rhs),
        BinaryOp::Mul => i128::from(lhs) * i128::from(rhs),
        BinaryOp::Div => {
            if rhs == 0 {
                return Err(EvalError::DivisionByZero { span });
            }
            i128::from(lhs) / i128::from(rhs)
        }
        BinaryOp::Mod => {
            if rhs == 0 {
                return Err(EvalError::ModuloByZero { span });
            }
            i128::from(lhs) % i128::from(rhs)
        }
        BinaryOp::And => i128::from(lhs & rhs),
        BinaryOp::Or => i128::from(lhs | rhs),
        BinaryOp::Xor => i128::from(lhs ^ rhs),
        BinaryOp::Shl => {
            if !(0..64).contains(&rhs) {
                return Err(EvalError::InvalidShift {
                    message: format!("shift amount {rhs} out of range"),
                    span,
                });
            }
            i128::from(lhs).wrapping_shl(rhs as u32)
        }
        BinaryOp::Shr => {
            if !(0..64).contains(&rhs) {
                return Err(EvalError::InvalidShift {
                    message: format!("shift amount {rhs} out of range"),
                    span,
                });
            }
            i128::from(lhs).wrapping_shr(rhs as u32)
        }
    };
    promote_int(v, span)
}

fn promote_float(l: &ConstValue, r: &ConstValue, val: f64) -> ConstValue {
    if matches!(l, ConstValue::Double(_)) || matches!(r, ConstValue::Double(_)) {
        ConstValue::Double(val)
    } else {
        ConstValue::Float(val as f32)
    }
}

// --------------------------------------------------------------------
// Range checks for typed targets
// --------------------------------------------------------------------

/// Casts `v` to `octet`, checks range 0..=255.
///
/// # Errors
/// `EvalError::OutOfRange` if the value does not fit.
pub fn cast_octet(v: &ConstValue, span: Span) -> Result<u8, EvalError> {
    let i = v.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: "cast(octet)".to_string(),
        message: format!("not an integer: {v:?}"),
        span,
    })?;
    u8::try_from(i).map_err(|_| EvalError::OutOfRange {
        kind: "octet",
        raw: i.to_string(),
        span,
    })
}

/// Casts `v` to `short`, checks range -32768..=32767.
///
/// # Errors
/// `EvalError::OutOfRange` if the value does not fit.
pub fn cast_short(v: &ConstValue, span: Span) -> Result<i16, EvalError> {
    let i = v.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: "cast(short)".to_string(),
        message: format!("not an integer: {v:?}"),
        span,
    })?;
    i16::try_from(i).map_err(|_| EvalError::OutOfRange {
        kind: "short",
        raw: i.to_string(),
        span,
    })
}

/// Casts `v` to `unsigned short`, checks range 0..=65535.
///
/// # Errors
/// `EvalError::OutOfRange` if the value does not fit.
pub fn cast_ushort(v: &ConstValue, span: Span) -> Result<u16, EvalError> {
    let i = v.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: "cast(ushort)".to_string(),
        message: format!("not an integer: {v:?}"),
        span,
    })?;
    u16::try_from(i).map_err(|_| EvalError::OutOfRange {
        kind: "unsigned short",
        raw: i.to_string(),
        span,
    })
}

/// Spec §7.4.1.4.3, S. 30: "If the type of an integer constant is
/// **long** or **unsigned long**, then each sub-expression of the
/// associated constant expression is treated as an **unsigned long**
/// by default, or a signed **long** for negated literals or negative
/// integer constants. It is an error if any sub-expression values
/// exceed the precision of the assigned type."
///
/// `TargetIntType` fixes the target range against which each
/// sub-expression result is checked. Thus `evaluate_int_with_target`
/// covers the spec requirement that not only the final result but
/// every intermediate step must stay within the target type range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetIntType {
    /// `octet` / `uint8` — 0..=255.
    Octet,
    /// `int8` — -128..=127.
    Int8,
    /// `short` / `int16` — -32_768..=32_767.
    Short,
    /// `unsigned short` / `uint16` — 0..=65_535.
    UShort,
    /// `long` / `int32` — -2_147_483_648..=2_147_483_647.
    Long,
    /// `unsigned long` / `uint32` — 0..=4_294_967_295.
    ULong,
    /// `long long` / `int64`.
    LongLong,
    /// `unsigned long long` / `uint64`.
    ULongLong,
}

impl TargetIntType {
    /// `(min, max)` as i128 for the target type range.
    #[must_use]
    pub const fn min_max(&self) -> (i128, i128) {
        match self {
            Self::Octet => (0, 255),
            Self::Int8 => (-128, 127),
            Self::Short => (-(1 << 15), (1 << 15) - 1),
            Self::UShort => (0, (1 << 16) - 1),
            Self::Long => (-(1 << 31), (1 << 31) - 1),
            Self::ULong => (0, (1 << 32) - 1),
            Self::LongLong => (i64::MIN as i128, i64::MAX as i128),
            Self::ULongLong => (0, u64::MAX as i128),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Octet => "octet",
            Self::Int8 => "int8",
            Self::Short => "short",
            Self::UShort => "unsigned short",
            Self::Long => "long",
            Self::ULong => "unsigned long",
            Self::LongLong => "long long",
            Self::ULongLong => "unsigned long long",
        }
    }
}

/// Spec §7.4.1.4.3 long-subexpr promotion: evaluates an integer-
/// Const expression with a range check per sub-step against the target type.
///
/// # Errors
/// `EvalError::OutOfRange` if any intermediate result (or the
/// final result) violates the target range.
pub fn evaluate_int_with_target(
    expr: &ConstExpr,
    syms: &SymbolTable,
    target: TargetIntType,
) -> Result<i128, EvalError> {
    let v = eval_int_recursive(expr, syms, target)?;
    let (min, max) = target.min_max();
    if v < min || v > max {
        return Err(EvalError::OutOfRange {
            kind: target.name(),
            raw: v.to_string(),
            span: const_expr_span(expr),
        });
    }
    Ok(v)
}

fn const_expr_span(e: &ConstExpr) -> Span {
    match e {
        ConstExpr::Literal(l) => l.span,
        ConstExpr::Scoped(s) => s.span,
        ConstExpr::Unary { span, .. } | ConstExpr::Binary { span, .. } => *span,
    }
}

/// zerodds-lint: recursion-depth 64
/// The const-expression depth is bounded by §7.2.5 preprocessor limits + the
/// production `<const_expr>` (Rule 13-19) to a flat, parser-enforced
/// depth; 64 is a generous upper bound for
/// realistic IDL const expressions.
fn eval_int_recursive(
    expr: &ConstExpr,
    syms: &SymbolTable,
    target: TargetIntType,
) -> Result<i128, EvalError> {
    let span = const_expr_span(expr);
    let (min, max) = target.min_max();
    let v: i128 = match expr {
        ConstExpr::Literal(_) | ConstExpr::Scoped(_) => {
            let value = evaluate(expr, syms)?;
            value
                .as_i64()
                .map(i128::from)
                .ok_or(EvalError::TypeMismatch {
                    operator: "integer-context".to_string(),
                    message: format!("non-integer leaf in integer const expr: {value:?}"),
                    span,
                })?
        }
        ConstExpr::Unary { op, operand, .. } => {
            let inner = eval_int_recursive(operand, syms, target)?;
            match op {
                UnaryOp::Plus => inner,
                UnaryOp::Minus => inner.checked_neg().ok_or(EvalError::OutOfRange {
                    kind: target.name(),
                    raw: format!("-({inner})"),
                    span,
                })?,
                UnaryOp::BitNot => !inner,
            }
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = eval_int_recursive(lhs, syms, target)?;
            let r = eval_int_recursive(rhs, syms, target)?;
            apply_int_binop(*op, l, r, target, span)?
        }
    };
    if v < min || v > max {
        return Err(EvalError::OutOfRange {
            kind: target.name(),
            raw: v.to_string(),
            span,
        });
    }
    Ok(v)
}

fn apply_int_binop(
    op: BinaryOp,
    l: i128,
    r: i128,
    target: TargetIntType,
    span: Span,
) -> Result<i128, EvalError> {
    match op {
        BinaryOp::Add => l.checked_add(r).ok_or(EvalError::OutOfRange {
            kind: target.name(),
            raw: format!("{l}+{r}"),
            span,
        }),
        BinaryOp::Sub => l.checked_sub(r).ok_or(EvalError::OutOfRange {
            kind: target.name(),
            raw: format!("{l}-{r}"),
            span,
        }),
        BinaryOp::Mul => l.checked_mul(r).ok_or(EvalError::OutOfRange {
            kind: target.name(),
            raw: format!("{l}*{r}"),
            span,
        }),
        BinaryOp::Div => {
            if r == 0 {
                return Err(EvalError::DivisionByZero { span });
            }
            Ok(l / r)
        }
        BinaryOp::Mod => {
            if r == 0 {
                return Err(EvalError::ModuloByZero { span });
            }
            Ok(l % r)
        }
        BinaryOp::And => Ok(l & r),
        BinaryOp::Or => Ok(l | r),
        BinaryOp::Xor => Ok(l ^ r),
        BinaryOp::Shl => {
            if !(0..64).contains(&r) {
                return Err(EvalError::InvalidShift {
                    message: format!("shift amount {r} out of range"),
                    span,
                });
            }
            Ok(l.wrapping_shl(r as u32))
        }
        BinaryOp::Shr => {
            if !(0..64).contains(&r) {
                return Err(EvalError::InvalidShift {
                    message: format!("shift amount {r} out of range"),
                    span,
                });
            }
            Ok(l.wrapping_shr(r as u32))
        }
    }
}

/// Validates that a `ConstValue::Enum` value matches the expected enum
/// type name. Spec §7.4.1.4.3, p. 33: "The constant name for
/// the value of an enumerated constant definition shall denote one of
/// the enumerators defined for the enumerated type of the constant."
///
/// `expected_type` is the fully-qualified type name of the const
/// decl slot (e.g. `"::M::Color"`). `v` must be a
/// `ConstValue::Enum` variant whose `type_name` matches
/// `expected_type`.
///
/// # Errors
/// `EvalError::TypeMismatch` if a cross-type assignment is attempted
/// or `v` is not an enum.
pub fn validate_enum_const_type(
    v: &ConstValue,
    expected_type: &str,
    span: Span,
) -> Result<(), EvalError> {
    match v {
        ConstValue::Enum { type_name, .. } if type_name == expected_type => Ok(()),
        ConstValue::Enum { type_name, .. } => Err(EvalError::TypeMismatch {
            operator: "enum-const-assign".to_string(),
            message: format!(
                "enum const of type {expected_type} cannot be assigned a value from type {type_name}"
            ),
            span,
        }),
        other => Err(EvalError::TypeMismatch {
            operator: "enum-const-assign".to_string(),
            message: format!("expected enum value of type {expected_type}, got {other:?}"),
            span,
        }),
    }
}

/// Casts `v` to `unsigned long`, checks range 0..=2^32-1.
///
/// # Errors
/// `EvalError::OutOfRange` if the value does not fit.
pub fn cast_ulong(v: &ConstValue, span: Span) -> Result<u32, EvalError> {
    let i = v.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: "cast(ulong)".to_string(),
        message: format!("not an integer: {v:?}"),
        span,
    })?;
    u32::try_from(i).map_err(|_| EvalError::OutOfRange {
        kind: "unsigned long",
        raw: i.to_string(),
        span,
    })
}

/// Casts `v` to `int8`, checks range -128..=127. Spec §7.4.13.4.4.
///
/// # Errors
/// `EvalError::OutOfRange` if the value does not fit.
pub fn cast_int8(v: &ConstValue, span: Span) -> Result<i8, EvalError> {
    let i = v.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: "cast(int8)".to_string(),
        message: format!("not an integer: {v:?}"),
        span,
    })?;
    i8::try_from(i).map_err(|_| EvalError::OutOfRange {
        kind: "int8",
        raw: i.to_string(),
        span,
    })
}

/// Casts `v` to `uint8`, checks range 0..=255. Spec §7.4.13.4.4.
///
/// # Errors
/// `EvalError::OutOfRange` if the value does not fit.
pub fn cast_uint8(v: &ConstValue, span: Span) -> Result<u8, EvalError> {
    let i = v.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: "cast(uint8)".to_string(),
        message: format!("not an integer: {v:?}"),
        span,
    })?;
    u8::try_from(i).map_err(|_| EvalError::OutOfRange {
        kind: "uint8",
        raw: i.to_string(),
        span,
    })
}

/// Spec §7.4.1.4.3, p. 32: "`<positive_int_const>` shall evaluate to
/// a positive integer constant." Returns the positive value or
/// an `OutOfRange` error on `<= 0`.
///
/// # Errors
/// `EvalError::OutOfRange` if the value is <= 0 or not an integer.
pub fn evaluate_positive_int(
    expr: &ConstExpr,
    syms: &SymbolTable,
    span: Span,
) -> Result<u64, EvalError> {
    let v = evaluate(expr, syms)?;
    let i = v.as_i64().ok_or_else(|| EvalError::TypeMismatch {
        operator: "positive_int_const".to_string(),
        message: format!("not an integer: {v:?}"),
        span,
    })?;
    if i <= 0 {
        return Err(EvalError::OutOfRange {
            kind: "positive_int",
            raw: i.to_string(),
            span,
        });
    }
    Ok(i as u64)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::ast::{Identifier, Literal, ScopedName};

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn int_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn str_lit(raw: &str) -> Literal {
        Literal {
            kind: LiteralKind::String,
            raw: raw.to_string(),
            span: sp(),
        }
    }

    fn char_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Char,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn float_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Floating,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn bool_ident(name: &str) -> ConstExpr {
        ConstExpr::Scoped(ScopedName {
            absolute: false,
            parts: vec![Identifier::new(name, sp())],
            span: sp(),
        })
    }

    fn syms() -> SymbolTable {
        SymbolTable::new()
    }

    #[test]
    fn int_promotion_long_default() {
        let v = evaluate(&int_lit("42"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Long(42));
    }

    #[test]
    fn int_promotion_to_ulong_when_too_large() {
        let v = evaluate(&int_lit("3000000000"), &syms()).unwrap();
        assert_eq!(v, ConstValue::ULong(3_000_000_000));
    }

    #[test]
    fn int_promotion_long_long_when_signed_huge() {
        let v = evaluate(&int_lit("9223372036854775807"), &syms()).unwrap();
        assert_eq!(v, ConstValue::LongLong(i64::MAX));
    }

    #[test]
    fn int_hex_literal_parsed() {
        let v = evaluate(&int_lit("0xFF"), &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(255));
    }

    #[test]
    fn int_octal_literal_parsed() {
        let v = evaluate(&int_lit("010"), &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(8));
    }

    #[test]
    fn octal_literal_with_digit_8_or_9_is_error() {
        // Spec §7.2.6.1: "The digits 8 and 9 are not octal digits and
        // thus are not allowed in an octal integer literal."
        let r8 = evaluate(&int_lit("018"), &syms());
        assert!(
            matches!(r8, Err(EvalError::InvalidLiteral { .. })),
            "018 must not be accepted as octal, was: {r8:?}"
        );
        let r9 = evaluate(&int_lit("079"), &syms());
        assert!(
            matches!(r9, Err(EvalError::InvalidLiteral { .. })),
            "079 must not be accepted as octal, was: {r9:?}"
        );
    }

    #[test]
    fn octet_range_check_ok() {
        let v = ConstValue::Long(255);
        assert_eq!(cast_octet(&v, sp()).unwrap(), 255);
    }

    #[test]
    fn octet_range_check_overflow_errors() {
        let v = ConstValue::Long(256);
        let err = cast_octet(&v, sp()).unwrap_err();
        assert!(matches!(err, EvalError::OutOfRange { kind: "octet", .. }));
    }

    #[test]
    fn octet_range_check_negative_errors() {
        let v = ConstValue::Long(-1);
        assert!(cast_octet(&v, sp()).is_err());
    }

    #[test]
    fn short_range_overflow_errors() {
        let v = ConstValue::Long(40_000);
        assert!(cast_short(&v, sp()).is_err());
    }

    #[test]
    fn boolean_true_resolves() {
        let v = evaluate(&bool_ident("TRUE"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Bool(true));
    }

    #[test]
    fn boolean_false_resolves() {
        let v = evaluate(&bool_ident("FALSE"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Bool(false));
    }

    #[test]
    fn char_literal_basic_ascii() {
        let v = evaluate(&char_lit("'A'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(0x41));
    }

    #[test]
    fn char_literal_newline_escape() {
        let v = evaluate(&char_lit("'\\n'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(0x0A));
    }

    #[test]
    fn char_literal_hex_escape() {
        let v = evaluate(&char_lit("'\\x41'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(0x41));
    }

    #[test]
    fn char_literal_octal_escape() {
        let v = evaluate(&char_lit("'\\101'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(0x41));
    }

    #[test]
    fn string_literal_concat() {
        let s = concat_strings(&[str_lit("\"foo\""), str_lit("\"bar\"")]).unwrap();
        assert_eq!(s, "foobar");
    }

    #[test]
    fn string_literal_with_escape_decoded() {
        let lits = [str_lit("\"a\\nb\"")];
        let s = concat_strings(&lits).unwrap();
        assert_eq!(s, "a\nb");
    }

    #[test]
    fn wstring_concat() {
        let wlit = Literal {
            kind: LiteralKind::WideString,
            raw: "L\"AB\"".into(),
            span: sp(),
        };
        let v = concat_wstrings(&[wlit.clone(), wlit]).unwrap();
        assert_eq!(v, vec![0x41, 0x42, 0x41, 0x42]);
    }

    // -----------------------------------------------------------------
    // §7.2.6.2.2 Edge-Cases — Hex/Octal/Unicode-Decoding (§1.3 Open-List)
    // -----------------------------------------------------------------

    #[test]
    fn char_literal_unicode_in_narrow_is_error() {
        // \u only allowed in wchar/wstring.
        let r = evaluate(&char_lit("'\\u0041'"), &syms());
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    #[test]
    fn wchar_literal_unicode_decodes() {
        let wlit = Literal {
            kind: LiteralKind::WideChar,
            raw: "L'\\u0041'".into(),
            span: sp(),
        };
        let v = evaluate(&ConstExpr::Literal(wlit), &syms()).unwrap();
        assert_eq!(v, ConstValue::WChar(0x41));
    }

    #[test]
    fn wchar_literal_unicode_three_digits_decodes() {
        // Spec §7.2.6.2.2: "The escape \uhhhh consists of a backslash
        // followed by the character u, followed by one, two, three, or
        // four hexadecimal digits." 3-Digit-Form ist also valide
        // (Phase 2.3).
        let wlit = Literal {
            kind: LiteralKind::WideChar,
            raw: "L'\\u041'".into(),
            span: sp(),
        };
        let v = evaluate(&ConstExpr::Literal(wlit), &syms()).unwrap();
        assert_eq!(v, ConstValue::WChar(0x41));
    }

    #[test]
    fn wchar_literal_unicode_one_digit_decodes() {
        let wlit = Literal {
            kind: LiteralKind::WideChar,
            raw: "L'\\u9'".into(),
            span: sp(),
        };
        let v = evaluate(&ConstExpr::Literal(wlit), &syms()).unwrap();
        assert_eq!(v, ConstValue::WChar(0x09));
    }

    #[test]
    fn wchar_literal_unicode_two_digits_decodes() {
        let wlit = Literal {
            kind: LiteralKind::WideChar,
            raw: "L'\\uAB'".into(),
            span: sp(),
        };
        let v = evaluate(&ConstExpr::Literal(wlit), &syms()).unwrap();
        assert_eq!(v, ConstValue::WChar(0xAB));
    }

    #[test]
    fn wchar_literal_unicode_zero_digits_is_error() {
        // \u without any hex digits must be an error.
        let wlit = Literal {
            kind: LiteralKind::WideChar,
            raw: "L'\\u'".into(),
            span: sp(),
        };
        let r = evaluate(&ConstExpr::Literal(wlit), &syms());
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    #[test]
    fn char_literal_hex_max_byte() {
        let v = evaluate(&char_lit("'\\xFF'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(0xFF));
    }

    #[test]
    fn char_literal_hex_no_digits_is_error() {
        let r = evaluate(&char_lit("'\\x'"), &syms());
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    #[test]
    fn char_literal_octal_max() {
        // \377 = 0xFF (highest 3-digit octal byte value).
        let v = evaluate(&char_lit("'\\377'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(0xFF));
    }

    #[test]
    fn char_literal_octal_overflow_is_range_error() {
        // \400 = 0o400 = 256 — overflow for a narrow char (u8).
        let r = evaluate(&char_lit("'\\400'"), &syms());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn char_literal_question_mark_escape() {
        // Spec Tab 7-9: \? is the trigraph escape for `?`.
        let v = evaluate(&char_lit("'\\?'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(b'?'));
    }

    #[test]
    fn string_literal_with_hex_and_octal_escapes() {
        let lits = [str_lit("\"\\x41\\102\\?\"")];
        let s = concat_strings(&lits).unwrap();
        assert_eq!(s, "AB?");
    }

    #[test]
    fn wstring_literal_with_unicode_escape() {
        let wlit = Literal {
            kind: LiteralKind::WideString,
            raw: "L\"\\u0041\\u0042\"".into(),
            span: sp(),
        };
        let v = concat_wstrings(&[wlit]).unwrap();
        assert_eq!(v, vec![0x41, 0x42]);
    }

    #[test]
    fn string_literal_with_unicode_escape_is_error() {
        // A narrow string must not have \u escapes.
        let lits = [str_lit("\"\\u0041\"")];
        let r = concat_strings(&lits);
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    // -----------------------------------------------------------------
    // §7.2.6.3 — NUL-Validation in String-Literals (§1.4 Open-List)
    // -----------------------------------------------------------------

    #[test]
    fn string_literal_with_octal_nul_is_error() {
        // \0 as an octal escape in a narrow string → Error (§7.2.6.3).
        let lits = [str_lit("\"abc\\0def\"")];
        let r = concat_strings(&lits);
        assert!(
            matches!(r, Err(EvalError::InvalidLiteral { .. })),
            "expected NUL-rejection, got {r:?}"
        );
    }

    #[test]
    fn string_literal_with_hex_nul_is_error() {
        // \x00 forbidden too.
        let lits = [str_lit("\"abc\\x00def\"")];
        let r = concat_strings(&lits);
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    #[test]
    fn wstring_literal_with_unicode_nul_is_error() {
        //   in wstring → Error.
        let wlit = Literal {
            kind: LiteralKind::WideString,
            raw: "L\"abc\\u0000def\"".into(),
            span: sp(),
        };
        let r = concat_wstrings(&[wlit]);
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    // -----------------------------------------------------------------
    // §7.2.6.1 — Octal Integer-Literale (§1.2 Open-List)
    // -----------------------------------------------------------------

    #[test]
    fn integer_literal_octal_with_8_is_error() {
        // `08` ist illegal: Octal nur 0-7.
        let r = evaluate(&int_lit("08"), &syms());
        assert!(
            matches!(r, Err(EvalError::InvalidLiteral { .. })),
            "expected InvalidLiteral for octal '08', got {r:?}"
        );
    }

    #[test]
    fn integer_literal_octal_with_9_is_error() {
        let r = evaluate(&int_lit("09"), &syms());
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    #[test]
    fn integer_literal_octal_max_value() {
        // 0777 = 7*64 + 7*8 + 7 = 511.
        let v = evaluate(&int_lit("0777"), &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(511));
    }

    #[test]
    fn integer_literal_decimal_zero_is_zero() {
        // `0` alone is legal (decimal 0, or octal 0 — both yield 0).
        let v = evaluate(&int_lit("0"), &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(0));
    }

    // -----------------------------------------------------------------
    // §7.2.6.5 Tab 7-11 — Fixed-Point Arithmetik (§1.5 Open-List)
    // -----------------------------------------------------------------

    fn fixed_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Fixed,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    #[test]
    fn fixed_literal_parses_digits_and_scale() {
        let v = evaluate(&fixed_lit("12.34d"), &syms()).unwrap();
        assert_eq!(
            v,
            ConstValue::Fixed {
                digits: "1234".into(),
                scale: 2,
            }
        );
    }

    #[test]
    fn fixed_add_same_scale() {
        // 1.50 + 0.25 = 1.75
        let add = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(fixed_lit("1.50d")),
            rhs: Box::new(fixed_lit("0.25d")),
            span: sp(),
        };
        let v = evaluate(&add, &syms()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                assert_eq!(digits, "175");
                assert_eq!(scale, 2);
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    #[test]
    fn fixed_add_different_scales_normalizes() {
        // 1.5 + 0.25 = 1.75 (scale 2)
        let add = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(fixed_lit("1.5d")),
            rhs: Box::new(fixed_lit("0.25d")),
            span: sp(),
        };
        let v = evaluate(&add, &syms()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                assert_eq!(scale, 2);
                assert_eq!(digits, "175");
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    #[test]
    fn fixed_sub_works() {
        // 1.0 - 0.25 = 0.75
        let sub = ConstExpr::Binary {
            op: BinaryOp::Sub,
            lhs: Box::new(fixed_lit("1.0d")),
            rhs: Box::new(fixed_lit("0.25d")),
            span: sp(),
        };
        let v = evaluate(&sub, &syms()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                assert_eq!(scale, 2);
                assert_eq!(digits, "75");
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    #[test]
    fn fixed_mul_adds_scales() {
        // 1.5 * 0.25 = 0.375 (scale 1+2=3)
        let mul = ConstExpr::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(fixed_lit("1.5d")),
            rhs: Box::new(fixed_lit("0.25d")),
            span: sp(),
        };
        let v = evaluate(&mul, &syms()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                assert_eq!(scale, 3);
                assert_eq!(digits, "375");
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    #[test]
    fn fixed_div_by_zero_errors() {
        let div = ConstExpr::Binary {
            op: BinaryOp::Div,
            lhs: Box::new(fixed_lit("1.0d")),
            rhs: Box::new(fixed_lit("0.0d")),
            span: sp(),
        };
        let r = evaluate(&div, &syms());
        assert!(matches!(r, Err(EvalError::DivisionByZero { .. })));
    }

    #[test]
    fn fixed_with_int_promotes() {
        // Fixed + Integer.
        let add = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(fixed_lit("1.5d")),
            rhs: Box::new(int_lit("2")),
            span: sp(),
        };
        let v = evaluate(&add, &syms()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                // 1.5 + 2 → 1.5 + 2.0 = 3.5 (scale 1)
                assert_eq!(scale, 1);
                assert_eq!(digits, "35");
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    #[test]
    fn integer_literal_octal_with_8_in_middle_is_error() {
        let r = evaluate(&int_lit("0178"), &syms());
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    #[test]
    fn char_literal_nul_is_allowed() {
        // §7.2.6.2: char literal '\0' is legal (unlike in a string body).
        let v = evaluate(&char_lit("'\\0'"), &syms()).unwrap();
        assert_eq!(v, ConstValue::Char(0x00));
    }

    #[test]
    fn const_expr_precedence_already_in_ast() {
        // 1 + 2 * 3 -- Tree: Add(1, Mul(2,3)).
        let mul = ConstExpr::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(int_lit("2")),
            rhs: Box::new(int_lit("3")),
            span: sp(),
        };
        let add = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(int_lit("1")),
            rhs: Box::new(mul),
            span: sp(),
        };
        let v = evaluate(&add, &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(7));
    }

    #[test]
    fn bitwise_or_works() {
        let or = ConstExpr::Binary {
            op: BinaryOp::Or,
            lhs: Box::new(int_lit("0x0F")),
            rhs: Box::new(int_lit("0xF0")),
            span: sp(),
        };
        let v = evaluate(&or, &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(0xFF));
    }

    #[test]
    fn shift_left_works() {
        let shl = ConstExpr::Binary {
            op: BinaryOp::Shl,
            lhs: Box::new(int_lit("1")),
            rhs: Box::new(int_lit("4")),
            span: sp(),
        };
        let v = evaluate(&shl, &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(16));
    }

    #[test]
    fn shift_right_works() {
        let shr = ConstExpr::Binary {
            op: BinaryOp::Shr,
            lhs: Box::new(int_lit("256")),
            rhs: Box::new(int_lit("4")),
            span: sp(),
        };
        let v = evaluate(&shr, &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(16));
    }

    #[test]
    fn division_by_zero_errors() {
        let div = ConstExpr::Binary {
            op: BinaryOp::Div,
            lhs: Box::new(int_lit("1")),
            rhs: Box::new(int_lit("0")),
            span: sp(),
        };
        let err = evaluate(&div, &syms()).unwrap_err();
        assert!(matches!(err, EvalError::DivisionByZero { .. }));
    }

    #[test]
    fn modulo_by_zero_errors() {
        let m = ConstExpr::Binary {
            op: BinaryOp::Mod,
            lhs: Box::new(int_lit("5")),
            rhs: Box::new(int_lit("0")),
            span: sp(),
        };
        let err = evaluate(&m, &syms()).unwrap_err();
        assert!(matches!(err, EvalError::ModuloByZero { .. }));
    }

    #[test]
    fn enum_resolution_via_symbol_table() {
        let mut syms = SymbolTable::new();
        syms.insert(
            "Color::RED",
            Symbol::EnumValue {
                type_name: "Color".into(),
                value: 0,
            },
        );
        let expr = ConstExpr::Scoped(ScopedName {
            absolute: false,
            parts: vec![Identifier::new("Color", sp()), Identifier::new("RED", sp())],
            span: sp(),
        });
        let v = evaluate(&expr, &syms).unwrap();
        assert_eq!(
            v,
            ConstValue::Enum {
                type_name: "Color".into(),
                value: 0
            }
        );
    }

    #[test]
    fn unresolved_name_errors() {
        let expr = ConstExpr::Scoped(ScopedName {
            absolute: false,
            parts: vec![Identifier::new("Unknown", sp())],
            span: sp(),
        });
        let err = evaluate(&expr, &syms()).unwrap_err();
        assert!(matches!(err, EvalError::UnresolvedName { .. }));
    }

    #[test]
    fn float_addition_promotes_to_double() {
        let add = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(float_lit("1.5")),
            rhs: Box::new(float_lit("2.5")),
            span: sp(),
        };
        let v = evaluate(&add, &syms()).unwrap();
        assert_eq!(v, ConstValue::Double(4.0));
    }

    #[test]
    fn unary_minus_negates_long() {
        let neg = ConstExpr::Unary {
            op: UnaryOp::Minus,
            operand: Box::new(int_lit("5")),
            span: sp(),
        };
        assert_eq!(evaluate(&neg, &syms()).unwrap(), ConstValue::Long(-5));
    }

    #[test]
    fn unary_plus_no_op_on_long() {
        // §7.4.1.4.3: "The + unary operator shall have no effect."
        let pos = ConstExpr::Unary {
            op: UnaryOp::Plus,
            operand: Box::new(int_lit("7")),
            span: sp(),
        };
        assert_eq!(evaluate(&pos, &syms()).unwrap(), ConstValue::Long(7));
    }

    #[test]
    fn modulo_identity_holds_for_positive_operands() {
        // §7.4.1.4.3: "(a/b)*b + a%b == a" — spec identity for
        // non-negative operands.
        let a = 17i32;
        let b = 5i32;
        let div = ConstExpr::Binary {
            op: BinaryOp::Div,
            lhs: Box::new(int_lit("17")),
            rhs: Box::new(int_lit("5")),
            span: sp(),
        };
        let modu = ConstExpr::Binary {
            op: BinaryOp::Mod,
            lhs: Box::new(int_lit("17")),
            rhs: Box::new(int_lit("5")),
            span: sp(),
        };
        let div_v = evaluate(&div, &syms()).unwrap();
        let mod_v = evaluate(&modu, &syms()).unwrap();
        let div_i = match div_v {
            ConstValue::Long(n) => n,
            _ => panic!("div not Long"),
        };
        let mod_i = match mod_v {
            ConstValue::Long(n) => n,
            _ => panic!("mod not Long"),
        };
        assert_eq!(div_i * b + mod_i, a, "(17/5)*5 + 17%5 must equal 17");
        assert!(
            mod_i >= 0,
            "remainder must be non-negative for non-negative operands"
        );
    }

    #[test]
    fn bitnot_inverts() {
        let n = ConstExpr::Unary {
            op: UnaryOp::BitNot,
            operand: Box::new(int_lit("0")),
            span: sp(),
        };
        assert_eq!(evaluate(&n, &syms()).unwrap(), ConstValue::Long(-1));
    }

    #[test]
    fn fixed_literal_records_scale() {
        let v = evaluate(
            &ConstExpr::Literal(Literal {
                kind: LiteralKind::Fixed,
                raw: "12.345d".into(),
                span: sp(),
            }),
            &syms(),
        )
        .unwrap();
        assert_eq!(
            v,
            ConstValue::Fixed {
                digits: "12345".into(),
                scale: 3
            }
        );
    }

    // -----------------------------------------------------------------
    // Phase 2.1 — §7.2.6.2.2 completeness tests for alphabetic
    // escapes (Tab. 7-9). Previously only \n and \? tested.
    // -----------------------------------------------------------------

    fn decode_char_via_eval(raw: &str) -> u8 {
        // Helper: returns the decoded byte from a char literal.
        let lit = Literal {
            kind: LiteralKind::Char,
            raw: format!("'{raw}'"),
            span: sp(),
        };
        match evaluate(&ConstExpr::Literal(lit), &syms()).unwrap() {
            ConstValue::Char(b) => b,
            other => panic!("expected Char, got {other:?}"),
        }
    }

    #[test]
    fn escape_sequences_table_7_9_alphabetic() {
        // Spec §7.2.6.2.2 Tab. 7-9: 11 alphabetic escapes with
        // expected bytes per Tab. 7-5 (ISO 646).
        let cases = [
            ("\\n", 0x0A_u8), // newline
            ("\\t", 0x09),    // horizontal tab
            ("\\v", 0x0B),    // vertical tab
            ("\\b", 0x08),    // backspace
            ("\\r", 0x0D),    // carriage return
            ("\\f", 0x0C),    // form feed
            ("\\a", 0x07),    // alert
            ("\\\\", 0x5C),   // backslash
            ("\\?", 0x3F),    // question mark
            ("\\'", 0x27),    // single quote
            ("\\\"", 0x22),   // double quote
        ];
        for (esc, expected) in cases {
            assert_eq!(
                decode_char_via_eval(esc),
                expected,
                "escape {esc} must yield byte 0x{expected:02X}"
            );
        }
    }

    // -----------------------------------------------------------------
    // Phase 2.2 — Unknown-Escape-Sequenz
    // -----------------------------------------------------------------

    #[test]
    fn unknown_escape_is_invalid_literal() {
        // Spec §7.2.6.2.2: "If the character following a backslash is
        // not one of those specified, the behavior is undefined." Repo
        // is stricter: a defined error.
        let lit = Literal {
            kind: LiteralKind::Char,
            raw: "'\\q'".into(),
            span: sp(),
        };
        let r = evaluate(&ConstExpr::Literal(lit), &syms());
        assert!(matches!(r, Err(EvalError::InvalidLiteral { .. })));
    }

    // -----------------------------------------------------------------
    // Phase 2.4 — §7.2.6.3 Adjacent-String-Distinctness + Size
    // -----------------------------------------------------------------

    #[test]
    fn adjacent_string_literals_keep_chars_distinct() {
        // Spec §7.2.6.3: `"\xA" "B"` → 2 Bytes [0x0A, 0x42], NICHT
        // ein einziges 0xAB.
        let parts = [
            Literal {
                kind: LiteralKind::String,
                raw: "\"\\xA\"".into(),
                span: sp(),
            },
            Literal {
                kind: LiteralKind::String,
                raw: "\"B\"".into(),
                span: sp(),
            },
        ];
        let s = concat_strings(&parts).unwrap();
        let bytes: Vec<u8> = s.bytes().collect();
        assert_eq!(
            bytes,
            vec![0x0A, 0x42],
            "adjacent literals must preserve distinct chars"
        );
    }

    #[test]
    fn string_size_after_concat() {
        // Spec §7.2.6.3: "The size of a string literal is the number of
        // character literals enclosed by the quotes, after concatenation."
        let parts = [
            Literal {
                kind: LiteralKind::String,
                raw: "\"ab\"".into(),
                span: sp(),
            },
            Literal {
                kind: LiteralKind::String,
                raw: "\"cd\"".into(),
                span: sp(),
            },
        ];
        let s = concat_strings(&parts).unwrap();
        assert_eq!(s.len(), 4);
        assert_eq!(s, "abcd");
    }

    // -----------------------------------------------------------------
    // Phase 2.5 — §7.4.1.4.3 Tab. 7-12 bitnot for ULong/LL/ULL
    // -----------------------------------------------------------------

    #[test]
    fn bitnot_inverts_unsigned_long() {
        // 2^32-1 - value: ~5_u32 = 0xFFFF_FFFA = 4_294_967_290.
        let v = bitnot(&ConstValue::ULong(5), sp()).unwrap();
        assert_eq!(v, ConstValue::ULong(0xFFFF_FFFA));
    }

    #[test]
    fn bitnot_inverts_long_long() {
        // -(value+1) — analog zu long, nur 64-bit.
        let v = bitnot(&ConstValue::LongLong(0), sp()).unwrap();
        assert_eq!(v, ConstValue::LongLong(-1));
        let v2 = bitnot(&ConstValue::LongLong(7), sp()).unwrap();
        assert_eq!(v2, ConstValue::LongLong(-8));
    }

    #[test]
    fn bitnot_inverts_unsigned_long_long() {
        // 2^64-1 - value.
        let v = bitnot(&ConstValue::ULongLong(0), sp()).unwrap();
        assert_eq!(v, ConstValue::ULongLong(u64::MAX));
        let v2 = bitnot(&ConstValue::ULongLong(7), sp()).unwrap();
        assert_eq!(v2, ConstValue::ULongLong(u64::MAX - 7));
    }

    // -----------------------------------------------------------------
    // Phase 2.6 — §7.4.1.4.3 Shift-Range-Validation
    // -----------------------------------------------------------------

    fn shift_expr(left: i64, op: BinaryOp, right: i64) -> ConstExpr {
        ConstExpr::Binary {
            op,
            lhs: Box::new(int_lit(&left.to_string())),
            rhs: Box::new(int_lit(&right.to_string())),
            span: sp(),
        }
    }

    #[test]
    fn shift_with_64_or_more_is_invalid() {
        // Spec §7.4.1.4.3: "right operand shall be in the range
        // 0 <= right operand < 64."
        let e = shift_expr(1, BinaryOp::Shl, 64);
        let r = evaluate(&e, &syms());
        assert!(
            matches!(r, Err(EvalError::InvalidShift { .. })),
            "got: {r:?}"
        );
    }

    #[test]
    fn shift_with_negative_right_operand_is_invalid() {
        let e = shift_expr(1, BinaryOp::Shl, -1);
        let r = evaluate(&e, &syms());
        assert!(
            matches!(r, Err(EvalError::InvalidShift { .. })),
            "got: {r:?}"
        );
    }

    // -----------------------------------------------------------------
    // Phase 2.7 — §7.4.1.4.3 Bitwise AND + XOR
    // -----------------------------------------------------------------

    #[test]
    fn bitwise_and_works() {
        let e = ConstExpr::Binary {
            op: BinaryOp::And,
            lhs: Box::new(int_lit("0xF0")),
            rhs: Box::new(int_lit("0x0F")),
            span: sp(),
        };
        let v = evaluate(&e, &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(0));
    }

    #[test]
    fn bitwise_xor_works() {
        let e = ConstExpr::Binary {
            op: BinaryOp::Xor,
            lhs: Box::new(int_lit("0xFF")),
            rhs: Box::new(int_lit("0x0F")),
            span: sp(),
        };
        let v = evaluate(&e, &syms()).unwrap();
        assert_eq!(v.as_i64(), Some(0xF0));
    }

    // -----------------------------------------------------------------
    // Phase 2.12 — §7.4.1.4.3 mixed-type infix prohibition
    // -----------------------------------------------------------------

    #[test]
    fn infix_mixed_int_float_is_type_mismatch() {
        // Spec §7.4.1.4.3: "An infix operator may combine two integer
        // types, floating point types or fixed point types, but not
        // mixtures of these."
        let e = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(int_lit("1")),
            rhs: Box::new(float_lit("2.5")),
            span: sp(),
        };
        let r = evaluate(&e, &syms());
        assert!(
            matches!(r, Err(EvalError::TypeMismatch { .. })),
            "got: {r:?}"
        );
    }

    // -----------------------------------------------------------------
    // Phase 2.14 — §7.4.1.4.4.2.1 ushort/ulong Range-Tests
    // -----------------------------------------------------------------

    #[test]
    fn unsigned_short_range_overflow_errors() {
        // The value 65536 does not fit in ushort (max 2^16-1 = 65535).
        let v = ConstValue::Long(65536);
        let r = cast_ushort(&v, sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn unsigned_short_negative_errors() {
        let v = ConstValue::Long(-1);
        let r = cast_ushort(&v, sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn unsigned_long_range_overflow_errors() {
        // 2^32 is out of range for ulong.
        let v = ConstValue::LongLong(1_i64 << 32);
        let r = cast_ulong(&v, sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn unsigned_long_negative_errors() {
        let v = ConstValue::Long(-1);
        let r = cast_ulong(&v, sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    // -----------------------------------------------------------------
    // Phase 2.13 — Enum-Cross-Type-Mismatch
    // -----------------------------------------------------------------

    #[test]
    fn enum_const_with_wrong_type_errors() {
        // Spec §7.4.1.4.3: `const Color another = M::medium;` is an error
        // if `M::medium` comes from enum Size, not Color.
        let v = ConstValue::Enum {
            type_name: "::M::Size".into(),
            value: 1,
        };
        let r = validate_enum_const_type(&v, "::Color", sp());
        assert!(
            matches!(r, Err(EvalError::TypeMismatch { .. })),
            "got: {r:?}"
        );
    }

    #[test]
    fn enum_const_with_matching_type_ok() {
        let v = ConstValue::Enum {
            type_name: "::Color".into(),
            value: 0,
        };
        validate_enum_const_type(&v, "::Color", sp()).unwrap();
    }

    // -----------------------------------------------------------------
    // Phase 2.15 — int8/uint8 Range-Tests (Spec §7.4.13.4.4 + Tab 7-26)
    // -----------------------------------------------------------------

    #[test]
    fn int8_range_check_ok() {
        assert_eq!(cast_int8(&ConstValue::Long(127), sp()).unwrap(), 127);
        assert_eq!(cast_int8(&ConstValue::Long(-128), sp()).unwrap(), -128);
    }

    #[test]
    fn int8_range_overflow_errors() {
        let r = cast_int8(&ConstValue::Long(128), sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn int8_range_negative_underflow_errors() {
        let r = cast_int8(&ConstValue::Long(-129), sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn uint8_range_check_ok() {
        assert_eq!(cast_uint8(&ConstValue::Long(255), sp()).unwrap(), 255);
        assert_eq!(cast_uint8(&ConstValue::Long(0), sp()).unwrap(), 0);
    }

    #[test]
    fn uint8_range_overflow_errors() {
        let r = cast_uint8(&ConstValue::Long(256), sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn uint8_negative_errors() {
        let r = cast_uint8(&ConstValue::Long(-1), sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    // -----------------------------------------------------------------
    // Phase 2.16 — `<positive_int_const>` Validation (Spec §7.4.1.4.3)
    // -----------------------------------------------------------------

    #[test]
    fn positive_int_const_one_is_ok() {
        let r = evaluate_positive_int(&int_lit("5"), &syms(), sp()).unwrap();
        assert_eq!(r, 5);
    }

    #[test]
    fn positive_int_const_zero_is_error() {
        let r = evaluate_positive_int(&int_lit("0"), &syms(), sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn positive_int_const_negative_is_error() {
        let neg = ConstExpr::Unary {
            op: UnaryOp::Minus,
            operand: Box::new(int_lit("3")),
            span: sp(),
        };
        let r = evaluate_positive_int(&neg, &syms(), sp());
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    // -----------------------------------------------------------------
    // Phase 2.8 — §7.4.1.4.3 31-digit cap with truncation (no rounding)
    // -----------------------------------------------------------------

    #[test]
    fn fixed_31_digit_cap_truncates_not_rounds() {
        // 12345678901234567 (17 digits) * 12345678901234567 (17 digits)
        // = 152415787532388345526596755677489 (33 digits — see Python:
        // 12345678901234567**2).
        // The 31-digit cap truncates the last 2 places without rounding:
        // → "1524157875323883455265967556774" (31 digits).
        // (truncation, no rounding — Spec §7.4.1.4.3 "rounding shall
        // not be performed".)
        let big = ConstValue::Fixed {
            digits: "12345678901234567".into(),
            scale: 0,
        };
        let v = apply_binary_fixed(BinaryOp::Mul, &big, &big, sp()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                assert_eq!(
                    digits.len(),
                    31,
                    "31-digit cap must apply, got {} digits: {digits}",
                    digits.len()
                );
                assert_eq!(digits, "1524157875323883455265967556774");
                // scale_old=0+0=0; saturating_sub at drop=2 → 0.
                assert_eq!(scale, 0);
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // Phase 2.9 — §7.4.1.4.3 62-Digit Zwischenergebnis (BigInt-Backing)
    //                       + sinf-Quotient via 31-Digit-Aufstockung
    // -----------------------------------------------------------------

    #[test]
    fn fixed_intermediate_62_digits_does_not_overflow() {
        // 36 + 36 = 72 digit multiply — with i128 this would overflow
        // (38-digit limit). With BigInt backing no problem; the 31-digit cap
        // kicks in at the end.
        let big = ConstValue::Fixed {
            digits: "123456789012345678901234567890123456".into(),
            scale: 0,
        };
        let v = apply_binary_fixed(BinaryOp::Mul, &big, &big, sp()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                assert_eq!(digits.len(), 31);
                // scale = 0 after cap (drop > old scale).
                assert_eq!(scale, 0);
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // Phase 2.10 — §7.4.1.4.3 Long-Subexpr-Promotion
    // -----------------------------------------------------------------

    #[test]
    fn long_const_subexpr_unsigned_by_default() {
        // Sub-Expr im long-Range, Final-Result im long-Range → ok.
        let e = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(int_lit("100")),
            rhs: Box::new(int_lit("200")),
            span: sp(),
        };
        let v = evaluate_int_with_target(&e, &syms(), TargetIntType::Long).unwrap();
        assert_eq!(v, 300);
    }

    #[test]
    fn long_const_intermediate_overflow_is_error() {
        // (2^31) * 2 overflowed unsigned long (2^32-1 = 4_294_967_295,
        // 2^32 = 4_294_967_296 is out of range). Spec requires that
        // each sub-expr result stays within the target range.
        let e = ConstExpr::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(int_lit("0x80000000")), // 2^31
            rhs: Box::new(int_lit("2")),
            span: sp(),
        };
        let r = evaluate_int_with_target(&e, &syms(), TargetIntType::ULong);
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })), "got: {r:?}");
    }

    #[test]
    fn long_const_final_value_in_range_after_intermediate_calc() {
        // (3 * 100_000) - 200_000 = 100_000 — fits in long, no
        // sub-step overflowed.
        let inner = ConstExpr::Binary {
            op: BinaryOp::Mul,
            lhs: Box::new(int_lit("3")),
            rhs: Box::new(int_lit("100000")),
            span: sp(),
        };
        let outer = ConstExpr::Binary {
            op: BinaryOp::Sub,
            lhs: Box::new(inner),
            rhs: Box::new(int_lit("200000")),
            span: sp(),
        };
        let v = evaluate_int_with_target(&outer, &syms(), TargetIntType::Long).unwrap();
        assert_eq!(v, 100_000);
    }

    #[test]
    fn short_const_intermediate_overflow_is_error() {
        // 30_000 + 30_000 = 60_000 > 32_767 (short max).
        let e = ConstExpr::Binary {
            op: BinaryOp::Add,
            lhs: Box::new(int_lit("30000")),
            rhs: Box::new(int_lit("30000")),
            span: sp(),
        };
        let r = evaluate_int_with_target(&e, &syms(), TargetIntType::Short);
        assert!(matches!(r, Err(EvalError::OutOfRange { .. })));
    }

    #[test]
    fn fixed_div_yields_high_precision_quotient() {
        // 1.0d / 3.0d — `1/3 = 0.333…`. With the sinf approximation the
        // quotient must have a high number of decimal places (cap at 31).
        let one = ConstValue::Fixed {
            digits: "1".into(),
            scale: 0,
        };
        let three = ConstValue::Fixed {
            digits: "3".into(),
            scale: 0,
        };
        let v = apply_binary_fixed(BinaryOp::Div, &one, &three, sp()).unwrap();
        match v {
            ConstValue::Fixed { digits, scale } => {
                // `1 * 10^31 / 3 = 333...3` (31 Threes), scale 31.
                assert_eq!(digits.len(), 31);
                assert!(
                    digits.chars().all(|c| c == '3'),
                    "expected 31 Threes, got: {digits}"
                );
                assert_eq!(scale, 31);
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // §7.2.6.2.1 / §7.2.6.3 — Cross-Assign wide↔narrow Char/String
    // -----------------------------------------------------------------

    #[test]
    fn wide_char_literal_to_char_const_is_error() {
        use crate::ast::ConstType;
        let v = ConstValue::WChar(0x58); // 'X'
        let r = check_const_decl_type_match(&ConstType::Char, &v, sp());
        assert!(matches!(
            r,
            Err(EvalError::CrossAssignWideNarrow {
                declared: "char",
                actual: "wchar",
                ..
            })
        ));
    }

    #[test]
    fn narrow_char_literal_to_wchar_const_is_error() {
        use crate::ast::ConstType;
        let v = ConstValue::Char(b'X');
        let r = check_const_decl_type_match(&ConstType::WideChar, &v, sp());
        assert!(matches!(
            r,
            Err(EvalError::CrossAssignWideNarrow {
                declared: "wchar",
                actual: "char",
                ..
            })
        ));
    }

    #[test]
    fn wide_string_literal_to_string_const_is_error() {
        use crate::ast::ConstType;
        let v = ConstValue::WString(vec![0x58u32]);
        let r = check_const_decl_type_match(&ConstType::String { wide: false }, &v, sp());
        assert!(matches!(
            r,
            Err(EvalError::CrossAssignWideNarrow {
                declared: "string",
                actual: "wstring",
                ..
            })
        ));
    }

    #[test]
    fn narrow_string_literal_to_wstring_const_is_error() {
        use crate::ast::ConstType;
        let v = ConstValue::String("X".into());
        let r = check_const_decl_type_match(&ConstType::String { wide: true }, &v, sp());
        assert!(matches!(
            r,
            Err(EvalError::CrossAssignWideNarrow {
                declared: "wstring",
                actual: "string",
                ..
            })
        ));
    }

    #[test]
    fn matching_char_const_decl_passes() {
        use crate::ast::ConstType;
        let v = ConstValue::Char(b'X');
        assert!(check_const_decl_type_match(&ConstType::Char, &v, sp()).is_ok());
        let w = ConstValue::WChar(0x58);
        assert!(check_const_decl_type_match(&ConstType::WideChar, &w, sp()).is_ok());
    }

    #[test]
    fn matching_string_const_decl_passes() {
        use crate::ast::ConstType;
        let v = ConstValue::String("X".into());
        assert!(check_const_decl_type_match(&ConstType::String { wide: false }, &v, sp()).is_ok());
        let w = ConstValue::WString(vec![0x58]);
        assert!(check_const_decl_type_match(&ConstType::String { wide: true }, &w, sp()).is_ok());
    }
}
