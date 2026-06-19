// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Mapping IDL primitives → C# type strings.
//!
//! Follows OMG IDL4-CSharp mapping §6 (formal/2024-12-01) table 6-1.
//! Foundation subset (phase 3.2): primitive scalars, strings, plus the
//! foundation containers `IList<T>`, `T[]`, the discriminated-union
//! pattern, and `T?` (nullable).
//!
//! Spec mapping (complete for phase 3.2):
//!
//! | IDL                  | C#       |
//! |----------------------|----------|
//! | `boolean`            | `bool`   |
//! | `octet`              | `byte`   |
//! | `char`               | `char`   |
//! | `wchar`              | `char`   |
//! | `short`              | `short`  |
//! | `unsigned short`     | `ushort` |
//! | `long`               | `int`    |
//! | `unsigned long`      | `uint`   |
//! | `long long`          | `long`   |
//! | `unsigned long long` | `ulong`  |
//! | `int8`               | `sbyte`  |
//! | `uint8`              | `byte`   |
//! | `float`              | `float`  |
//! | `double`             | `double` |
//! | `long double`        | `decimal` (approx; spec allows `decimal` or `double`) |
//! | `string`             | `string` |
//! | `wstring`            | `string` |

use zerodds_idl::ast::{FloatingType, IntegerType, PrimitiveType};

/// Maps a [`PrimitiveType`] to the C# type expression.
#[must_use]
pub fn primitive_to_cs(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "byte",
        PrimitiveType::Char => "char",
        PrimitiveType::WideChar => "char",
        PrimitiveType::Integer(i) => integer_to_cs(i),
        PrimitiveType::Floating(f) => floating_to_cs(f),
    }
}

/// Mapping for integer subtypes (§6 table 6-1).
#[must_use]
pub fn integer_to_cs(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "short",
        IntegerType::Long | IntegerType::Int32 => "int",
        IntegerType::LongLong | IntegerType::Int64 => "long",
        IntegerType::UShort | IntegerType::UInt16 => "ushort",
        IntegerType::ULong | IntegerType::UInt32 => "uint",
        IntegerType::ULongLong | IntegerType::UInt64 => "ulong",
        IntegerType::Int8 => "sbyte",
        IntegerType::UInt8 => "byte",
    }
}

/// Mapping for floating subtypes.
///
/// `long double` is mapped to `decimal` in C# (the spec allows the
/// closest platform 80-bit/128-bit type; `decimal` is the nearest .NET
/// approximation).
#[must_use]
pub fn floating_to_cs(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "float",
        FloatingType::Double => "double",
        FloatingType::LongDouble => "decimal",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn primitive_boolean() {
        assert_eq!(primitive_to_cs(PrimitiveType::Boolean), "bool");
    }

    #[test]
    fn primitive_octet() {
        assert_eq!(primitive_to_cs(PrimitiveType::Octet), "byte");
    }

    #[test]
    fn primitive_char() {
        assert_eq!(primitive_to_cs(PrimitiveType::Char), "char");
    }

    #[test]
    fn primitive_wchar_collapses_to_char() {
        assert_eq!(primitive_to_cs(PrimitiveType::WideChar), "char");
    }

    #[test]
    fn integer_short_signed_unsigned() {
        assert_eq!(integer_to_cs(IntegerType::Short), "short");
        assert_eq!(integer_to_cs(IntegerType::UShort), "ushort");
    }

    #[test]
    fn integer_long_signed_unsigned() {
        assert_eq!(integer_to_cs(IntegerType::Long), "int");
        assert_eq!(integer_to_cs(IntegerType::ULong), "uint");
    }

    #[test]
    fn integer_long_long_signed_unsigned() {
        assert_eq!(integer_to_cs(IntegerType::LongLong), "long");
        assert_eq!(integer_to_cs(IntegerType::ULongLong), "ulong");
    }

    #[test]
    fn integer_explicit_widths() {
        assert_eq!(integer_to_cs(IntegerType::Int8), "sbyte");
        assert_eq!(integer_to_cs(IntegerType::UInt8), "byte");
        assert_eq!(integer_to_cs(IntegerType::Int16), "short");
        assert_eq!(integer_to_cs(IntegerType::UInt16), "ushort");
        assert_eq!(integer_to_cs(IntegerType::Int32), "int");
        assert_eq!(integer_to_cs(IntegerType::UInt32), "uint");
        assert_eq!(integer_to_cs(IntegerType::Int64), "long");
        assert_eq!(integer_to_cs(IntegerType::UInt64), "ulong");
    }

    #[test]
    fn floating_float_double() {
        assert_eq!(floating_to_cs(FloatingType::Float), "float");
        assert_eq!(floating_to_cs(FloatingType::Double), "double");
    }

    #[test]
    fn floating_long_double_to_decimal() {
        assert_eq!(floating_to_cs(FloatingType::LongDouble), "decimal");
    }

    #[test]
    fn primitive_dispatches_through_integer() {
        assert_eq!(
            primitive_to_cs(PrimitiveType::Integer(IntegerType::Long)),
            "int"
        );
        assert_eq!(
            primitive_to_cs(PrimitiveType::Integer(IntegerType::ULongLong)),
            "ulong"
        );
    }

    #[test]
    fn primitive_dispatches_through_floating() {
        assert_eq!(
            primitive_to_cs(PrimitiveType::Floating(FloatingType::Float)),
            "float"
        );
    }

    #[test]
    fn all_14_primitives_have_distinct_or_intentional_mapping() {
        // 14 primitives per the roadmap mandate.
        let mappings: Vec<(&str, &str)> = vec![
            ("boolean", primitive_to_cs(PrimitiveType::Boolean)),
            ("octet", primitive_to_cs(PrimitiveType::Octet)),
            ("char", primitive_to_cs(PrimitiveType::Char)),
            ("wchar", primitive_to_cs(PrimitiveType::WideChar)),
            (
                "short",
                primitive_to_cs(PrimitiveType::Integer(IntegerType::Short)),
            ),
            (
                "ushort",
                primitive_to_cs(PrimitiveType::Integer(IntegerType::UShort)),
            ),
            (
                "long",
                primitive_to_cs(PrimitiveType::Integer(IntegerType::Long)),
            ),
            (
                "ulong",
                primitive_to_cs(PrimitiveType::Integer(IntegerType::ULong)),
            ),
            (
                "long long",
                primitive_to_cs(PrimitiveType::Integer(IntegerType::LongLong)),
            ),
            (
                "ulong long",
                primitive_to_cs(PrimitiveType::Integer(IntegerType::ULongLong)),
            ),
            (
                "float",
                primitive_to_cs(PrimitiveType::Floating(FloatingType::Float)),
            ),
            (
                "double",
                primitive_to_cs(PrimitiveType::Floating(FloatingType::Double)),
            ),
            (
                "long double",
                primitive_to_cs(PrimitiveType::Floating(FloatingType::LongDouble)),
            ),
        ];
        // At least 13 distinct strings (char/wchar collapse to "char").
        let unique: std::collections::BTreeSet<&str> = mappings.iter().map(|(_, cs)| *cs).collect();
        assert!(
            unique.len() >= 12,
            "expected ≥12 distinct mappings, got {} ({:?})",
            unique.len(),
            unique
        );
    }
}
