// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bitset/Bitmask-Validierung (C4.6 §1.9 / Spec §7.4.13.4.3).
//!
//! - Bitmask: `@bit_bound`-Wert >= max(position) + 1, max 64.
//! - Bitmask: `@position`-Werte unique + im Range `[0, bit_bound)`.
//! - Bitset: `bit_bound` einzelner Bitfields ≤ 64; Summe ≤ 64.

use crate::ast::{
    Annotation, AnnotationParams, BitmaskDecl, BitsetDecl, ConstExpr, ConstrTypeDecl, Definition,
    LiteralKind, Specification, TypeDecl,
};
use crate::errors::Span;

/// Bitset/Bitmask-Validierungs-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitfieldValidationError {
    /// `position(N)` >= `bit_bound`.
    PositionOutOfRange {
        /// Bitmask-Name.
        bitmask: String,
        /// Position-Wert.
        position: u32,
        /// Effektiver bit_bound.
        bit_bound: u32,
        /// Quellort.
        span: Span,
    },
    /// Doppelter `position`-Wert.
    DuplicatePosition {
        /// Bitmask-Name.
        bitmask: String,
        /// Doppelter Wert.
        position: u32,
        /// Quellort.
        span: Span,
    },
    /// `@bit_bound` > 64.
    BitBoundTooLarge {
        /// Konstrukt-Name (Bitmask oder Bitset).
        name: String,
        /// Wert.
        value: u32,
        /// Quellort.
        span: Span,
    },
    /// Bitset: Summe der Widths > 64.
    BitsetTotalTooLarge {
        /// Bitset-Name.
        name: String,
        /// Tatsaechliche Summe.
        total: u32,
        /// Quellort.
        span: Span,
    },
    /// Bitfield-Width > 64.
    BitfieldWidthTooLarge {
        /// Bitset-Name.
        bitset: String,
        /// Width.
        width: u32,
        /// Quellort.
        span: Span,
    },
    /// §7.4.13.4.3.2 — Bitfield-Width > Storage-Cap des dest_type.
    /// Boolean→1, Octet→8, Short/UShort→16, Long/ULong→32,
    /// LongLong/ULongLong→64.
    BitfieldExceedsStorageCap {
        /// Bitset-Name.
        bitset: String,
        /// Width.
        width: u32,
        /// Maximum-Width fuer den dest_type.
        cap: u32,
        /// Name des dest_type.
        dest_type: &'static str,
        /// Quellort.
        span: Span,
    },
}

/// Top-Level: Bitset+Bitmask-Validierung pro Specification.
#[must_use]
pub fn validate_bitfields(spec: &Specification) -> Vec<BitfieldValidationError> {
    let mut errs = Vec::new();
    for d in &spec.definitions {
        walk(d, &mut errs);
    }
    errs
}

/// zerodds-lint: recursion-depth 32
fn walk(d: &Definition, errs: &mut Vec<BitfieldValidationError>) {
    match d {
        Definition::Module(m) => {
            for inner in &m.definitions {
                walk(inner, errs);
            }
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
            validate_bitmask(b, errs);
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
            validate_bitset(b, errs);
        }
        _ => {}
    }
}

fn extract_int_arg(p: &AnnotationParams) -> Option<u32> {
    if let AnnotationParams::Single(ConstExpr::Literal(l)) = p {
        if matches!(l.kind, LiteralKind::Integer) {
            return l.raw.parse::<u32>().ok();
        }
    }
    None
}

fn extract_annotation<'a>(anns: &'a [Annotation], name: &str) -> Option<&'a Annotation> {
    anns.iter()
        .find(|a| a.name.parts.last().map(|p| p.text.as_str()) == Some(name))
}

/// Bitmask-Validierung.
pub fn validate_bitmask(b: &BitmaskDecl, errs: &mut Vec<BitfieldValidationError>) {
    let bit_bound = extract_annotation(&b.annotations, "bit_bound")
        .and_then(|a| extract_int_arg(&a.params))
        .unwrap_or(32); // Default lt. Spec §7.3.1.2.1.6 = 32

    if bit_bound > 64 {
        errs.push(BitfieldValidationError::BitBoundTooLarge {
            name: b.name.text.clone(),
            value: bit_bound,
            span: b.span,
        });
    }

    let mut seen: Vec<u32> = Vec::new();
    let mut next_implicit: u32 = 0;
    for v in &b.values {
        let pos = extract_annotation(&v.annotations, "position")
            .and_then(|a| extract_int_arg(&a.params))
            .unwrap_or(next_implicit);
        next_implicit = pos + 1;
        if pos >= bit_bound {
            errs.push(BitfieldValidationError::PositionOutOfRange {
                bitmask: b.name.text.clone(),
                position: pos,
                bit_bound,
                span: v.span,
            });
        }
        if seen.contains(&pos) {
            errs.push(BitfieldValidationError::DuplicatePosition {
                bitmask: b.name.text.clone(),
                position: pos,
                span: v.span,
            });
        } else {
            seen.push(pos);
        }
    }
}

