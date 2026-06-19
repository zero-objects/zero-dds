// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL type → Rust type mapping.
//!
//! OMG IDL4 §7.4 + XTypes 1.3 §7.2.2 (type system). The mapping follows
//! Rust idiom: signed integers to `i*`, unsigned to `u*`, floats to
//! `f32`/`f64`, `string` to `String`, `sequence<T>` to `Vec<T>`,
//! `T[N]` to `[T; N]`.

use core::cell::{Cell, RefCell};

use zerodds_idl::ast::types::{
    ConstExpr, FloatingType, IntegerType, PrimitiveType, ScopedName, SequenceType, StringType,
    TypeSpec,
};

use crate::error::{Result, RustGenError};

thread_local! {
    /// Codegen target for IDL `any`: `false` = DDS (`zerodds_dcps::DdsAny`,
    /// XCDR2 self-describing), `true` = CORBA (`zerodds_cdr::CorbaAny`,
    /// classic CDR TypeCode+Value). Set by the respective codegen entry
    /// (`generate_rust_module` depending on `cdr_only`;
    /// `generate_corba_rust_module`). Thread-local: each codegen run is
    /// isolated on its thread, so parallel runs (e.g. `cargo test`) do
    /// not clobber each other.
    static ANY_TARGET_CORBA: Cell<bool> = const { Cell::new(false) };
}

/// Sets the `any` codegen target (see [`ANY_TARGET_CORBA`]). Idempotent; each
/// codegen run sets it on entry, so there is no stale state between runs.
pub fn set_any_target_corba(corba: bool) {
    ANY_TARGET_CORBA.with(|c| c.set(corba));
}

thread_local! {
    /// Simple names of all interfaces declared in the current run. A
    /// scoped name whose last part appears here is an
    /// **interface reference** and maps (like `Object`) to
    /// `zerodds_corba_rust::ObjectReference` — an IOR on the wire
    /// (§15.3.3). Otherwise prevents e.g. `NamingContext new_context()`
    /// from referencing the trait itself (dyn-compatibility cycle).
    /// Thread-local (like ANY_TARGET_CORBA), so parallel codegen runs do
    /// not clobber each other; also avoids registry threading through ~25
    /// `rust_type_for` callers.
    static INTERFACE_REFS: RefCell<std::collections::BTreeSet<String>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
}

/// Registers the interface simple names of the current codegen run (replaces
/// the previous set). The CORBA codegen calls this on entry; the DDS codegen
/// leaves the set empty (there are no interface references there).
pub fn set_interface_refs<I: IntoIterator<Item = String>>(names: I) {
    INTERFACE_REFS.with(|r| {
        let mut g = r.borrow_mut();
        g.clear();
        g.extend(names);
    });
}

/// `true` if `name` is an interface registered during the run.
fn is_interface_ref(name: &str) -> bool {
    INTERFACE_REFS.with(|r| r.borrow().contains(name))
}

fn any_rust_type() -> &'static str {
    if ANY_TARGET_CORBA.with(Cell::get) {
        "zerodds_cdr::CorbaAny"
    } else {
        "zerodds_dcps::DdsAny"
    }
}

/// Maps an IDL type spec to a Rust type expression (e.g. `i32`,
/// `Vec<f64>`, `[u8; 16]`).
///
/// `field_size_hint` is `None` for "a type without a fixed-size bound"
/// (Vec, String, ...) and `Some(bytes)` for fixed-size types — passed on
/// by the caller for the KEY_HOLDER_MAX_SIZE computation.
///
/// # Errors
/// `Unsupported` if the IDL construct lies outside the DDS DataType
/// scope (Fixed, Map, Any).
/// zerodds-lint: recursion-depth 8
/// IDL sequence nesting — 8 levels are enough for realistic use cases
/// (sequence<sequence<sequence<...>>>).
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
        // `any`: DDS → DdsAny (XCDR2), CORBA → CorbaAny (classic CDR).
        TypeSpec::Any => Ok(any_rust_type().to_string()),
    }
}

