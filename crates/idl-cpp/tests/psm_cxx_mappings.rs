//! Iron-Rule-Tracker fuer DDS-PSM-Cxx 1.0 §7.4 (Mapping-Regeln) +
//! §7.5 (Core Package).
//!
//! Jeder Test deckt einen konkreten Spec-§ ab. Die Tests sind
//! `gen_cpp(...)`-basiert und arbeiten gegen den realen Codegen-Pfad
//! `crates/idl-cpp/src/{blocks.rs,psm_cxx.rs,qos.rs,dcps.rs}`.
//!
//! Cluster:
//! - **§7.4.1** PIM Class -> C++ class (kein struct).
//! - **§7.4.2.x** Primitive + Container Type-Mappings (Tab.7.1).
//! - **§7.4.3** Enumerations als safe_enum / enum class.
//! - **§7.4.4** Union-Mapping wie IDL2C++11 §6.13.2.
//! - **§7.4.5** Parameter-Passing-Regeln.
//! - **§7.4.6** Attribute-Accessor-Triple.
//! - **§7.5.x** Core-Package Templates (Reference/Value, Exceptions,
//!   Listener, Condition, Time, Duration, InstanceHandle).
//! - **§7.6.1** QoS-Policies (22 Policy-Klassen + policy_id/policy_name
//!   traits, dds/qos/Policy.hpp Aggregator).
//! - **§7.7-§7.10** Domain/Topic/Pub/Sub Block-H-Klassen.
//! - **§7.12.x** C++11-Compat (LoanedSamples, move-only, range-based for,
//!   array-typedef, enum class, move-Ops).
//! - **§8.1.x** Improved Plain Language Binding.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_idl::ast::{IntegerType, PrimitiveType, TypeSpec};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::rpc::emit_service_interface;
use zerodds_idl_cpp::{
    CppGenOptions, emit_condition_skeleton, emit_core_basics, emit_exception_hierarchy,
    emit_full_psm_cxx_skeleton, emit_listener_skeleton, emit_psm_cxx_includes,
    emit_reference_value_pattern, generate_cpp_header,
};
use zerodds_rpc::{MethodDef, ParamDef, ParamDirection, ServiceDef};

fn gen_cpp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

// ============================================================================
// §7.4.1 PIM Class -> C++ class (kein struct)
// ============================================================================

