// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C5.2: DDS-PSM-CXX 1.0 header skeleton layer.
//!
//! Instead of a full codegen (which would be a Rust→C++ cross-compile),
//! this layer provides **static header templates** for the `dds::core::*`
//! namespace, plus codegen helpers that emit these templates for a concrete
//! topic IDL.
//!
//! Spec reference: OMG DDS-PSM-CXX 1.0 (formal/22-08-04).
//!
//! # Components
//! - `dds::core::Reference<T>`/`dds::core::Value<T>` pattern (Spec §7.1.6).
//! - Exception hierarchy (Spec §7.1.7).
//! - Listener / Status / Condition / WaitSet (Spec §8.3).
//! - Domain/Topic/Pub/Sub classes (Spec §8.1).
//!
//! # API
//! - [`emit_psm_cxx_includes`]: produces the `#include` block for a
//!   concrete topic name.
//! - [`emit_reference_value_pattern`]: emits the Reference/Value
//!   templates (`Reference<T>`, `Value<T>`).
//! - [`emit_listener_skeleton`]: emits the listener classes with the
//!   13 status callbacks from Block F.
//! - [`emit_exception_hierarchy`]: emits the DDS exception classes.
//!
//! The templates live statically under `templates/dds-psm-cxx/*.hpp.tmpl`
//! and are embedded at build time via `include_str!`.

use core::fmt::Write;

use crate::error::CppGenError;

/// Static templates (embedded via `include_str!`).
const TPL_CORE_HPP: &str = include_str!("../templates/dds-psm-cxx/core.hpp.tmpl");
const TPL_REFERENCE_HPP: &str = include_str!("../templates/dds-psm-cxx/reference.hpp.tmpl");
const TPL_EXCEPTIONS_HPP: &str = include_str!("../templates/dds-psm-cxx/exceptions.hpp.tmpl");
const TPL_LISTENER_HPP: &str = include_str!("../templates/dds-psm-cxx/listener.hpp.tmpl");
const TPL_CONDITION_HPP: &str = include_str!("../templates/dds-psm-cxx/condition.hpp.tmpl");

/// Produces the `#include` block for a concrete topic.
///
/// The block contains both the `dds::core::*` headers (Reference, Value,
/// Status, QoS, Entity) and the standard-library includes needed for the
/// topic bindings.
///
/// # Arguments
/// - `participant_name`: name of the DomainParticipant header (without
///   the `.hpp` suffix). Emitted as `#include "<name>.hpp"`.
///
/// # Errors
/// Returns [`CppGenError::InvalidName`] if `participant_name` is empty
/// or contains unsafe characters (`/`, `\`, `..`).
pub fn emit_psm_cxx_includes(participant_name: &str) -> Result<String, CppGenError> {
    if participant_name.is_empty() {
        return Err(CppGenError::InvalidName {
            name: participant_name.to_string(),
            reason: "participant header name must not be empty".into(),
        });
    }
    // Defensive validation: no path-traversal characters.
    if participant_name.contains('/')
        || participant_name.contains('\\')
        || participant_name.contains("..")
    {
        return Err(CppGenError::InvalidName {
            name: participant_name.to_string(),
            reason: "participant header name must not contain path separators".into(),
        });
    }

    let mut out = String::new();
    writeln!(out, "// dds-psm-cxx-1.0 includes (generated).").map_err(fmt_err)?;
    writeln!(out, "#include <cstdint>").map_err(fmt_err)?;
    writeln!(out, "#include <memory>").map_err(fmt_err)?;
    writeln!(out, "#include <string>").map_err(fmt_err)?;
    writeln!(out, "#include <vector>").map_err(fmt_err)?;
    writeln!(out, "#include <exception>").map_err(fmt_err)?;
    writeln!(out, "#include <utility>").map_err(fmt_err)?;
    writeln!(out, "#include <dds/core/core.hpp>").map_err(fmt_err)?;
    writeln!(out, "#include <dds/core/reference.hpp>").map_err(fmt_err)?;
    writeln!(out, "#include <dds/core/exceptions.hpp>").map_err(fmt_err)?;
    writeln!(out, "#include <dds/core/listener.hpp>").map_err(fmt_err)?;
    writeln!(out, "#include <dds/core/condition.hpp>").map_err(fmt_err)?;
    writeln!(out, "#include \"{participant_name}.hpp\"").map_err(fmt_err)?;
    Ok(out)
}

/// Emits the Reference/Value pattern from Spec §7.1.6.
///
/// `dds::core::Reference<T>` is a nullable smart-pointer wrapper,
/// `dds::core::Value<T>` is a by-value wrapper. Both are emitted as
/// template classes in the `dds::core` namespace.
///
/// # Errors
/// Returns [`CppGenError::Internal`] if writing into the
/// `String` buffer fails.
pub fn emit_reference_value_pattern(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_REFERENCE_HPP);
    if !TPL_REFERENCE_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emits the DDS exception hierarchy from Spec §7.1.7.
///
/// # Errors
/// Returns [`CppGenError::Internal`] if writing fails.
pub fn emit_exception_hierarchy(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_EXCEPTIONS_HPP);
    if !TPL_EXCEPTIONS_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emits the listener skeleton from Spec §8.3.
///
/// Provides listener classes with all 13 status callbacks from Block F as
/// virtual methods with the default implementation `{}`.
///
/// # Errors
/// Returns [`CppGenError::Internal`] if writing fails.
pub fn emit_listener_skeleton(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_LISTENER_HPP);
    if !TPL_LISTENER_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emits the Condition/WaitSet classes from Spec §8.3.
