// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPS-Fehlertypen. An OMG DDS 1.4 §2.2.2.1 `ReturnCode_t` angelehnt,
//! aber als Rust-Result<T, DdsError> — deutlich angenehmer als
//! sprayed-error-codes.

extern crate alloc;
use alloc::string::String;

/// Fehler aus DCPS-Operationen. Halbwegs analog zum Spec-
/// `ReturnCode_t`-Enum; wir lassen allerdings `RETCODE_OK` weg
/// (stattdessen `Result::Ok`) und mergen `BAD_PARAMETER` +
/// `PRECONDITION_NOT_MET` wo die Unterscheidung nichts hilft.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DdsError {
    /// Ungueltiger Parameter (z.B. leerer Topic-Name).
    BadParameter {
        /// Welcher Parameter, kurz.
        what: &'static str,
    },
    /// Operation paesst nicht zum Entity-Zustand (z.B. `write` auf
    /// einen DataWriter, dessen Participant beendet wurde).
    PreconditionNotMet {
        /// Kurze Beschreibung.
        reason: &'static str,
    },
    /// Unterliegende Wire/CDR-Operation ist gefehlschlagen.
    WireError {
        /// Nachricht, statisch oder dynamisch.
        message: String,
    },
    /// QoS-Policy-Set nicht konsistent. Beispiel: Reliability=
    /// Best-Effort + History=KeepAll ohne resource_limits — Spec
    /// §2.2.3 nennt einige inkompatible Kombinationen.
    InconsistentPolicy {
        /// Welche Policy(s) kollidieren.
        what: &'static str,
    },
    /// Transport-Operation fehlgeschlagen.
    TransportError {
        /// Statisches Label des Fehlers.
        label: &'static str,
    },
    /// Resource-Limit erreicht (max_samples, max_instances, etc.).
    OutOfResources {
        /// Welches Limit.
        what: &'static str,
    },
    /// Timeout bei blockierender Operation (`take_w_timeout`, etc.).
    Timeout,
    /// Operation nicht implementiert in v1.2.
    Unsupported {
        /// Kurz-Beschreibung.
        feature: &'static str,
    },
    /// Operation durch Security-Policy untersagt — DDS-Security 1.2
    /// §7.3.25, DCPS 1.4 §2.2.2.1.1 ReturnCode_t = NOT_ALLOWED_BY_SECURITY.
    /// Der Permissions-/Access-Plugin hat das Lesen/Schreiben auf einem
    /// geschuetzten Topic abgelehnt; der Caller darf nicht erfahren,
    /// **welches** Permissions-Detail die Ursache war (Information-Leak).
    NotAllowedBySecurity {
        /// Kurze, sicherheits-unkritische Beschreibung.
        what: &'static str,
    },
    /// `IllegalOperation` — die Operation passt strukturell nicht
    /// (z.B. write() auf einen DataReader). DCPS 1.4 §2.2.2.1.1
    /// ReturnCode_t = ILLEGAL_OPERATION.
    IllegalOperation {
        /// Was nicht geht.
        what: &'static str,
    },
    /// `ImmutablePolicy` — set_qos auf eine post-enable immutable QoS-
    /// Policy. DCPS 1.4 §2.2.2.1.1 ReturnCode_t = IMMUTABLE_POLICY.
    ImmutablePolicy {
        /// Welche Policy.
        policy: &'static str,
    },
    /// `AlreadyDeleted` — Operation auf einer geloeschten Entity.
    /// DCPS 1.4 §2.2.2.1.1 ReturnCode_t = ALREADY_DELETED.
    AlreadyDeleted,
    /// `NoData` — read/take ohne neue Samples. DCPS 1.4 §2.2.2.1.1
    /// ReturnCode_t = NO_DATA. Wird in der Praxis oft als leerer
    /// Sample-Vec zurueckgegeben statt als Error.
    NoData,
    /// `Error` — generischer, nicht-spezifischer Fehler. DCPS 1.4
    /// §2.2.2.1.1 ReturnCode_t = ERROR. Letzte Wahl wenn keine
    /// spezifischere Variante passt; vermeidet Information-Leak
    /// gegenueber dem Caller.
    Other {
        /// Kurze Beschreibung.
        reason: &'static str,
    },
    /// `NotEnabled` — Operation auf einer Entity, die noch nicht via
    /// `enable()` aktiviert wurde. DCPS 1.4 §2.2.2.1.1 ReturnCode_t =
    /// NOT_ENABLED. §2.1.2 — Entities sind nach `create_*` disabled
    /// bis `enable()`; viele Ops MUESSEN dann NotEnabled liefern.
    NotEnabled,
}

