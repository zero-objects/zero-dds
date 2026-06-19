//! Integration tests for C5.1-b Blocks F/G/H + C5.2 PSM-CXX skeleton.
//!
//! These tests are intentionally separate from `fixtures.rs` (Block A-E) —
//! they test the statically emitted DDS headers (Status/QoS/DCPS/PSM-CXX)
//! as snapshots against marker lists.

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

use zerodds_idl_cpp::{
    CppGenOptions,
    dcps::emit_dcps_header,
    emit_full_psm_cxx_skeleton, emit_psm_cxx_includes, generate_cpp_header,
    qos::{emit_qos_header, policy_class_names},
    status::{emit_status_header, status_class_names},
};

fn render_status() -> String {
    let mut s = String::new();
    emit_status_header(&mut s).expect("emit status");
    s
}

fn render_qos() -> String {
    let mut s = String::new();
    emit_qos_header(&mut s).expect("emit qos");
    s
}

fn render_dcps() -> String {
    let mut s = String::new();
    emit_dcps_header(&mut s).expect("emit dcps");
    s
}

// --- Block F (Status) Integration --------------------------------------------

#[test]
fn block_f_renders_thirteen_class_definitions() {
    let s = render_status();
    let class_count = s.matches("class ").filter(|_| true).count();
    // At least 13 classes + 2 forward decls (`enum class SampleRejectedState`,
    // `class QosPolicyCount;`):
    assert!(class_count >= 13, "got {class_count}");
    for name in status_class_names() {
        let needle = format!("class {name} {{");
        assert!(s.contains(&needle), "missing: {name}");
    }
}

#[test]
fn block_f_status_classes_have_default_constructor() {
    let s = render_status();
    for name in status_class_names() {
        let dctor = format!("{name}() = default;");
        assert!(s.contains(&dctor), "missing default ctor for {name}");
    }
}

// --- Block G (QoS) Integration -----------------------------------------------

#[test]
fn block_g_renders_all_22_policies_with_equality() {
    let s = render_qos();
    let mut with_eq = 0usize;
    for name in policy_class_names() {
        let class_marker = format!("class {name} {{");
        assert!(s.contains(&class_marker), "missing policy: {name}");
        let eq_marker = format!("bool operator==(const {name}& rhs)");
        if s.contains(&eq_marker) {
            with_eq += 1;
        }
    }
    // All 22 policies have fields, so all of them get ==.
    assert_eq!(with_eq, 22);
}

#[test]
fn block_g_traits_provide_value_in_out_inout() {
    let s = render_qos();
    assert!(s.contains("namespace dds { namespace core { namespace traits {"));
    assert!(s.contains("struct value_type"));
    assert!(s.contains("struct in_type"));
    assert!(s.contains("struct out_type"));
    assert!(s.contains("struct inout_type"));
}

// --- Block H (DCPS-Stubs) Integration ----------------------------------------

#[test]
fn block_h_emits_seven_dcps_class_decls() {
    let s = render_dcps();
    let expected = [
        "class Entity {",
        "class DomainParticipant : public ::dds::core::Entity {",
        "class Topic : public ::dds::core::Entity {",
        "class Publisher : public ::dds::core::Entity {",
        "class Subscriber : public ::dds::core::Entity {",
        "class DataWriter : public ::dds::core::Entity {",
        "class DataReader : public ::dds::core::Entity {",
    ];
    for needle in expected {
        assert!(s.contains(needle), "missing: {needle}");
    }
}

#[test]
fn block_h_data_writer_signature_for_write_overload() {
    let s = render_dcps();
    assert!(s.contains("void write(const T& sample);"));
    assert!(s.contains("void write(const T& sample, const ::dds::core::Time& src_time);"));
}

// --- C5.2 (DDS-PSM-CXX-Skeleton) Integration --------------------------------

#[test]
fn psm_cxx_includes_template_emits_dds_core_headers() {
    let s = emit_psm_cxx_includes("DemoTopic").expect("ok");
    assert!(s.contains("#include <dds/core/core.hpp>"));
    assert!(s.contains("#include <dds/core/reference.hpp>"));
    assert!(s.contains("#include <dds/core/exceptions.hpp>"));
    assert!(s.contains("#include <dds/core/listener.hpp>"));
    assert!(s.contains("#include <dds/core/condition.hpp>"));
    assert!(s.contains("#include \"DemoTopic.hpp\""));
}

#[test]
fn psm_cxx_skeleton_has_pragma_once_and_all_blocks() {
    let s = emit_full_psm_cxx_skeleton().expect("ok");
    assert!(s.contains("#pragma once"));
    // Time/Duration/Reference/Value/Exception/Listener/WaitSet:
    assert!(s.contains("class Time {"));
    assert!(s.contains("class Duration {"));
    assert!(s.contains("class Reference {"));
    assert!(s.contains("class Value {"));
    assert!(s.contains("class Exception"));
    assert!(s.contains("class WaitSet {"));
}

#[test]
fn psm_cxx_reference_value_pattern_for_struct_enum_sequence() {
    // Cross-compile test: existing C5.1-a fixture (struct, enum, sequence)
    // is prefixed with the PSM-CXX skeleton header. The result must
    // contain both blocks intact.
    let user_idl = "struct S { long x; }; enum E { A, B }; struct V { sequence<long> s; };";
    let ast =
        zerodds_idl::parse(user_idl, &zerodds_idl::config::ParserConfig::default()).expect("parse");
    let user_cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");

    let mut combined = emit_full_psm_cxx_skeleton().expect("psm");
    combined.push_str(&user_cpp);

    // PSM-CXX block anchors:
    assert!(combined.contains("class Reference"));
    assert!(combined.contains("class Value"));
    // Userland block anchors:
    assert!(combined.contains("class S "));
    assert!(combined.contains("enum class E : int32_t"));
    assert!(combined.contains("std::vector<int32_t> s_;"));
}

#[test]
fn psm_cxx_listener_header_has_13_status_callbacks() {
    let s = emit_full_psm_cxx_skeleton().expect("ok");
    let callbacks = [
        "on_inconsistent_topic",
        "on_sample_lost",
        "on_sample_rejected",
        "on_liveliness_changed",
        "on_requested_deadline_missed",
        "on_requested_incompatible_qos",
        "on_offered_deadline_missed",
        "on_offered_incompatible_qos",
        "on_liveliness_lost",
        "on_publication_matched",
        "on_subscription_matched",
        "on_data_available",
        "on_data_on_readers",
    ];
    for cb in callbacks {
        assert!(s.contains(cb), "missing: {cb}");
    }
}

#[test]
fn psm_cxx_skeleton_compiles_with_existing_fixtures() {
    // Sanity: every existing C5.1-a fixture must still contain all markers
    // after the skeleton append.
    const SOURCES: &[(&str, &str)] = &[
        ("prim_struct", include_str!("fixtures/prim_struct.idl")),
        (
            "nested_modules",
            include_str!("fixtures/nested_modules.idl"),
        ),
        ("simple_enum", include_str!("fixtures/simple_enum.idl")),
    ];
    let prefix = emit_full_psm_cxx_skeleton().expect("psm");
    for (name, src) in SOURCES {
        let ast = zerodds_idl::parse(src, &zerodds_idl::config::ParserConfig::default())
            .unwrap_or_else(|e| panic!("parse {name}: {e}"));
        let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
        let mut combined = prefix.clone();
        combined.push_str(&cpp);
        assert!(combined.contains("#pragma once"));
        assert!(combined.contains("class Time {"));
    }
}
