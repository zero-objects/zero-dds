// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 → C++17-Header-Codegen (OMG IDL4-CPP-Mapping, formal/2018-07-01).
//!
//! Crate `zerodds-idl-cpp` — Foundation des Sprach-Bindings (Cluster C5.1-a).
//!
//! Safety classification: **SAFE (std-only)**. Reines Build-Zeit-Tool —
//! `forbid(unsafe_code)`, kein no_std-Use-Case.
//!
//! # Scope (C5.1-a)
//! - Block A: Header-Layout (`#pragma once`, `namespace`, includes).
//! - Block B: Primitive-Mapping (boolean → bool, octet → uint8_t, ...).
//! - Block C: struct/enum/union/typedef/sequence/array/inheritance.
//! - Block D: Exception → `class X : public std::exception`.
//! - Block E: Time/Duration → DDS::Time_t / DDS::Duration_t.
//!
//! # C5.1-b Erweiterungen
//! - Block F: Status-Mapping (13 Status-Klassen, [`status`]).
//! - Block G: QoS-Policy + Type-Traits (22 Policies, [`qos`]).
//! - Block H: DCPS-Entity-Header-Stubs ([`dcps`]).
//!
//! # C5.2 Erweiterungen
//! - DDS-PSM-CXX-Header-Skeleton-Layer ([`psm_cxx`]).
//!
//! # C6.1.D-cpp Erweiterungen
//! - DDS-RPC C++ PSM-Codegen ([`rpc`]) — Service-Interface, Requester,
//!   Replier, ServiceTraits + RemoteException-Hierarchie. Spec §10.
//!
//! # Bewusst nicht im Crate
//! - Bitset/Bitmask, Map, Fixed, Any, Interface, Valuetype.
//! - Linker-Tests (statische Header-Generation reicht).
//!
//! # Beispiel
//!
//! ```
//! use zerodds_idl::config::ParserConfig;
//! use zerodds_idl_cpp::{generate_cpp_header, CppGenOptions};
//!
//! let ast = zerodds_idl::parse(
//!     "module M { struct S { long x; }; };",
//!     &ParserConfig::default(),
//! )
//! .expect("parse");
//! let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
//! assert!(cpp.contains("namespace M"));
//! assert!(cpp.contains("class S"));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(
    clippy::manual_pattern_char_comparison,
    clippy::if_same_then_else,
    clippy::collapsible_if,
    clippy::useless_conversion,
    clippy::approx_constant
)]

pub(crate) mod amqp;
pub(crate) mod bitset;
pub mod c_mode;
pub(crate) mod corba_traits;
pub mod dcps;
pub mod emitter;
pub mod error;
pub mod psm_cxx;
pub mod qos;
pub mod rpc;
pub mod status;
pub mod type_map;
pub(crate) mod verbatim;

pub use c_mode::{CGenOptions, generate_c_header};
pub use error::CppGenError;
pub use psm_cxx::{
    emit_condition_skeleton, emit_core_basics, emit_exception_hierarchy,
    emit_full_psm_cxx_skeleton, emit_listener_skeleton, emit_psm_cxx_includes,
    emit_reference_value_pattern,
};

use zerodds_idl::ast::Specification;

/// Konfiguration des Code-Generators.
#[derive(Debug, Clone)]
pub struct CppGenOptions {
    /// Optionaler aeusserer Namespace, in den der gesamte Header gewickelt
    /// wird. `None` oder leer = kein Wrapper.
    pub namespace_prefix: Option<String>,
    /// Optionaler include-Guard-Prefix (Kommentar-Marker zusaetzlich zu
    /// `#pragma once`). Foundation legt nur `#pragma once`; der Prefix
    /// erscheint als Kommentar.
    pub include_guard_prefix: Option<String>,
    /// Indent-Breite in Leerzeichen. Default 4.
    pub indent_width: usize,
    /// Spec §7.2.3 / §8.1.2 / §8.1.3 — opt-in: fügt am Ende des
    /// generierten Headers per-Type AMQP-Codec-Helper an
    /// (`to_amqp_value`, `to_json_string`). Default `false`, weil
    /// die emittierten Calls einen kleinen C++-Runtime-Header
    /// `<zerodds/amqp/codec.hpp>` voraussetzen, der als separate
    /// Library-Crate kommt.
    pub emit_amqp_helpers: bool,
    /// Annex A.1 (idl4-cpp-1.0) — opt-in: fügt am Ende
    /// CORBA-spezifische Trait-Spezialisierungen
    /// (`CORBA::traits<T>::value_type/in_type/out_type/inout_type`)
    /// pro Top-Level-Type an. Default `false`.
    pub emit_corba_traits: bool,
}

