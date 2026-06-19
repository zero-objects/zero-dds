// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Block-G: QoS policy + type traits (Spec idl4-cpp-1.0 §7.5).
//!
//! Emits the 22 OMG-DDS-1.4 QoS policies as C++17 structs in the
//! `dds::core::policy` namespace. Each policy gets:
//! - default constructor + special members,
//! - reference-pattern getters (mutable + const),
//! - a setter pair (`const T&` + `T&&` move setter),
//! - equality operators `==`/`!=` (clean comparison where relevant).
//!
//! Spec table: DDS 1.4 §2.2.3 (Table 2.6/2.7) plus DDS-XTypes §7.6.4
//! for DataRepresentation/TypeConsistencyEnforcement.
//!
//! Additionally, the type traits from idl4-cpp §7.1.4 Tab.7.1
//! (`value_type<T>`, `in_type<T>`, `out_type<T>`, `inout_type<T>`) are
//! emitted as templates in the `dds::core` namespace.

use core::fmt::Write;

use crate::error::CppGenError;

/// Description of a QoS policy field.
#[derive(Debug, Clone, Copy)]
struct QosField {
    /// Field name (snake_case).
    name: &'static str,
    /// C++ type expression.
    cpp_ty: &'static str,
    /// Default value (C++ initialization expression).
    default_init: &'static str,
}

/// Description of a QoS policy class.
#[derive(Debug, Clone, Copy)]
struct QosSpec {
    /// Class name (PascalCase).
    name: &'static str,
    /// Fields of the policy.
    fields: &'static [QosField],
    /// Spec reference (DDS 1.4 §2.2.3 + table row).
    spec_ref: &'static str,
}