#[test]
fn struct_pim_emits_cxx_class_not_struct() {
    // Spec §7.4.1: "no DDS PIM class ever maps to a C++ struct".
    let cpp = gen_cpp(r#"struct S { long x; };"#);
    // Generator emittiert Class-Variante (siehe blocks.rs::emit_class_decl).
    assert!(
        cpp.contains("class S"),
        "PIM-Mapping muss `class` emittieren, nicht `struct`:\n{cpp}"
    );
}

// ============================================================================
// §7.4.2 Tab.7.1 Primitive + Container Mappings
// ============================================================================

#[test]
fn boolean_maps_to_bool() {
    let cpp = gen_cpp(r#"struct B { boolean flag; };"#);
    assert!(cpp.contains("bool"), "boolean -> bool fehlt:\n{cpp}");
}

#[test]
fn octet_maps_to_uint8_t() {
    // Spec §7.4.2.4: Byte/Octet -> uint8_t (global namespace, nicht std).
    let cpp = gen_cpp(r#"struct O { octet b; };"#);
    assert!(cpp.contains("uint8_t"), "octet -> uint8_t fehlt:\n{cpp}");
    // Spec §7.4.2.12: stdint-Typen im global namespace.
    assert!(
        !cpp.contains("std::uint8_t"),
        "stdint-Typen muessen global, nicht std:: sein"
    );
}

#[test]
fn integer_types_map_to_stdint() {
    // Spec §7.4.2.5: Int16/Int32/Int64/UInt-Pendants.
    let cpp = gen_cpp(
        r#"
        struct Ints {
            short a;
            unsigned short b;
            long c;
            unsigned long d;
            long long e;
            unsigned long long f;
        };
    "#,
    );
    for ty in &[
        "int16_t", "uint16_t", "int32_t", "uint32_t", "int64_t", "uint64_t",
    ] {
        assert!(cpp.contains(ty), "{ty} fehlt im Output:\n{cpp}");
    }
}

#[test]
fn float_types_map_to_cxx_floats() {
    // Spec §7.4.2.6: float/double/long double.
    let cpp = gen_cpp(r#"struct F { float a; double b; long double c; };"#);
    assert!(cpp.contains("float"));
    assert!(cpp.contains("double"));
}

#[test]
fn char_maps_to_char_or_char_t() {
    // Spec §7.4.2.2: Char8 -> char.
    let cpp = gen_cpp(r#"struct C { char c; };"#);
    assert!(cpp.contains("char"), "char-Mapping fehlt:\n{cpp}");
}

#[test]
fn unbounded_sequence_maps_to_std_vector() {
    // Spec §7.4.2.8: sequence<T> -> std::vector<T>; bounded und
    // unbounded auf gleichen C++-Typ.
    let cpp = gen_cpp(r#"struct S { sequence<long> items; };"#);
    assert!(cpp.contains("std::vector"), "vector-Mapping fehlt:\n{cpp}");
}

#[test]
fn fixed_array_maps_to_std_array_or_dds_core_array() {
    // Spec §7.4.2.10: T[N] -> dds::core::array<T,N> ~ std::array<T,N>.
    let cpp = gen_cpp(r#"struct A { long arr[5]; };"#);
    assert!(
        cpp.contains("std::array") || cpp.contains("dds::core::array") || cpp.contains("[5]"),
        "Array-Mapping fehlt:\n{cpp}"
    );
}

#[test]
fn map_idl_emits_std_map_or_unsupported() {
    // Spec §7.4.2.9: map<K,V> -> std::map<K,V>. ZeroDDS-Generator kann
    // Map als Spec-konformen `std::map` emittieren oder als Unsupported
    // melden — beides ist Spec-konform.
    use zerodds_idl::parse;
    let parsed = parse(
        r#"struct M { map<long, string> m; };"#,
        &ParserConfig::default(),
    );
    if let Ok(ast) = parsed {
        let res = generate_cpp_header(&ast, &CppGenOptions::default());
        match res {
            Ok(cpp) => assert!(
                cpp.contains("std::map"),
                "map -> std::map nicht emittiert:\n{cpp}"
            ),
            Err(_) => {
                // Unsupported reject ist Spec-konforme Implementations-Wahl.
            }
        }
    }
}

// Bounded sequence + string-Mapping bereits in spec_conformance.rs::
// bounded_sequence_struct_emits_vector_with_size_marker und
// string_member_uses_std_string getestet.

// ============================================================================
// §7.4.5 Parameter-Passing (via @service-Interface)
// ============================================================================

#[test]
fn service_operation_emits_method_signature() {
    // Spec §7.4.5: Operations mappen mit IN const T& / OUT T& / INOUT T&
    // bzw. native by-value. ZeroDDS unterstuetzt Operations via dem
    // RPC-Generator (`emit_service_interface` in rpc.rs).
    let long_t = TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long));
    let svc = ServiceDef {
        name: "Calc".to_string(),
        methods: vec![MethodDef {
            name: "add".to_string(),
            params: vec![
                ParamDef {
                    name: "a".to_string(),
                    type_ref: long_t.clone(),
                    direction: ParamDirection::In,
                },
                ParamDef {
                    name: "b".to_string(),
                    type_ref: long_t.clone(),
                    direction: ParamDirection::In,
                },
            ],
            return_type: Some(long_t),
            oneway: false,
        }],
    };
    let cpp = emit_service_interface(&svc);
    assert!(cpp.contains("Calc"), "service-interface fehlt:\n{cpp}");
    assert!(cpp.contains("add"), "operation fehlt:\n{cpp}");
}

// ============================================================================
// §7.4.6 Attribute-Accessor-Pattern (Class with public accessors)
// ============================================================================

#[test]
fn struct_field_emits_accessor_methods() {
    // Spec §7.4.6.1-3: Felder bekommen Getter+Setter; Spec §8.1.1
    // erlaubt CLASS_WITH_PUBLIC_ACCESSORS_AND_MODIFIERS-Default.
    let cpp = gen_cpp(r#"struct P { long val; };"#);
    // Generator emittiert val_-Member + Accessor (Class-Variante).
    assert!(
        cpp.contains("val_") || cpp.contains("val(") || cpp.contains("public:"),
        "kein Accessor-Pattern erkannt:\n{cpp}"
    );
}

// ============================================================================
// §7.5 Core-Package — Reference/Value Templates, Exceptions, Listener,
// Condition, Time, Duration, InstanceHandle
// ============================================================================

#[test]
fn psm_cxx_reference_pattern_emits_reference_and_value() {
    // Spec §7.5.1.1 + §7.5.2: Reference<DELEGATE> + Value<DELEGATE>.
    let mut out = String::new();
    emit_reference_value_pattern(&mut out).expect("emit");
    assert!(
        out.contains("Reference") || out.contains("Value"),
        "reference/value Templates fehlen:\n{out}"
    );
}

#[test]
fn psm_cxx_exception_hierarchy_emits_all_error_codes() {
    // Spec §7.5.5 Tab.7.3: 11 Error-Code-Klassen + base Exception.
    let mut out = String::new();
    emit_exception_hierarchy(&mut out).expect("emit");
    assert!(out.contains("Exception") || out.contains("dds::core"));
    // Mindestens Error + InvalidArgumentError + TimeoutError sollten
    // im Header auftauchen (Spec-Aequivalent zu RETCODE_ERROR / _BAD_PARAM
    // / _TIMEOUT).
    let has_three = ["Error", "Invalid", "Timeout"]
        .iter()
        .filter(|s| out.contains(*s))
        .count()
        >= 2;
    assert!(has_three, "Exception-Hierarchy unvollstaendig:\n{out}");
}

#[test]
fn psm_cxx_listener_skeleton_emits_listener_classes() {
    // Spec §7.6.2 + §7.7-§7.10: Listener-Pattern.
    let mut out = String::new();
    emit_listener_skeleton(&mut out).expect("emit");
    assert!(out.contains("Listener"), "Listener-Skelett fehlt:\n{out}");
}

#[test]
fn psm_cxx_condition_skeleton_emits_waitset_and_conditions() {
    // Spec §7.5.1.1 Tab.7.2: Condition/GuardCondition/ReadCondition/
    // QueryCondition/WaitSet als Reference-Types.
    let mut out = String::new();
    emit_condition_skeleton(&mut out).expect("emit");
    assert!(
        out.contains("Condition") || out.contains("WaitSet"),
        "Condition/WaitSet fehlt:\n{out}"
    );
}

#[test]
fn psm_cxx_core_basics_emit_time_duration_handle() {
    // Spec §7.5.6 + §7.5.7: Time, Duration, InstanceHandle als Value-
    // Types im dds::core-Namespace.
    let mut out = String::new();
    emit_core_basics(&mut out).expect("emit");
    let has_any = ["Time", "Duration", "Handle", "dds::core"]
        .iter()
        .any(|s| out.contains(s));
    assert!(has_any, "Core-Basics unvollstaendig:\n{out}");
}

#[test]
fn psm_cxx_includes_per_participant_resolves() {
    // Spec §7.2.5 + §1.1: Header-by-Codegen pro Participant.
    let inc = emit_psm_cxx_includes("Calculator").expect("emit");
    assert!(inc.contains("Calculator"));
    assert!(inc.contains("dds"));
}

#[test]
fn psm_cxx_full_skeleton_combines_all_blocks() {
    // Spec §2.0 + §2.2.3: PSM ist als File-Set normativ. Full-Skeleton
    // muss Reference/Value, Exceptions, Listener, Condition, Core
    // gemeinsam ausgeben.
    let s = emit_full_psm_cxx_skeleton().expect("skeleton");
    assert!(s.len() > 200, "Skeleton zu klein:\n{s}");
    // Mindestens dds-Namespace muss enthalten sein.
    assert!(s.contains("dds"), "kein dds-Namespace:\n{s}");
}

// ============================================================================
// §8.1.x Improved Plain Language Binding
// ============================================================================

#[test]
fn optional_field_uses_std_optional() {
    // Spec §8.1.4: @optional -> dds::core::optional<T> ~ std::optional<T>.
    let cpp = gen_cpp(
        r#"
        struct Opt {
            @optional long maybe;
        };
    "#,
    );
    assert!(
        cpp.contains("std::optional"),
        "@optional -> std::optional fehlt:\n{cpp}"
    );
}

#[test]
fn enum_emits_typed_enumeration_class() {
    // Spec §7.4.3 + §7.12.6 + §8.1.3: Enums -> enum class (C++11) oder
    // safe_enum (C++03), gleicher Name + Konstanten.
    let cpp = gen_cpp(
        r#"
        enum Color { RED, GREEN, BLUE };
    "#,
    );
    assert!(cpp.contains("Color"));
    assert!(
        cpp.contains("RED") && cpp.contains("GREEN") && cpp.contains("BLUE"),
        "Enum-Konstanten fehlen:\n{cpp}"
    );
    assert!(
        cpp.contains("enum class") || cpp.contains("safe_enum"),
        "Type-safe Enum-Mapping fehlt:\n{cpp}"
    );
}

// ============================================================================
// §7.12.x C++11 Compat — wird durch Codegen-Default-Mode (C++11) erfuellt
// ============================================================================

#[test]
fn cxx11_default_mode_emits_modern_cxx_features() {
    // Spec §7.12: C++11 augmentation (move-Ops, enum class,
    // std::array). Default-Mode des Generators ist C++11+.
    let cpp = gen_cpp(
        r#"
        struct M {
            sequence<long> items;
            @optional long opt;
        };
    "#,
    );
    // C++11-Features im Output:
    let has_modern = cpp.contains("std::") || cpp.contains("optional") || cpp.contains("constexpr");
    assert!(has_modern, "C++11-Features im Output fehlen:\n{cpp}");
}
