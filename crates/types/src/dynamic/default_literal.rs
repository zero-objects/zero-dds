// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@default`'s text, read against a member's type. XTypes 1.3 §7.3.4.5.4.
//!
//! One parser for both directions a `@default` literal travels:
//! [`super::to_ast`] reads it off a peer's `CompleteTypeObject`, and
//! [`super::builder::DynamicTypeBuilder::add_member`] takes it from a
//! caller building a type at run time through
//! [`super::builder::NewMember::with_default`]. A text that does not read
//! in the member's type is refused by [`DynamicError::Inconsistent`] in
//! both cases, through this one function — two parsers could disagree
//! about which texts are legal, and a member built here would then read
//! back a value a member built from the wire never could, or the other
//! way round.

use alloc::format;
use alloc::string::{String, ToString};

use zerodds_ast::{
    Ast, CharWidth, ConstExpr, ConstExprNode, ConstValue, Literal, PrimitiveKind as AstPrimitive,
    TypeKind as AstKind, TypeNode,
};
use zerodds_ast_core::Id;

use super::error::DynamicError;

/// How deep a chain of `typedef`s is followed to reach the type a
/// `@default` is actually read against. The same bound every other walk
/// over a possibly-cyclic model uses in this module.
const MAX_ALIAS_DEPTH: usize = 64;

/// `@default`'s text, parsed against the member's own type — the only
/// type §7.3.4.5.4 offers to read it in. A text the type cannot read is
/// refused by name, never defaulted.
///
/// # Errors
/// [`DynamicError::Unresolved`] where `member_type` — or the type at the
/// end of its `typedef` chain — is not a node this model answers for;
/// [`DynamicError::Inconsistent`] where `text` does not read in that
/// type.
pub(super) fn default_expr(
    ast: &mut Ast,
    text: &str,
    member_type: Id<TypeNode>,
    at: &str,
) -> Result<Id<ConstExprNode>, DynamicError> {
    let mut effective = member_type;
    for _ in 0..MAX_ALIAS_DEPTH {
        let Ok(node) = ast.type_(effective) else {
            break;
        };
        let AstKind::Alias { aliased } = &node.kind else {
            break;
        };
        let Some(next) = aliased.resolved() else {
            break;
        };
        effective = next;
    }
    let kind = ast
        .type_(effective)
        .map_err(|_| DynamicError::Unresolved { at: at.to_string() })?
        .kind
        .clone();
    let refuse = || DynamicError::Inconsistent {
        detail: format!("`@default(\"{text}\")` at {at} does not read in the member's type"),
    };
    let literal = match &kind {
        AstKind::Primitive(AstPrimitive::Boolean) => match text {
            "TRUE" => Literal::Boolean(true),
            "FALSE" => Literal::Boolean(false),
            _ => return Err(refuse()),
        },
        AstKind::Primitive(
            AstPrimitive::Octet
            | AstPrimitive::UInt8
            | AstPrimitive::Int8
            | AstPrimitive::Int16
            | AstPrimitive::UInt16
            | AstPrimitive::Int32
            | AstPrimitive::UInt32
            | AstPrimitive::Int64
            | AstPrimitive::UInt64,
        )
        | AstKind::Enumeration { .. }
        | AstKind::Bitmask { .. } => Literal::Integer(text.parse::<i128>().map_err(|_| refuse())?),
        AstKind::Primitive(
            AstPrimitive::Float32 | AstPrimitive::Float64 | AstPrimitive::Float128,
        ) => {
            // Validated as a float, stored in the two halves it was
            // written in — the same shape the frontend lowers to.
            let (mantissa, exponent) = match text.split_once(['e', 'E']) {
                Some((mantissa, exponent)) => (mantissa.to_string(), Some(exponent.to_string())),
                None => (text.to_string(), None),
            };
            if text.parse::<f64>().is_err() {
                return Err(refuse());
            }
            Literal::Float { mantissa, exponent }
        }
        AstKind::Primitive(AstPrimitive::Char) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(v), None) => Literal::Char(v),
                _ => return Err(refuse()),
            }
        }
        AstKind::Primitive(AstPrimitive::WChar) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(v), None) => Literal::WChar(v as u32),
                _ => return Err(refuse()),
            }
        }
        AstKind::String {
            width: CharWidth::Char,
            ..
        } => Literal::String(text.to_string()),
        AstKind::String {
            width: CharWidth::WChar,
            ..
        } => Literal::WString(text.chars().map(|c| c as u32).collect()),
        AstKind::Fixed { scale, .. } => {
            let digits: String = text.chars().filter(|c| *c != '.').collect();
            if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                return Err(refuse());
            }
            Literal::Fixed {
                digits,
                scale: *scale,
            }
        }
        _ => return Err(refuse()),
    };
    ast.add_expr(ConstExpr::Literal(literal))
        .map_err(|error| DynamicError::model(&error))
}