/// 22 QoS policies from OMG DDS 1.4. The order follows Table 2.6 +
/// extensions from DDS-Security 1.1 / DDS-XTypes 1.3.
#[allow(clippy::too_many_lines)]
const POLICIES: &[QosSpec] = &[
    QosSpec {
        name: "UserDataQosPolicy",
        spec_ref: "§2.2.3.1",
        fields: &[QosField {
            name: "value",
            cpp_ty: "std::vector<uint8_t>",
            default_init: "{}",
        }],
    },
    QosSpec {
        name: "TopicDataQosPolicy",
        spec_ref: "§2.2.3.2",
        fields: &[QosField {
            name: "value",
            cpp_ty: "std::vector<uint8_t>",
            default_init: "{}",
        }],
    },
    QosSpec {
        name: "GroupDataQosPolicy",
        spec_ref: "§2.2.3.3",
        fields: &[QosField {
            name: "value",
            cpp_ty: "std::vector<uint8_t>",
            default_init: "{}",
        }],
    },
    QosSpec {
        name: "DurabilityQosPolicy",
        spec_ref: "§2.2.3.4",
        fields: &[QosField {
            name: "kind",
            cpp_ty: "::dds::core::policy::DurabilityKind",
            default_init: "::dds::core::policy::DurabilityKind::Volatile",
        }],
    },
    QosSpec {
        name: "DurabilityServiceQosPolicy",
        spec_ref: "§2.2.3.5",
        fields: &[
            QosField {
                name: "service_cleanup_delay",
                cpp_ty: "::dds::core::Duration",
                default_init: "::dds::core::Duration::zero()",
            },
            QosField {
                name: "history_kind",
                cpp_ty: "::dds::core::policy::HistoryKind",
                default_init: "::dds::core::policy::HistoryKind::KeepLast",
            },
            QosField {
                name: "history_depth",
                cpp_ty: "int32_t",
                default_init: "1",
            },
            QosField {
                name: "max_samples",
                cpp_ty: "int32_t",
                default_init: "-1",
            },
            QosField {
                name: "max_instances",
                cpp_ty: "int32_t",
                default_init: "-1",
            },
            QosField {
                name: "max_samples_per_instance",
                cpp_ty: "int32_t",
                default_init: "-1",
            },
        ],
    },
    QosSpec {
        name: "PresentationQosPolicy",
        spec_ref: "§2.2.3.6",
        fields: &[
            QosField {
                name: "access_scope",
                cpp_ty: "::dds::core::policy::PresentationAccessScopeKind",
                default_init: "::dds::core::policy::PresentationAccessScopeKind::Instance",
            },
            QosField {
                name: "coherent_access",
                cpp_ty: "bool",
                default_init: "false",
            },
            QosField {
                name: "ordered_access",
                cpp_ty: "bool",
                default_init: "false",
            },
        ],
    },
    QosSpec {
        name: "DeadlineQosPolicy",
        spec_ref: "§2.2.3.7",
        fields: &[QosField {
            name: "period",
            cpp_ty: "::dds::core::Duration",
            default_init: "::dds::core::Duration::infinite()",
        }],
    },
    QosSpec {
        name: "LatencyBudgetQosPolicy",
        spec_ref: "§2.2.3.8",
        fields: &[QosField {
            name: "duration",
            cpp_ty: "::dds::core::Duration",
            default_init: "::dds::core::Duration::zero()",
        }],
    },
    QosSpec {
        name: "OwnershipQosPolicy",
        spec_ref: "§2.2.3.9",
        fields: &[QosField {
            name: "kind",
            cpp_ty: "::dds::core::policy::OwnershipKind",
            default_init: "::dds::core::policy::OwnershipKind::Shared",
        }],
    },
    QosSpec {
        name: "OwnershipStrengthQosPolicy",
        spec_ref: "§2.2.3.10",
        fields: &[QosField {
            name: "value",
            cpp_ty: "int32_t",
            default_init: "0",
        }],
    },
    QosSpec {
        name: "LivelinessQosPolicy",
        spec_ref: "§2.2.3.11",
        fields: &[
            QosField {
                name: "kind",
                cpp_ty: "::dds::core::policy::LivelinessKind",
                default_init: "::dds::core::policy::LivelinessKind::Automatic",
            },
            QosField {
                name: "lease_duration",
                cpp_ty: "::dds::core::Duration",
                default_init: "::dds::core::Duration::infinite()",
            },
        ],
    },
    QosSpec {
        name: "TimeBasedFilterQosPolicy",
        spec_ref: "§2.2.3.12",
        fields: &[QosField {
            name: "minimum_separation",
            cpp_ty: "::dds::core::Duration",
            default_init: "::dds::core::Duration::zero()",
        }],
    },
    QosSpec {
        name: "PartitionQosPolicy",
        spec_ref: "§2.2.3.13",
        fields: &[QosField {
            name: "name",
            cpp_ty: "std::vector<std::string>",
            default_init: "{}",
        }],
    },
    QosSpec {
        name: "ReliabilityQosPolicy",
        spec_ref: "§2.2.3.14",
        fields: &[
            QosField {
                name: "kind",
                cpp_ty: "::dds::core::policy::ReliabilityKind",
                default_init: "::dds::core::policy::ReliabilityKind::BestEffort",
            },
            QosField {
                name: "max_blocking_time",
                cpp_ty: "::dds::core::Duration",
                default_init: "::dds::core::Duration::from_millis(100)",
            },
        ],
    },
    QosSpec {
        name: "TransportPriorityQosPolicy",
        spec_ref: "§2.2.3.15",
        fields: &[QosField {
            name: "value",
            cpp_ty: "int32_t",
            default_init: "0",
        }],
    },
    QosSpec {
        name: "LifespanQosPolicy",
        spec_ref: "§2.2.3.16",
        fields: &[QosField {
            name: "duration",
            cpp_ty: "::dds::core::Duration",
            default_init: "::dds::core::Duration::infinite()",
        }],
    },
    QosSpec {
        name: "DestinationOrderQosPolicy",
        spec_ref: "§2.2.3.17",
        fields: &[QosField {
            name: "kind",
            cpp_ty: "::dds::core::policy::DestinationOrderKind",
            default_init: "::dds::core::policy::DestinationOrderKind::ByReceptionTimestamp",
        }],
    },
    QosSpec {
        name: "HistoryQosPolicy",
        spec_ref: "§2.2.3.18",
        fields: &[
            QosField {
                name: "kind",
                cpp_ty: "::dds::core::policy::HistoryKind",
                default_init: "::dds::core::policy::HistoryKind::KeepLast",
            },
            QosField {
                name: "depth",
                cpp_ty: "int32_t",
                default_init: "1",
            },
        ],
    },
    QosSpec {
        name: "ResourceLimitsQosPolicy",
        spec_ref: "§2.2.3.19",
        fields: &[
            QosField {
                name: "max_samples",
                cpp_ty: "int32_t",
                default_init: "-1",
            },
            QosField {
                name: "max_instances",
                cpp_ty: "int32_t",
                default_init: "-1",
            },
            QosField {
                name: "max_samples_per_instance",
                cpp_ty: "int32_t",
                default_init: "-1",
            },
        ],
    },
    QosSpec {
        name: "EntityFactoryQosPolicy",
        spec_ref: "§2.2.3.20",
        fields: &[QosField {
            name: "autoenable_created_entities",
            cpp_ty: "bool",
            default_init: "true",
        }],
    },
    QosSpec {
        name: "WriterDataLifecycleQosPolicy",
        spec_ref: "§2.2.3.21",
        fields: &[QosField {
            name: "autodispose_unregistered_instances",
            cpp_ty: "bool",
            default_init: "true",
        }],
    },
    QosSpec {
        name: "ReaderDataLifecycleQosPolicy",
        spec_ref: "§2.2.3.22",
        fields: &[
            QosField {
                name: "autopurge_nowriter_samples_delay",
                cpp_ty: "::dds::core::Duration",
                default_init: "::dds::core::Duration::infinite()",
            },
            QosField {
                name: "autopurge_disposed_samples_delay",
                cpp_ty: "::dds::core::Duration",
                default_init: "::dds::core::Duration::infinite()",
            },
        ],
    },
];

