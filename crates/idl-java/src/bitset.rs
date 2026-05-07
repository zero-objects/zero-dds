// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bitset/Bitmask → Java-Codegen (C5.4-b Cluster E).
//!
//! Java hat keine native Bitset-Syntax. Wir waehlen folgendes Mapping
//! (dokumentiert in `runtime/README.md`):
//!
//! ## Bitmask
//! `@bit_bound(N) bitmask Flags { @position(K) READ, ... };`
//! → ein Inner-Enum `Flag` mit den Werten + ein Wrapper-Class
//! `Flags`, deren Feld `EnumSet<Flag> bits` ist. Vorteil ggue.
//! `java.util.BitSet`: typsicher, idiomatisch, statisch begrenzt auf
//! die deklarierten Werte. `bit_bound` >= 64 → Mapping bleibt; der
//! parser erzwingt schon `bit_bound <= 64`.
//!
//! ## Bitset
//! `bitset MyBits { bitfield<3> a; bitfield<5> b; };`
//! → eine Klasse `MyBits` mit privatem `long bits`-Feld + Getter/
//! Setter pro Bitfield, die Maske + Shift inline in den Code
//! emittieren. Anonyme Bitfields (`name == None`) erhoehen nur den
//! Bit-Cursor. Summe der Widths > 64 → harter Fehler
//! [`JavaGenError::UnsupportedConstruct`].
//!
//! Beide Konstrukte sind self-contained — sie referenzieren keine
//! Runtime-Hilfen. Die Annotations `@bit_bound` und `@position` werden
//! im Member-Layout aufgegangen, nicht im Java-Quelltext gespiegelt.

use std::fmt::Write;

use zerodds_idl::ast::{
    Annotation, AnnotationParams, BitmaskDecl, BitsetDecl, ConstExpr, LiteralKind,
};

use crate::JavaGenOptions;
use crate::emitter::{JavaFile, fmt_err, indent_unit, wrap_compilation_unit_default};
use crate::error::JavaGenError;
use crate::keywords::sanitize_identifier;

const DEFAULT_BIT_BOUND: u32 = 32;
/// Java-`long` ist 64 Bit signed — das ist die maximale Breite, die
/// wir fuer Bitset/Bitmask abbilden koennen.
const MAX_BIT_BOUND: u32 = 64;