///
/// # Errors
/// Returns [`CppGenError::Internal`] if writing fails.
pub fn emit_condition_skeleton(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_CONDITION_HPP);
    if !TPL_CONDITION_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emits the `dds::core` base headers (Time, Duration, InstanceHandle,
/// Sample<T>).
///
/// # Errors
/// Returns [`CppGenError::Internal`] if writing fails.
pub fn emit_core_basics(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_CORE_HPP);
    if !TPL_CORE_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Produces a complete DDS-PSM-CXX skeleton header including all
/// static templates. In the fixture tests it is checked merged against the
/// existing C5.1-a outputs.
///
/// # Errors
/// Returns [`CppGenError::Internal`] if writing fails.
pub fn emit_full_psm_cxx_skeleton() -> Result<String, CppGenError> {
    let mut out = String::new();
    writeln!(
        out,
        "// Generated dds-psm-cxx-1.0 skeleton (zerodds idl-cpp C5.2)."
    )
    .map_err(fmt_err)?;
    writeln!(out, "#pragma once").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "#include <cstdint>").map_err(fmt_err)?;
    writeln!(out, "#include <memory>").map_err(fmt_err)?;
    writeln!(out, "#include <string>").map_err(fmt_err)?;
    writeln!(out, "#include <vector>").map_err(fmt_err)?;
    writeln!(out, "#include <exception>").map_err(fmt_err)?;
    writeln!(out, "#include <utility>").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    emit_core_basics(&mut out)?;
    emit_reference_value_pattern(&mut out)?;
    emit_exception_hierarchy(&mut out)?;
    emit_listener_skeleton(&mut out)?;
    emit_condition_skeleton(&mut out)?;
    Ok(out)
}

fn fmt_err(_: core::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn includes_with_valid_name() {
        let s = emit_psm_cxx_includes("MyParticipant").expect("ok");
        assert!(s.contains("#include <dds/core/core.hpp>"));
        assert!(s.contains("#include <dds/core/reference.hpp>"));
        assert!(s.contains("#include \"MyParticipant.hpp\""));
    }

    #[test]
    fn includes_rejects_empty() {
        let res = emit_psm_cxx_includes("");
        assert!(matches!(res, Err(CppGenError::InvalidName { .. })));
    }

    #[test]
    fn includes_rejects_path_traversal() {
        let res = emit_psm_cxx_includes("../etc/passwd");
        assert!(matches!(res, Err(CppGenError::InvalidName { .. })));
        let res2 = emit_psm_cxx_includes("a/b");
        assert!(matches!(res2, Err(CppGenError::InvalidName { .. })));
        let res3 = emit_psm_cxx_includes("a\\b");
        assert!(matches!(res3, Err(CppGenError::InvalidName { .. })));
    }

    #[test]
    fn reference_pattern_emits_reference_template() {
        let mut s = String::new();
        emit_reference_value_pattern(&mut s).expect("ok");
        assert!(s.contains("namespace dds { namespace core {"));
        assert!(s.contains("class Reference {"));
        assert!(s.contains("class Value {"));
    }

    #[test]
    fn exception_hierarchy_emits_dds_exception_classes() {
        let mut s = String::new();
        emit_exception_hierarchy(&mut s).expect("ok");
        assert!(s.contains("class Exception"));
        assert!(s.contains("class PreconditionNotMetError"));
        assert!(s.contains("class NotEnabledError"));
        assert!(s.contains("class OutOfResourcesError"));
        assert!(s.contains("class IllegalOperationError"));
    }

    #[test]
    fn listener_skeleton_has_13_callbacks() {
        let mut s = String::new();
        emit_listener_skeleton(&mut s).expect("ok");
        // Listener callbacks for all 13 status classes (DCPS Spec):
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
            assert!(s.contains(cb), "missing callback: {cb}");
        }
    }

    #[test]
    fn condition_skeleton_has_waitset() {
        let mut s = String::new();
        emit_condition_skeleton(&mut s).expect("ok");
        assert!(s.contains("class Condition"));
        assert!(s.contains("class WaitSet"));
        assert!(s.contains("class GuardCondition"));
        assert!(s.contains("class StatusCondition"));
        assert!(s.contains("class ReadCondition"));
    }

    #[test]
    fn core_basics_define_time_duration_handle() {
        let mut s = String::new();
        emit_core_basics(&mut s).expect("ok");
        assert!(s.contains("class Time"));
        assert!(s.contains("class Duration"));
        assert!(s.contains("class InstanceHandle"));
        assert!(s.contains("Sample"));
    }

    #[test]
    fn full_skeleton_combines_all_blocks() {
        let s = emit_full_psm_cxx_skeleton().expect("ok");
        assert!(s.contains("#pragma once"));
        assert!(s.contains("class Time"));
        assert!(s.contains("class Reference"));
        assert!(s.contains("class Exception"));
        assert!(s.contains("on_data_available"));
        assert!(s.contains("class WaitSet"));
    }

    #[test]
    fn full_skeleton_namespaces_are_dds_core() {
        let s = emit_full_psm_cxx_skeleton().expect("ok");
        // At least three occurrences of "namespace dds":
        let count = s.matches("namespace dds").count();
        assert!(count >= 3, "expected >=3 namespace dds blocks, got {count}");
    }
}
