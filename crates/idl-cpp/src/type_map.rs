// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Mapping IDL-Primitive → C++-Type-Strings.
//!
//! Follows the OMG IDL4-CPP mapping §7.4 Table 7.5 and Table 7.6 (formal/2018-07-01).
//! Only the foundation subset (Block B): primitive scalars, strings, as well as the
//! Foundation-Container [`std::vector`], [`std::array`], [`std::variant`],
//! [`std::optional`].

use zerodds_idl::ast::{FloatingType, IntegerType, PrimitiveType};

use crate::error::CppGenError;

/// Reserved C++17 keywords that are forbidden as identifiers.
///
/// Source: ISO/IEC 14882:2017 §5.11 (table 5). The list is intentionally
/// not complete — it covers the frequent collisions an IDL mapping can
/// hit (the token class for type specifiers and storage classes).
/// Extensible in C5.1-b.
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

/// Checks whether an identifier is a C++ keyword.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    CPP_RESERVED.contains(&name)
}

/// Check + error conversion: returns Err if `name` is reserved.
///
/// # Errors
/// Returns [`CppGenError::InvalidName`] if `name` is a reserved C++
/// keyword.
pub fn check_identifier(name: &str) -> Result<(), CppGenError> {
    if is_reserved(name) {
        return Err(CppGenError::InvalidName {
            name: name.to_string(),
            reason: "reserved C++ keyword".to_string(),
        });
    }
    Ok(())
}

/// Maps a [`PrimitiveType`] to the C++ type expression (as `&'static str`).
///
/// Spec reference: §7.4 table 7.5.
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

/// Mapping for integer subtypes.
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

/// Mapping for floating subtypes. `long double` is reported as
/// [`CppGenError::UnsupportedConstruct`] (block E — outside the current scope).
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
