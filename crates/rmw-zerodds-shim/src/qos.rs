// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The QoS of a ROS 2 publisher or subscription, from `rmw_qos_profile_t` to
//! the DDS endpoint (REP-2009, `rmw/types.h`).
//!
//! The C layer (`rmw_c/rmw_zerodds.c`) copies each `rmw_qos_profile_t` into
//! the flat [`RmwZerodsQos`]: the rmw enum values unchanged, `rmw_time_t`
//! as nanoseconds with `0` for both `RMW_DURATION_UNSPECIFIED` and
//! `RMW_DURATION_INFINITE`. [`resolve`] turns it into what the endpoint is
//! created with, and the same resolved values go back to the C layer as the
//! endpoint's actual QoS.

/// `rmw_qos_profile_t`, flattened for the C boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RmwZerodsQos {
    /// `rmw_qos_history_policy_e`: 0 SYSTEM_DEFAULT, 1 KEEP_LAST, 2 KEEP_ALL.
    pub history: u32,
    /// `depth`; `0` under KEEP_LAST is `RMW_QOS_POLICY_DEPTH_SYSTEM_DEFAULT`.
    pub depth: u64,
    /// `rmw_qos_reliability_policy_e`: 0 SYSTEM_DEFAULT, 1 RELIABLE,
    /// 2 BEST_EFFORT, 4 BEST_AVAILABLE.
    pub reliability: u32,
    /// `rmw_qos_durability_policy_e`: 0 SYSTEM_DEFAULT, 1 TRANSIENT_LOCAL,
    /// 2 VOLATILE, 4 BEST_AVAILABLE.
    pub durability: u32,
    /// `deadline`, nanoseconds, `0` = none.
    pub deadline_ns: u64,
    /// `lifespan`, nanoseconds, `0` = none.
    pub lifespan_ns: u64,
    /// `rmw_qos_liveliness_policy_e`: 0 SYSTEM_DEFAULT, 1 AUTOMATIC,
    /// 2 MANUAL_BY_NODE (deprecated), 3 MANUAL_BY_TOPIC, 5 BEST_AVAILABLE.
    pub liveliness: u32,
    /// `liveliness_lease_duration`, nanoseconds, `0` = none.
    pub liveliness_lease_ns: u64,
    /// `avoid_ros_namespace_conventions`: `1` = the DDS topic is the ROS
    /// name without the `rt/` prefix.
    pub avoid_ros_namespace_conventions: u32,
}

/// The rmw enum values this module reads.
mod rmw {
    pub const SYSTEM_DEFAULT: u32 = 0;
    pub const KEEP_LAST: u32 = 1;
    pub const KEEP_ALL: u32 = 2;
    pub const RELIABLE: u32 = 1;
    pub const BEST_EFFORT: u32 = 2;
    pub const TRANSIENT_LOCAL: u32 = 1;
    pub const VOLATILE: u32 = 2;
    pub const BEST_AVAILABLE: u32 = 4;
    pub const AUTOMATIC: u32 = 1;
    pub const MANUAL_BY_NODE: u32 = 2;
    pub const MANUAL_BY_TOPIC: u32 = 3;
    pub const LIVELINESS_BEST_AVAILABLE: u32 = 5;
}

/// Which side an endpoint is on: `BEST_AVAILABLE` resolves differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// A publisher offers.
    Publisher,
    /// A subscription requests.
    Subscription,
}

/// The QoS an endpoint is created with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    /// RELIABLE, else BEST_EFFORT.
    pub reliable: bool,
    /// TRANSIENT_LOCAL, else VOLATILE.
    pub transient_local: bool,
    /// KEEP_LAST depth; `None` = KEEP_ALL.
    pub keep_last: Option<u32>,
    /// DEADLINE, ns, `0` = none.
    pub deadline_ns: u64,
    /// LIFESPAN, ns, `0` = none.
    pub lifespan_ns: u64,
    /// `0` AUTOMATIC, `1` MANUAL_BY_PARTICIPANT, `2` MANUAL_BY_TOPIC.
    pub liveliness_kind: i32,
    /// LIVELINESS lease, ns, `0` = none.
    pub liveliness_lease_ns: u64,
    /// The DDS topic carries no `rt/` prefix.
    pub avoid_ros_namespace_conventions: bool,
}