/// Writes the complete QoS header.
///
/// # Errors
/// Returns [`CppGenError::Internal`] if writing to the
/// `String` buffer fails.
pub fn emit_qos_header(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "// Block-G: DDS-QoS-Policies (Spec dds-1.4 §2.2.3).").map_err(fmt_err)?;
    writeln!(
        out,
        "namespace dds {{ namespace core {{ namespace policy {{"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Kind enums (a forward decl suffices — full definitions live in the
    // PSM-CXX runtime).
    writeln!(
        out,
        "// Forward declarations of the kind enums (see DDS 1.4 §2.2.3)."
    )
    .map_err(fmt_err)?;
    for kind in [
        "DurabilityKind",
        "PresentationAccessScopeKind",
        "OwnershipKind",
        "LivelinessKind",
        "ReliabilityKind",
        "DestinationOrderKind",
        "HistoryKind",
    ] {
        writeln!(out, "enum class {kind} : int32_t;").map_err(fmt_err)?;
    }
    writeln!(out, "using QosPolicyId = int32_t;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    for p in POLICIES {
        emit_policy_class(out, p)?;
    }

    writeln!(out, "}} }} }} // namespace dds::core::policy").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    emit_type_traits(out)?;

    Ok(())
}

/// Returns the list of all emitted policy class names.
#[must_use]
pub fn policy_class_names() -> Vec<&'static str> {
    POLICIES.iter().map(|p| p.name).collect()
}

