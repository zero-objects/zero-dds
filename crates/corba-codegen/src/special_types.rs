// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex-A.1 special types — Spec OMG CORBA 3.3.
//!
//! 13 IDL language constructs that require special mapping treatment
//! in the three OMG PSMs (C++, C#, Java).

/// Target language for the codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetLanguage {
    /// C++ (`formal/2008-01-09` IDL-to-C++).
    Cpp,
    /// C# / .NET (`formal/2003-09-04` IDL-to-CSharp).
    CSharp,
    /// Java (`formal/2008-01-04` IDL-to-Java).
    Java,
    /// Rust (`zerodds-corba-rust-1.0` vendor spec — no official
    /// OMG mapping for Rust).
    Rust,
}

/// The 13 special types from Annex-A.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialType {
    /// `Object` (Spec Annex-A.1).
    Object,
    /// `ValueBase` (value-type root).
    ValueBase,
    /// `AbstractBase` (abstract-interface root).
    AbstractBase,
    /// `Native` reference (Spec §7.5.5).
    NativeRef,
    /// `TypeCode` (Spec §10.7).
    TypeCode,
    /// `CORBA::any` (Spec §7.6).
    Any,
    /// `sequence<any>` — explicitly a special type because caller
    /// PSMs need special helper classes.
    SequenceOfAny,
    /// `string` (Spec §7.4.4) — marshalled as length-prefix
    /// + null terminator.
    String,
    /// `wstring` (Spec §7.4.5) — UTF-16 with an endianness mark.
    WString,
    /// `Time` (Spec §7.4.6) — `unsigned long long` ticks since UTC.
    Time,
    /// `fixed<digits, scale>` (Spec §7.4.10) — decimal with at most
    /// 31 digits.
    Fixed,
    /// `unsigned long long` (Spec §7.4.1.5).
    ULongLong,
    /// `long double` (Spec §7.4.1.4).
    LongDouble,
}

/// Returns the mapping of a special type in the target language.
///
/// Spec sources:
/// * IDL-to-C++ (`formal/2008-01-09`) Tab 1-3.
/// * IDL-to-CSharp (`formal/2003-09-04`) Annex A.
/// * IDL-to-Java (`formal/2008-01-04`) Tab 4-1.
#[must_use]
pub fn language_mapping(t: SpecialType, lang: TargetLanguage) -> &'static str {
    match (t, lang) {
        (SpecialType::Object, TargetLanguage::Cpp) => "CORBA::Object_var",
        (SpecialType::Object, TargetLanguage::CSharp) => "global::omg.org.CORBA.Object",
        (SpecialType::Object, TargetLanguage::Java) => "org.omg.CORBA.Object",
        (SpecialType::ValueBase, TargetLanguage::Cpp) => "CORBA::ValueBase",
        (SpecialType::ValueBase, TargetLanguage::CSharp) => "global::omg.org.CORBA.ValueBase",
        (SpecialType::ValueBase, TargetLanguage::Java) => "java.io.Serializable",
        (SpecialType::AbstractBase, TargetLanguage::Cpp) => "CORBA::AbstractBase_var",
        (SpecialType::AbstractBase, TargetLanguage::CSharp) => "global::omg.org.CORBA.AbstractBase",
        (SpecialType::AbstractBase, TargetLanguage::Java) => "org.omg.CORBA.portable.IDLEntity",
        (SpecialType::NativeRef, TargetLanguage::Cpp) => "void*",
        (SpecialType::NativeRef, TargetLanguage::CSharp) => "object",
        (SpecialType::NativeRef, TargetLanguage::Java) => "java.lang.Object",
        (SpecialType::TypeCode, TargetLanguage::Cpp) => "CORBA::TypeCode_var",
        (SpecialType::TypeCode, TargetLanguage::CSharp) => "global::omg.org.CORBA.TypeCode",
        (SpecialType::TypeCode, TargetLanguage::Java) => "org.omg.CORBA.TypeCode",
        (SpecialType::Any, TargetLanguage::Cpp) => "CORBA::Any",
        (SpecialType::Any, TargetLanguage::CSharp) => "global::omg.org.CORBA.Any",
        (SpecialType::Any, TargetLanguage::Java) => "org.omg.CORBA.Any",
        (SpecialType::SequenceOfAny, TargetLanguage::Cpp) => "CORBA::AnySeq",
        (SpecialType::SequenceOfAny, TargetLanguage::CSharp) => "global::omg.org.CORBA.AnySeq",
        (SpecialType::SequenceOfAny, TargetLanguage::Java) => "org.omg.CORBA.AnySeqHelper",
        (SpecialType::String, TargetLanguage::Cpp) => "CORBA::String_var",
        (SpecialType::String, TargetLanguage::CSharp) => "string",
        (SpecialType::String, TargetLanguage::Java) => "java.lang.String",
        (SpecialType::WString, TargetLanguage::Cpp) => "CORBA::WString_var",
        (SpecialType::WString, TargetLanguage::CSharp) => "string",
        (SpecialType::WString, TargetLanguage::Java) => "java.lang.String",
        (SpecialType::Time, TargetLanguage::Cpp) => "TimeBase::TimeT",
        (SpecialType::Time, TargetLanguage::CSharp) => "ulong",
        (SpecialType::Time, TargetLanguage::Java) => "long",
        (SpecialType::Fixed, TargetLanguage::Cpp) => "CORBA::Fixed",
        (SpecialType::Fixed, TargetLanguage::CSharp) => "decimal",
        (SpecialType::Fixed, TargetLanguage::Java) => "java.math.BigDecimal",
        (SpecialType::ULongLong, TargetLanguage::Cpp) => "CORBA::ULongLong",
        (SpecialType::ULongLong, TargetLanguage::CSharp) => "ulong",
        (SpecialType::ULongLong, TargetLanguage::Java) => "long",
        (SpecialType::LongDouble, TargetLanguage::Cpp) => "CORBA::LongDouble",
        (SpecialType::LongDouble, TargetLanguage::CSharp) => "decimal",
        (SpecialType::LongDouble, TargetLanguage::Java) => "double",

        // Rust mappings (zerodds-corba-rust-1.0).
        (SpecialType::Object, TargetLanguage::Rust) => "zerodds_corba_rust::ObjectReference",
        (SpecialType::ValueBase, TargetLanguage::Rust) => "dyn zerodds_corba_rust::ValueBase",
        (SpecialType::AbstractBase, TargetLanguage::Rust) => "dyn zerodds_corba_rust::ValueBase",
        (SpecialType::NativeRef, TargetLanguage::Rust) => "::core::ffi::c_void",
        (SpecialType::TypeCode, TargetLanguage::Rust) => "zerodds_corba_rust::TypeCode",
        (SpecialType::Any, TargetLanguage::Rust) => "zerodds_dcps::DdsAny",
        (SpecialType::SequenceOfAny, TargetLanguage::Rust) => {
            "::std::vec::Vec<zerodds_dcps::DdsAny>"
        }
        (SpecialType::String, TargetLanguage::Rust) => "::std::string::String",
        (SpecialType::WString, TargetLanguage::Rust) => "::std::string::String",
        (SpecialType::Time, TargetLanguage::Rust) => "u64",
        (SpecialType::Fixed, TargetLanguage::Rust) => "zerodds_cdr::fixed::Fixed",
        (SpecialType::ULongLong, TargetLanguage::Rust) => "u64",
        (SpecialType::LongDouble, TargetLanguage::Rust) => "f64",
    }
}

