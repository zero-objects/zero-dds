// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-PSM-Konstanten aus DDS-DCPS 1.4 §2.3.3 (Spec).
//!
//! Diese Konstanten sind Pflicht-Bestandteil des IDL-PSM und werden von
//! Anwendungen + Sprach-Bindings konsumiert. Wir spiegeln sie hier
//! 1:1 als Rust-Konstanten, damit Bindings (C/C++/Java/Python) sie
//! ohne Magic-Number-Drift uebernehmen koennen.

#![allow(missing_docs)]

extern crate alloc;

/// Status-Kind-Bitmask-Werte (Spec §2.2.4.1 Tab. 2.10 + IDL-PSM
/// `STATUS_MASK_*`).
pub mod status {
    /// `INCONSISTENT_TOPIC_STATUS`
    pub const INCONSISTENT_TOPIC: u32 = 1 << 0;
    /// `OFFERED_DEADLINE_MISSED_STATUS`
    pub const OFFERED_DEADLINE_MISSED: u32 = 1 << 1;
    /// `REQUESTED_DEADLINE_MISSED_STATUS`
    pub const REQUESTED_DEADLINE_MISSED: u32 = 1 << 2;
    /// `OFFERED_INCOMPATIBLE_QOS_STATUS`
    pub const OFFERED_INCOMPATIBLE_QOS: u32 = 1 << 5;
    /// `REQUESTED_INCOMPATIBLE_QOS_STATUS`
    pub const REQUESTED_INCOMPATIBLE_QOS: u32 = 1 << 6;
    /// `SAMPLE_LOST_STATUS`
    pub const SAMPLE_LOST: u32 = 1 << 7;
    /// `SAMPLE_REJECTED_STATUS`
    pub const SAMPLE_REJECTED: u32 = 1 << 8;
    /// `DATA_ON_READERS_STATUS`
    pub const DATA_ON_READERS: u32 = 1 << 9;
    /// `DATA_AVAILABLE_STATUS`
    pub const DATA_AVAILABLE: u32 = 1 << 10;
    /// `LIVELINESS_LOST_STATUS`
    pub const LIVELINESS_LOST: u32 = 1 << 11;
    /// `LIVELINESS_CHANGED_STATUS`
    pub const LIVELINESS_CHANGED: u32 = 1 << 12;
    /// `PUBLICATION_MATCHED_STATUS`
    pub const PUBLICATION_MATCHED: u32 = 1 << 13;
    /// `SUBSCRIPTION_MATCHED_STATUS`
    pub const SUBSCRIPTION_MATCHED: u32 = 1 << 14;
    /// `STATUS_MASK_NONE` — kein Status aktiv.
    pub const NONE: u32 = 0;
    /// `STATUS_MASK_ANY` — alle 13 Standard-Statuses.
    pub const ANY: u32 = INCONSISTENT_TOPIC
        | OFFERED_DEADLINE_MISSED
        | REQUESTED_DEADLINE_MISSED
        | OFFERED_INCOMPATIBLE_QOS
        | REQUESTED_INCOMPATIBLE_QOS
        | SAMPLE_LOST
        | SAMPLE_REJECTED
        | DATA_ON_READERS
        | DATA_AVAILABLE
        | LIVELINESS_LOST
        | LIVELINESS_CHANGED
        | PUBLICATION_MATCHED
        | SUBSCRIPTION_MATCHED;
}

/// QoS-Policy-Ids (Spec §2.2.3 Tab. 2.13). 32-bit u32-Werte; wenig
/// Cross-Vendor-Konsens — wir folgen der OMG-Spec-Numerierung.
pub mod qos_policy_id {
    pub const INVALID: u32 = 0;
    pub const USER_DATA: u32 = 1;
    pub const DURABILITY: u32 = 2;
    pub const PRESENTATION: u32 = 3;
    pub const DEADLINE: u32 = 4;
    pub const LATENCY_BUDGET: u32 = 5;
    pub const OWNERSHIP: u32 = 6;
    pub const OWNERSHIP_STRENGTH: u32 = 7;
    pub const LIVELINESS: u32 = 8;
    pub const TIME_BASED_FILTER: u32 = 9;
    pub const PARTITION: u32 = 10;
    pub const RELIABILITY: u32 = 11;
    pub const DESTINATION_ORDER: u32 = 12;
    pub const HISTORY: u32 = 13;
    pub const RESOURCE_LIMITS: u32 = 14;
    pub const ENTITY_FACTORY: u32 = 15;
    pub const WRITER_DATA_LIFECYCLE: u32 = 16;
    pub const READER_DATA_LIFECYCLE: u32 = 17;
    pub const TOPIC_DATA: u32 = 18;
    pub const GROUP_DATA: u32 = 19;
    pub const TRANSPORT_PRIORITY: u32 = 20;
    pub const LIFESPAN: u32 = 21;
    pub const DURABILITY_SERVICE: u32 = 22;
    /// XTypes 1.3 §7.6.2 — TypeConsistencyEnforcement-QoS-Policy-Id.
    /// Wird von F-TYPES-3 Reader-Match-Pfad als Reject-Reason gesetzt
    /// wenn Reader und Writer inkompatible TypeIdentifier haben.
    pub const TYPE_CONSISTENCY_ENFORCEMENT: u32 = 23;
    /// XTypes 1.3 §7.6.3 — DataRepresentation-QoS-Policy-Id.
    pub const DATA_REPRESENTATION: u32 = 24;
}

