// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Block-F: DDS-Status-Mapping (Spec idl4-cpp-1.0 §7.4 + dds-1.4 §2.2.4.1).
//!
//! Emittiert die 13 OMG-DCPS-Status-Strukturen als C++17-Header in den
//! Namespace `dds::core::status`. Die Klassen folgen dem Reference-Pattern
//! aus C5.1-a (private Storage `field_`, mutable + const Getter, Setter).
//!
//! Spec-Quellen:
//! - DDS-PSM-CXX 1.0 §7.5.5 (Listener / Status / Condition).
//! - DDS 1.4 §2.2.4.1 (Communication-Status).
//!
//! # Liste der 13 Status-Klassen
//!
//! | Status                          | Counter-Felder                                       |
//! |---------------------------------|------------------------------------------------------|
//! | InconsistentTopicStatus         | total_count, total_count_change                      |
//! | SampleLostStatus                | total_count, total_count_change                      |
//! | SampleRejectedStatus            | total_count, total_count_change, last_reason, last_instance_handle |
//! | LivelinessChangedStatus         | alive_count, not_alive_count, alive_count_change, not_alive_count_change, last_publication_handle |
//! | RequestedDeadlineMissedStatus   | total_count, total_count_change, last_instance_handle |
//! | RequestedIncompatibleQosStatus  | total_count, total_count_change, last_policy_id, policies |
//! | OfferedDeadlineMissedStatus     | total_count, total_count_change, last_instance_handle |
//! | OfferedIncompatibleQosStatus    | total_count, total_count_change, last_policy_id, policies |
//! | LivelinessLostStatus            | total_count, total_count_change                      |
//! | PublicationMatchedStatus        | total_count, total_count_change, current_count, current_count_change, last_subscription_handle |
//! | SubscriptionMatchedStatus       | total_count, total_count_change, current_count, current_count_change, last_publication_handle |
//! | DataAvailableStatus             | (marker only)                                        |
//! | DataOnReadersStatus             | (marker only)                                        |
//!
//! Die Statusse mit `(marker only)` sind reine Notification-Marker ohne
//! Counter (Spec DDS 1.4 §2.2.4.1.1.1). Sie werden trotzdem als leere
//! Klassen emittiert, damit Listener-Signaturen einheitlich sind.

use core::fmt::Write;

use crate::error::CppGenError;

/// Beschreibung eines DDS-Status-Felds.
#[derive(Debug, Clone, Copy)]
struct StatusField {
    /// Feld-Name (snake_case wie in Spec).
    name: &'static str,
    /// C++-Typ-Ausdruck.
    cpp_ty: &'static str,
}

/// Beschreibung einer DDS-Status-Klasse.
#[derive(Debug, Clone, Copy)]
struct StatusSpec {
    /// Klassen-Name (PascalCase wie in Spec).
    name: &'static str,
    /// Felder der Status-Klasse.
    fields: &'static [StatusField],
    /// Doc-Kommentar mit Spec-Referenz.
    spec_ref: &'static str,
}

/// Vollstaendige Liste der 13 DCPS-Status-Strukturen.
const STATUSES: &[StatusSpec] = &[
    StatusSpec {
        name: "InconsistentTopicStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.4",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
        ],
    },
    StatusSpec {
        name: "SampleLostStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.5",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
        ],
    },
    StatusSpec {
        name: "SampleRejectedStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.6",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_reason",
                cpp_ty: "::dds::core::status::SampleRejectedState",
            },
            StatusField {
                name: "last_instance_handle",
                cpp_ty: "::dds::core::InstanceHandle",
            },
        ],
    },
    StatusSpec {
        name: "LivelinessChangedStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.7",
        fields: &[
            StatusField {
                name: "alive_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "not_alive_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "alive_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "not_alive_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_publication_handle",
                cpp_ty: "::dds::core::InstanceHandle",
            },
        ],
    },
    StatusSpec {
        name: "RequestedDeadlineMissedStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.8",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_instance_handle",
                cpp_ty: "::dds::core::InstanceHandle",
            },
        ],
    },
    StatusSpec {
        name: "RequestedIncompatibleQosStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.9",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_policy_id",
                cpp_ty: "::dds::core::policy::QosPolicyId",
            },
            StatusField {
                name: "policies",
                cpp_ty: "std::vector<::dds::core::status::QosPolicyCount>",
            },
        ],
    },
    StatusSpec {
        name: "OfferedDeadlineMissedStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.10",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_instance_handle",
                cpp_ty: "::dds::core::InstanceHandle",
            },
        ],
    },
    StatusSpec {
        name: "OfferedIncompatibleQosStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.11",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_policy_id",
                cpp_ty: "::dds::core::policy::QosPolicyId",
            },
            StatusField {
                name: "policies",
                cpp_ty: "std::vector<::dds::core::status::QosPolicyCount>",
            },
        ],
    },
    StatusSpec {
        name: "LivelinessLostStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.12",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
        ],
    },
    StatusSpec {
        name: "PublicationMatchedStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.13",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "current_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "current_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_subscription_handle",
                cpp_ty: "::dds::core::InstanceHandle",
            },
        ],
    },
    StatusSpec {
        name: "SubscriptionMatchedStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.14",
        fields: &[
            StatusField {
                name: "total_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "total_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "current_count",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "current_count_change",
                cpp_ty: "int32_t",
            },
            StatusField {
                name: "last_publication_handle",
                cpp_ty: "::dds::core::InstanceHandle",
            },
        ],
    },
    StatusSpec {
        name: "DataAvailableStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.2 (marker)",
        fields: &[],
    },
    StatusSpec {
        name: "DataOnReadersStatus",
        spec_ref: "DDS 1.4 §2.2.4.1.3 (marker)",
        fields: &[],
    },
];