/// Returns all 13 special types in spec order.
#[must_use]
pub const fn all_special_types() -> [SpecialType; 13] {
    [
        SpecialType::Object,
        SpecialType::ValueBase,
        SpecialType::AbstractBase,
        SpecialType::NativeRef,
        SpecialType::TypeCode,
        SpecialType::Any,
        SpecialType::SequenceOfAny,
        SpecialType::String,
        SpecialType::WString,
        SpecialType::Time,
        SpecialType::Fixed,
        SpecialType::ULongLong,
        SpecialType::LongDouble,
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn exactly_13_special_types() {
        assert_eq!(all_special_types().len(), 13);
    }

    #[test]
    fn each_special_type_has_mapping_in_each_language() {
        for t in all_special_types() {
            for l in [
                TargetLanguage::Cpp,
                TargetLanguage::CSharp,
                TargetLanguage::Java,
            ] {
                let m = language_mapping(t, l);
                assert!(!m.is_empty(), "missing mapping for {t:?} in {l:?}");
            }
        }
    }

    #[test]
    fn cpp_object_uses_var_helper() {
        assert_eq!(
            language_mapping(SpecialType::Object, TargetLanguage::Cpp),
            "CORBA::Object_var"
        );
    }

    #[test]
    fn java_string_maps_to_java_lang_string() {
        assert_eq!(
            language_mapping(SpecialType::String, TargetLanguage::Java),
            "java.lang.String"
        );
        assert_eq!(
            language_mapping(SpecialType::WString, TargetLanguage::Java),
            "java.lang.String"
        );
    }

    #[test]
    fn csharp_fixed_maps_to_decimal() {
        assert_eq!(
            language_mapping(SpecialType::Fixed, TargetLanguage::CSharp),
            "decimal"
        );
    }

    #[test]
    fn java_typecode_uses_omg_corba_namespace() {
        assert_eq!(
            language_mapping(SpecialType::TypeCode, TargetLanguage::Java),
            "org.omg.CORBA.TypeCode"
        );
    }
}
