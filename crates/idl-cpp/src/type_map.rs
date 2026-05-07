// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Mapping IDL-Primitive → C++-Type-Strings.
//!
//! Folgt OMG IDL4-CPP-Mapping §7.4 Tabelle 7.5 und Tabelle 7.6 (formal/2018-07-01).
//! Nur die Foundation-Subset (Block B): primitive Skalare, Strings, sowie die
//! Foundation-Container [`std::vector`], [`std::array`], [`std::variant`],
//! [`std::optional`].

use zerodds_idl::ast::{FloatingType, IntegerType, PrimitiveType};

use crate::error::CppGenError;

/// Reservierte C++17-Schluesselwoerter, die als Identifier verboten sind.
///
/// Quelle: ISO/IEC 14882:2017 §5.11 (Tabelle 5). Die Liste ist intentional
/// nicht vollstaendig — sie deckt die haeufigen Kollisionen ab, die ein
/// IDL-Mapping treffen kann (Token-Class fuer Type-Specifier und
/// Storage-Klassen). Erweiterbar in C5.1-b.
pub(crate) const CPP_RESERVED: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char16_t",
    "char32_t",
    "class",
    "compl",
    "const",
    "constexpr",
    "const_cast",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// Prueft, ob ein Identifier ein C++-Keyword ist.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    CPP_RESERVED.contains(&name)
}

/// Pruefen + Fehler-Konversion: liefert Err, wenn `name` reserviert ist.
///
/// # Errors
/// Gibt [`CppGenError::InvalidName`] zurueck, wenn `name` ein
/// reserviertes C++-Keyword ist.
pub fn check_identifier(name: &str) -> Result<(), CppGenError> {
    if is_reserved(name) {
        return Err(CppGenError::InvalidName {
            name: name.to_string(),
            reason: "reserved C++ keyword".to_string(),
        });
    }
    Ok(())
}

/// Mappt eine [`PrimitiveType`] auf den C++-Typ-Ausdruck (als `&'static str`).
///
/// Spec-Referenz: §7.4 Tabelle 7.5.
#[must_use]
pub fn primitive_to_cpp(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "uint8_t",
        PrimitiveType::Char => "char",
        PrimitiveType::WideChar => "wchar_t",
        PrimitiveType::Integer(i) => integer_to_cpp(i),
        PrimitiveType::Floating(f) => floating_to_cpp(f),
    }
}

/// Mapping fuer Integer-Subtypen.
#[must_use]
pub fn integer_to_cpp(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "int16_t",
        IntegerType::Long | IntegerType::Int32 => "int32_t",
        IntegerType::LongLong | IntegerType::Int64 => "int64_t",
        IntegerType::UShort | IntegerType::UInt16 => "uint16_t",
        IntegerType::ULong | IntegerType::UInt32 => "uint32_t",
        IntegerType::ULongLong | IntegerType::UInt64 => "uint64_t",
        IntegerType::Int8 => "int8_t",
        IntegerType::UInt8 => "uint8_t",
    }
}

/// Mapping fuer Floating-Subtypen. `long double` wird als
/// [`CppGenError::UnsupportedConstruct`] gemeldet (Block-E-außerhalb des aktuellen Scopes).
#[must_use]
pub fn floating_to_cpp(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "float",
        FloatingType::Double => "double",
        FloatingType::LongDouble => "long double",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn primitive_boolean() {
        assert_eq!(primitive_to_cpp(PrimitiveType::Boolean), "bool");
    }

    #[test]
    fn primitive_octet() {
        assert_eq!(primitive_to_cpp(PrimitiveType::Octet), "uint8_t");
    }

    #[test]
    fn primitive_char() {
        assert_eq!(primitive_to_cpp(PrimitiveType::Char), "char");
    }

    #[test]
    fn primitive_wchar() {
        assert_eq!(primitive_to_cpp(PrimitiveType::WideChar), "wchar_t");
    }

    #[test]
    fn integer_short_signed_unsigned() {
        assert_eq!(integer_to_cpp(IntegerType::Short), "int16_t");
        assert_eq!(integer_to_cpp(IntegerType::UShort), "uint16_t");
    }

    #[test]
    fn integer_long_signed_unsigned() {
        assert_eq!(integer_to_cpp(IntegerType::Long), "int32_t");
        assert_eq!(integer_to_cpp(IntegerType::ULong), "uint32_t");
    }

    #[test]
    fn integer_long_long_signed_unsigned() {
        assert_eq!(integer_to_cpp(IntegerType::LongLong), "int64_t");
        assert_eq!(integer_to_cpp(IntegerType::ULongLong), "uint64_t");
    }

    #[test]
    fn integer_explicit_widths() {
        assert_eq!(integer_to_cpp(IntegerType::Int8), "int8_t");
        assert_eq!(integer_to_cpp(IntegerType::UInt8), "uint8_t");
        assert_eq!(integer_to_cpp(IntegerType::Int16), "int16_t");
        assert_eq!(integer_to_cpp(IntegerType::UInt16), "uint16_t");
        assert_eq!(integer_to_cpp(IntegerType::Int32), "int32_t");
        assert_eq!(integer_to_cpp(IntegerType::UInt32), "uint32_t");
        assert_eq!(integer_to_cpp(IntegerType::Int64), "int64_t");
        assert_eq!(integer_to_cpp(IntegerType::UInt64), "uint64_t");
    }

    #[test]
    fn floating_float_double() {
        assert_eq!(floating_to_cpp(FloatingType::Float), "float");
        assert_eq!(floating_to_cpp(FloatingType::Double), "double");
    }

    #[test]
    fn reserved_class_is_rejected() {
        assert!(is_reserved("class"));
        assert!(check_identifier("class").is_err());
    }

    #[test]
    fn reserved_int_is_rejected() {
        assert!(is_reserved("int"));
    }

    #[test]
    fn non_reserved_identifier_passes() {
        assert!(!is_reserved("Foo"));
        assert!(check_identifier("Foo").is_ok());
    }

    #[test]
    fn check_returns_invalidname_with_reason() {
        let err = check_identifier("template").expect_err("must reject 'template'");
        match err {
            CppGenError::InvalidName { reason, name } => {
                assert_eq!(name, "template");
                assert!(reason.to_lowercase().contains("reserved"));
            }
            other => panic!("unexpected err variant: {other:?}"),
        }
    }
}
