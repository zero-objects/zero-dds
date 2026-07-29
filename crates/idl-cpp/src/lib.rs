// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 → C++17-Header-Codegen (OMG IDL4-CPP-Mapping, formal/2018-07-01).
//!
//! Crate `zerodds-idl-cpp` — foundation of the language binding (cluster C5.1-a).
//!
//! Safety classification: **SAFE (std-only)**. Pure build-time tool —
//! `forbid(unsafe_code)`, no no_std use case.
//!
//! # Scope (C5.1-a)
//! - Block A: header layout (`#pragma once`, `namespace`, includes).
//! - Block B: primitive mapping (boolean → bool, octet → uint8_t, ...).
//! - Block C: struct/enum/union/typedef/sequence/array/inheritance.
//! - Block D: exception → `class X : public std::exception`.
//! - Block E: Time/Duration → DDS::Time_t / DDS::Duration_t.
//!
//! # C5.1-b extensions
//! - Block F: status mapping (13 status classes, [`status`]).
//! - Block G: QoS policy + type traits (22 policies, [`qos`]).
//! - Block H: DCPS entity header stubs ([`dcps`]).
//!
//! # C5.2 extensions
//! - DDS-PSM-CXX header skeleton layer ([`psm_cxx`]).
//!
//! # C6.1.D-cpp extensions
//! - DDS-RPC C++ PSM codegen ([`rpc`]) — service interface, requester,
//!   replier, ServiceTraits + RemoteException hierarchy. Spec §10.
//!
//! # Intentionally not in the crate
//! - Bitset/Bitmask, Map, Fixed, Any, Interface, Valuetype.
//! - Linker tests (static header generation is sufficient).
//!
//! # Example
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
pub mod c_keywords;
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