/// Schreibt den vollstaendigen Status-Header in `out`.
///
/// Der Header enthaelt einen Block-Kommentar mit Spec-Referenz, oeffnet den
/// Namespace `dds::core::status`, emittiert die 13 Status-Klassen und
/// schliesst die Namespaces wieder.
///
/// # Errors
/// Liefert [`CppGenError::Internal`], wenn das Schreiben in den
/// `String`-Buffer scheitert (sollte praktisch nie auftreten).
pub fn emit_status_header(out: &mut String) -> Result<(), CppGenError> {
    writeln!(
        out,
        "// Block-F: DDS-Status-Strukturen (Spec dds-1.4 §2.2.4.1)."
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "namespace dds {{ namespace core {{ namespace status {{"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Forward-decls fuer Hilfstypen; volle Definitionen liegen ausserhalb
    // dieses Codegens (in der DDS-PSM-CXX-Runtime).
    writeln!(out, "// Forward-Declarations fuer Hilfstypen.").map_err(fmt_err)?;
    writeln!(out, "enum class SampleRejectedState : int32_t;").map_err(fmt_err)?;
    writeln!(out, "class QosPolicyCount;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    for s in STATUSES {
        emit_status_class(out, s)?;
    }

    writeln!(out, "}} }} }} // namespace dds::core::status").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Liefert die Liste aller emittierten Status-Klassen-Namen.
#[must_use]
pub fn status_class_names() -> Vec<&'static str> {
    STATUSES.iter().map(|s| s.name).collect()
}

fn emit_status_class(out: &mut String, s: &StatusSpec) -> Result<(), CppGenError> {
    writeln!(out, "/// {} ({})", s.name, s.spec_ref).map_err(fmt_err)?;
    writeln!(out, "class {} {{", s.name).map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(out, "    {}() = default;", s.name).map_err(fmt_err)?;
    writeln!(out, "    ~{}() = default;", s.name).map_err(fmt_err)?;
    writeln!(out, "    {0}(const {0}&) = default;", s.name).map_err(fmt_err)?;
    writeln!(out, "    {0}({0}&&) noexcept = default;", s.name).map_err(fmt_err)?;
    writeln!(out, "    {0}& operator=(const {0}&) = default;", s.name).map_err(fmt_err)?;
    writeln!(out, "    {0}& operator=({0}&&) noexcept = default;", s.name).map_err(fmt_err)?;

    if !s.fields.is_empty() {
        writeln!(out).map_err(fmt_err)?;
        // Reset-Helper: setzt alle *_change-Felder auf 0 (Spec DDS 1.4
        // §2.2.4.1.1: Reset on read).
        writeln!(
            out,
            "    /// Reset-on-read: setzt alle *_change-Felder auf 0."
        )
        .map_err(fmt_err)?;
        writeln!(out, "    void reset_changes() {{").map_err(fmt_err)?;
        for f in s.fields {
            if f.name.ends_with("_change") {
                writeln!(out, "        {}_ = 0;", f.name).map_err(fmt_err)?;
            }
        }
        writeln!(out, "    }}").map_err(fmt_err)?;
    }

    if !s.fields.is_empty() {
        writeln!(out).map_err(fmt_err)?;
        // Const + mutable Getter, Setter (const-ref + Move-Setter).
        for f in s.fields {
            writeln!(
                out,
                "    const {ty}& {name}() const {{ return {name}_; }}",
                ty = f.cpp_ty,
                name = f.name
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "    {ty}& {name}() {{ return {name}_; }}",
                ty = f.cpp_ty,
                name = f.name
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "    void {name}(const {ty}& value) {{ {name}_ = value; }}",
                ty = f.cpp_ty,
                name = f.name
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "    void {name}({ty}&& value) {{ {name}_ = std::move(value); }}",
                ty = f.cpp_ty,
                name = f.name
            )
            .map_err(fmt_err)?;
        }

        writeln!(out).map_err(fmt_err)?;
        writeln!(out, "private:").map_err(fmt_err)?;
        for f in s.fields {
            writeln!(out, "    {ty} {name}_{{}};", ty = f.cpp_ty, name = f.name)
                .map_err(fmt_err)?;
        }
    }

    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn fmt_err(_: core::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    fn render() -> String {
        let mut s = String::new();
        emit_status_header(&mut s).expect("emit");
        s
    }

    #[test]
    fn status_namespace_is_dds_core_status() {
        let s = render();
        assert!(s.contains("namespace dds { namespace core { namespace status {"));
        assert!(
            s.contains("}}} // namespace dds::core::status")
                || s.contains("} } } // namespace dds::core::status")
        );
    }

    #[test]
    fn sample_lost_status_has_total_count_fields() {
        let s = render();
        assert!(s.contains("class SampleLostStatus {"));
        assert!(s.contains("int32_t total_count_{};"));
        assert!(s.contains("int32_t total_count_change_{};"));
    }

    #[test]
    fn sample_rejected_status_has_last_reason() {
        let s = render();
        assert!(s.contains("class SampleRejectedStatus {"));
        assert!(s.contains("SampleRejectedState"));
        assert!(s.contains("last_instance_handle"));
    }

    #[test]
    fn liveliness_changed_status_has_alive_and_not_alive() {
        let s = render();
        assert!(s.contains("class LivelinessChangedStatus {"));
        assert!(s.contains("alive_count_{};"));
        assert!(s.contains("not_alive_count_{};"));
        assert!(s.contains("alive_count_change_{};"));
        assert!(s.contains("not_alive_count_change_{};"));
    }

    #[test]
    fn matched_status_has_current_count_pair() {
        let s = render();
        assert!(s.contains("class PublicationMatchedStatus {"));
        assert!(s.contains("class SubscriptionMatchedStatus {"));
        assert!(s.contains("current_count_{};"));
        assert!(s.contains("current_count_change_{};"));
    }

    #[test]
    fn incompatible_qos_status_has_policy_count_vector() {
        let s = render();
        assert!(s.contains("class RequestedIncompatibleQosStatus {"));
        assert!(s.contains("class OfferedIncompatibleQosStatus {"));
        assert!(s.contains("std::vector<::dds::core::status::QosPolicyCount> policies_{};"));
    }

    #[test]
    fn marker_statuses_emit_empty_class() {
        let s = render();
        assert!(s.contains("class DataAvailableStatus {"));
        assert!(s.contains("class DataOnReadersStatus {"));
        // Marker-Statusse haben keine Felder, daher kein `int32_t total_count_`.
        // Aber sie emittieren Default-Konstruktor:
        assert!(s.contains("DataAvailableStatus() = default;"));
        assert!(s.contains("DataOnReadersStatus() = default;"));
    }

    #[test]
    fn reset_changes_resets_only_change_fields() {
        let s = render();
        // Look at the InconsistentTopicStatus::reset_changes() body:
        let needle = "class InconsistentTopicStatus {";
        let start = s.find(needle).expect("class header");
        let after = &s[start..];
        let reset_pos = after.find("reset_changes()").expect("reset method");
        let reset_block = &after[reset_pos..];
        // Inside, we set total_count_change_ to 0 but never total_count_.
        let close = reset_block.find("    }").expect("end of method");
        let body = &reset_block[..close];
        assert!(body.contains("total_count_change_ = 0;"));
        assert!(!body.contains("total_count_ = 0;"));
    }

    #[test]
    fn all_thirteen_status_classes_emitted() {
        let names = status_class_names();
        assert_eq!(names.len(), 13);
        let s = render();
        for n in names {
            let class_marker = format!("class {n} {{");
            assert!(s.contains(&class_marker), "missing class: {n}");
        }
    }

    #[test]
    fn move_setter_variant_present() {
        let s = render();
        // Move-Setter mit && + std::move:
        assert!(s.contains("void total_count(int32_t&& value)"));
        assert!(s.contains("std::move(value);"));
    }

    #[test]
    fn const_getter_returns_const_ref() {
        let s = render();
        assert!(s.contains("const int32_t& total_count() const"));
        assert!(s.contains("int32_t& total_count() {"));
    }

    #[test]
    fn copy_and_move_special_members_defaulted() {
        let s = render();
        assert!(s.contains("SampleLostStatus(const SampleLostStatus&) = default;"));
        assert!(s.contains("SampleLostStatus(SampleLostStatus&&) noexcept = default;"));
    }
}
