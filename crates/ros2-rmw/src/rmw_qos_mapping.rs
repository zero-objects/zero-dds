// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REP-2009 RMW-QoS-Mapping — `rmw_qos_profile_t` (C struct) → DDS-QoS.
//!
//! Spec REP-2009 §3 + `rmw/qos_profiles.h` (rmw 4.x). One C enum per
//! QoS dimension, which we map to the DDS counterpart.

use crate::qos_profiles::{Durability, History, QosProfile, Reliability};

/// `rmw_qos_history_policy_t` — REP-2009 §3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RmwHistory {
    /// `SYSTEM_DEFAULT` — RMW-Implementation entscheidet.
    SystemDefault = 0,
    /// `KEEP_LAST(depth)`.
    KeepLast = 1,
    /// `KEEP_ALL`.
    KeepAll = 2,
    /// `UNKNOWN`.
    Unknown = 3,
}

/// `rmw_qos_reliability_policy_t` — REP-2009 §3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RmwReliability {
    /// `SYSTEM_DEFAULT`.
    SystemDefault = 0,
    /// `RELIABLE`.
    Reliable = 1,
    /// `BEST_EFFORT`.
    BestEffort = 2,
    /// `UNKNOWN`.
    Unknown = 3,
    /// `BEST_AVAILABLE` — best-available from REP-2009 §3.2.
    BestAvailable = 4,
}

/// `rmw_qos_durability_policy_t` — REP-2009 §3.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RmwDurability {
    /// `SYSTEM_DEFAULT`.
    SystemDefault = 0,
    /// `TRANSIENT_LOCAL`.
    TransientLocal = 1,
    /// `VOLATILE`.
    Volatile = 2,
    /// `UNKNOWN`.
    Unknown = 3,
    /// `BEST_AVAILABLE`.
    BestAvailable = 4,
}

/// `rmw_qos_profile_t` — REP-2009 §3 (C-layout-conformant).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct RmwQosProfile {
    /// History policy.
    pub history: RmwHistory,
    /// History depth (only relevant for `KeepLast`).
    pub depth: usize,
    /// Reliability policy.
    pub reliability: RmwReliability,
    /// Durability policy.
    pub durability: RmwDurability,
    /// `avoid_ros_namespace_conventions` — if `true`, the
    /// `rt/` prefix is omitted in topic mangling (Spec REP-2009 §3.7).
    pub avoid_ros_namespace_conventions: bool,
}

impl RmwQosProfile {
    /// Default profile from REP-2009 §4 (`rmw_qos_profile_default`):
    /// `KEEP_LAST(10) + RELIABLE + VOLATILE`.
    #[must_use]
    pub const fn default_profile() -> Self {
        Self {
            history: RmwHistory::KeepLast,
            depth: 10,
            reliability: RmwReliability::Reliable,
            durability: RmwDurability::Volatile,
            avoid_ros_namespace_conventions: false,
        }
    }

    /// `rmw_qos_profile_sensor_data` — Spec REP-2003 §2 + REP-2009 §4.
    /// `KEEP_LAST(5) + BEST_EFFORT + VOLATILE`.
    #[must_use]
    pub const fn sensor_data() -> Self {
        Self {
            history: RmwHistory::KeepLast,
            depth: 5,
            reliability: RmwReliability::BestEffort,
            durability: RmwDurability::Volatile,
            avoid_ros_namespace_conventions: false,
        }
    }

    /// `rmw_qos_profile_parameters` — Spec REP-2009 §4.
    /// `KEEP_LAST(1000) + RELIABLE + VOLATILE`.
    #[must_use]
    pub const fn parameters() -> Self {
        Self {
            history: RmwHistory::KeepLast,
            depth: 1000,
            reliability: RmwReliability::Reliable,
            durability: RmwDurability::Volatile,
            avoid_ros_namespace_conventions: false,
        }
    }