/// Configuration of the code generator.
#[derive(Debug, Clone)]
pub struct CppGenOptions {
    /// Optional outer namespace that wraps the entire header.
    /// `None` or empty = no wrapper.
    pub namespace_prefix: Option<String>,
    /// Optional include-guard prefix (comment marker in addition to
    /// `#pragma once`). The foundation emits only `#pragma once`; the prefix
    /// appears as a comment.
    pub include_guard_prefix: Option<String>,
    /// Indent width in spaces. Default 4.
    pub indent_width: usize,
    /// Spec §7.2.3 / §8.1.2 / §8.1.3 — opt-in: appends per-type AMQP codec
    /// helpers (`to_amqp_value`, `to_json_string`) at the end of the
    /// generated header. Default `false`, because the emitted calls
    /// require a small C++ runtime header `<zerodds/amqp/codec.hpp>`
    /// that ships as a separate library crate.
    pub emit_amqp_helpers: bool,
    /// Annex A.1 (idl4-cpp-1.0) — opt-in: appends CORBA-specific trait
    /// specializations
    /// (`CORBA::traits<T>::value_type/in_type/out_type/inout_type`)
    /// per top-level type at the end. Default `false`.
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

/// Block-E: mapping of Time/Duration identifiers to C++ type strings.
///
/// When an IDL member references `Time_t` (single-component scoped name),
/// it is mapped to `DDS::Time_t`. Spec source: dds-psm-cxx §6.4.
pub(crate) const TIME_DURATION_TYPES: &[(&str, &str)] = &[
    ("Time_t", "DDS::Time_t"),
    ("Duration_t", "DDS::Duration_t"),
    ("Time", "DDS::Time_t"),
    ("Duration", "DDS::Duration_t"),
];

/// Produces a complete C++17 header from an IDL specification.
///
/// # Errors
/// - [`CppGenError::UnsupportedConstruct`]: IDL construct outside the current scope
///   (e.g. `interface`, `valuetype`, `fixed`, `any`, `map`, `bitset`,
///   `bitmask`).
/// - [`CppGenError::InvalidName`]: An identifier collides with a
///   reserved C++ keyword.
/// - [`CppGenError::InheritanceCycle`]: Direct or indirect
///   self-inheritance in the struct graph.
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

/// Convenience variant with the `emit_corba_traits` flag enabled.
///
/// Identical to [`generate_cpp_header`], but forces
/// `opts.emit_corba_traits = true`. Cross-ref: `idl4-cpp-1.0` Annex A.1.
///
/// # Errors
/// Same as [`generate_cpp_header`].
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

/// Convenience variant with the `emit_amqp_helpers` flag enabled.
///
/// Identical to [`generate_cpp_header`], but forces
/// `opts.emit_amqp_helpers = true`. Useful for tests and tooling
/// that want to select the AMQP bindings path explicitly.
///
/// # Errors
/// Same as [`generate_cpp_header`].
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
        // No namespace open without a module.
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
        // Base reference is absolutely qualified (`::Parent`) so it resolves at
        // any scope — see the type-path registry in emitter.rs.
        assert!(cpp.contains("class Child : public ::Parent"));
    }

    /// Regression for Bug B (unqualified nested types) + Bug F (typedef-to-
    /// primitive members silently dropped). Models the NGVA shape (AEP-4754
    /// Vol V §8.1.3): typedef-in-units + enum + nested struct as members.
    #[test]
    fn ngva_typedef_serialized_and_nested_qualified() {
        let cpp = gen_cpp(
            "module nga { \
               typedef double CurrentInAmpsType; \
               enum CoordinateSystemType { COORDINATE_SYSTEM_TYPE__BNG }; \
               struct LinearVelocity2DType { double xComponent; double yComponent; }; \
               struct Navigation_Resource_Specification { \
                 @key string<32>      vehicleId; \
                 CurrentInAmpsType    batteryCurrent; \
                 CoordinateSystemType coordinateSystem; \
                 LinearVelocity2DType velocity; \
               }; };",
        );
        // Bug F: no member is skipped (typedef-to-primitive resolves to double).
        assert!(
            !cpp.contains("not supported (skip)"),
            "typedef/nested members must not be skipped (Bug F)"
        );
        // Bug B: nested struct + enum referenced with absolute namespace.
        assert!(
            cpp.contains("::nga::LinearVelocity2DType"),
            "nested struct must be absolutely qualified (Bug B)"
        );
        assert!(
            cpp.contains("::nga::CoordinateSystemType"),
            "nested enum must be absolutely qualified (Bug B)"
        );
    }

    /// Regression for Bug G: a self-referential recursive type
    /// (XTypes §7.4.5) once stack-overflowed the generator (the encodability
    /// walk `scoped_struct` → `typespec_supported` → `scoped_struct` cycled
    /// forever). It must now generate without panicking, store the recursion
    /// behind a `std::vector` (heap indirection), and serialize the recursive
    /// member via the splice path — never dropping it ("not supported (skip)").
    #[test]
    fn recursive_struct_generates_without_overflow_and_no_skip() {
        let cpp = gen_cpp(
            "module conf { \
               struct TreeNode { long value; sequence<TreeNode> children; }; \
             };",
        );
        assert!(
            cpp.contains("std::vector<::conf::TreeNode>"),
            "recursion must be stored behind a vector (heap indirection)"
        );
        assert!(
            !cpp.contains("not supported (skip)"),
            "recursive member must be serialized, not dropped (no wire data loss)"
        );
        // The splice path references the type's own type_support, not an
        // (infinite) inline expansion.
        assert!(
            cpp.contains("topic_type_support<::conf::TreeNode>"),
            "recursive member must splice via its own topic_type_support"
        );
    }

    /// Regression for Bug G: a forward-declared struct that is then
    /// self-referential must also generate cleanly (the forward decl was the
    /// second crash trigger).
    #[test]
    fn forward_declared_recursive_struct_generates() {
        let cpp = gen_cpp(
            "module conf { \
               struct Node; \
               struct Node { long v; sequence<Node> next; }; \
             };",
        );
        assert!(cpp.contains("std::vector<::conf::Node>"));
        assert!(!cpp.contains("not supported (skip)"));
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
        // "class_field" is not reserved; test with an annotation trick:
        // we force a reserved name via the builder API.
        // Instead, we check the path directly through check_identifier:
        let res = type_map::check_identifier("class");
        assert!(matches!(res, Err(CppGenError::InvalidName { .. })));
        let _ = ast; // unused but illustrates the idea
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
    fn struct_has_field_order_constructor() {
        // The field-order ctor enables brace-init `S t{23, "A7"};` (the
        // website C++ snippet). Default ctor + accessors stay intact.
        let cpp = gen_cpp("struct S { long celsius; string sensor_id; };");
        assert!(
            cpp.contains("S(int32_t celsius, std::string sensor_id)"),
            "field-order ctor signature missing:\n{cpp}"
        );
        assert!(
            cpp.contains(": celsius_(std::move(celsius)), sensor_id_(std::move(sensor_id)) {}"),
            "member-init list missing:\n{cpp}"
        );
        // Default ctor must still be present (brace-init coexists with it).
        assert!(cpp.contains("S() = default;"));
    }

    #[test]
    fn zero_field_struct_has_no_field_order_constructor() {
        // A no-member struct must NOT emit a parameterless ctor — it would be
        // ambiguous with the defaulted default constructor.
        let cpp = gen_cpp("struct Empty { };");
        assert!(cpp.contains("Empty() = default;"));
        // Only the defaulted ctor line should mention `Empty(`; no extra
        // `Empty()` redeclaration and no `std::move`.
        assert_eq!(
            cpp.matches("Empty(").count(),
            2, // `Empty() = default;` and `~Empty() = default;` (destructor)
            "unexpected extra constructor for zero-field struct:\n{cpp}"
        );
        assert!(!cpp.contains("std::move"));
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