fn emit_policy_class(out: &mut String, p: &QosSpec) -> Result<(), CppGenError> {
    writeln!(out, "/// {} ({})", p.name, p.spec_ref).map_err(fmt_err)?;
    writeln!(out, "class {} {{", p.name).map_err(fmt_err)?;
    writeln!(out, "public:").map_err(fmt_err)?;
    writeln!(out, "    {}() = default;", p.name).map_err(fmt_err)?;
    writeln!(out, "    ~{}() = default;", p.name).map_err(fmt_err)?;
    writeln!(out, "    {0}(const {0}&) = default;", p.name).map_err(fmt_err)?;
    writeln!(out, "    {0}({0}&&) noexcept = default;", p.name).map_err(fmt_err)?;
    writeln!(out, "    {0}& operator=(const {0}&) = default;", p.name).map_err(fmt_err)?;
    writeln!(out, "    {0}& operator=({0}&&) noexcept = default;", p.name).map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    for f in p.fields {
        // Const-Getter:
        writeln!(
            out,
            "    const {ty}& {name}() const {{ return {name}_; }}",
            ty = f.cpp_ty,
            name = f.name
        )
        .map_err(fmt_err)?;
        // Mutable-Getter:
        writeln!(
            out,
            "    {ty}& {name}() {{ return {name}_; }}",
            ty = f.cpp_ty,
            name = f.name
        )
        .map_err(fmt_err)?;
        // Setter (const-ref):
        writeln!(
            out,
            "    void {name}(const {ty}& value) {{ {name}_ = value; }}",
            ty = f.cpp_ty,
            name = f.name
        )
        .map_err(fmt_err)?;
        // Setter (Move-Variant):
        writeln!(
            out,
            "    void {name}({ty}&& value) {{ {name}_ = std::move(value); }}",
            ty = f.cpp_ty,
            name = f.name
        )
        .map_err(fmt_err)?;
    }

    if !p.fields.is_empty() {
        writeln!(out).map_err(fmt_err)?;
        // Equality operator: compares all fields.
        write!(
            out,
            "    bool operator==(const {}& rhs) const {{ return ",
            p.name
        )
        .map_err(fmt_err)?;
        for (i, f) in p.fields.iter().enumerate() {
            if i > 0 {
                write!(out, " && ").map_err(fmt_err)?;
            }
            write!(out, "{name}_ == rhs.{name}_", name = f.name).map_err(fmt_err)?;
        }
        writeln!(out, "; }}").map_err(fmt_err)?;
        writeln!(
            out,
            "    bool operator!=(const {0}& rhs) const {{ return !(*this == rhs); }}",
            p.name
        )
        .map_err(fmt_err)?;
    }

    if !p.fields.is_empty() {
        writeln!(out).map_err(fmt_err)?;
        writeln!(out, "private:").map_err(fmt_err)?;
        for f in p.fields {
            writeln!(
                out,
                "    {ty} {name}_{{{init}}};",
                ty = f.cpp_ty,
                name = f.name,
                init = f.default_init
            )
            .map_err(fmt_err)?;
        }
    }

    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Emits the type traits from idl4-cpp §7.1.4 Tab.7.1.
///
/// - `value_type<T>`: by-value Typ.
/// - `in_type<T>`: const-ref-Argument-Typ.
/// - `out_type<T>`: out-Parameter-Ref.
/// - `inout_type<T>`: inout-Parameter-Ref.
fn emit_type_traits(out: &mut String) -> Result<(), CppGenError> {
    writeln!(out, "// idl4-cpp §7.1.4 Tab.7.1 — Argument-Type-Traits.").map_err(fmt_err)?;
    writeln!(
        out,
        "namespace dds {{ namespace core {{ namespace traits {{"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "struct value_type {{ using type = T; }};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "struct in_type {{ using type = const T&; }};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "struct out_type {{ using type = T&; }};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "struct inout_type {{ using type = T&; }};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    // Convenience-Aliase (`_t`-Suffix wie in std):
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "using value_type_t = typename value_type<T>::type;").map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "using in_type_t = typename in_type<T>::type;").map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "using out_type_t = typename out_type<T>::type;").map_err(fmt_err)?;
    writeln!(out, "template <typename T>").map_err(fmt_err)?;
    writeln!(out, "using inout_type_t = typename inout_type<T>::type;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "}} }} }} // namespace dds::core::traits").map_err(fmt_err)?;
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
        emit_qos_header(&mut s).expect("emit");
        s
    }

    #[test]
    fn qos_namespace_is_dds_core_policy() {
        let s = render();
        assert!(s.contains("namespace dds { namespace core { namespace policy {"));
        assert!(s.contains("} } } // namespace dds::core::policy"));
    }

    #[test]
    fn reliability_qos_policy_has_kind_and_max_blocking_time() {
        let s = render();
        assert!(s.contains("class ReliabilityQosPolicy {"));
        assert!(s.contains("ReliabilityKind kind_"));
        assert!(s.contains("::dds::core::Duration max_blocking_time_"));
    }

    #[test]
    fn move_setter_uses_rvalue_and_std_move() {
        let s = render();
        assert!(s.contains("void max_blocking_time(::dds::core::Duration&& value)"));
        assert!(s.contains("max_blocking_time_ = std::move(value);"));
    }

    #[test]
    fn const_getter_returns_const_ref() {
        let s = render();
        assert!(s.contains("const ::dds::core::policy::ReliabilityKind& kind() const"));
        assert!(s.contains("::dds::core::policy::ReliabilityKind& kind()"));
    }

    #[test]
    fn mutable_getter_returns_mutable_ref() {
        let s = render();
        // The mutable getter signature without "const":
        let needle = "::dds::core::Duration& max_blocking_time() {";
        assert!(s.contains(needle));
    }

    #[test]
    fn history_qos_policy_has_kind_and_depth() {
        let s = render();
        assert!(s.contains("class HistoryQosPolicy {"));
        assert!(s.contains("HistoryKind kind_"));
        assert!(s.contains("int32_t depth_{1}"));
    }

    #[test]
    fn resource_limits_has_three_fields() {
        let s = render();
        assert!(s.contains("class ResourceLimitsQosPolicy {"));
        assert!(s.contains("int32_t max_samples_{-1}"));
        assert!(s.contains("int32_t max_instances_{-1}"));
        assert!(s.contains("int32_t max_samples_per_instance_{-1}"));
    }

    #[test]
    fn deadline_qos_policy_default_is_infinite() {
        let s = render();
        assert!(s.contains("class DeadlineQosPolicy {"));
        assert!(s.contains("Duration period_{::dds::core::Duration::infinite()}"));
    }

    #[test]
    fn user_data_uses_byte_vector() {
        let s = render();
        assert!(s.contains("class UserDataQosPolicy {"));
        assert!(s.contains("std::vector<uint8_t> value_"));
    }

    #[test]
    fn partition_qos_policy_uses_string_vector() {
        let s = render();
        assert!(s.contains("class PartitionQosPolicy {"));
        assert!(s.contains("std::vector<std::string> name_"));
    }

    #[test]
    fn all_22_policies_emitted() {
        let names = policy_class_names();
        assert_eq!(names.len(), 22);
        let s = render();
        for n in names {
            assert!(s.contains(&format!("class {n} {{")), "missing: {n}");
        }
    }

    #[test]
    fn type_traits_namespace_emitted() {
        let s = render();
        assert!(s.contains("namespace dds { namespace core { namespace traits {"));
        assert!(s.contains("struct value_type"));
        assert!(s.contains("struct in_type"));
        assert!(s.contains("struct out_type"));
        assert!(s.contains("struct inout_type"));
    }

    #[test]
    fn type_traits_value_type_is_t_by_value() {
        let s = render();
        assert!(s.contains("struct value_type { using type = T; };"));
    }

    #[test]
    fn type_traits_in_type_is_const_ref() {
        let s = render();
        assert!(s.contains("struct in_type { using type = const T&; };"));
    }

    #[test]
    fn type_traits_out_and_inout_are_ref() {
        let s = render();
        assert!(s.contains("struct out_type { using type = T&; };"));
        assert!(s.contains("struct inout_type { using type = T&; };"));
    }

    #[test]
    fn type_trait_aliases_with_t_suffix() {
        let s = render();
        assert!(s.contains("using value_type_t = typename value_type<T>::type;"));
        assert!(s.contains("using in_type_t = typename in_type<T>::type;"));
        assert!(s.contains("using out_type_t = typename out_type<T>::type;"));
        assert!(s.contains("using inout_type_t = typename inout_type<T>::type;"));
    }

    #[test]
    fn equality_operator_compares_fields() {
        let s = render();
        // ReliabilityQosPolicy compares both fields:
        assert!(s.contains("kind_ == rhs.kind_ && max_blocking_time_ == rhs.max_blocking_time_"));
        assert!(s.contains("bool operator!=(const ReliabilityQosPolicy& rhs)"));
    }

    #[test]
    fn entity_factory_default_is_true() {
        let s = render();
        assert!(s.contains("class EntityFactoryQosPolicy {"));
        assert!(s.contains("bool autoenable_created_entities_{true}"));
    }

    #[test]
    fn ownership_strength_default_is_zero() {
        let s = render();
        assert!(s.contains("class OwnershipStrengthQosPolicy {"));
        assert!(s.contains("int32_t value_{0}"));
    }

    #[test]
    fn liveliness_qos_policy_has_kind_and_lease() {
        let s = render();
        assert!(s.contains("class LivelinessQosPolicy {"));
        assert!(s.contains("LivelinessKind kind_"));
        assert!(s.contains("Duration lease_duration_"));
    }
}
