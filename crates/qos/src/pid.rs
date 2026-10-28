// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! PID constants for QoS policies (DDSI-RTPS §9.6.3.2).
//!
//! Each QoS value is encapsulated in a SEDP ParameterList as a
//! `{ pid, length, value }` triple. The PIDs are fixed in
//! DDSI-RTPS 2.5 Table 9.18 "ParameterId values for the built-in endpoints".
//! We duplicate them here (instead of importing zerodds-rtps),
//! to keep `zerodds-qos` independent of `zerodds-rtps`.

/// DDSI-RTPS 2.5 Table 9.18. 16-bit parameter IDs.
///
/// The values are OMG-public and stable across all vendors.
#[non_exhaustive]
pub struct Pid;

impl Pid {
    // ---------------------------------------------------------------------
    // QoS-Policies (§9.6.3.2).
    // ---------------------------------------------------------------------
    /// `PID_USER_DATA`.
    pub const USER_DATA: u16 = 0x002c;
    /// `PID_TOPIC_DATA`.
    pub const TOPIC_DATA: u16 = 0x002e;
    /// `PID_GROUP_DATA`.
    pub const GROUP_DATA: u16 = 0x002d;
    /// `PID_DURABILITY`.
    pub const DURABILITY: u16 = 0x001d;
    /// `PID_DURABILITY_SERVICE`.
    pub const DURABILITY_SERVICE: u16 = 0x001e;
    /// `PID_DEADLINE`.
    pub const DEADLINE: u16 = 0x0023;
    /// `PID_LATENCY_BUDGET`.
    pub const LATENCY_BUDGET: u16 = 0x0027;
    /// `PID_LIVELINESS`.
    pub const LIVELINESS: u16 = 0x001b;
    /// `PID_RELIABILITY`.
    pub const RELIABILITY: u16 = 0x001a;
    /// `PID_LIFESPAN`.
    pub const LIFESPAN: u16 = 0x002b;
    /// `PID_DESTINATION_ORDER`.
    pub const DESTINATION_ORDER: u16 = 0x0025;
    /// `PID_HISTORY`.
    pub const HISTORY: u16 = 0x0040;
    /// `PID_RESOURCE_LIMITS`.
    pub const RESOURCE_LIMITS: u16 = 0x0041;
    /// `PID_OWNERSHIP`.
    pub const OWNERSHIP: u16 = 0x001f;
    /// `PID_OWNERSHIP_STRENGTH`.
    pub const OWNERSHIP_STRENGTH: u16 = 0x0006;
    /// `PID_PRESENTATION`.
    pub const PRESENTATION: u16 = 0x0021;
    /// `PID_PARTITION`.
    pub const PARTITION: u16 = 0x0029;
    /// `PID_TIME_BASED_FILTER`.
    pub const TIME_BASED_FILTER: u16 = 0x0004;
    /// `PID_TRANSPORT_PRIORITY`.
    pub const TRANSPORT_PRIORITY: u16 = 0x0049;
    /// `PID_READER_DATA_LIFECYCLE` — OMG has not standardized fixed PIDs
    /// for DataLifecycle policies; we mark the values with
    /// the vendor flag (`0x8000`), so that foreign stacks (Cyclone, Fast-DDS)
    /// can ignore unknown PIDs per RTPS §9.6.3.2.1, instead of
    /// inferring a standardized value.
    pub const READER_DATA_LIFECYCLE: u16 = 0x8046;
    /// `PID_WRITER_DATA_LIFECYCLE` — likewise with the vendor flag.
    pub const WRITER_DATA_LIFECYCLE: u16 = 0x8045;

    // ---------------------------------------------------------------------
    // Sentinel.
    // ---------------------------------------------------------------------
    /// `PID_SENTINEL` — marks the end of a ParameterList.
    pub const SENTINEL: u16 = 0x0001;
}

#[cfg(test)]
mod tests {
    use super::Pid;

    /// DDS 1.4 §2.2.2.1.3: the abstract `QosPolicy` root "has an attribute
    /// **name** that is used to identify uniquely each QoS policy". On the
    /// wire that identity is the PID, and a policy that shared one with
    /// another would be indistinguishable to every peer — a silent
    /// mis-decode rather than a refusal. So the twenty declared policy
    /// identifiers are checked to be pairwise distinct, and distinct from
    /// the sentinel that ends the list.
    #[test]
    fn every_qos_policy_has_a_distinct_identifier() {
        let policies: [(&str, u16); 20] = [
            ("USER_DATA", Pid::USER_DATA),
            ("TOPIC_DATA", Pid::TOPIC_DATA),
            ("GROUP_DATA", Pid::GROUP_DATA),
            ("DURABILITY", Pid::DURABILITY),
            ("DURABILITY_SERVICE", Pid::DURABILITY_SERVICE),
            ("DEADLINE", Pid::DEADLINE),
            ("LATENCY_BUDGET", Pid::LATENCY_BUDGET),
            ("LIVELINESS", Pid::LIVELINESS),
            ("RELIABILITY", Pid::RELIABILITY),
            ("LIFESPAN", Pid::LIFESPAN),
            ("DESTINATION_ORDER", Pid::DESTINATION_ORDER),
            ("HISTORY", Pid::HISTORY),
            ("RESOURCE_LIMITS", Pid::RESOURCE_LIMITS),
            ("OWNERSHIP", Pid::OWNERSHIP),
            ("OWNERSHIP_STRENGTH", Pid::OWNERSHIP_STRENGTH),
            ("PRESENTATION", Pid::PRESENTATION),
            ("PARTITION", Pid::PARTITION),
            ("TIME_BASED_FILTER", Pid::TIME_BASED_FILTER),
            ("READER_DATA_LIFECYCLE", Pid::READER_DATA_LIFECYCLE),
            ("WRITER_DATA_LIFECYCLE", Pid::WRITER_DATA_LIFECYCLE),
        ];
        for (i, (left_name, left)) in policies.iter().enumerate() {
            assert_ne!(
                *left,
                Pid::SENTINEL,
                "{left_name} collides with PID_SENTINEL"
            );
            for (right_name, right) in policies.iter().skip(i + 1) {
                assert_ne!(
                    left, right,
                    "{left_name} and {right_name} share the identifier {left:#06x}"
                );
            }
        }
    }

    /// The two policies OMG left without a standard PID carry the vendor
    /// bit, so a foreign stack skips them instead of decoding them as
    /// whatever standard parameter happens to sit at that number.
    #[test]
    fn the_two_unstandardised_policies_are_marked_vendor_specific() {
        const VENDOR_BIT: u16 = 0x8000;
        assert_ne!(Pid::READER_DATA_LIFECYCLE & VENDOR_BIT, 0);
        assert_ne!(Pid::WRITER_DATA_LIFECYCLE & VENDOR_BIT, 0);
        // And every standardised one does not carry it.
        for standard in [
            Pid::USER_DATA,
            Pid::DURABILITY,
            Pid::HISTORY,
            Pid::RELIABILITY,
            Pid::TRANSPORT_PRIORITY,
        ] {
            assert_eq!(
                standard & VENDOR_BIT,
                0,
                "{standard:#06x} is not a vendor PID"
            );
        }
    }
}
