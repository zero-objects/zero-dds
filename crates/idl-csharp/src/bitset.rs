// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bitset/Bitmask → C# 12 Codegen (Spec idl4-csharp 1.0 §7.14.3.2/3).
//!
//! ## Bitset
//!
//! `bitset MyBits { bitfield<3> a; bitfield<5> b; };` →
//! ```csharp
//! public struct MyBits {
//!     public ulong Value;
//!     public ulong A {
//!         readonly get => (Value >> 0) & 0x7UL;
//!         set => Value = (Value & ~(0x7UL << 0)) | ((value & 0x7UL) << 0);
//!     }
//!     ...
//! }
//! ```
//!
//! ## Bitmask
//!
//! `@bit_bound(N) bitmask Flags { @position(K) READ, ... };` →
//! ```csharp
//! [Flags]
//! public enum Flags : <Underlying> {
//!     READ = 1UL << K,
//!     ...
//! }
//! ```
//! `[Flags]` aktiviert C#-Bitwise-Operatoren-Defaults; Pascal-Case-
//! Konvention gilt analog zu Struct-Properties.

use std::fmt::Write;

use zerodds_idl::ast::{
    Annotation, AnnotationParams, BitmaskDecl, BitsetDecl, ConstExpr, LiteralKind,
};

use crate::error::CsGenError;
use crate::keywords::escape_identifier;

const DEFAULT_BIT_BOUND: u32 = 32;
const MAX_BIT_BOUND: u32 = 64;

fn fmt_err(_e: std::fmt::Error) -> CsGenError {
    CsGenError::Internal("string formatting failed".into())
}

fn underlying_type_for(bit_bound: u32) -> &'static str {
    match bit_bound {
        0..=8 => "byte",
        9..=16 => "ushort",
        17..=32 => "uint",
        _ => "ulong",
    }
}

fn pascal_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn emit_bitmask(
    out: &mut String,
    indent: &str,
    inner: &str,
    b: &BitmaskDecl,
) -> Result<(), CsGenError> {
    let name = escape_identifier(&b.name.text)?;
    let bit_bound =
        extract_int_annotation(&b.annotations, "bit_bound").unwrap_or(DEFAULT_BIT_BOUND);
    if bit_bound > MAX_BIT_BOUND {
        return Err(CsGenError::UnsupportedConstruct {
            construct: format!("bitmask bit_bound {bit_bound} > 64"),
            context: Some(name),
        });
    }
    let underlying = underlying_type_for(bit_bound);

    writeln!(
        out,
        "{indent}/// <summary>IDL `@bit_bound({bit_bound})`</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}[System.Flags]").map_err(fmt_err)?;
    writeln!(out, "{indent}public enum {name} : {underlying}").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let mut next_pos: u32 = 0;
    for v in &b.values {
        let pos = extract_int_annotation(&v.annotations, "position").unwrap_or(next_pos);
        next_pos = pos.saturating_add(1);
        // Enum-Literale behalten ihre IDL-Schreibung (Spec idl4-csharp
        // §7.2.4.4.4: "names of the IDL enumeration constants").
        let lit_name = escape_identifier(&v.name.text)?;
        writeln!(out, "{inner}{lit_name} = 1UL << {pos},").map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

pub(crate) fn emit_bitset(
    out: &mut String,
    indent: &str,
    inner: &str,
    b: &BitsetDecl,
) -> Result<(), CsGenError> {
    let name = escape_identifier(&b.name.text)?;

    let mut total_width: u32 = 0;
    let mut entries: Vec<(Option<String>, u32, u32)> = Vec::new();
    for f in &b.bitfields {
        let width =
            const_expr_to_u32(&f.spec.width).ok_or_else(|| CsGenError::UnsupportedConstruct {
                construct: "bitset width must be const integer".into(),
                context: Some(name.clone()),
            })?;
        let offset = total_width;
        total_width =
            total_width
                .checked_add(width)
                .ok_or_else(|| CsGenError::UnsupportedConstruct {
                    construct: "bitset total width overflow".into(),
                    context: Some(name.clone()),
                })?;
        if total_width > MAX_BIT_BOUND {
            return Err(CsGenError::UnsupportedConstruct {
                construct: format!("bitset total width {total_width} > {MAX_BIT_BOUND}"),
                context: Some(name.clone()),
            });
        }
        let field_name = f.name.as_ref().map(|id| pascal_case(&id.text));
        entries.push((field_name, width, offset));
    }

    writeln!(out, "{indent}public struct {name}").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    writeln!(out, "{inner}public ulong Value;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    for (field_name, width, offset) in &entries {
        let Some(fname) = field_name else { continue };
        let mask: u64 = if *width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        writeln!(out, "{inner}public ulong {fname}").map_err(fmt_err)?;
        writeln!(out, "{inner}{{").map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}    readonly get => (Value >> {offset}) & 0x{mask:X}UL;"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}    set => Value = (Value & ~(0x{mask:X}UL << {offset})) | ((value & 0x{mask:X}UL) << {offset});"
        )
        .map_err(fmt_err)?;
        writeln!(out, "{inner}}}").map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
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