/// Bitset-Validierung.
pub fn validate_bitset(b: &BitsetDecl, errs: &mut Vec<BitfieldValidationError>) {
    let mut total: u32 = 0;
    for bf in &b.bitfields {
        let width = if let ConstExpr::Literal(l) = &bf.spec.width {
            l.raw.parse::<u32>().unwrap_or(0)
        } else {
            0
        };
        if width > 64 {
            errs.push(BitfieldValidationError::BitfieldWidthTooLarge {
                bitset: b.name.text.clone(),
                width,
                span: bf.span,
            });
        }
        // §7.4.13.4.3.2: width darf nicht groesser sein als der
        // Storage-Cap des dest_type (falls angegeben).
        if let Some((cap, name)) = bf.spec.dest_type.and_then(dest_type_cap) {
            if width > cap {
                errs.push(BitfieldValidationError::BitfieldExceedsStorageCap {
                    bitset: b.name.text.clone(),
                    width,
                    cap,
                    dest_type: name,
                    span: bf.span,
                });
            }
        }
        total = total.saturating_add(width);
    }
    if total > 64 {
        errs.push(BitfieldValidationError::BitsetTotalTooLarge {
            name: b.name.text.clone(),
            total,
            span: b.span,
        });
    }
}

/// §7.4.13.4.3.2 — Storage-Cap pro dest_type.
fn dest_type_cap(dt: crate::ast::PrimitiveType) -> Option<(u32, &'static str)> {
    use crate::ast::{IntegerType, PrimitiveType};
    Some(match dt {
        PrimitiveType::Boolean => (1, "boolean"),
        PrimitiveType::Octet => (8, "octet"),
        PrimitiveType::Char | PrimitiveType::WideChar => return None,
        PrimitiveType::Integer(it) => match it {
            IntegerType::Int8 | IntegerType::UInt8 => (8, "int8/uint8"),
            IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
                (16, "short/uint16")
            }
            IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => {
                (32, "long/uint32")
            }
            IntegerType::LongLong
            | IntegerType::ULongLong
            | IntegerType::Int64
            | IntegerType::UInt64 => (64, "long long/uint64"),
        },
        PrimitiveType::Floating(_) => return None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn parse_to_ast(src: &str) -> Specification {
        parse(src, &ParserConfig::default()).expect("parse ok")
    }

    #[test]
    fn position_within_bit_bound_ok() {
        let ast = parse_to_ast("@bit_bound(8) bitmask Flags { @position(0) F0, @position(1) F1 };");
        let errs = validate_bitfields(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn position_out_of_range_errors() {
        let ast = parse_to_ast("@bit_bound(4) bitmask Flags { @position(0) F0, @position(8) F1 };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, BitfieldValidationError::PositionOutOfRange { .. }))
        );
    }

    #[test]
    fn duplicate_position_errors() {
        let ast = parse_to_ast("bitmask Flags { @position(2) F0, @position(2) F1 };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, BitfieldValidationError::DuplicatePosition { .. }))
        );
    }

    #[test]
    fn implicit_positions_increment() {
        let ast = parse_to_ast("@bit_bound(4) bitmask Flags { F0, F1, F2, F3 };");
        let errs = validate_bitfields(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn implicit_positions_overflow_bound() {
        let ast = parse_to_ast("@bit_bound(2) bitmask Flags { F0, F1, F2 };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, BitfieldValidationError::PositionOutOfRange { .. }))
        );
    }

    #[test]
    fn bit_bound_above_64_errors() {
        let ast = parse_to_ast("@bit_bound(128) bitmask Flags { F0 };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, BitfieldValidationError::BitBoundTooLarge { .. }))
        );
    }

    // §7.4.13.4.3.2 — Bitfield-Width vs Storage-Type-Cap

    #[test]
    fn bitfield_width_within_storage_cap_ok() {
        // boolean cap = 1, octet cap = 8, short cap = 16, long cap = 32.
        let ast = parse_to_ast(
            "bitset BS {\n\
                bitfield<1, boolean> b;\n\
                bitfield<8, octet> o;\n\
                bitfield<16, short> s;\n\
            };",
        );
        let errs = validate_bitfields(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn bitfield_size_exceeds_octet_destination_is_error() {
        // octet hat Cap 8.
        let ast = parse_to_ast("bitset BS { bitfield<9, octet> b; };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter().any(|e| matches!(
                e,
                BitfieldValidationError::BitfieldExceedsStorageCap {
                    cap: 8,
                    width: 9,
                    ..
                }
            )),
            "got {errs:?}"
        );
    }

    #[test]
    fn bitfield_size_exceeds_short_destination_is_error() {
        // short hat Cap 16.
        let ast = parse_to_ast("bitset BS { bitfield<17, short> b; };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter().any(|e| matches!(
                e,
                BitfieldValidationError::BitfieldExceedsStorageCap {
                    cap: 16,
                    width: 17,
                    ..
                }
            )),
            "got {errs:?}"
        );
    }
}