impl core::fmt::Display for DdsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadParameter { what } => write!(f, "bad parameter: {what}"),
            Self::PreconditionNotMet { reason } => write!(f, "precondition not met: {reason}"),
            Self::WireError { message } => write!(f, "wire/cdr error: {message}"),
            Self::InconsistentPolicy { what } => write!(f, "inconsistent qos policy: {what}"),
            Self::TransportError { label } => write!(f, "transport error: {label}"),
            Self::OutOfResources { what } => write!(f, "out of resources: {what}"),
            Self::Timeout => f.write_str("dds timeout"),
            Self::Unsupported { feature } => write!(f, "not supported in v1.2: {feature}"),
            Self::NotAllowedBySecurity { what } => {
                write!(f, "not allowed by security policy: {what}")
            }
            Self::IllegalOperation { what } => write!(f, "illegal operation: {what}"),
            Self::ImmutablePolicy { policy } => {
                write!(f, "qos policy is immutable post-enable: {policy}")
            }
            Self::AlreadyDeleted => f.write_str("entity already deleted"),
            Self::NoData => f.write_str("no data available"),
            Self::Other { reason } => write!(f, "dds error: {reason}"),
            Self::NotEnabled => f.write_str("entity not enabled"),
        }
    }
}

/// OMG DDS 1.4 `ReturnCode_t`-Werte. Disjoint mit `Result::Ok` —
/// `RETCODE_OK = 0` ist nicht in dieser Liste, weil Ok via
/// `Result::Ok` repraesentiert wird.
pub mod return_code {
    /// RETCODE_OK = 0 (nur fuer Wire-Mapping; Rust-API nutzt `Ok`).
    pub const OK: i32 = 0;
    /// RETCODE_ERROR.
    pub const ERROR: i32 = 1;
    /// RETCODE_UNSUPPORTED.
    pub const UNSUPPORTED: i32 = 2;
    /// RETCODE_BAD_PARAMETER.
    pub const BAD_PARAMETER: i32 = 3;
    /// RETCODE_PRECONDITION_NOT_MET.
    pub const PRECONDITION_NOT_MET: i32 = 4;
    /// RETCODE_OUT_OF_RESOURCES.
    pub const OUT_OF_RESOURCES: i32 = 5;
    /// RETCODE_NOT_ENABLED.
    pub const NOT_ENABLED: i32 = 6;
    /// RETCODE_IMMUTABLE_POLICY.
    pub const IMMUTABLE_POLICY: i32 = 7;
    /// RETCODE_INCONSISTENT_POLICY.
    pub const INCONSISTENT_POLICY: i32 = 8;
    /// RETCODE_ALREADY_DELETED.
    pub const ALREADY_DELETED: i32 = 9;
    /// RETCODE_TIMEOUT.
    pub const TIMEOUT: i32 = 10;
    /// RETCODE_NO_DATA.
    pub const NO_DATA: i32 = 11;
    /// RETCODE_ILLEGAL_OPERATION.
    pub const ILLEGAL_OPERATION: i32 = 12;
    /// RETCODE_NOT_ALLOWED_BY_SECURITY.
    pub const NOT_ALLOWED_BY_SECURITY: i32 = 13;
}