/// Resolves a requested profile.
///
/// `SYSTEM_DEFAULT` takes the DDS 1.4 default for the endpoint (KEEP_LAST(1),
/// VOLATILE, AUTOMATIC, infinite times), except reliability, which stays
/// RELIABLE on both sides as this RMW always did. `BEST_AVAILABLE` offers
/// the strongest value on a publisher (RELIABLE, TRANSIENT_LOCAL, AUTOMATIC)
/// and requests the weakest on a subscription (BEST_EFFORT, VOLATILE,
/// AUTOMATIC), which matches every peer (REP-2009 §3.7).
///
/// # Errors
/// The field and value of anything outside the rmw enums, `UNKNOWN`
/// included: no rclcpp profile carries it, and creating an endpoint with a
/// guessed value would be a QoS nobody asked for.
pub fn resolve(q: &RmwZerodsQos, side: Side) -> Result<Resolved, String> {
    let bad = |field: &str, v: u32| Err(format!("{field} = {v} is not an rmw value"));
    let keep_last = match q.history {
        rmw::SYSTEM_DEFAULT => Some(1),
        rmw::KEEP_LAST => Some(u32::try_from(q.depth).unwrap_or(u32::MAX).max(1)),
        rmw::KEEP_ALL => None,
        v => return bad("history", v),
    };
    let publisher = side == Side::Publisher;
    let reliable = match q.reliability {
        rmw::SYSTEM_DEFAULT | rmw::RELIABLE => true,
        rmw::BEST_EFFORT => false,
        rmw::BEST_AVAILABLE => publisher,
        v => return bad("reliability", v),
    };
    let transient_local = match q.durability {
        rmw::SYSTEM_DEFAULT | rmw::VOLATILE => false,
        rmw::TRANSIENT_LOCAL => true,
        rmw::BEST_AVAILABLE => publisher,
        v => return bad("durability", v),
    };
    let liveliness_kind = match q.liveliness {
        rmw::SYSTEM_DEFAULT | rmw::AUTOMATIC | rmw::LIVELINESS_BEST_AVAILABLE => 0,
        rmw::MANUAL_BY_NODE => 1,
        rmw::MANUAL_BY_TOPIC => 2,
        v => return bad("liveliness", v),
    };
    Ok(Resolved {
        reliable,
        transient_local,
        keep_last,
        deadline_ns: q.deadline_ns,
        lifespan_ns: if publisher { q.lifespan_ns } else { 0 },
        liveliness_kind,
        liveliness_lease_ns: q.liveliness_lease_ns,
        avoid_ros_namespace_conventions: q.avoid_ros_namespace_conventions != 0,
    })
}

impl Resolved {
    /// The C-API endpoint QoS.
    #[must_use]
    pub fn endpoint(&self) -> zerodds::ZeroDdsEndpointQos {
        zerodds::ZeroDdsEndpointQos {
            reliable: i32::from(self.reliable),
            transient_local: i32::from(self.transient_local),
            history_keep_all: i32::from(self.keep_last.is_none()),
            history_depth: self.keep_last.unwrap_or(1),
            deadline_ns: self.deadline_ns,
            lifespan_ns: self.lifespan_ns,
            liveliness_kind: self.liveliness_kind,
            liveliness_lease_ns: self.liveliness_lease_ns,
            // A ROS QoS profile has no RESOURCE_LIMITS.
            ..zerodds::zerodds_endpoint_qos_default()
        }
    }

