// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 Standard QoS Profiles — REP-2003 + RMW Defaults.

/// QoS-Reliability — DDS-Mapping (`rmw_qos_reliability_policy_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reliability {
    /// `SYSTEM_DEFAULT` — sentinel: do not set, use the DDS implementation
    /// default (`rmw_qos_profile_system_default`).
    SystemDefault,
    /// `RELIABLE` — packets retransmitted on loss (DDS Reliable).
    Reliable,
    /// `BEST_EFFORT` — no retransmission (DDS BestEffort).
    BestEffort,
    /// `UNKNOWN` — unbestimmt (Reflection-/Fehlerpfade,
    /// `rmw_qos_profile_unknown`).
    Unknown,
}

/// QoS-Durability — DDS-Mapping (`rmw_qos_durability_policy_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Durability {
    /// `SYSTEM_DEFAULT` — Sentinel: DDS-Default verwenden.
    SystemDefault,
    /// `VOLATILE` — late-joining subscribers don't get past samples.
    Volatile,
    /// `TRANSIENT_LOCAL` — last-N samples cached for late-joiners.
    TransientLocal,
    /// `UNKNOWN` — unbestimmt.
    Unknown,
}

/// QoS-History — DDS-Mapping (`rmw_qos_history_policy_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum History {
    /// `SYSTEM_DEFAULT` — Sentinel: DDS-Default-History/-Depth verwenden.
    SystemDefault,
    /// `KEEP_LAST(n)`.
    KeepLast(u32),
    /// `KEEP_ALL`.
    KeepAll,
    /// `UNKNOWN` — unbestimmt.
    Unknown,
}

/// ROS 2 QoS Profile (Subset). Spec REP-2003 + `rmw/qos_profiles.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QosProfile {
    /// Reliability.
    pub reliability: Reliability,
    /// Durability.
    pub durability: Durability,
    /// History.
    pub history: History,
    /// Liveliness lease — `None` = system-default/unspecified (DDS-Default,
    /// effektiv INFINITE).
    pub liveliness_lease_secs: Option<u32>,
    /// Deadline — `None` = system-default/unspecified (DDS-Default, effektiv
    /// INFINITE).
    pub deadline_secs: Option<u32>,
}

/// Standard profiles from `rmw/qos_profiles.h` and REP-2003.
pub mod profiles {
    use super::*;

