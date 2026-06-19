// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Mapping of IDL primitives to Java type strings.
//!
//! Follows OMG IDL4-Java-Mapping v1.0 §6 (Type Mapping). Java has no
//! native unsigned integer types — the spec-conformant workaround:
//!
//! - `unsigned short` → Java `int` (not `short`, to cover the full
//!   value range).
//! - `unsigned long` → Java `long` (not `int`).
//! - `unsigned long long` → Java `long` (with doc note; the default is
//!   the `long` variant; `BigInteger` would be the full solution,
//!   but is outside the current scope).
//!
//! `boolean` → `boolean`, `octet` → `byte`, `char`/`wchar` → `char`,
//! `string`/`wstring` → `String`, `float`/`double` → `float`/`double`.

use zerodds_idl::ast::{FloatingType, IntegerType, PrimitiveType};

/// Mapping from a [`PrimitiveType`] to its Java type.
#[must_use]
pub fn primitive_to_java(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "boolean",
        PrimitiveType::Octet => "byte",
        PrimitiveType::Char => "char",
        PrimitiveType::WideChar => "char",
        PrimitiveType::Integer(i) => integer_to_java(i),
        PrimitiveType::Floating(f) => floating_to_java(f),
    }
}

/// Mapping from integer subtypes.
///
/// Spec-conformant unsigned workaround:
/// - `unsigned short` → `int` (16-bit range does not fit in `short`).
/// - `unsigned long` → `long` (32-bit range does not fit in `int`).
/// - `unsigned long long` → `long` (with sign wrap; see doc).
#[must_use]
pub fn integer_to_java(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "short",
        IntegerType::Long | IntegerType::Int32 => "int",
        IntegerType::LongLong | IntegerType::Int64 => "long",
        // Unsigned workaround: use the next wider signed type.
        IntegerType::UShort | IntegerType::UInt16 => "int",
        IntegerType::ULong | IntegerType::UInt32 => "long",
        // unsigned long long: Java has no 64-bit unsigned integer.
        // Default: `long` (signed, with doc comment in the emitter).
        IntegerType::ULongLong | IntegerType::UInt64 => "long",
        IntegerType::Int8 => "byte",
        IntegerType::UInt8 => "short",
    }
}

/// Mapping from floating-point subtypes. `long double` is outside the current scope and is
/// handled in `typespec_to_java` as [`crate::error::JavaGenError::UnsupportedConstruct`].
#[must_use]
pub fn floating_to_java(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "float",
        FloatingType::Double => "double",
        FloatingType::LongDouble => "double",
    }
}

/// Returns `true` if an IDL integer type requires the Java unsigned workaround
/// (doc note in the generator).
#[must_use]
pub fn is_unsigned(i: IntegerType) -> bool {
    matches!(
        i,
        IntegerType::UShort
            | IntegerType::UInt16
            | IntegerType::ULong
            | IntegerType::UInt32
            | IntegerType::ULongLong
            | IntegerType::UInt64
            | IntegerType::UInt8
    )
}

/// Boxed wrapper class for a primitive Java type, used for generics
/// (e.g. `List<Integer>` instead of `List<int>`).
#[must_use]
pub fn primitive_to_java_boxed(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "Boolean",
        PrimitiveType::Octet => "Byte",
        PrimitiveType::Char => "Character",
        PrimitiveType::WideChar => "Character",
        PrimitiveType::Integer(i) => integer_to_java_boxed(i),
        PrimitiveType::Floating(f) => floating_to_java_boxed(f),
    }
}

/// Boxed variant for integer subtypes.
#[must_use]
pub fn integer_to_java_boxed(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "Short",
        IntegerType::Long | IntegerType::Int32 => "Integer",
        IntegerType::LongLong | IntegerType::Int64 => "Long",
        IntegerType::UShort | IntegerType::UInt16 => "Integer",
        IntegerType::ULong | IntegerType::UInt32 => "Long",
        IntegerType::ULongLong | IntegerType::UInt64 => "Long",
        IntegerType::Int8 => "Byte",
        IntegerType::UInt8 => "Short",
    }
}

