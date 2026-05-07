// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C5.2: DDS-PSM-CXX 1.0 Header-Skeleton-Layer.
//!
//! Statt eines vollen Codegens (der Rust→C++-Cross-Compile waere) liefert
//! diese Schicht **statische Header-Templates** fuer den `dds::core::*`-
//! Namespace, plus Codegen-Helper, die diese Templates fuer ein konkretes
//! Topic-IDL emittieren.
//!
//! Spec-Referenz: OMG DDS-PSM-CXX 1.0 (formal/22-08-04).
//!
//! # Bestandteile
//! - `dds::core::Reference<T>`/`dds::core::Value<T>`-Pattern (Spec §7.1.6).
//! - Exception-Hierarchie (Spec §7.1.7).
//! - Listener / Status / Condition / WaitSet (Spec §8.3).
//! - Domain/Topic/Pub/Sub-Klassen (Spec §8.1).
//!
//! # API
//! - [`emit_psm_cxx_includes`]: erzeugt `#include`-Block fuer einen
//!   konkreten Topic-Namen.
//! - [`emit_reference_value_pattern`]: emittiert die Reference/Value-
//!   Templates (`Reference<T>`, `Value<T>`).
//! - [`emit_listener_skeleton`]: emittiert die Listener-Klassen mit den
//!   13 Status-Callbacks aus Block F.
//! - [`emit_exception_hierarchy`]: emittiert die DDS-Exception-Klassen.
//!
//! Die Templates liegen statisch unter `templates/dds-psm-cxx/*.hpp.tmpl`
//! und werden via `include_str!` zur Build-Zeit eingebettet.

use core::fmt::Write;

use crate::error::CppGenError;

/// Statische Templates (eingebettet via `include_str!`).
const TPL_CORE_HPP: &str = include_str!("../templates/dds-psm-cxx/core.hpp.tmpl");
const TPL_REFERENCE_HPP: &str = include_str!("../templates/dds-psm-cxx/reference.hpp.tmpl");
const TPL_EXCEPTIONS_HPP: &str = include_str!("../templates/dds-psm-cxx/exceptions.hpp.tmpl");
const TPL_LISTENER_HPP: &str = include_str!("../templates/dds-psm-cxx/listener.hpp.tmpl");
const TPL_CONDITION_HPP: &str = include_str!("../templates/dds-psm-cxx/condition.hpp.tmpl");

/// Erzeugt den `#include`-Block fuer einen konkreten Topic.
///
/// Der Block enthaelt sowohl die `dds::core::*`-Header (Reference, Value,
/// Status, QoS, Entity) als auch die Standard-Bibliotheks-Includes, die
/// fuer die Topic-Bindings benoetigt werden.
///
/// # Argumente
/// - `participant_name`: Name des DomainParticipant-Headers (ohne
///   `.hpp`-Suffix). Wird als `#include "<name>.hpp"` emittiert.
///
/// # Errors
/// Liefert [`CppGenError::InvalidName`], wenn `participant_name` leer ist
/// oder unsichere Zeichen (`/`, `\`, `..`) enthaelt.
pub fn emit_psm_cxx_includes(participant_name: &str) -> Result<String, CppGenError> {
    if participant_name.is_empty() {
        return Err(CppGenError::InvalidName {
            name: participant_name.to_string(),
            reason: "participant header name must not be empty".into(),
        });
    }
    // Defensive Validierung: keine Path-Traversal-Zeichen.
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

/// Emittiert das Reference/Value-Pattern aus Spec §7.1.6.
///
/// `dds::core::Reference<T>` ist ein nullable Smart-Pointer-Wrapper,
/// `dds::core::Value<T>` ist ein by-value-Wrapper. Beide werden als
/// Template-Klassen im `dds::core`-Namespace emittiert.
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben in den
/// `String`-Buffer scheitert.
pub fn emit_reference_value_pattern(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_REFERENCE_HPP);
    if !TPL_REFERENCE_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emittiert die DDS-Exception-Hierarchie aus Spec §7.1.7.
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben scheitert.
pub fn emit_exception_hierarchy(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_EXCEPTIONS_HPP);
    if !TPL_EXCEPTIONS_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emittiert das Listener-Skeleton aus Spec §8.3.
///
/// Liefert Listener-Klassen mit allen 13 Status-Callbacks aus Block F als
/// virtuelle Methoden mit Default-Implementation `{}`.
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben scheitert.
pub fn emit_listener_skeleton(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_LISTENER_HPP);
    if !TPL_LISTENER_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emittiert die Condition/WaitSet-Klassen aus Spec §8.3.
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben scheitert.
pub fn emit_condition_skeleton(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_CONDITION_HPP);
    if !TPL_CONDITION_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Emittiert die `dds::core`-Basis-Header (Time, Duration, InstanceHandle,
/// Sample<T>).
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben scheitert.
pub fn emit_core_basics(out: &mut String) -> Result<(), CppGenError> {
    out.push_str(TPL_CORE_HPP);
    if !TPL_CORE_HPP.ends_with('\n') {
        out.push('\n');
    }
    Ok(())
}

/// Erzeugt einen vollstaendigen DDS-PSM-CXX-Skeleton-Header inkl. aller
/// statischer Templates. Wird in den Fixture-Tests gegen die existierenden
/// C5.1-a-Outputs gemerged geprueft.
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben scheitert.
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
        // Listener-Callbacks fuer alle 13 Status-Klassen (DCPS Spec):
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
        // Mindestens drei Erscheinungen von "namespace dds":
        let count = s.matches("namespace dds").count();
        assert!(count >= 3, "expected >=3 namespace dds blocks, got {count}");
    }
}
