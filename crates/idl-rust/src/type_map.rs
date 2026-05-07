// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-Type → Rust-Type Mapping.
//!
//! OMG IDL4 §7.4 + XTypes 1.3 §7.2.2 (Type-System). Mapping richtet sich
//! nach Rust-Idiomatik: vorzeichenbehaftete Integer auf `i*`, vorzeichenlose
//! auf `u*`, Floats auf `f32`/`f64`, `string` auf `String`,
//! `sequence<T>` auf `Vec<T>`, `T[N]` auf `[T; N]`.

use zerodds_idl::ast::types::{
    ConstExpr, FloatingType, IntegerType, PrimitiveType, ScopedName, SequenceType, StringType,
    TypeSpec,
};

use crate::error::{Result, RustGenError};

/// Mapped einen IDL-Type-Spec auf einen Rust-Type-Ausdruck (z.B. `i32`,
/// `Vec<f64>`, `[u8; 16]`).
///
/// `field_size_hint` ist `None` fuer „Type ohne fixed-size-Bound" (Vec,
/// String, ...) und `Some(bytes)` fuer fixed-size-Types — wird vom
/// Caller fuer KEY_HOLDER_MAX_SIZE-Berechnung weitergereicht.
///
/// # Errors
/// `Unsupported` wenn das IDL-Konstrukt ausserhalb des DDS-DataType-
/// Scopes liegt (Fixed, Map, Any).
/// zerodds-lint: recursion-depth 8
/// IDL-Sequence-Schachtelung — 8 Ebenen reichen fuer realistische Use-
/// Cases (sequence<sequence<sequence<...>>>).
pub fn rust_type_for(spec: &TypeSpec) -> Result<String> {
    match spec {
        TypeSpec::Primitive(p) => Ok(rust_primitive(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(rust_scoped(s)),
        TypeSpec::Sequence(seq) => rust_sequence(seq),
        TypeSpec::String(s) => Ok(rust_string(s)),
        TypeSpec::Fixed(f) => {
            let p = const_expr_as_usize(&f.digits).ok_or(RustGenError::InvalidAnnotation {
                name: "fixed-digits".to_string(),
                reason: "non-integer P",
            })?;
            let s = const_expr_as_usize(&f.scale).ok_or(RustGenError::InvalidAnnotation {
                name: "fixed-scale".to_string(),
                reason: "non-integer S",
            })?;
            Ok(format!("zerodds_cdr::fixed::Fixed<{p}, {s}>"))
        }
        TypeSpec::Map(m) => rust_map(m),
        TypeSpec::Any => Ok("zerodds_dcps::DdsAny".to_string()),
    }
}

/// zerodds-lint: recursion-depth 8
/// IDL-Map-Schachtelung — 8 Ebenen reichen fuer realistische Use-Cases.
fn rust_map(m: &zerodds_idl::ast::types::MapType) -> Result<String> {
    let key = rust_type_for(m.key.as_ref())?;
    let value = rust_type_for(m.value.as_ref())?;
    Ok(format!("::std::collections::BTreeMap<{key}, {value}>"))
}

/// IDL-Primitive → Rust-Primitive.
#[must_use]
pub fn rust_primitive(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Integer(i) => rust_integer(i),
        PrimitiveType::Floating(f) => rust_floating(f),
        PrimitiveType::Char => "char",
        PrimitiveType::WideChar => "char",
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "u8",
    }
}

/// Wire-Size eines IDL-Primitives in Bytes (0 wenn nicht fixed-size).
#[must_use]
pub fn primitive_wire_size(p: PrimitiveType) -> usize {
    match p {
        PrimitiveType::Integer(i) => integer_wire_size(i),
        PrimitiveType::Floating(FloatingType::Float) => 4,
        PrimitiveType::Floating(FloatingType::Double) => 8,
        PrimitiveType::Floating(FloatingType::LongDouble) => 16,
        PrimitiveType::Char => 1,
        PrimitiveType::WideChar => 4,
        PrimitiveType::Boolean => 1,
        PrimitiveType::Octet => 1,
    }
}

fn rust_integer(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "i16",
        IntegerType::Long | IntegerType::Int32 => "i32",
        IntegerType::LongLong | IntegerType::Int64 => "i64",
        IntegerType::UShort | IntegerType::UInt16 => "u16",
        IntegerType::ULong | IntegerType::UInt32 => "u32",
        IntegerType::ULongLong | IntegerType::UInt64 => "u64",
        IntegerType::Int8 => "i8",
        IntegerType::UInt8 => "u8",
    }
}