    /// `rmw_qos_profile_services_default` — Spec REP-2009 §4.
    /// `KEEP_LAST(10) + RELIABLE + VOLATILE`.
    #[must_use]
    pub const fn services_default() -> Self {
        Self {
            history: RmwHistory::KeepLast,
            depth: 10,
            reliability: RmwReliability::Reliable,
            durability: RmwDurability::Volatile,
            avoid_ros_namespace_conventions: false,
        }
    }
}

/// Mapping `RmwQosProfile` → DDS-QoS (`crate::qos_profiles::QosProfile`).
///
/// `SystemDefault` and `Unknown` are **passed through as sentinels** — the
/// resolution ("use the DDS default" or "unspecified") happens only at
/// DDS-entity creation, where each field's DDS default is known (a
/// `SystemDefault` field is simply not set there). Spec REP-2009 §3.7:
/// `BestAvailable` is match-time resolution; on the sender side we map to
/// `BestEffort`/`Volatile`.
#[must_use]
pub fn rmw_to_dds(profile: &RmwQosProfile) -> QosProfile {
    let history = match profile.history {
        RmwHistory::SystemDefault => History::SystemDefault,
        RmwHistory::KeepLast => History::KeepLast(profile.depth as u32),
        RmwHistory::KeepAll => History::KeepAll,
        RmwHistory::Unknown => History::Unknown,
    };
    let reliability = match profile.reliability {
        RmwReliability::SystemDefault => Reliability::SystemDefault,
        RmwReliability::Reliable => Reliability::Reliable,
        RmwReliability::BestEffort | RmwReliability::BestAvailable => Reliability::BestEffort,
        RmwReliability::Unknown => Reliability::Unknown,
    };
    let durability = match profile.durability {
        RmwDurability::SystemDefault => Durability::SystemDefault,
        RmwDurability::TransientLocal => Durability::TransientLocal,
        RmwDurability::Volatile | RmwDurability::BestAvailable => Durability::Volatile,
        RmwDurability::Unknown => Durability::Unknown,
    };
    QosProfile {
        history,
        reliability,
        durability,
        liveliness_lease_secs: None,
        deadline_secs: None,
    }
}