    /// The actual QoS in rmw terms, as `rmw_*_get_actual_qos` reports it.
    #[must_use]
    pub fn actual(&self) -> RmwZerodsQos {
        RmwZerodsQos {
            history: if self.keep_last.is_some() {
                rmw::KEEP_LAST
            } else {
                rmw::KEEP_ALL
            },
            depth: u64::from(self.keep_last.unwrap_or(0)),
            reliability: if self.reliable {
                rmw::RELIABLE
            } else {
                rmw::BEST_EFFORT
            },
            durability: if self.transient_local {
                rmw::TRANSIENT_LOCAL
            } else {
                rmw::VOLATILE
            },
            deadline_ns: self.deadline_ns,
            lifespan_ns: self.lifespan_ns,
            liveliness: match self.liveliness_kind {
                1 => rmw::MANUAL_BY_NODE,
                2 => rmw::MANUAL_BY_TOPIC,
                _ => rmw::AUTOMATIC,
            },
            liveliness_lease_ns: self.liveliness_lease_ns,
            avoid_ros_namespace_conventions: u32::from(self.avoid_ros_namespace_conventions),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn profile(history: u32, depth: u64, reliability: u32, durability: u32) -> RmwZerodsQos {
        RmwZerodsQos {
            history,
            depth,
            reliability,
            durability,
            ..RmwZerodsQos::default()
        }
    }

    #[test]
    fn the_rclcpp_profiles_reach_the_endpoint_whole() {
        // rmw_qos_profile_default: KEEP_LAST(10), RELIABLE, VOLATILE.
        let r = resolve(&profile(1, 10, 1, 2), Side::Publisher).unwrap();
        assert_eq!(
            (r.reliable, r.transient_local, r.keep_last),
            (true, false, Some(10))
        );
        // tf_static style: KEEP_LAST(1), RELIABLE, TRANSIENT_LOCAL.
        let r = resolve(&profile(1, 1, 1, 1), Side::Publisher).unwrap();
        assert!(r.transient_local, "TRANSIENT_LOCAL used to be dropped");
        assert_eq!(r.keep_last, Some(1));
        // sensor_data: KEEP_LAST(5), BEST_EFFORT.
        let r = resolve(&profile(1, 5, 2, 2), Side::Subscription).unwrap();
        assert_eq!((r.reliable, r.keep_last), (false, Some(5)));
        // parameter_events style KEEP_ALL.
        assert_eq!(
            resolve(&profile(2, 0, 1, 2), Side::Publisher)
                .unwrap()
                .keep_last,
            None
        );
    }

    #[test]
    fn system_default_and_best_available_resolve_per_side() {
        let d = resolve(&profile(0, 0, 0, 0), Side::Subscription).unwrap();
        assert_eq!(
            (d.reliable, d.transient_local, d.keep_last),
            (true, false, Some(1))
        );
        let p = resolve(&profile(1, 3, 4, 4), Side::Publisher).unwrap();
        assert!(
            p.reliable && p.transient_local,
            "a publisher offers the strongest"
        );
        let s = resolve(&profile(1, 3, 4, 4), Side::Subscription).unwrap();
        assert!(
            !s.reliable && !s.transient_local,
            "a subscription requests the weakest"
        );
    }

    #[test]
    fn times_liveliness_and_the_prefix_switch_are_kept() {
        let q = RmwZerodsQos {
            history: 1,
            depth: 7,
            reliability: 1,
            durability: 2,
            deadline_ns: 100_000_000,
            lifespan_ns: 2_000_000_000,
            liveliness: 3,
            liveliness_lease_ns: 1_000_000_000,
            avoid_ros_namespace_conventions: 1,
        };
        let r = resolve(&q, Side::Publisher).unwrap();
        assert_eq!(r.deadline_ns, 100_000_000);
        assert_eq!(r.lifespan_ns, 2_000_000_000);
        assert_eq!(r.liveliness_kind, 2);
        assert!(r.avoid_ros_namespace_conventions);
        assert_eq!(
            r.actual(),
            q,
            "a fully specified profile is its own actual QoS"
        );
        assert_eq!(resolve(&q, Side::Subscription).unwrap().lifespan_ns, 0);
    }

    #[test]
    fn a_value_outside_the_rmw_enums_is_refused() {
        for q in [
            profile(3, 1, 1, 2),
            profile(1, 1, 3, 2),
            profile(1, 1, 1, 3),
        ] {
            assert!(resolve(&q, Side::Publisher).is_err(), "{q:?}");
        }
    }
}