/// `default_expr`'s inverse: the literal text that reads back to
/// `expr` through it — for a caller that needs to carry one member's
/// `@default` from one model into another (a graft) rather than out to
/// the OMG wire form.
///
/// Not [`crate::from_ast`]'s own `default_text`: that one narrows to
/// what `AppliedBuiltinMemberAnnotations.default_value` can hold —
/// §7.3.4.5.4's record is one narrow `string`, so a `WString`/`WChar`
/// default has no wire spelling and that function refuses it. Nothing
/// here writes to the wire; a `WString`/`WChar` default reads back into
/// this same model's own arena exactly as a narrow one does; there is
/// no reason to refuse it.
///
/// Lossless for every literal kind `default_expr` accepts: an
/// integer, a float's mantissa/exponent, a character, a string and a
/// `fixed<P,S>`'s digits are already stored as text or as an exact
/// integer in the model's own [`ConstValue`], so round-tripping through
/// text here reorders nothing and rounds nothing.
///
/// # Errors
/// [`DynamicError::Unresolved`] where `expr` is not a constant `ast`
/// can evaluate, or where a wide value's code point is not a `char`;
/// [`DynamicError::Unrepresentable`] for a [`ConstValue`] kind this does
/// not know (`#[non_exhaustive]`).
pub fn default_expr_text(
    ast: &Ast,
    expr: Id<ConstExprNode>,
    at: &str,
) -> Result<String, DynamicError> {
    let value = ast
        .eval_const(expr)
        .map_err(|_| DynamicError::Unresolved { at: at.to_string() })?;
    Ok(match value {
        ConstValue::Boolean(true) => "TRUE".to_string(),
        ConstValue::Boolean(false) => "FALSE".to_string(),
        ConstValue::Integer(v) => format!("{v}"),
        ConstValue::Float { mantissa, exponent } => match exponent {
            Some(exponent) => format!("{mantissa}e{exponent}"),
            None => mantissa,
        },
        ConstValue::Fixed { digits, scale } => crate::from_ast::fixed_text(&digits, scale),
        ConstValue::Char(v) => v.to_string(),
        ConstValue::WChar(code) => char_of(code, at)?.to_string(),
        ConstValue::String(v) => v,
        ConstValue::WString(codes) => {
            let mut out = String::with_capacity(codes.len());
            for code in codes {
                out.push(char_of(code, at)?);
            }
            out
        }
        _ => {
            return Err(DynamicError::Unrepresentable {
                what: at.to_string(),
                kind: "a `@default` value no literal text carries",
            });
        }
    })
}

fn char_of(code: u32, at: &str) -> Result<char, DynamicError> {
    char::from_u32(code).ok_or_else(|| DynamicError::Unresolved { at: at.to_string() })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_ast::Bound;

    /// `default_expr` then [`default_expr_text`] gets the same text
    /// back, for every kind [`super::builder::NewMember::with_default`]
    /// actually reaches — the round trip `graft`
    /// (`crates/rmw-zerodds-shim/src/dynamic.rs`) leans on to carry a
    /// member's `@default` from one model's arena into another's.
    fn round_trips(kind: AstKind, text: &str) {
        let mut ast = Ast::new();
        let root = ast.root_module();
        let type_ = ast.add_type(root, "t", kind).expect("the type declares");
        let expr = default_expr(&mut ast, text, type_, "t").expect("the text reads in its type");
        let back = default_expr_text(&ast, expr, "t").expect("the expression reads back");
        assert_eq!(back, text);
    }

    #[test]
    fn boolean_round_trips() {
        round_trips(AstKind::Primitive(AstPrimitive::Boolean), "TRUE");
        round_trips(AstKind::Primitive(AstPrimitive::Boolean), "FALSE");
    }

    #[test]
    fn integer_round_trips() {
        round_trips(AstKind::Primitive(AstPrimitive::Int32), "-42");
        round_trips(
            AstKind::Primitive(AstPrimitive::UInt64),
            "18446744073709551615",
        );
    }

    #[test]
    fn float_round_trips_mantissa_and_exponent() {
        round_trips(AstKind::Primitive(AstPrimitive::Float64), "2.5");
        round_trips(AstKind::Primitive(AstPrimitive::Float64), "1e-10");
    }

    #[test]
    fn char_round_trips() {
        round_trips(AstKind::Primitive(AstPrimitive::Char), "q");
    }

    #[test]
    fn wchar_round_trips() {
        round_trips(AstKind::Primitive(AstPrimitive::WChar), "中");
    }

    #[test]
    fn string_round_trips() {
        round_trips(
            AstKind::String {
                width: CharWidth::Char,
                bound: Bound::Unbounded,
            },
            "hello, graft",
        );
    }

    #[test]
    fn wstring_round_trips() {
        round_trips(
            AstKind::String {
                width: CharWidth::WChar,
                bound: Bound::Unbounded,
            },
            "héllo 中文",
        );
    }
}