    /// `rmw_qos_profile_default` — DEFAULT profile for normal ROS-2
    /// topics: Reliable + Volatile + KeepLast(10).
    pub const DEFAULT: QosProfile = QosProfile {
        reliability: Reliability::Reliable,
        durability: Durability::Volatile,
        history: History::KeepLast(10),
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `rmw_qos_profile_sensor_data` — REP-2003 §"Sensor Driver QoS":
    /// BestEffort + Volatile + KeepLast(5).
    pub const SENSOR_DATA: QosProfile = QosProfile {
        reliability: Reliability::BestEffort,
        durability: Durability::Volatile,
        history: History::KeepLast(5),
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// REP-2003 §"Map QoS": Reliable + TransientLocal + KeepLast(1).
    pub const MAP: QosProfile = QosProfile {
        reliability: Reliability::Reliable,
        durability: Durability::TransientLocal,
        history: History::KeepLast(1),
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `rmw_qos_profile_parameters` — Parameters: Reliable + Volatile +
    /// KeepLast(1000).
    pub const PARAMETERS: QosProfile = QosProfile {
        reliability: Reliability::Reliable,
        durability: Durability::Volatile,
        history: History::KeepLast(1000),
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `rmw_qos_profile_services_default` — Services: Reliable +
    /// Volatile + KeepLast(10).
    pub const SERVICES_DEFAULT: QosProfile = QosProfile {
        reliability: Reliability::Reliable,
        durability: Durability::Volatile,
        history: History::KeepLast(10),
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `rmw_qos_profile_parameter_events` — Parameter-Events: Reliable +
    /// Volatile + KeepLast(1000).
    pub const PARAMETER_EVENTS: QosProfile = QosProfile {
        reliability: Reliability::Reliable,
        durability: Durability::Volatile,
        history: History::KeepLast(1000),
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `rmw_qos_profile_system_default` — **every field** as a
    /// `*_SYSTEM_DEFAULT` sentinel (Spec: implementation-defined; each
    /// field's DDS default is used, not the rmw DEFAULT values).
    /// The resolution happens only at DDS-entity creation — a
    /// `SystemDefault` field is *not set* there.
    pub const SYSTEM_DEFAULT: QosProfile = QosProfile {
        reliability: Reliability::SystemDefault,
        durability: Durability::SystemDefault,
        history: History::SystemDefault,
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `rmw_qos_profile_unknown` — **every field** as an `*_UNKNOWN` sentinel.
    /// The RMW API uses this profile in reflection/error paths
    /// (e.g. when `rmw_get_endpoint_info` returns no valid value).
    pub const UNKNOWN: QosProfile = QosProfile {
        reliability: Reliability::Unknown,
        durability: Durability::Unknown,
        history: History::Unknown,
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `is_unknown(profile)` — detects the `rmw_qos_profile_unknown`
    /// (all policy fields as the `Unknown` sentinel).
    #[must_use]
    pub fn is_unknown(p: &QosProfile) -> bool {
        matches!(p.reliability, Reliability::Unknown)
            && matches!(p.durability, Durability::Unknown)
            && matches!(p.history, History::Unknown)
    }

    /// `is_system_default(profile)` — detects the
    /// `rmw_qos_profile_system_default` (all policy fields as the
    /// `SystemDefault` sentinel).
    #[must_use]
    pub fn is_system_default(p: &QosProfile) -> bool {
        matches!(p.reliability, Reliability::SystemDefault)
            && matches!(p.durability, Durability::SystemDefault)
            && matches!(p.history, History::SystemDefault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_reliable_volatile_keep_last_10() {
        // RMW Default.
        let p = profiles::DEFAULT;
        assert_eq!(p.reliability, Reliability::Reliable);
        assert_eq!(p.durability, Durability::Volatile);
        assert_eq!(p.history, History::KeepLast(10));
    }

    #[test]
    fn sensor_data_profile_matches_rep_2003_specification() {
        // REP-2003 — BestEffort + Volatile + KeepLast(5).
        let p = profiles::SENSOR_DATA;
        assert_eq!(p.reliability, Reliability::BestEffort);
        assert_eq!(p.durability, Durability::Volatile);
        assert_eq!(p.history, History::KeepLast(5));
    }

    #[test]
    fn map_profile_matches_rep_2003_specification() {
        // REP-2003 — Reliable + TransientLocal + KeepLast(1).
        let p = profiles::MAP;
        assert_eq!(p.reliability, Reliability::Reliable);
        assert_eq!(p.durability, Durability::TransientLocal);
        assert_eq!(p.history, History::KeepLast(1));
    }

    #[test]
    fn parameters_profile_uses_keep_last_1000() {
        let p = profiles::PARAMETERS;
        assert_eq!(p.history, History::KeepLast(1000));
        assert_eq!(p.reliability, Reliability::Reliable);
    }

    #[test]
    fn services_default_matches_rmw_defaults() {
        let p = profiles::SERVICES_DEFAULT;
        assert_eq!(p.reliability, Reliability::Reliable);
        assert_eq!(p.history, History::KeepLast(10));
    }

    #[test]
    fn parameter_events_uses_keep_last_1000() {
        let p = profiles::PARAMETER_EVENTS;
        assert_eq!(p.history, History::KeepLast(1000));
    }

    #[test]
    fn system_default_is_all_sentinels() {
        // Spec-faithful: every field is the *_SYSTEM_DEFAULT sentinel, NOT
        // an alias to the rmw DEFAULT values.
        let p = profiles::SYSTEM_DEFAULT;
        assert_eq!(p.reliability, Reliability::SystemDefault);
        assert_eq!(p.durability, Durability::SystemDefault);
        assert_eq!(p.history, History::SystemDefault);
        assert!(profiles::is_system_default(&p));
        assert_ne!(p, profiles::DEFAULT);
    }

    #[test]
    fn history_keep_last_distinct_from_keep_all() {
        assert_ne!(History::KeepLast(10), History::KeepAll);
        assert_ne!(History::KeepLast(1), History::KeepLast(2));
    }

    #[test]
    fn liveliness_and_deadline_default_to_infinite() {
        assert_eq!(profiles::DEFAULT.liveliness_lease_secs, None);
        assert_eq!(profiles::DEFAULT.deadline_secs, None);
    }

    #[test]
    fn unknown_profile_is_distinct_from_default() {
        assert_ne!(profiles::UNKNOWN, profiles::DEFAULT);
    }

    #[test]
    fn unknown_profile_is_all_unknown() {
        let p = profiles::UNKNOWN;
        assert_eq!(p.reliability, Reliability::Unknown);
        assert_eq!(p.durability, Durability::Unknown);
        assert_eq!(p.history, History::Unknown);
    }

    #[test]
    fn is_unknown_recognizes_unknown_profile() {
        assert!(profiles::is_unknown(&profiles::UNKNOWN));
    }

    #[test]
    fn is_unknown_rejects_real_and_system_default_profiles() {
        assert!(!profiles::is_unknown(&profiles::DEFAULT));
        assert!(!profiles::is_unknown(&profiles::SENSOR_DATA));
        assert!(!profiles::is_unknown(&profiles::SYSTEM_DEFAULT));
    }

    #[test]
    fn system_default_and_unknown_are_distinct() {
        assert_ne!(profiles::SYSTEM_DEFAULT, profiles::UNKNOWN);
        assert!(!profiles::is_system_default(&profiles::UNKNOWN));
        assert!(!profiles::is_unknown(&profiles::SYSTEM_DEFAULT));
    }
}
