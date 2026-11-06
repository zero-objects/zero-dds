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
    /// `UNKNOWN` — undetermined (Reflection/Error paths,
    /// `rmw_qos_profile_unknown`).
    Unknown,
}

/// QoS-Durability — DDS-Mapping (`rmw_qos_durability_policy_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Durability {
    /// `SYSTEM_DEFAULT` — Sentinel: Use DDS default.
    SystemDefault,
    /// `VOLATILE` — late-joining subscribers don't get past samples.
    Volatile,
    /// `TRANSIENT_LOCAL` — last-N samples cached for late-joiners.
    TransientLocal,
    /// `UNKNOWN` — undetermined.
    Unknown,
}

/// QoS-History — DDS-Mapping (`rmw_qos_history_policy_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum History {
    /// `SYSTEM_DEFAULT` — Sentinel: Use DDS default history/depth.
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
    /// effectively INFINITE).
    pub liveliness_lease_secs: Option<u32>,
    /// Deadline — `None` = system-default/unspecified (DDS-Default, effectively
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

    fn concrete(reliability: Reliability, durability: Durability, depth: u32) -> QosProfile {
        QosProfile {
            reliability,
            durability,
            history: History::KeepLast(depth),
            liveliness_lease_secs: None,
            deadline_secs: None,
        }
    }

    /// REP-2003 §"Map Quality of Service": maps over a reliable
    /// transient-local topic, a single map deep.
    #[test]
    fn rep2003_map_qos_is_reliable_transient_local_keep_last_1() {
        assert_eq!(
            profiles::MAP,
            concrete(Reliability::Reliable, Durability::TransientLocal, 1)
        );
    }

    /// REP-2003 §"Sensor Driver Quality of Service": consumers of sensor
    /// data use `SensorDataQoS`, which is `rmw_qos_profile_sensor_data`
    /// of rmw/qos_profiles.h: BEST_EFFORT, VOLATILE, KEEP_LAST 5.
    #[test]
    fn rep2003_sensor_data_qos_is_best_effort_volatile_keep_last_5() {
        assert_eq!(
            profiles::SENSOR_DATA,
            concrete(Reliability::BestEffort, Durability::Volatile, 5)
        );
    }

    /// rmw/qos_profiles.h: `rmw_qos_profile_default` and
    /// `_services_default` RELIABLE, VOLATILE, KEEP_LAST 10;
    /// `_parameters` and `_parameter_events` RELIABLE, VOLATILE, KEEP_LAST
    /// 1000; deadline, lifespan and liveliness lease of all four
    /// RMW_QOS_*_DEFAULT, which leaves them infinite.
    #[test]
    fn rmw_qos_profiles_h_the_concrete_profiles_have_the_header_values() {
        let reliable = |depth| concrete(Reliability::Reliable, Durability::Volatile, depth);
        assert_eq!(profiles::DEFAULT, reliable(10));
        assert_eq!(profiles::SERVICES_DEFAULT, reliable(10));
        assert_eq!(profiles::PARAMETERS, reliable(1000));
        assert_eq!(profiles::PARAMETER_EVENTS, reliable(1000));
    }

    /// rmw/qos_profiles.h: `rmw_qos_profile_system_default` has every
    /// policy at its SYSTEM_DEFAULT value and `rmw_qos_profile_unknown`
    /// every policy at UNKNOWN; neither is the other or the default.
    #[test]
    fn rmw_qos_profiles_h_system_default_and_unknown_are_all_sentinels() {
        let p = profiles::SYSTEM_DEFAULT;
        assert_eq!(p.reliability, Reliability::SystemDefault);
        assert_eq!(p.durability, Durability::SystemDefault);
        assert_eq!(p.history, History::SystemDefault);
        assert!(profiles::is_system_default(&p));
        assert!(!profiles::is_unknown(&p));

        let p = profiles::UNKNOWN;
        assert_eq!(p.reliability, Reliability::Unknown);
        assert_eq!(p.durability, Durability::Unknown);
        assert_eq!(p.history, History::Unknown);
        assert!(profiles::is_unknown(&p));
        assert!(!profiles::is_system_default(&p));

        for real in [profiles::DEFAULT, profiles::SENSOR_DATA, profiles::MAP] {
            assert!(!profiles::is_unknown(&real), "{real:?}");
            assert!(!profiles::is_system_default(&real), "{real:?}");
        }
    }
}
