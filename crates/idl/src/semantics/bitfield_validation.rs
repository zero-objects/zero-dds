// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bitset/bitmask validation (C4.6 §1.9 / spec §7.4.13.4.3).
//!
//! - Bitmask: `@bit_bound` value >= max(position) + 1, max 64.
//! - Bitmask: `@position` values unique + in the range `[0, bit_bound)`.
//! - Bitset: `bit_bound` of individual bitfields ≤ 64; sum ≤ 64.

use crate::ast::{
    Annotation, AnnotationParams, BitmaskDecl, BitsetDecl, ConstExpr, ConstrTypeDecl, Definition,
    LiteralKind, Specification, TypeDecl,
};
use crate::errors::Span;

/// Bitset/bitmask validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitfieldValidationError {
    /// `position(N)` >= `bit_bound`.
    PositionOutOfRange {
        /// Bitmask name.
        bitmask: String,
        /// Position value.
        position: u32,
        /// Effective bit_bound.
        bit_bound: u32,
        /// Source location.
        span: Span,
    },
    /// Duplicate `position` value.
    DuplicatePosition {
        /// Bitmask name.
        bitmask: String,
        /// Duplicate value.
        position: u32,
        /// Source location.
        span: Span,
    },
    /// `@bit_bound` > 64.
    BitBoundTooLarge {
        /// Construct name (bitmask or bitset).
        name: String,
        /// Value.
        value: u32,
        /// Source location.
        span: Span,
    },
    /// Bitset: sum of widths > 64.
    BitsetTotalTooLarge {
        /// Bitset name.
        name: String,
        /// Actual sum.
        total: u32,
        /// Source location.
        span: Span,
    },
    /// Bitfield width > 64.
    BitfieldWidthTooLarge {
        /// Bitset name.
        bitset: String,
        /// Width.
        width: u32,
        /// Source location.
        span: Span,
    },
    /// §7.4.13.4.3.2 — bitfield width > storage cap of the dest_type.
    /// Boolean→1, Octet→8, Short/UShort→16, Long/ULong→32,
    /// LongLong/ULongLong→64.
    BitfieldExceedsStorageCap {
        /// Bitset name.
        bitset: String,
        /// Width.
        width: u32,
        /// Maximum width for the dest_type.
        cap: u32,
        /// Name of the dest_type.
        dest_type: &'static str,
        /// Source location.
        span: Span,
    },
    /// §7.4.13.4.3 — two bitfields occupy overlapping bit ranges. A bitset
    /// packs its bitfields consecutively; an explicit `@position(N)` may pin a
    /// field's starting bit. A pinned field that overlaps the range already
    /// taken by a preceding field (explicit or implicitly advanced) is a
    /// collision — the two fields would share storage bits.
    BitfieldPositionCollision {
        /// Bitset name.
        bitset: String,
        /// Starting bit of the colliding field.
        position: u32,
        /// Width of the colliding field.
        width: u32,
        /// Source location.
        span: Span,
    },
}

impl core::fmt::Display for BitfieldValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PositionOutOfRange {
                bitmask,
                position,
                bit_bound,
                ..
            } => write!(
                f,
                "bitmask '{bitmask}': @position({position}) is out of range (bit_bound {bit_bound})"
            ),
            Self::DuplicatePosition {
                bitmask, position, ..
            } => write!(f, "bitmask '{bitmask}': duplicate @position({position})"),
            Self::BitBoundTooLarge { name, value, .. } => {
                write!(f, "'{name}': @bit_bound({value}) exceeds the maximum of 64")
            }
            Self::BitsetTotalTooLarge { name, total, .. } => write!(
                f,
                "bitset '{name}': total bitfield width {total} exceeds 64"
            ),
            Self::BitfieldWidthTooLarge { bitset, width, .. } => {
                write!(f, "bitset '{bitset}': bitfield width {width} exceeds 64")
            }
            Self::BitfieldExceedsStorageCap {
                bitset,
                width,
                cap,
                dest_type,
                ..
            } => write!(
                f,
                "bitset '{bitset}': bitfield width {width} exceeds the {dest_type} storage cap of {cap}"
            ),
            Self::BitfieldPositionCollision {
                bitset,
                position,
                width,
                ..
            } => write!(
                f,
                "bitset '{bitset}': bitfield at @position({position}) (width {width}) collides with a preceding bitfield's bit range"
            ),
        }
    }
}

impl std::error::Error for BitfieldValidationError {}

/// Top level: bitset+bitmask validation per specification.
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

/// Bitmask validation.
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

/// Bitset validation.
pub fn validate_bitset(b: &BitsetDecl, errs: &mut Vec<BitfieldValidationError>) {
    let mut total: u32 = 0;
    // Consecutive-packing cursor and the bit ranges already claimed, so an
    // explicit `@position(N)` that overlaps a preceding field is caught
    // (§7.4.13.4.3). Ranges are `[start, end)`.
    let mut cursor: u32 = 0;
    let mut occupied: Vec<(u32, u32)> = Vec::new();
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
        // §7.4.13.4.3.2: width must not be larger than the
        // storage cap of the dest_type (if specified).
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
        // §7.4.13.4.3: a bitfield starts at its explicit `@position(N)` or, in
        // its absence, at the running cursor. Overlap with any range already
        // claimed by a preceding field is a collision.
        let start = extract_annotation(&bf.annotations, "position")
            .and_then(|a| extract_int_arg(&a.params))
            .unwrap_or(cursor);
        let end = start.saturating_add(width);
        if width > 0 {
            if occupied.iter().any(|&(os, oe)| start < oe && os < end) {
                errs.push(BitfieldValidationError::BitfieldPositionCollision {
                    bitset: b.name.text.clone(),
                    position: start,
                    width,
                    span: bf.span,
                });
            }
            occupied.push((start, end));
        }
        cursor = end;
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
        // octet has cap 8.
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
        // short has cap 16.
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

    // §7.4.13.4.3 — bitfield @position collisions.

    #[test]
    fn sequential_bitfields_do_not_collide() {
        let ast = parse_to_ast("bitset BS { bitfield<3> a; bitfield<5> b; bitfield<8> c; };");
        let errs = validate_bitfields(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }

    #[test]
    fn explicit_positions_overlap_is_collision() {
        let ast =
            parse_to_ast("bitset BS { @position(0) bitfield<4> a; @position(2) bitfield<4> b; };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, BitfieldValidationError::BitfieldPositionCollision { .. })),
            "got {errs:?}"
        );
    }

    #[test]
    fn explicit_position_colliding_with_implicit_cursor_is_collision() {
        // `a` implicitly takes [0,4); `@position(2)` pins `b` into [2,6) — overlap.
        let ast = parse_to_ast("bitset BS { bitfield<4> a; @position(2) bitfield<4> b; };");
        let errs = validate_bitfields(&ast);
        assert!(
            errs.iter()
                .any(|e| matches!(e, BitfieldValidationError::BitfieldPositionCollision { .. })),
            "got {errs:?}"
        );
    }

    #[test]
    fn explicit_positions_without_overlap_pass() {
        // Adjacent, non-overlapping explicit positions.
        let ast =
            parse_to_ast("bitset BS { @position(0) bitfield<2> a; @position(2) bitfield<2> b; };");
        let errs = validate_bitfields(&ast);
        assert!(errs.is_empty(), "got {errs:?}");
    }
}
