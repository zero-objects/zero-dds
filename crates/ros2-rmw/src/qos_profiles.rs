// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! ROS 2 Standard QoS Profiles — REP-2003 + RMW Defaults.

/// QoS-Reliability — DDS-Mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reliability {
    /// `RELIABLE` — packets retransmitted on loss (DDS Reliable).
    Reliable,
    /// `BEST_EFFORT` — no retransmission (DDS BestEffort).
    BestEffort,
}

/// QoS-Durability — DDS-Mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Durability {
    /// `VOLATILE` — late-joining subscribers don't get past samples.
    Volatile,
    /// `TRANSIENT_LOCAL` — last-N samples cached for late-joiners.
    TransientLocal,
}

/// QoS-History — DDS-Mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum History {
    /// `KEEP_LAST(n)`.
    KeepLast(u32),
    /// `KEEP_ALL`.
    KeepAll,
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
    /// Liveliness lease — `None` = INFINITE.
    pub liveliness_lease_secs: Option<u32>,
    /// Deadline — `None` = INFINITE.
    pub deadline_secs: Option<u32>,
}

/// Standard-Profile aus `rmw/qos_profiles.h` und REP-2003.
pub mod profiles {
    use super::*;

    /// `rmw_qos_profile_default` — DEFAULT-Profile fuer normale ROS-2-
    /// Topics: Reliable + Volatile + KeepLast(10).
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

    /// `rmw_qos_profile_system_default` — Implementation-defined; wir
    /// liefern dasselbe wie DEFAULT (siehe Spec-Coverage-Doc fuer
    /// Sentinel-vs-Alias-Diskussion).
    pub const SYSTEM_DEFAULT: QosProfile = DEFAULT;

    /// `rmw_qos_profile_unknown` — Sentinel fuer ungesetzte/
    /// fehlerhafte Werte. Die RMW-API verwendet dieses Profile in
    /// Fehlerpfaden (z.B. wenn `rmw_get_endpoint_info` keinen
    /// gueltigen Wert liefert).
    ///
    /// Wir modellieren das als marker-Konstante mit
    /// `KeepLast(0)`-History, was in der Spec-Bedeutung "kein
    /// gueltiger Wert" entspricht.
    pub const UNKNOWN: QosProfile = QosProfile {
        reliability: Reliability::BestEffort,
        durability: Durability::Volatile,
        history: History::KeepLast(0),
        liveliness_lease_secs: None,
        deadline_secs: None,
    };

    /// `is_unknown(profile)` — Predicate fuer den Spec-konformen
    /// "ungesetzt"-Marker.
    #[must_use]
    pub fn is_unknown(p: &QosProfile) -> bool {
        matches!(p.history, History::KeepLast(0))
            && p.reliability == Reliability::BestEffort
            && p.liveliness_lease_secs.is_none()
            && p.deadline_secs.is_none()
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
    fn system_default_aliases_default() {
        assert_eq!(profiles::SYSTEM_DEFAULT, profiles::DEFAULT);
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
    fn unknown_profile_uses_keep_last_zero_marker() {
        assert_eq!(profiles::UNKNOWN.history, History::KeepLast(0));
    }

    #[test]
    fn is_unknown_recognizes_unknown_profile() {
        assert!(profiles::is_unknown(&profiles::UNKNOWN));
    }

    #[test]
    fn is_unknown_rejects_real_profiles() {
        assert!(!profiles::is_unknown(&profiles::DEFAULT));
        assert!(!profiles::is_unknown(&profiles::SENSOR_DATA));
    }
}