/// zerodds-lint: recursion-depth 8
/// IDL map nesting — 8 levels are enough for realistic use cases.
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
        // IDL char = 8-bit (1 byte), wchar = UTF-16 code unit (2 byte LE) —
        // consistent with zerodds-xcdr2-rust-1.0 §123, the TypeIdentifier
        // (Char8/Char16) and idl-cpp/-java/-csharp/-c. Rust `char` (4 byte
        // Unicode scalar) is NOT a correct mapping — neither for classic
        // CORBA CDR (§9.3.1.5) nor for XCDR2 — and breaks wire interop.
        PrimitiveType::Char => "u8",
        PrimitiveType::WideChar => "u16",
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "u8",
    }
}

/// Wire size of an IDL primitive in bytes (0 if not fixed-size).
#[must_use]
pub fn primitive_wire_size(p: PrimitiveType) -> usize {
    match p {
        PrimitiveType::Integer(i) => integer_wire_size(i),
        PrimitiveType::Floating(FloatingType::Float) => 4,
        PrimitiveType::Floating(FloatingType::Double) => 8,
        PrimitiveType::Floating(FloatingType::LongDouble) => 16,
        PrimitiveType::Char => 1,
        PrimitiveType::WideChar => 2,
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
        FloatingType::LongDouble => "f64", // Rust has no f128, fallback
    }
}

fn rust_scoped(s: &ScopedName) -> String {
    // `Object` is a reserved CORBA pseudo-type (no user-declared
    // definition) that the parser models as a scoped name (§7.4.6.3). In
    // the CORBA codegen it is the object reference; an IOR on the wire
    // (zerodds_corba_rust::ObjectReference impl CdrEncode as IOR §15.3.3).
    if s.parts.len() == 1 && s.parts[0].text == "Object" {
        return "zerodds_corba_rust::ObjectReference".to_string();
    }
    // A reference to a declared interface → object reference (IOR), not the
    // trait itself (otherwise a dyn cycle, e.g. `NamingContext new_context()`).
    if let Some(last) = s.parts.last() {
        if is_interface_ref(&last.text) {
            return "zerodds_corba_rust::ObjectReference".to_string();
        }
    }
    s.parts
        .iter()
        .map(|p| escape_keyword(&p.text))
        .collect::<Vec<_>>()
        .join("::")
}

/// Wraps an IDL identifier in Rust raw-identifier form `r#…` if it is a
/// Rust reserved keyword. Spec-ref: zerodds-idl-rust-1.0 §6.2.
#[must_use]
pub fn escape_keyword(ident: &str) -> String {
    if is_rust_keyword(ident) {
        format!("r#{ident}")
    } else {
        ident.to_string()
    }
}

/// List of Rust 2024 edition reserved keywords. Source:
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
        // wstring → distinct `WString` wrapper: GIOP-1.2 wire = uint32
        // length-in-octets + UTF-16 code units (message byte order), no
        // terminator. NOT `String` (UTF-8) — otherwise wstring would be
        // indistinguishable from string on the wire and incompatible with foreign ORBs.
        "zerodds_cdr::WString".to_string()
    } else {
        "String".to_string()
    }
}

/// Computes the wire-size bound for a fixed-size IDL type spec.
/// Returns `None` for sequence, string, scoped (= dynamic / unclear
/// size).
#[must_use]
pub fn wire_size_bound(spec: &TypeSpec) -> Option<usize> {
    match spec {
        TypeSpec::Primitive(p) => Some(primitive_wire_size(*p)),
        TypeSpec::Sequence(_) | TypeSpec::String(_) | TypeSpec::Scoped(_) => None,
        TypeSpec::Fixed(_) | TypeSpec::Map(_) | TypeSpec::Any => None,
    }
}

/// Evaluates a `ConstExpr` as `usize` (only for simple integer
/// literals — array sizes, sequence bounds, string bounds).
#[must_use]
pub fn const_expr_as_usize(expr: &ConstExpr) -> Option<usize> {
    use zerodds_idl::ast::types::{ConstExpr as CE, LiteralKind};
    match expr {
        CE::Literal(lit) if lit.kind == LiteralKind::Integer => {
            // raw can be e.g. "42", "0x10", "0b1010", "0o17".
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
