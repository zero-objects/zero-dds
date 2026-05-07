// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Mapping IDL-Primitive → Java-Type-Strings.
//!
//! Folgt OMG IDL4-Java-Mapping v1.0 §6 (Type Mapping). Java hat keine
//! native unsigned-Integer-Typen — der Spec-konforme Workaround:
//!
//! - `unsigned short` → Java `int` (nicht `short`, um den vollen
//!   Wertebereich abzubilden).
//! - `unsigned long` → Java `long` (nicht `int`).
//! - `unsigned long long` → Java `long` (mit Doc-Hinweis; Default ist
//!   die `long`-Variante; `BigInteger` waere die volle Loesung,
//!   bleibt hier außerhalb des aktuellen Scopes).
//!
//! `boolean` → `boolean`, `octet` → `byte`, `char`/`wchar` → `char`,
//! `string`/`wstring` → `String`, `float`/`double` → `float`/`double`.

use zerodds_idl::ast::{FloatingType, IntegerType, PrimitiveType};

/// Mapping fuer eine [`PrimitiveType`] auf den Java-Typ.
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

/// Mapping fuer Integer-Subtypen.
///
/// Spec-konformer Unsigned-Workaround:
/// - `unsigned short` → `int` (16 Bit Range passt nicht in `short`).
/// - `unsigned long` → `long` (32 Bit Range passt nicht in `int`).
/// - `unsigned long long` → `long` (mit Vorzeichen-Treppe; siehe Doc).
#[must_use]
pub fn integer_to_java(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "short",
        IntegerType::Long | IntegerType::Int32 => "int",
        IntegerType::LongLong | IntegerType::Int64 => "long",
        // Unsigned-Workaround: nimm den naechst-groesseren signed Typ.
        IntegerType::UShort | IntegerType::UInt16 => "int",
        IntegerType::ULong | IntegerType::UInt32 => "long",
        // unsigned long long: Java hat keinen 64-Bit-unsigned-Integer.
        // Default: `long` (Vorzeichen, mit Doc-Comment im Emitter).
        IntegerType::ULongLong | IntegerType::UInt64 => "long",
        IntegerType::Int8 => "byte",
        IntegerType::UInt8 => "short",
    }
}

/// Mapping fuer Floating-Subtypen. `long double` ist außerhalb des aktuellen Scopes und wird
/// in `typespec_to_java` als [`crate::error::JavaGenError::UnsupportedConstruct`]
/// behandelt.
#[must_use]
pub fn floating_to_java(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "float",
        FloatingType::Double => "double",
        FloatingType::LongDouble => "double",
    }
}

/// Liefert `true`, wenn ein IDL-Integer-Type fuer Java unsigned-Workaround
/// braucht (Doc-Hinweis im Generator).
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

/// Wrapper-Klasse fuer ein primitive Java-Type, fuer Generics
/// (z.B. `List<Integer>` statt `List<int>`).
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

/// Boxed-Variante fuer Integer-Subtypen.
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

/// Boxed-Variante fuer Floating-Subtypen.
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
        // Spec §6.2: unsigned short → int (16-Bit-Wertebereich passt
        // nicht in Java-`short`, da signed).
        assert_eq!(integer_to_java(IntegerType::UShort), "int");
    }

    #[test]
    fn integer_unsigned_long_widens_to_long() {
        assert_eq!(integer_to_java(IntegerType::ULong), "long");
    }

    #[test]
    fn integer_unsigned_long_long_keeps_long() {
        // Default-Workaround: `long` (vorzeichen-treppe akzeptiert,
        // Stretch-Goal: BigInteger).
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
