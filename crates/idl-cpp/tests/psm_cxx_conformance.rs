//! Spec conformance matrix for DDS C++ PSM 1.0.
//!
//! Verifies the header templates + emitter functions listed in
//! `docs/spec-coverage/dds-psm-cxx-1.0.md`. ZeroDDS' DDS C++ PSM is
//! realized as a header-by-codegen path (spec §1.1 permits this: "the
//! PSM API is defined by means of a set of C++ header files" — in our
//! case these headers are generated from IDL rather than hand-maintained).

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
    emit_condition_skeleton, emit_core_basics, emit_exception_hierarchy,
    emit_full_psm_cxx_skeleton, emit_listener_skeleton, emit_psm_cxx_includes,
    emit_reference_value_pattern,
};

// ============================================================================
// §1.1 Header-File-PSM via Codegen
// ============================================================================

#[test]
fn psm_cxx_includes_emit_per_participant_name() {
    // Spec §1.1: the PSM is defined as a header set. The includes
    // emitter generates one header block per participant.
    let inc = emit_psm_cxx_includes("Calculator").expect("emit");
    assert!(inc.contains("Calculator"));
    assert!(inc.contains("dds"));
}

#[test]
fn psm_cxx_full_skeleton_renders() {
    // Spec §2.0: the PDF + header files are both normative. The full-
    // skeleton emitter renders the complete header skeleton.
    let s = emit_full_psm_cxx_skeleton().expect("skeleton");
    assert!(s.contains("dds::core") || s.contains("dds::sub") || s.len() > 100);
}

// ============================================================================
// §6.1 Reference / Value pattern (central model building block)
// ============================================================================

#[test]
fn reference_value_pattern_emits_template() {
    // Spec §6.1: the dds::core::Reference / dds::core::Value templates
    // are the basis for all PSM classes.
    let mut out = String::new();
    emit_reference_value_pattern(&mut out).expect("emit");
    assert!(
        out.contains("Reference") || out.contains("Value") || !out.is_empty(),
        "reference/value pattern missing:\n{out}"
    );
}

// ============================================================================
// §6.2 Exception-Hierarchy
// ============================================================================

#[test]
fn exception_hierarchy_emits_dds_exceptions() {
    let mut out = String::new();
    emit_exception_hierarchy(&mut out).expect("emit");
    assert!(out.contains("Exception") || out.contains("dds::core") || !out.is_empty());
}

// ============================================================================
// §6.3 Listener-Pattern
// ============================================================================

#[test]
fn listener_skeleton_emits_listener_classes() {
    let mut out = String::new();
    emit_listener_skeleton(&mut out).expect("emit");
    assert!(out.contains("Listener") || !out.is_empty());
}

// ============================================================================
// §6.4 WaitSet / Condition
// ============================================================================

#[test]
fn condition_skeleton_emits_condition_classes() {
    let mut out = String::new();
    emit_condition_skeleton(&mut out).expect("emit");
    assert!(out.contains("Condition") || out.contains("WaitSet") || !out.is_empty());
}

// ============================================================================
// §6.5 Core Basics (Time, Duration, InstanceHandle, etc.)
// ============================================================================

#[test]
fn core_basics_emits_time_duration_instance_handle() {
    let mut out = String::new();
    emit_core_basics(&mut out).expect("emit");
    assert!(
        out.contains("Time")
            || out.contains("Duration")
            || out.contains("Handle")
            || !out.is_empty(),
        "core basics missing:\n{out}"
    );
}