/// Bitmask → Wrapper-Klasse `Flags` mit nested-Enum `Flag` und
/// `EnumSet<Flag> bits`-Feld.
pub(crate) fn emit_bitmask_file(
    b: &BitmaskDecl,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&b.name.text)?;
    let ind = indent_unit(opts);
    let bit_bound =
        extract_int_annotation(&b.annotations, "bit_bound").unwrap_or(DEFAULT_BIT_BOUND);
    if bit_bound > MAX_BIT_BOUND {
        return Err(JavaGenError::UnsupportedConstruct {
            construct: format!("bitmask bit_bound {bit_bound} > 64"),
            context: Some(b.name.text.clone()),
        });
    }

    let mut body = String::new();
    writeln!(body, "public final class {class} {{").map_err(fmt_err)?;
    writeln!(body, "{ind}/** IDL `@bit_bound({bit_bound})` */").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public static final int BIT_BOUND = {bit_bound};"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // Inner enum `Flag` with explicit bit-positions.
    writeln!(body, "{ind}/** Bit-position enum for {class}. */").map_err(fmt_err)?;
    writeln!(body, "{ind}public enum Flag {{").map_err(fmt_err)?;
    let mut next_pos: u32 = 0;
    let n_values = b.values.len();
    for (idx, v) in b.values.iter().enumerate() {
        let name = sanitize_identifier(&v.name.text)?;
        let pos = extract_int_annotation(&v.annotations, "position").unwrap_or(next_pos);
        next_pos = pos + 1;
        let sep = if idx + 1 == n_values { ';' } else { ',' };
        writeln!(body, "{ind}{ind}{name}({pos}){sep}").map_err(fmt_err)?;
    }
    if n_values == 0 {
        // Empty bitmask — emit a marker `;` to keep enum syntactically valid.
        writeln!(body, "{ind}{ind};").map_err(fmt_err)?;
    }
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}private final int position;").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}Flag(int position) {{ this.position = position; }}"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}public int position() {{ return position; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // Wrapper field + accessors.
    writeln!(
        body,
        "{ind}private java.util.EnumSet<Flag> bits = java.util.EnumSet.noneOf(Flag.class);",
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "{ind}public {class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public java.util.EnumSet<Flag> bits() {{ return bits; }}"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public void setBits(java.util.EnumSet<Flag> bits) \
         {{ this.bits = java.util.EnumSet.copyOf(bits); }}",
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public boolean isSet(Flag f) {{ return bits.contains(f); }}",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}public void set(Flag f) {{ bits.add(f); }}").map_err(fmt_err)?;
    writeln!(body, "{ind}public void clear(Flag f) {{ bits.remove(f); }}").map_err(fmt_err)?;
    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Bitset → wrapper-class with `long bits` + accessor pair per
/// (named) bitfield.
pub(crate) fn emit_bitset_file(
    b: &BitsetDecl,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&b.name.text)?;
    let ind = indent_unit(opts);

    // Cumulative bit-position cursor.
    let mut cursor: u32 = 0;
    // Pre-validate total width and gather (name, width, offset) tuples.
    struct Field {
        name: String,
        width: u32,
        offset: u32,
        anonymous: bool,
    }
    let mut fields: Vec<Field> = Vec::new();
    for bf in &b.bitfields {
        let width =
            bitfield_width(&bf.spec.width).ok_or_else(|| JavaGenError::UnsupportedConstruct {
                construct: format!(
                    "bitfield width must be integer literal in '{}'",
                    b.name.text
                ),
                context: Some(b.name.text.clone()),
            })?;
        if width == 0 || width > MAX_BIT_BOUND {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: format!("bitfield width {width} (must be 1..=64)"),
                context: Some(b.name.text.clone()),
            });
        }
        let offset = cursor;
        cursor = cursor.saturating_add(width);
        if cursor > MAX_BIT_BOUND {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: format!("bitset cumulative width {cursor} > 64 (Java-long-Backing)"),
                context: Some(b.name.text.clone()),
            });
        }
        let (sane_name, anonymous) = match &bf.name {
            Some(id) => (sanitize_identifier(&id.text)?, false),
            None => (format!("_pad_{offset}"), true),
        };
        fields.push(Field {
            name: sane_name,
            width,
            offset,
            anonymous,
        });
    }

    let mut body = String::new();
    writeln!(body, "public final class {class} {{").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}/** Cumulative bit-storage (width = {cursor}). */"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}public static final int BIT_WIDTH = {cursor};").map_err(fmt_err)?;
    writeln!(body, "{ind}private long bits;").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "{ind}public {class}() {{}}").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class}(long bits) {{ this.bits = bits; }}",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}public long rawBits() {{ return bits; }}").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public void setRawBits(long bits) {{ this.bits = bits; }}",
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    for f in &fields {
        if f.anonymous {
            // Padding-Bitfield — kein Accessor, nur Layout-Kommentar.
            writeln!(
                body,
                "{ind}// padding: width={} offset={}",
                f.width, f.offset,
            )
            .map_err(fmt_err)?;
            continue;
        }
        let mask: u64 = if f.width == 64 {
            u64::MAX
        } else {
            (1u64 << f.width) - 1
        };
        let cap = capitalize(&f.name);
        let return_ty = if f.width <= 32 { "int" } else { "long" };
        let cast_in = if f.width <= 32 { "(int)" } else { "" };
        let cast_set = if f.width <= 32 { "(long)" } else { "" };
        writeln!(body, "{ind}/** width={} offset={} */", f.width, f.offset,).map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}public {return_ty} get{cap}() {{ return {cast_in}((bits >>> {offset}) & 0x{mask:X}L); }}",
            offset = f.offset,
        )
        .map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}public void set{cap}({return_ty} value) {{ \
             bits = (bits & ~(0x{mask:X}L << {offset})) | \
             ((({cast_set}value) & 0x{mask:X}L) << {offset}); }}",
            offset = f.offset,
        )
        .map_err(fmt_err)?;
    }

    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bitfield_width(e: &ConstExpr) -> Option<u32> {
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

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::JavaGenOptions;
    use zerodds_idl::config::ParserConfig;

    fn parse(src: &str) -> zerodds_idl::ast::Specification {
        zerodds_idl::parse(src, &ParserConfig::default()).expect("parse")
    }

    fn first_bitmask(ast: &zerodds_idl::ast::Specification) -> &BitmaskDecl {
        for d in &ast.definitions {
            if let zerodds_idl::ast::Definition::Type(zerodds_idl::ast::TypeDecl::Constr(
                zerodds_idl::ast::ConstrTypeDecl::Bitmask(b),
            )) = d
            {
                return b;
            }
        }
        panic!("no bitmask in fixture");
    }

    fn first_bitset(ast: &zerodds_idl::ast::Specification) -> &BitsetDecl {
        for d in &ast.definitions {
            if let zerodds_idl::ast::Definition::Type(zerodds_idl::ast::TypeDecl::Constr(
                zerodds_idl::ast::ConstrTypeDecl::Bitset(b),
            )) = d
            {
                return b;
            }
        }
        panic!("no bitset in fixture");
    }

    #[test]
    fn bitmask_default_bit_bound_is_32() {
        let ast = parse("bitmask Flags { READ, WRITE };");
        let b = first_bitmask(&ast);
        let f = emit_bitmask_file(b, "", &JavaGenOptions::default()).expect("ok");
        assert!(f.source.contains("BIT_BOUND = 32"));
        assert!(f.source.contains("READ(0)"));
        assert!(f.source.contains("WRITE(1)"));
    }

    #[test]
    fn bitmask_explicit_bit_bound_8_emits() {
        let ast = parse("@bit_bound(8) bitmask Flags { F0, F1 };");
        let b = first_bitmask(&ast);
        let f = emit_bitmask_file(b, "", &JavaGenOptions::default()).expect("ok");
        assert!(f.source.contains("BIT_BOUND = 8"));
    }

    #[test]
    fn bitmask_position_overrides_implicit() {
        let ast = parse("@bit_bound(16) bitmask M { @position(3) A, B };");
        let b = first_bitmask(&ast);
        let f = emit_bitmask_file(b, "", &JavaGenOptions::default()).expect("ok");
        assert!(f.source.contains("A(3)"));
        // B follows at position 4 (next_implicit after explicit 3).
        assert!(f.source.contains("B(4)"));
    }

    #[test]
    fn bitmask_uses_enumset_field() {
        let ast = parse("@bit_bound(8) bitmask M { A };");
        let b = first_bitmask(&ast);
        let f = emit_bitmask_file(b, "", &JavaGenOptions::default()).expect("ok");
        assert!(f.source.contains("java.util.EnumSet<Flag>"));
        assert!(f.source.contains("public boolean isSet(Flag f)"));
    }

    #[test]
    fn bitmask_too_large_bit_bound_errors() {
        // Construct synthetic — parser would reject >64 too, but we
        // also defend in the emitter.
        let mut b = BitmaskDecl {
            name: zerodds_idl::ast::Identifier::new("M", zerodds_idl::errors::Span::SYNTHETIC),
            values: alloc::vec![],
            annotations: alloc::vec![],
            span: zerodds_idl::errors::Span::SYNTHETIC,
        };
        // craft @bit_bound(65)
        b.annotations.push(synth_annotation("bit_bound", "65"));
        let res = emit_bitmask_file(&b, "", &JavaGenOptions::default());
        assert!(matches!(
            res,
            Err(JavaGenError::UnsupportedConstruct { .. })
        ));
    }

    #[test]
    fn bitset_simple_emits_long_backing() {
        let ast = parse("bitset MyBits { bitfield<3> a; bitfield<5> b; };");
        let b = first_bitset(&ast);
        let f = emit_bitset_file(b, "", &JavaGenOptions::default()).expect("ok");
        assert!(f.source.contains("public final class MyBits"));
        assert!(f.source.contains("private long bits;"));
        assert!(f.source.contains("public int getA()"));
        assert!(f.source.contains("public int getB()"));
        // a: width=3, offset=0 → mask 0x7
        assert!(f.source.contains("0x7L"));
        // b: width=5, offset=3 → mask 0x1F
        assert!(f.source.contains("0x1FL"));
    }

    #[test]
    fn bitset_total_width_over_64_errors() {
        let ast = parse("bitset Big { bitfield<40> a; bitfield<30> b; };");
        let b = first_bitset(&ast);
        let res = emit_bitset_file(b, "", &JavaGenOptions::default());
        assert!(
            matches!(res, Err(JavaGenError::UnsupportedConstruct { .. })),
            "expected UnsupportedConstruct, got {res:?}",
        );
    }

    #[test]
    fn bitset_with_anonymous_padding_skips_accessor() {
        let ast = parse("bitset P { bitfield<2> a; bitfield<3>; bitfield<4> b; };");
        let b = first_bitset(&ast);
        let f = emit_bitset_file(b, "", &JavaGenOptions::default()).expect("ok");
        // accessor for a + b but not the padding.
        assert!(f.source.contains("public int getA()"));
        assert!(f.source.contains("public int getB()"));
        assert!(f.source.contains("padding: width=3 offset=2"));
    }

    #[test]
    fn bitset_64bit_field_uses_long_return() {
        let ast = parse("bitset Big { bitfield<64> a; };");
        let b = first_bitset(&ast);
        let f = emit_bitset_file(b, "", &JavaGenOptions::default()).expect("ok");
        assert!(f.source.contains("public long getA()"));
    }

    #[test]
    fn bitset_large_field_above_32_returns_long() {
        let ast = parse("bitset Big { bitfield<40> a; };");
        let b = first_bitset(&ast);
        let f = emit_bitset_file(b, "", &JavaGenOptions::default()).expect("ok");
        assert!(f.source.contains("public long getA()"));
    }

    // ---- synthetic helpers (avoid IDL parse for >64 bit_bound which is rejected) ----

    extern crate alloc;

    fn synth_annotation(name: &str, raw: &str) -> zerodds_idl::ast::Annotation {
        use zerodds_idl::ast::{Identifier, Literal, ScopedName};
        use zerodds_idl::errors::Span;
        zerodds_idl::ast::Annotation {
            name: ScopedName {
                absolute: false,
                parts: alloc::vec![Identifier::new(name, Span::SYNTHETIC)],
                span: Span::SYNTHETIC,
            },
            params: AnnotationParams::Single(ConstExpr::Literal(Literal {
                kind: LiteralKind::Integer,
                raw: raw.to_string(),
                span: Span::SYNTHETIC,
            })),
            span: Span::SYNTHETIC,
        }
    }
}