fn integer_wire_size(i: IntegerType) -> usize {
    match i {
        IntegerType::Int8 | IntegerType::UInt8 => 1,
        IntegerType::Short | IntegerType::Int16 | IntegerType::UShort | IntegerType::UInt16 => 2,
        IntegerType::Long | IntegerType::Int32 | IntegerType::ULong | IntegerType::UInt32 => 4,
        IntegerType::LongLong
        | IntegerType::Int64
        | IntegerType::ULongLong
        | IntegerType::UInt64 => 8,
    }
}

fn rust_floating(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "f32",
        FloatingType::Double => "f64",
        FloatingType::LongDouble => "f64", // Rust hat kein f128, fallback
    }
}

fn rust_scoped(s: &ScopedName) -> String {
    s.parts
        .iter()
        .map(|p| escape_keyword(&p.text))
        .collect::<Vec<_>>()
        .join("::")
}

/// Wrappt einen IDL-Identifier in Rust-Raw-Identifier-Form `r#…`,
/// wenn er ein Rust-Reserved-Keyword ist. Spec-Ref: zerodds-idl-rust-1.0
/// §6.2.
#[must_use]
pub fn escape_keyword(ident: &str) -> String {
    if is_rust_keyword(ident) {
        format!("r#{ident}")
    } else {
        ident.to_string()
    }
}

/// Liste der Rust-2024-Edition-Reserved-Keywords. Quelle:
/// `https://doc.rust-lang.org/reference/keywords.html` —
/// Strict-Keywords + reservierte Future-Keywords.
#[must_use]
pub fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        // Strict keywords
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
        | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
        | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self" | "static"
        | "struct" | "super" | "trait" | "true" | "type" | "unsafe" | "use" | "where"
        | "while"
        // 2018+ keywords
        | "async" | "await" | "dyn"
        // 2024+ keywords
        | "gen"
        // Reserved keywords
        | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override"
        | "priv" | "typeof" | "unsized" | "virtual" | "yield" | "try"
        // Underscore is technically reserved
        | "_"
    )
}

/// zerodds-lint: recursion-depth 8
fn rust_sequence(seq: &SequenceType) -> Result<String> {
    let elem = rust_type_for(&seq.elem)?;
    Ok(format!("Vec<{elem}>"))
}

fn rust_string(s: &StringType) -> String {
    if s.wide {
        // wstring → String (Rust String ist UTF-8, nicht UTF-16, aber das
        // ist die idiomatischste Wahl. Wer UTF-16 braucht baut sich einen
        // wrapper).
        "String".to_string()
    } else {
        "String".to_string()
    }
}

/// Berechnet den Wire-Size-Bound fuer eine fixed-size IDL-Type-Spec.
/// Liefert `None` fuer Sequence, String, Scoped (= dynamische / unklare
/// Groesse).
#[must_use]
pub fn wire_size_bound(spec: &TypeSpec) -> Option<usize> {
    match spec {
        TypeSpec::Primitive(p) => Some(primitive_wire_size(*p)),
        TypeSpec::Sequence(_) | TypeSpec::String(_) | TypeSpec::Scoped(_) => None,
        TypeSpec::Fixed(_) | TypeSpec::Map(_) | TypeSpec::Any => None,
    }
}

/// Wertet eine `ConstExpr` als `usize` aus (nur fuer einfache integer-
/// literals — Array-Sizes, Sequence-Bounds, String-Bounds).
#[must_use]
pub fn const_expr_as_usize(expr: &ConstExpr) -> Option<usize> {
    use zerodds_idl::ast::types::{ConstExpr as CE, LiteralKind};
    match expr {
        CE::Literal(lit) if lit.kind == LiteralKind::Integer => {
            // raw kann z.B. "42", "0x10", "0b1010", "0o17" sein.
            parse_integer_literal(&lit.raw)
        }
        _ => None,
    }
}

fn parse_integer_literal(raw: &str) -> Option<usize> {
    let trimmed = raw.trim_end_matches(['u', 'U', 'l', 'L']);
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        usize::from_str_radix(hex, 16).ok()
    } else if let Some(oct) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        usize::from_str_radix(oct, 8).ok()
    } else if let Some(bin) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        usize::from_str_radix(bin, 2).ok()
    } else {
        trimmed.parse::<usize>().ok()
    }
}