/// Reverse mapping DDS → RMW. Handy for ROS-2 reflection APIs
/// (e.g. `rmw_get_endpoint_info` calls).
#[must_use]
pub fn dds_to_rmw(profile: &QosProfile) -> RmwQosProfile {
    let (history, depth) = match profile.history {
        History::SystemDefault => (RmwHistory::SystemDefault, 0),
        History::KeepAll => (RmwHistory::KeepAll, 0),
        History::KeepLast(d) => (RmwHistory::KeepLast, d as usize),
        History::Unknown => (RmwHistory::Unknown, 0),
    };
    let reliability = match profile.reliability {
        Reliability::SystemDefault => RmwReliability::SystemDefault,
        Reliability::Reliable => RmwReliability::Reliable,
        Reliability::BestEffort => RmwReliability::BestEffort,
        Reliability::Unknown => RmwReliability::Unknown,
    };
    let durability = match profile.durability {
        Durability::SystemDefault => RmwDurability::SystemDefault,
        Durability::TransientLocal => RmwDurability::TransientLocal,
        Durability::Volatile => RmwDurability::Volatile,
        Durability::Unknown => RmwDurability::Unknown,
    };
    RmwQosProfile {
        history,
        depth,
        reliability,
        durability,
        avoid_ros_namespace_conventions: false,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_matches_rmw_spec() {
        // REP-2009 §4 default: KEEP_LAST(10) + RELIABLE + VOLATILE.
        let p = RmwQosProfile::default_profile();
        assert_eq!(p.history, RmwHistory::KeepLast);
        assert_eq!(p.depth, 10);
        assert_eq!(p.reliability, RmwReliability::Reliable);
        assert_eq!(p.durability, RmwDurability::Volatile);
    }

    #[test]
    fn sensor_data_is_best_effort() {
        let p = RmwQosProfile::sensor_data();
        assert_eq!(p.reliability, RmwReliability::BestEffort);
        assert_eq!(p.depth, 5);
    }

    #[test]
    fn parameters_uses_keep_last_1000() {
        let p = RmwQosProfile::parameters();
        assert_eq!(p.depth, 1000);
        assert_eq!(p.reliability, RmwReliability::Reliable);
    }

    #[test]
    fn rmw_to_dds_round_trip_default() {
        let rmw = RmwQosProfile::default_profile();
        let dds = rmw_to_dds(&rmw);
        let rmw2 = dds_to_rmw(&dds);
        assert_eq!(rmw.reliability, rmw2.reliability);
        assert_eq!(rmw.durability, rmw2.durability);
        assert_eq!(rmw.depth, rmw2.depth);
    }

    #[test]
    fn rmw_to_dds_keep_all_passes_through() {
        let rmw = RmwQosProfile {
            history: RmwHistory::KeepAll,
            depth: 0,
            reliability: RmwReliability::Reliable,
            durability: RmwDurability::Volatile,
            avoid_ros_namespace_conventions: false,
        };
        let dds = rmw_to_dds(&rmw);
        assert!(matches!(dds.history, History::KeepAll));
    }

    #[test]
    fn rmw_best_available_maps_to_best_effort_on_sender() {
        let rmw = RmwQosProfile {
            history: RmwHistory::KeepLast,
            depth: 5,
            reliability: RmwReliability::BestAvailable,
            durability: RmwDurability::Volatile,
            avoid_ros_namespace_conventions: false,
        };
        let dds = rmw_to_dds(&rmw);
        assert_eq!(dds.reliability, Reliability::BestEffort);
    }

    #[test]
    fn rmw_system_default_passes_through_as_sentinel() {
        // Spec-faithful: SYSTEM_DEFAULT is NOT resolved hard to Reliable/Volatile
        // but passed through as a sentinel (resolution only at
        // DDS-entity creation).
        let rmw = RmwQosProfile {
            history: RmwHistory::SystemDefault,
            depth: 0,
            reliability: RmwReliability::SystemDefault,
            durability: RmwDurability::SystemDefault,
            avoid_ros_namespace_conventions: false,
        };
        let dds = rmw_to_dds(&rmw);
        assert_eq!(dds.reliability, Reliability::SystemDefault);
        assert_eq!(dds.durability, Durability::SystemDefault);
        assert_eq!(dds.history, History::SystemDefault);
        // Round-trip preserves the sentinel.
        let back = dds_to_rmw(&dds);
        assert_eq!(back.reliability, RmwReliability::SystemDefault);
        assert_eq!(back.durability, RmwDurability::SystemDefault);
        assert_eq!(back.history, RmwHistory::SystemDefault);
    }

    #[test]
    fn rmw_unknown_passes_through_as_sentinel() {
        let rmw = RmwQosProfile {
            history: RmwHistory::Unknown,
            depth: 0,
            reliability: RmwReliability::Unknown,
            durability: RmwDurability::Unknown,
            avoid_ros_namespace_conventions: false,
        };
        let dds = rmw_to_dds(&rmw);
        assert!(crate::qos_profiles::profiles::is_unknown(&dds));
    }

    #[test]
    fn transient_local_round_trips() {
        let rmw = RmwQosProfile {
            history: RmwHistory::KeepLast,
            depth: 1,
            reliability: RmwReliability::Reliable,
            durability: RmwDurability::TransientLocal,
            avoid_ros_namespace_conventions: false,
        };
        let dds = rmw_to_dds(&rmw);
        assert_eq!(dds.durability, Durability::TransientLocal);
    }

    #[test]
    fn services_default_uses_keep_last_10() {
        let p = RmwQosProfile::services_default();
        assert_eq!(p.depth, 10);
        assert_eq!(p.reliability, RmwReliability::Reliable);
    }

    #[test]
    fn enum_repr_is_c_compatible() {
        // Spec REP-2009 §3.x: enum values must match rmw C-API.
        assert_eq!(RmwHistory::SystemDefault as u32, 0);
        assert_eq!(RmwHistory::KeepLast as u32, 1);
        assert_eq!(RmwHistory::KeepAll as u32, 2);
        assert_eq!(RmwReliability::Reliable as u32, 1);
        assert_eq!(RmwReliability::BestEffort as u32, 2);
    }
}