impl Default for CppGenOptions {
    fn default() -> Self {
        Self {
            namespace_prefix: None,
            include_guard_prefix: None,
            indent_width: 4,
            emit_amqp_helpers: false,
            emit_corba_traits: false,
        }
    }
}

/// Block-E: Mapping von Time/Duration-Identifiern auf C++-Type-Strings.
///
/// Wenn ein IDL-Member `Time_t` referenziert (single-component scoped name),
/// wird er auf `DDS::Time_t` gemappt. Spec-Quelle: dds-psm-cxx §6.4.
pub(crate) const TIME_DURATION_TYPES: &[(&str, &str)] = &[
    ("Time_t", "DDS::Time_t"),
    ("Duration_t", "DDS::Duration_t"),
    ("Time", "DDS::Time_t"),
    ("Duration", "DDS::Duration_t"),
];

/// Erzeugt einen vollstaendigen C++17-Header aus einer IDL-Specification.
///
/// # Errors
/// - [`CppGenError::UnsupportedConstruct`]: IDL-Konstrukt außerhalb des aktuellen Scopes
///   (z.B. `interface`, `valuetype`, `fixed`, `any`, `map`, `bitset`,
///   `bitmask`).
/// - [`CppGenError::InvalidName`]: Ein Identifier kollidiert mit einem
///   reservierten C++-Keyword.
/// - [`CppGenError::InheritanceCycle`]: Direkte oder indirekte
///   Self-Inheritance im Struct-Graphen.
pub fn generate_cpp_header(
    ast: &Specification,
    opts: &CppGenOptions,
) -> Result<String, CppGenError> {
    let mut out = emitter::emit_header(ast, opts)?;
    if opts.emit_amqp_helpers {
        amqp::emit_amqp_helpers(&mut out, ast)?;
    }
    if opts.emit_corba_traits {
        corba_traits::emit_corba_traits(&mut out, ast)?;
    }
    Ok(out)
}

/// Convenience-Variante mit aktiviertem `emit_corba_traits`-Flag.
///
/// Identisch zu [`generate_cpp_header`], aber zwingt
/// `opts.emit_corba_traits = true`. Cross-Ref: `idl4-cpp-1.0` Annex A.1.
///
/// # Errors
/// Wie [`generate_cpp_header`].
pub fn generate_cpp_header_with_corba_traits(
    ast: &Specification,
    opts: &CppGenOptions,
) -> Result<String, CppGenError> {
    let opts = CppGenOptions {
        emit_corba_traits: true,
        ..opts.clone()
    };
    generate_cpp_header(ast, &opts)
}