/// Boxed variant for floating-point subtypes.
#[must_use]
pub fn floating_to_java_boxed(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "Float",
        FloatingType::Double => "Double",
        FloatingType::LongDouble => "Double",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn primitive_boolean() {
        assert_eq!(primitive_to_java(PrimitiveType::Boolean), "boolean");
    }

    #[test]
    fn primitive_octet() {
        assert_eq!(primitive_to_java(PrimitiveType::Octet), "byte");
    }

    #[test]
    fn primitive_char() {
        assert_eq!(primitive_to_java(PrimitiveType::Char), "char");
    }

    #[test]
    fn primitive_wchar() {
        assert_eq!(primitive_to_java(PrimitiveType::WideChar), "char");
    }

    #[test]
    fn integer_short_signed() {
        assert_eq!(integer_to_java(IntegerType::Short), "short");
    }

    #[test]
    fn integer_long_signed() {
        assert_eq!(integer_to_java(IntegerType::Long), "int");
    }

    #[test]
    fn integer_long_long_signed() {
        assert_eq!(integer_to_java(IntegerType::LongLong), "long");
    }

    #[test]
    fn integer_unsigned_short_widens_to_int() {
        // Spec §6.2: unsigned short → int (16-bit value range does not
        // fit in Java `short`, since it is signed).
        assert_eq!(integer_to_java(IntegerType::UShort), "int");
    }

    #[test]
    fn integer_unsigned_long_widens_to_long() {
        assert_eq!(integer_to_java(IntegerType::ULong), "long");
    }

    #[test]
    fn integer_unsigned_long_long_keeps_long() {
        // Default workaround: `long` (sign wrap accepted,
        // stretch goal: BigInteger).
        assert_eq!(integer_to_java(IntegerType::ULongLong), "long");
    }

    #[test]
    fn integer_explicit_widths() {
        assert_eq!(integer_to_java(IntegerType::Int8), "byte");
        assert_eq!(integer_to_java(IntegerType::UInt8), "short");
        assert_eq!(integer_to_java(IntegerType::Int16), "short");
        assert_eq!(integer_to_java(IntegerType::UInt16), "int");
        assert_eq!(integer_to_java(IntegerType::Int32), "int");
        assert_eq!(integer_to_java(IntegerType::UInt32), "long");
        assert_eq!(integer_to_java(IntegerType::Int64), "long");
        assert_eq!(integer_to_java(IntegerType::UInt64), "long");
    }

    #[test]
    fn floating_float_double() {
        assert_eq!(floating_to_java(FloatingType::Float), "float");
        assert_eq!(floating_to_java(FloatingType::Double), "double");
    }

    #[test]
    fn unsigned_marker_is_correct() {
        assert!(!is_unsigned(IntegerType::Short));
        assert!(is_unsigned(IntegerType::UShort));
        assert!(!is_unsigned(IntegerType::Long));
        assert!(is_unsigned(IntegerType::ULong));
        assert!(is_unsigned(IntegerType::ULongLong));
    }

    #[test]
    fn boxed_long_is_long() {
        assert_eq!(integer_to_java_boxed(IntegerType::LongLong), "Long");
    }

    #[test]
    fn boxed_int_is_integer() {
        assert_eq!(integer_to_java_boxed(IntegerType::Long), "Integer");
    }

    #[test]
    fn boxed_unsigned_long_is_long() {
        assert_eq!(integer_to_java_boxed(IntegerType::ULong), "Long");
    }

    #[test]
    fn boxed_primitive_boolean_is_capital_boolean() {
        assert_eq!(primitive_to_java_boxed(PrimitiveType::Boolean), "Boolean");
    }

    #[test]
    fn boxed_octet_is_byte_capital() {
        assert_eq!(primitive_to_java_boxed(PrimitiveType::Octet), "Byte");
    }

    #[test]
    fn boxed_floats() {
        assert_eq!(floating_to_java_boxed(FloatingType::Float), "Float");
        assert_eq!(floating_to_java_boxed(FloatingType::Double), "Double");
    }
}
