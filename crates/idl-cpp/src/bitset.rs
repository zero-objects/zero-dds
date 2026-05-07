// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bitset/Bitmask → C++17-Codegen (Spec idl4-cpp 1.0 §7.14.3.2/3).
//!
//! ## Bitset
//!
//! `bitset MyBits { bitfield<3> a; bitfield<5> b; };` →
//! ```cpp
//! struct MyBits {
//!     uint64_t value{};
//!     uint64_t a() const { return (value >> 0) & 0x7; }
//!     void a(uint64_t v) { value = (value & ~(0x7ULL << 0)) | ((v & 0x7) << 0); }
//!     ...
//! };
//! ```
//! Anonyme Bitfields (kein Name) erhoehen nur den Bit-Cursor.
//! Summe der Widths > 64 → harter Fehler.
//!
//! ## Bitmask
//!
//! `@bit_bound(N) bitmask Flags { @position(K) READ, ... };` →
//! ```cpp
//! enum class Flags : uint{N}_t {
//!     READ = 1ULL << K,
//!     ...
//! };
//! constexpr Flags operator|(Flags a, Flags b) noexcept {
//!     return static_cast<Flags>(static_cast<uint{N}_t>(a) | static_cast<uint{N}_t>(b));
//! }
//! /* analog &, ^, ~ */
//! ```
//! Cross-Ref: idl4-java analog mit `EnumSet<Flag>`. Der C++-Pfad
//! nutzt `enum class : underlying_type` direkt — typsicher und
//! ohne Wrapper.

use std::fmt::Write;

use zerodds_idl::ast::{
    Annotation, AnnotationParams, BitmaskDecl, BitsetDecl, ConstExpr, LiteralKind,
};

use crate::error::CppGenError;

const DEFAULT_BIT_BOUND: u32 = 32;
const MAX_BIT_BOUND: u32 = 64;

fn fmt_err(_e: std::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

/// Liefert den passenden `uintN_t`-Underlying-Type fuer ein gegebenes
/// `bit_bound` (Spec §7.14.3.3 Tab.7.12).
fn underlying_type_for(bit_bound: u32) -> &'static str {
    match bit_bound {
        0..=8 => "uint8_t",
        9..=16 => "uint16_t",
        17..=32 => "uint32_t",
        _ => "uint64_t",
    }
}

/// Bitmask → `enum class : underlying` mit Bitwise-Operatoren.
pub(crate) fn emit_bitmask(
    out: &mut String,
    indent: &str,
    inner: &str,
    b: &BitmaskDecl,
) -> Result<(), CppGenError> {
    let name = &b.name.text;
    let bit_bound =
        extract_int_annotation(&b.annotations, "bit_bound").unwrap_or(DEFAULT_BIT_BOUND);
    if bit_bound > MAX_BIT_BOUND {
        return Err(CppGenError::UnsupportedConstruct {
            construct: format!("bitmask bit_bound {bit_bound} > 64"),
            context: Some(name.clone()),
        });
    }
    let underlying = underlying_type_for(bit_bound);

    writeln!(out, "{indent}/** IDL `@bit_bound({bit_bound})` */").map_err(fmt_err)?;
    writeln!(out, "{indent}enum class {name} : {underlying} {{").map_err(fmt_err)?;
    let mut next_pos: u32 = 0;
    for v in &b.values {
        let pos = extract_int_annotation(&v.annotations, "position").unwrap_or(next_pos);
        next_pos = pos.saturating_add(1);
        writeln!(out, "{inner}{} = 1ULL << {pos},", v.name.text).map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}};").map_err(fmt_err)?;

    // Bitwise Operators (Spec §7.14.3.3): |, &, ^, ~ + Compound-Forms.
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}constexpr {name} operator|({name} a, {name} b) noexcept {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}return static_cast<{name}>(static_cast<{underlying}>(a) | static_cast<{underlying}>(b));"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}constexpr {name} operator&({name} a, {name} b) noexcept {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}return static_cast<{name}>(static_cast<{underlying}>(a) & static_cast<{underlying}>(b));"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}constexpr {name} operator^({name} a, {name} b) noexcept {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}return static_cast<{name}>(static_cast<{underlying}>(a) ^ static_cast<{underlying}>(b));"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}constexpr {name} operator~({name} a) noexcept {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}return static_cast<{name}>(~static_cast<{underlying}>(a));"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Bitset → `struct` mit `uint64_t value` + Getter/Setter pro
/// Bitfield (Mask + Shift inline).
pub(crate) fn emit_bitset(
    out: &mut String,
    indent: &str,
    inner: &str,
    b: &BitsetDecl,
) -> Result<(), CppGenError> {
    let name = &b.name.text;

    // Width-Calculation analog Java-Path.
    let mut total_width: u32 = 0;
    let mut entries: Vec<(Option<&str>, u32, u32)> = Vec::new(); // (name, width, offset)
    for f in &b.bitfields {
        let width =
            const_expr_to_u32(&f.spec.width).ok_or_else(|| CppGenError::UnsupportedConstruct {
                construct: "bitset width must be const integer".into(),
                context: Some(name.clone()),
            })?;
        let offset = total_width;
        total_width =
            total_width
                .checked_add(width)
                .ok_or_else(|| CppGenError::UnsupportedConstruct {
                    construct: "bitset total width overflow".into(),
                    context: Some(name.clone()),
                })?;
        if total_width > MAX_BIT_BOUND {
            return Err(CppGenError::UnsupportedConstruct {
                construct: format!("bitset total width {total_width} > {MAX_BIT_BOUND}"),
                context: Some(name.clone()),
            });
        }
        entries.push((f.name.as_ref().map(|i| i.text.as_str()), width, offset));
    }

    writeln!(out, "{indent}struct {name} {{").map_err(fmt_err)?;
    writeln!(out, "{inner}uint64_t value{{}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    for (field_name, width, offset) in &entries {
        let Some(fname) = field_name else {
            continue; // anonyme Padding-Bitfields nicht exposen
        };
        let mask: u64 = if *width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        writeln!(
            out,
            "{inner}uint64_t {fname}() const noexcept {{ return (value >> {offset}) & 0x{mask:X}ULL; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}void {fname}(uint64_t v) noexcept {{ value = (value & ~(0x{mask:X}ULL << {offset})) | ((v & 0x{mask:X}ULL) << {offset}); }}"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn const_expr_to_u32(e: &ConstExpr) -> Option<u32> {
    if let ConstExpr::Literal(l) = e {
        if matches!(l.kind, LiteralKind::Integer) {
            return l.raw.parse::<u32>().ok();
        }
    }
    None
}

fn extract_int_annotation(anns: &[Annotation], name: &str) -> Option<u32> {
    let a = anns
        .iter()
        .find(|a| a.name.parts.last().map(|p| p.text.as_str()) == Some(name))?;
    if let AnnotationParams::Single(ConstExpr::Literal(l)) = &a.params {
        if matches!(l.kind, LiteralKind::Integer) {
            return l.raw.parse::<u32>().ok();
        }
    }
    None
}