impl DdsError {
    /// Mappt den Fehler auf den OMG `ReturnCode_t`-Integer-Wert
    /// (DCPS 1.4 §2.2.2.1.1). `WireError` und `TransportError` werden
    /// auf RETCODE_ERROR (1) abgebildet — das ist Spec-konform: beide
    /// sind unspezifizierte Implementations-Fehler.
    #[must_use]
    pub fn as_return_code(&self) -> i32 {
        match self {
            Self::BadParameter { .. } => return_code::BAD_PARAMETER,
            Self::PreconditionNotMet { .. } => return_code::PRECONDITION_NOT_MET,
            Self::WireError { .. } | Self::TransportError { .. } | Self::Other { .. } => {
                return_code::ERROR
            }
            Self::InconsistentPolicy { .. } => return_code::INCONSISTENT_POLICY,
            Self::OutOfResources { .. } => return_code::OUT_OF_RESOURCES,
            Self::Timeout => return_code::TIMEOUT,
            Self::Unsupported { .. } => return_code::UNSUPPORTED,
            Self::NotAllowedBySecurity { .. } => return_code::NOT_ALLOWED_BY_SECURITY,
            Self::IllegalOperation { .. } => return_code::ILLEGAL_OPERATION,
            Self::ImmutablePolicy { .. } => return_code::IMMUTABLE_POLICY,
            Self::AlreadyDeleted => return_code::ALREADY_DELETED,
            Self::NoData => return_code::NO_DATA,
            Self::NotEnabled => return_code::NOT_ENABLED,
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DdsError {}

/// Kurzform fuer DCPS-Ergebnisse.
pub type Result<T> = core::result::Result<T, DdsError>;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // ---- §2.2.1.1.x ReturnCode_t Mapping ----

    #[test]
    fn rc_error_maps_for_wire_and_transport_and_other() {
        // Spec §2.2.1.1.2 RC ERROR — generischer Fehler.
        assert_eq!(
            DdsError::WireError {
                message: "x".into()
            }
            .as_return_code(),
            return_code::ERROR
        );
        assert_eq!(
            DdsError::TransportError { label: "y" }.as_return_code(),
            return_code::ERROR
        );
        assert_eq!(
            DdsError::Other { reason: "z" }.as_return_code(),
            return_code::ERROR
        );
    }

    #[test]
    fn rc_unsupported_maps() {
        assert_eq!(
            DdsError::Unsupported { feature: "f" }.as_return_code(),
            return_code::UNSUPPORTED
        );
    }

    #[test]
    fn rc_bad_parameter_maps() {
        assert_eq!(
            DdsError::BadParameter { what: "p" }.as_return_code(),
            return_code::BAD_PARAMETER
        );
    }

    #[test]
    fn rc_precondition_not_met_maps() {
        assert_eq!(
            DdsError::PreconditionNotMet { reason: "r" }.as_return_code(),
            return_code::PRECONDITION_NOT_MET
        );
    }

    #[test]
    fn rc_out_of_resources_maps() {
        assert_eq!(
            DdsError::OutOfResources { what: "samples" }.as_return_code(),
            return_code::OUT_OF_RESOURCES
        );
    }

    #[test]
    fn rc_not_enabled_maps() {
        assert_eq!(
            DdsError::NotEnabled.as_return_code(),
            return_code::NOT_ENABLED
        );
    }

    #[test]
    fn rc_immutable_policy_maps() {
        assert_eq!(
            DdsError::ImmutablePolicy {
                policy: "Reliability"
            }
            .as_return_code(),
            return_code::IMMUTABLE_POLICY
        );
    }

    #[test]
    fn rc_inconsistent_policy_maps() {
        assert_eq!(
            DdsError::InconsistentPolicy { what: "x" }.as_return_code(),
            return_code::INCONSISTENT_POLICY
        );
    }

    #[test]
    fn rc_already_deleted_maps() {
        assert_eq!(
            DdsError::AlreadyDeleted.as_return_code(),
            return_code::ALREADY_DELETED
        );
    }

    #[test]
    fn rc_timeout_maps() {
        assert_eq!(DdsError::Timeout.as_return_code(), return_code::TIMEOUT);
    }

    #[test]
    fn rc_no_data_maps() {
        assert_eq!(DdsError::NoData.as_return_code(), return_code::NO_DATA);
    }

    #[test]
    fn rc_illegal_operation_maps() {
        assert_eq!(
            DdsError::IllegalOperation {
                what: "write_on_reader"
            }
            .as_return_code(),
            return_code::ILLEGAL_OPERATION
        );
    }

    #[test]
    fn rc_not_allowed_by_security_maps() {
        assert_eq!(
            DdsError::NotAllowedBySecurity { what: "topic" }.as_return_code(),
            return_code::NOT_ALLOWED_BY_SECURITY
        );
    }

    #[test]
    fn rc_constants_have_spec_values() {
        // OMG DDS 1.4 §2.2.2.1.1 numerische Spec-Werte. Ein Drift hier
        // bricht jeden Wire-Mapping-Test in C++/Java-Bridges.
        assert_eq!(return_code::OK, 0);
        assert_eq!(return_code::ERROR, 1);
        assert_eq!(return_code::UNSUPPORTED, 2);
        assert_eq!(return_code::BAD_PARAMETER, 3);
        assert_eq!(return_code::PRECONDITION_NOT_MET, 4);
        assert_eq!(return_code::OUT_OF_RESOURCES, 5);
        assert_eq!(return_code::NOT_ENABLED, 6);
        assert_eq!(return_code::IMMUTABLE_POLICY, 7);
        assert_eq!(return_code::INCONSISTENT_POLICY, 8);
        assert_eq!(return_code::ALREADY_DELETED, 9);
        assert_eq!(return_code::TIMEOUT, 10);
        assert_eq!(return_code::NO_DATA, 11);
        assert_eq!(return_code::ILLEGAL_OPERATION, 12);
        assert_eq!(return_code::NOT_ALLOWED_BY_SECURITY, 13);
    }

    #[test]
    fn rc_display_does_not_leak_security_details() {
        // §2.2.2.1.1 NOT_ALLOWED_BY_SECURITY — Caller darf nicht den
        // Permissions-Detail-Grund erfahren (Information-Leak).
        let e = DdsError::NotAllowedBySecurity { what: "Topic A" };
        let s = format!("{e}");
        assert!(s.contains("not allowed by security"));
        assert!(s.contains("Topic A"));
        // Sollte NICHT konkrete Permissions-Internals erwaehnen.
        assert!(!s.to_lowercase().contains("permission file"));
        assert!(!s.to_lowercase().contains("ca cert"));
    }
}