/// `Sample/View/InstanceState`-Bitmask-Konstanten (Spec §2.2.2.5.1
/// Tab. 2.6/2.7/2.8 + IDL-PSM).
pub mod state {
    // SampleStateKind
    pub const READ_SAMPLE_STATE: u32 = 1 << 0;
    pub const NOT_READ_SAMPLE_STATE: u32 = 1 << 1;
    pub const ANY_SAMPLE_STATE: u32 = READ_SAMPLE_STATE | NOT_READ_SAMPLE_STATE;

    // ViewStateKind
    pub const NEW_VIEW_STATE: u32 = 1 << 0;
    pub const NOT_NEW_VIEW_STATE: u32 = 1 << 1;
    pub const ANY_VIEW_STATE: u32 = NEW_VIEW_STATE | NOT_NEW_VIEW_STATE;

    // InstanceStateKind
    pub const ALIVE_INSTANCE_STATE: u32 = 1 << 0;
    pub const NOT_ALIVE_DISPOSED_INSTANCE_STATE: u32 = 1 << 1;
    pub const NOT_ALIVE_NO_WRITERS_INSTANCE_STATE: u32 = 1 << 2;
    pub const NOT_ALIVE_INSTANCE_STATE: u32 =
        NOT_ALIVE_DISPOSED_INSTANCE_STATE | NOT_ALIVE_NO_WRITERS_INSTANCE_STATE;
    pub const ANY_INSTANCE_STATE: u32 = ALIVE_INSTANCE_STATE | NOT_ALIVE_INSTANCE_STATE;
}

/// Resource-Limit-Sentinels (Spec §2.2.2.5.3).
///
/// `LENGTH_UNLIMITED = -1` ist die Konvention fuer "keine Begrenzung"
/// in `max_samples`, `max_instances`, `max_samples_per_instance`.
pub const LENGTH_UNLIMITED: i32 = -1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mask_any_covers_13_bits() {
        let bits = status::ANY.count_ones();
        assert_eq!(bits, 13);
    }

    #[test]
    fn status_mask_none_is_zero() {
        assert_eq!(status::NONE, 0);
    }

    #[test]
    fn sample_state_any_is_or_of_two() {
        assert_eq!(
            state::ANY_SAMPLE_STATE,
            state::READ_SAMPLE_STATE | state::NOT_READ_SAMPLE_STATE
        );
    }

    #[test]
    fn instance_state_not_alive_is_or_of_disposed_and_no_writers() {
        assert_eq!(
            state::NOT_ALIVE_INSTANCE_STATE,
            state::NOT_ALIVE_DISPOSED_INSTANCE_STATE | state::NOT_ALIVE_NO_WRITERS_INSTANCE_STATE
        );
    }

    #[test]
    fn length_unlimited_is_minus_one() {
        assert_eq!(LENGTH_UNLIMITED, -1);
    }

    #[test]
    fn qos_policy_ids_are_distinct() {
        let ids = [
            qos_policy_id::USER_DATA,
            qos_policy_id::DURABILITY,
            qos_policy_id::PRESENTATION,
            qos_policy_id::DEADLINE,
            qos_policy_id::LATENCY_BUDGET,
            qos_policy_id::OWNERSHIP,
            qos_policy_id::OWNERSHIP_STRENGTH,
            qos_policy_id::LIVELINESS,
            qos_policy_id::TIME_BASED_FILTER,
            qos_policy_id::PARTITION,
            qos_policy_id::RELIABILITY,
            qos_policy_id::DESTINATION_ORDER,
            qos_policy_id::HISTORY,
            qos_policy_id::RESOURCE_LIMITS,
            qos_policy_id::ENTITY_FACTORY,
            qos_policy_id::WRITER_DATA_LIFECYCLE,
            qos_policy_id::READER_DATA_LIFECYCLE,
            qos_policy_id::TOPIC_DATA,
            qos_policy_id::GROUP_DATA,
            qos_policy_id::TRANSPORT_PRIORITY,
            qos_policy_id::LIFESPAN,
            qos_policy_id::DURABILITY_SERVICE,
        ];
        let mut seen = alloc::collections::BTreeSet::new();
        for id in ids {
            assert!(seen.insert(id));
        }
        assert_eq!(seen.len(), 22); // 22 Standard-Policies
    }
}
