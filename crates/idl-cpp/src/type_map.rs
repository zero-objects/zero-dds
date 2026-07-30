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
/// Retained for the RPC/legacy callers that still want a hard rejection
/// and for the unit tests; the topic/type codegen now *escapes* instead
/// (see [`escape_cpp_ident`]).
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

/// Returns an identifier guaranteed usable as a bare C++ token.
///
/// - If `name` is collision-free: returned unchanged.
/// - If `name` is a reserved C++ keyword: a trailing `_` is appended
///   (`int` -> `int_`, `class` -> `class_`).
///
/// A trailing underscore is *always* a legal C++ identifier: only a
/// LEADING underscore followed by an uppercase letter, or a name
/// containing a double underscore, is reserved to the implementation
/// (ISO/IEC 14882:2017 §5.10). Since no [`CPP_RESERVED`] entry ends in
/// `_`, the escaped form is never itself reserved (asserted by the
/// `escaped_form_is_never_itself_reserved` guard test).
///
/// This mirrors the thin C-mode backend's [`crate::c_keywords::escape_c_ident`].
/// The escaping is name-LOCAL: it only touches the C++ identifier text, never
/// the DDS type name on the wire, member ids, `@key` membership or key-hash
/// layout, so it cannot move the XCDR2 wire format.
#[must_use]
pub fn escape_cpp_ident(name: &str) -> String {
    if is_reserved(name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
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
    fn escape_keyword_yields_trailing_underscore() {
        assert_eq!(escape_cpp_ident("class"), "class_");
        assert_eq!(escape_cpp_ident("int"), "int_");
        assert_eq!(escape_cpp_ident("template"), "template_");
        assert_eq!(escape_cpp_ident("operator"), "operator_");
        assert_eq!(escape_cpp_ident("delete"), "delete_");
    }

    #[test]
    fn escape_non_keyword_unchanged() {
        assert_eq!(escape_cpp_ident("Foo"), "Foo");
        assert_eq!(escape_cpp_ident("my_field"), "my_field");
        // A name that merely *contains* a keyword is untouched.
        assert_eq!(escape_cpp_ident("class_field"), "class_field");
    }

    #[test]
    fn all_keywords_escape_to_trailing_underscore() {
        for kw in CPP_RESERVED {
            assert_eq!(escape_cpp_ident(kw), format!("{kw}_"));
        }
    }

    #[test]
    fn escaped_form_is_never_itself_reserved() {
        // Structurally guaranteed because no CPP_RESERVED entry ends in `_`
        // (see `no_reserved_word_ends_in_underscore`); asserted defensively so
        // the escaping can never collide back onto another keyword.
        for kw in CPP_RESERVED {
            let escaped = escape_cpp_ident(kw);
            assert!(!is_reserved(&escaped), "{escaped} must not be reserved");
        }
    }

    #[test]
    fn no_reserved_word_ends_in_underscore() {
        // Guard: the trailing-underscore escape is only legal because no C++
        // keyword itself ends in `_`. If a future keyword addition broke this,
        // `escape_cpp_ident("kw")` could equal another keyword.
        for kw in CPP_RESERVED {
            assert!(
                !kw.ends_with('_'),
                "CPP_RESERVED entry {kw:?} ends in '_' — breaks the escape invariant"
            );
        }
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