/// Convenience-Variante mit aktiviertem `emit_amqp_helpers`-Flag.
///
/// Identisch zu [`generate_cpp_header`], aber zwingt
/// `opts.emit_amqp_helpers = true`. Nützlich für Tests und
/// Tooling, die den AMQP-Bindings-Pfad explizit auswählen wollen.
///
/// # Errors
/// Wie [`generate_cpp_header`].
pub fn generate_cpp_header_with_amqp(
    ast: &Specification,
    opts: &CppGenOptions,
) -> Result<String, CppGenError> {
    let opts = CppGenOptions {
        emit_amqp_helpers: true,
        ..opts.clone()
    };
    generate_cpp_header(ast, &opts)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_idl::config::ParserConfig;

    fn gen_cpp(src: &str) -> String {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed");
        generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen must succeed")
    }

    #[test]
    fn empty_source_emits_only_preamble() {
        let cpp = gen_cpp("");
        assert!(cpp.contains("#pragma once"));
        assert!(cpp.contains("Generated by zerodds idl-cpp"));
        // Kein Namespace-Open ohne Module.
        assert!(!cpp.contains("namespace M {"));
    }

    #[test]
    fn empty_module_emits_namespace() {
        let cpp = gen_cpp("module M {};");
        assert!(cpp.contains("namespace M {"));
        assert!(cpp.contains("} // namespace M"));
    }

    #[test]
    fn three_level_modules_nest() {
        let cpp = gen_cpp("module A { module B { module C {}; }; };");
        assert!(cpp.contains("namespace A {"));
        assert!(cpp.contains("namespace B {"));
        assert!(cpp.contains("namespace C {"));
        assert!(cpp.contains("} // namespace C"));
        assert!(cpp.contains("} // namespace B"));
        assert!(cpp.contains("} // namespace A"));
    }

    #[test]
    fn primitive_struct_member_uses_correct_cpp_types() {
        let cpp = gen_cpp(
            "struct S { boolean b; octet o; short s; long l; long long ll; \
             unsigned short us; unsigned long ul; unsigned long long ull; \
             float f; double d; };",
        );
        assert!(cpp.contains("bool b_;"));
        assert!(cpp.contains("uint8_t o_;"));
        assert!(cpp.contains("int16_t s_;"));
        assert!(cpp.contains("int32_t l_;"));
        assert!(cpp.contains("int64_t ll_;"));
        assert!(cpp.contains("uint16_t us_;"));
        assert!(cpp.contains("uint32_t ul_;"));
        assert!(cpp.contains("uint64_t ull_;"));
        assert!(cpp.contains("float f_;"));
        assert!(cpp.contains("double d_;"));
    }

    #[test]
    fn string_member_requires_string_include() {
        let cpp = gen_cpp("struct S { string name; };");
        assert!(cpp.contains("#include <string>"));
        assert!(cpp.contains("std::string name_;"));
    }

    #[test]
    fn sequence_member_uses_vector() {
        let cpp = gen_cpp("struct S { sequence<long> data; };");
        assert!(cpp.contains("#include <vector>"));
        assert!(cpp.contains("std::vector<int32_t> data_;"));
    }

    #[test]
    fn array_member_uses_std_array() {
        let cpp = gen_cpp("struct S { long matrix[3][4]; };");
        assert!(cpp.contains("#include <array>"));
        assert!(cpp.contains("std::array<std::array<int32_t, 4>, 3>"));
    }

    #[test]
    fn enum_emits_enum_class_int32_t() {
        let cpp = gen_cpp("enum Color { RED, GREEN, BLUE };");
        assert!(cpp.contains("enum class Color : int32_t"));
        assert!(cpp.contains("RED,"));
        assert!(cpp.contains("BLUE,"));
    }

    #[test]
    fn typedef_emits_using_alias() {
        let cpp = gen_cpp("typedef long MyInt;");
        assert!(cpp.contains("using MyInt = int32_t;"));
    }

    #[test]
    fn inheritance_emits_public_base() {
        let cpp = gen_cpp("struct Parent { long x; }; struct Child : Parent { long y; };");
        assert!(cpp.contains("class Child : public Parent"));
    }

    #[test]
    fn keyed_struct_marker_appears() {
        let cpp = gen_cpp("struct S { @key long id; long val; };");
        assert!(cpp.contains("// @key"));
    }

    #[test]
    fn optional_member_uses_std_optional() {
        let cpp = gen_cpp("struct S { @optional long maybe; };");
        assert!(cpp.contains("#include <optional>"));
        assert!(cpp.contains("std::optional<int32_t>"));
    }

    #[test]
    fn exception_inherits_std_exception() {
        let cpp = gen_cpp("exception NotFound { string what_; };");
        assert!(cpp.contains("#include <exception>"));
        assert!(cpp.contains("class NotFound : public std::exception"));
    }

    #[test]
    fn union_uses_std_variant() {
        let cpp = gen_cpp(
            "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        );
        assert!(cpp.contains("#include <variant>"));
        assert!(cpp.contains("std::variant<"));
        assert!(cpp.contains("// case default"));
    }

    #[test]
    fn time_t_member_maps_to_dds_time_t() {
        let cpp = gen_cpp("struct S { Time_t t; };");
        assert!(cpp.contains("DDS::Time_t"));
    }

    #[test]
    fn duration_t_member_maps_to_dds_duration_t() {
        let cpp = gen_cpp("struct S { Duration_t d; };");
        assert!(cpp.contains("DDS::Duration_t"));
    }

    #[test]
    fn reserved_field_name_is_rejected() {
        let ast = zerodds_idl::parse("struct S { long class_field; };", &ParserConfig::default())
            .expect("parse");
        // "class_field" ist nicht reserviert; Test mit Annotation-Trick:
        // wir erzwingen via Builder-API einen Reserved-Name.
        // Stattdessen pruefen wir den Pfad direkt ueber check_identifier:
        let res = type_map::check_identifier("class");
        assert!(matches!(res, Err(CppGenError::InvalidName { .. })));
        let _ = ast; // ungenutzt aber zeigt die Idee
    }

    #[test]
    fn empty_source_includes_cstdint() {
        let cpp = gen_cpp("");
        assert!(cpp.contains("#include <cstdint>"));
    }

    #[test]
    fn header_starts_with_generated_marker() {
        let cpp = gen_cpp("");
        assert!(cpp.starts_with("// Generated by zerodds idl-cpp."));
    }

    #[test]
    fn pragma_once_appears_exactly_once() {
        let cpp = gen_cpp("module M { struct S { long x; }; };");
        let count = cpp.matches("#pragma once").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn struct_has_default_constructor() {
        let cpp = gen_cpp("struct S { long x; };");
        assert!(cpp.contains("S() = default;"));
        assert!(cpp.contains("~S() = default;"));
    }

    #[test]
    fn struct_has_mutable_and_const_getter() {
        let cpp = gen_cpp("struct S { long x; };");
        // Mutable getter: returns int32_t&; const-version present too.
        assert!(cpp.contains("int32_t& x()"));
        assert!(cpp.contains("const int32_t& x() const"));
    }

    #[test]
    fn struct_has_setter() {
        let cpp = gen_cpp("struct S { long x; };");
        assert!(cpp.contains("void x(const int32_t& value)"));
    }

    #[test]
    fn namespace_prefix_option_wraps_output() {
        let ast =
            zerodds_idl::parse("struct S { long x; };", &ParserConfig::default()).expect("parse");
        let opts = CppGenOptions {
            namespace_prefix: Some("zerodds".into()),
            ..Default::default()
        };
        let cpp = generate_cpp_header(&ast, &opts).expect("gen");
        assert!(cpp.contains("namespace zerodds {"));
        assert!(cpp.contains("} // namespace zerodds"));
    }

    #[test]
    fn non_service_interface_emits_pure_virtual_class() {
        let ast = zerodds_idl::parse("interface I { void op(); };", &ParserConfig::default())
            .expect("parse");
        let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("ok");
        assert!(cpp.contains("class I"));
        assert!(cpp.contains("virtual ~I()"));
        assert!(cpp.contains("= 0;"));
    }

    #[test]
    fn const_decl_emits_constexpr() {
        let cpp = gen_cpp("const long MAX = 100;");
        assert!(cpp.contains("constexpr int32_t MAX = 100;"));
    }

    #[test]
    fn options_have_sensible_defaults() {
        let o = CppGenOptions::default();
        assert_eq!(o.indent_width, 4);
        assert!(o.namespace_prefix.is_none());
        assert!(o.include_guard_prefix.is_none());
    }

    #[test]
    fn options_clone_works() {
        let o = CppGenOptions {
            namespace_prefix: Some("foo".into()),
            include_guard_prefix: Some("FOO_".into()),
            indent_width: 2,
            emit_amqp_helpers: false,
            emit_corba_traits: false,
        };
        let cloned = o.clone();
        assert_eq!(cloned.indent_width, 2);
        assert_eq!(cloned.namespace_prefix.as_deref(), Some("foo"));
    }
}
