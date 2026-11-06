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
    /// `SYSTEM_DEFAULT` — RMW implementation decides.
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

    fn profile(
        history: RmwHistory,
        depth: usize,
        reliability: RmwReliability,
        durability: RmwDurability,
    ) -> RmwQosProfile {
        RmwQosProfile {
            history,
            depth,
            reliability,
            durability,
            avoid_ros_namespace_conventions: false,
        }
    }

    /// rmw/types.h: `rmw_qos_history_policy_e` SYSTEM_DEFAULT 0,
    /// KEEP_LAST 1, KEEP_ALL 2, UNKNOWN 3; `rmw_qos_reliability_policy_e`
    /// SYSTEM_DEFAULT 0, RELIABLE 1, BEST_EFFORT 2, UNKNOWN 3,
    /// BEST_AVAILABLE 4; `rmw_qos_durability_policy_e` SYSTEM_DEFAULT 0,
    /// TRANSIENT_LOCAL 1, VOLATILE 2, UNKNOWN 3, BEST_AVAILABLE 4.
    #[test]
    fn rmw_types_h_the_policy_enums_have_the_header_values() {
        assert_eq!(RmwHistory::SystemDefault as u32, 0);
        assert_eq!(RmwHistory::KeepLast as u32, 1);
        assert_eq!(RmwHistory::KeepAll as u32, 2);
        assert_eq!(RmwHistory::Unknown as u32, 3);
        assert_eq!(RmwReliability::SystemDefault as u32, 0);
        assert_eq!(RmwReliability::Reliable as u32, 1);
        assert_eq!(RmwReliability::BestEffort as u32, 2);
        assert_eq!(RmwReliability::Unknown as u32, 3);
        assert_eq!(RmwReliability::BestAvailable as u32, 4);
        assert_eq!(RmwDurability::SystemDefault as u32, 0);
        assert_eq!(RmwDurability::TransientLocal as u32, 1);
        assert_eq!(RmwDurability::Volatile as u32, 2);
        assert_eq!(RmwDurability::Unknown as u32, 3);
        assert_eq!(RmwDurability::BestAvailable as u32, 4);
    }

    /// rmw/qos_profiles.h: `rmw_qos_profile_default` and
    /// `rmw_qos_profile_services_default` KEEP_LAST 10, RELIABLE,
    /// VOLATILE; `rmw_qos_profile_sensor_data` KEEP_LAST 5, BEST_EFFORT,
    /// VOLATILE; `rmw_qos_profile_parameters` KEEP_LAST 1000, RELIABLE,
    /// VOLATILE.
    #[test]
    fn rmw_qos_profiles_h_the_rmw_side_profiles_have_the_header_values() {
        use RmwDurability::Volatile;
        use RmwHistory::KeepLast;
        use RmwReliability::{BestEffort, Reliable};
        assert_eq!(
            RmwQosProfile::default_profile(),
            profile(KeepLast, 10, Reliable, Volatile)
        );
        assert_eq!(
            RmwQosProfile::services_default(),
            profile(KeepLast, 10, Reliable, Volatile)
        );
        assert_eq!(
            RmwQosProfile::sensor_data(),
            profile(KeepLast, 5, BestEffort, Volatile)
        );
        assert_eq!(
            RmwQosProfile::parameters(),
            profile(KeepLast, 1000, Reliable, Volatile)
        );
    }

    /// rmw/types.h: each concrete policy value is the DDS policy of the
    /// same name, KEEP_LAST with its depth, in both directions.
    #[test]
    fn rmw_types_h_a_concrete_policy_maps_to_the_dds_policy_of_its_name() {
        use RmwDurability::{TransientLocal, Volatile};
        use RmwHistory::{KeepAll, KeepLast};
        use RmwReliability::{BestEffort, Reliable};
        for (rmw, history, reliability, durability) in [
            (
                profile(KeepLast, 10, Reliable, Volatile),
                History::KeepLast(10),
                Reliability::Reliable,
                Durability::Volatile,
            ),
            (
                profile(KeepLast, 1, Reliable, TransientLocal),
                History::KeepLast(1),
                Reliability::Reliable,
                Durability::TransientLocal,
            ),
            (
                profile(KeepAll, 0, BestEffort, Volatile),
                History::KeepAll,
                Reliability::BestEffort,
                Durability::Volatile,
            ),
        ] {
            let dds = rmw_to_dds(&rmw);
            assert_eq!(dds.history, history, "{rmw:?}");
            assert_eq!(dds.reliability, reliability, "{rmw:?}");
            assert_eq!(dds.durability, durability, "{rmw:?}");
            assert_eq!(dds_to_rmw(&dds), rmw);
        }
    }

    /// rmw/types.h: BEST_AVAILABLE (since Iron) takes the policy that
    /// matches every remote endpoint; on the sending side that is
    /// BEST_EFFORT and VOLATILE.
    #[test]
    fn rmw_types_h_best_available_is_best_effort_and_volatile_on_the_sending_side() {
        let dds = rmw_to_dds(&profile(
            RmwHistory::KeepLast,
            5,
            RmwReliability::BestAvailable,
            RmwDurability::BestAvailable,
        ));
        assert_eq!(dds.reliability, Reliability::BestEffort);
        assert_eq!(dds.durability, Durability::Volatile);
    }

    /// rmw/types.h: SYSTEM_DEFAULT and UNKNOWN are values of their own
    /// and pass the mapping as such in both directions; the middleware
    /// resolves SYSTEM_DEFAULT when it creates the entity.
    #[test]
    fn rmw_types_h_system_default_and_unknown_pass_through_as_sentinels() {
        let rmw = profile(
            RmwHistory::SystemDefault,
            0,
            RmwReliability::SystemDefault,
            RmwDurability::SystemDefault,
        );
        let dds = rmw_to_dds(&rmw);
        assert_eq!(dds.reliability, Reliability::SystemDefault);
        assert_eq!(dds.durability, Durability::SystemDefault);
        assert_eq!(dds.history, History::SystemDefault);
        assert_eq!(dds_to_rmw(&dds), rmw);

        let rmw = profile(
            RmwHistory::Unknown,
            0,
            RmwReliability::Unknown,
            RmwDurability::Unknown,
        );
        let dds = rmw_to_dds(&rmw);
        assert!(crate::qos_profiles::profiles::is_unknown(&dds));
        assert_eq!(dds_to_rmw(&dds), rmw);
    }
}
