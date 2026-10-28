// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-PSM constants from DDS-DCPS 1.4 §2.3.3 (Spec).
//!
//! These constants are a mandatory part of the IDL-PSM and are consumed
//! by applications and language bindings. We mirror them here 1:1 as
//! Rust constants so that bindings (C/C++/Java/Python) can adopt them
//! without magic-number drift.

#![allow(missing_docs)]

extern crate alloc;

/// Status-kind bitmask values (Spec §2.2.4.1 Tab. 2.10 + IDL-PSM
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
    /// `STATUS_MASK_NONE` — no status active.
    pub const NONE: u32 = 0;
    /// `STATUS_MASK_ANY` — all 13 standard statuses.
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
    // The names of the IDL (§2.3.3: `<STATUS>_STATUS`).
    #[doc = "`INCONSISTENT_TOPIC_STATUS`"]
    pub const INCONSISTENT_TOPIC_STATUS: u32 = INCONSISTENT_TOPIC;
    #[doc = "`OFFERED_DEADLINE_MISSED_STATUS`"]
    pub const OFFERED_DEADLINE_MISSED_STATUS: u32 = OFFERED_DEADLINE_MISSED;
    #[doc = "`REQUESTED_DEADLINE_MISSED_STATUS`"]
    pub const REQUESTED_DEADLINE_MISSED_STATUS: u32 = REQUESTED_DEADLINE_MISSED;
    #[doc = "`OFFERED_INCOMPATIBLE_QOS_STATUS`"]
    pub const OFFERED_INCOMPATIBLE_QOS_STATUS: u32 = OFFERED_INCOMPATIBLE_QOS;
    #[doc = "`REQUESTED_INCOMPATIBLE_QOS_STATUS`"]
    pub const REQUESTED_INCOMPATIBLE_QOS_STATUS: u32 = REQUESTED_INCOMPATIBLE_QOS;
    #[doc = "`SAMPLE_LOST_STATUS`"]
    pub const SAMPLE_LOST_STATUS: u32 = SAMPLE_LOST;
    #[doc = "`SAMPLE_REJECTED_STATUS`"]
    pub const SAMPLE_REJECTED_STATUS: u32 = SAMPLE_REJECTED;
    #[doc = "`DATA_ON_READERS_STATUS`"]
    pub const DATA_ON_READERS_STATUS: u32 = DATA_ON_READERS;
    #[doc = "`DATA_AVAILABLE_STATUS`"]
    pub const DATA_AVAILABLE_STATUS: u32 = DATA_AVAILABLE;
    #[doc = "`LIVELINESS_LOST_STATUS`"]
    pub const LIVELINESS_LOST_STATUS: u32 = LIVELINESS_LOST;
    #[doc = "`LIVELINESS_CHANGED_STATUS`"]
    pub const LIVELINESS_CHANGED_STATUS: u32 = LIVELINESS_CHANGED;
    #[doc = "`PUBLICATION_MATCHED_STATUS`"]
    pub const PUBLICATION_MATCHED_STATUS: u32 = PUBLICATION_MATCHED;
    #[doc = "`SUBSCRIPTION_MATCHED_STATUS`"]
    pub const SUBSCRIPTION_MATCHED_STATUS: u32 = SUBSCRIPTION_MATCHED;
}

/// QoS policy ids (Spec §2.2.3 Tab. 2.13). 32-bit u32 values; little
/// cross-vendor consensus — we follow the OMG spec numbering.
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
    /// XTypes 1.3 Annex D: `DATA_REPRESENTATION_QOS_POLICY_ID = 23`.
    pub const DATA_REPRESENTATION: u32 = 23;
    /// XTypes 1.3 Annex D: `TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID = 24`.
    /// Set as the reject reason by the reader-match path when reader and
    /// writer have incompatible TypeIdentifiers.
    pub const TYPE_CONSISTENCY_ENFORCEMENT: u32 = 24;
    // The names of the IDL (§2.3.3: `<NAME>_QOS_POLICY_ID`, without the underscores inside the name).
    #[doc = "`INVALID_QOS_POLICY_ID`"]
    pub const INVALID_QOS_POLICY_ID: u32 = INVALID;
    #[doc = "`USERDATA_QOS_POLICY_ID`"]
    pub const USERDATA_QOS_POLICY_ID: u32 = USER_DATA;
    #[doc = "`DURABILITY_QOS_POLICY_ID`"]
    pub const DURABILITY_QOS_POLICY_ID: u32 = DURABILITY;
    #[doc = "`PRESENTATION_QOS_POLICY_ID`"]
    pub const PRESENTATION_QOS_POLICY_ID: u32 = PRESENTATION;
    #[doc = "`DEADLINE_QOS_POLICY_ID`"]
    pub const DEADLINE_QOS_POLICY_ID: u32 = DEADLINE;
    #[doc = "`LATENCYBUDGET_QOS_POLICY_ID`"]
    pub const LATENCYBUDGET_QOS_POLICY_ID: u32 = LATENCY_BUDGET;
    #[doc = "`OWNERSHIP_QOS_POLICY_ID`"]
    pub const OWNERSHIP_QOS_POLICY_ID: u32 = OWNERSHIP;
    #[doc = "`OWNERSHIPSTRENGTH_QOS_POLICY_ID`"]
    pub const OWNERSHIPSTRENGTH_QOS_POLICY_ID: u32 = OWNERSHIP_STRENGTH;
    #[doc = "`LIVELINESS_QOS_POLICY_ID`"]
    pub const LIVELINESS_QOS_POLICY_ID: u32 = LIVELINESS;
    #[doc = "`TIMEBASEDFILTER_QOS_POLICY_ID`"]
    pub const TIMEBASEDFILTER_QOS_POLICY_ID: u32 = TIME_BASED_FILTER;
    #[doc = "`PARTITION_QOS_POLICY_ID`"]
    pub const PARTITION_QOS_POLICY_ID: u32 = PARTITION;
    #[doc = "`RELIABILITY_QOS_POLICY_ID`"]
    pub const RELIABILITY_QOS_POLICY_ID: u32 = RELIABILITY;
    #[doc = "`DESTINATIONORDER_QOS_POLICY_ID`"]
    pub const DESTINATIONORDER_QOS_POLICY_ID: u32 = DESTINATION_ORDER;
    #[doc = "`HISTORY_QOS_POLICY_ID`"]
    pub const HISTORY_QOS_POLICY_ID: u32 = HISTORY;
    #[doc = "`RESOURCELIMITS_QOS_POLICY_ID`"]
    pub const RESOURCELIMITS_QOS_POLICY_ID: u32 = RESOURCE_LIMITS;
    #[doc = "`ENTITYFACTORY_QOS_POLICY_ID`"]
    pub const ENTITYFACTORY_QOS_POLICY_ID: u32 = ENTITY_FACTORY;
    #[doc = "`WRITERDATALIFECYCLE_QOS_POLICY_ID`"]
    pub const WRITERDATALIFECYCLE_QOS_POLICY_ID: u32 = WRITER_DATA_LIFECYCLE;
    #[doc = "`READERDATALIFECYCLE_QOS_POLICY_ID`"]
    pub const READERDATALIFECYCLE_QOS_POLICY_ID: u32 = READER_DATA_LIFECYCLE;
    #[doc = "`TOPICDATA_QOS_POLICY_ID`"]
    pub const TOPICDATA_QOS_POLICY_ID: u32 = TOPIC_DATA;
    #[doc = "`GROUPDATA_QOS_POLICY_ID`"]
    pub const GROUPDATA_QOS_POLICY_ID: u32 = GROUP_DATA;
    #[doc = "`TRANSPORTPRIORITY_QOS_POLICY_ID`"]
    pub const TRANSPORTPRIORITY_QOS_POLICY_ID: u32 = TRANSPORT_PRIORITY;
    #[doc = "`LIFESPAN_QOS_POLICY_ID`"]
    pub const LIFESPAN_QOS_POLICY_ID: u32 = LIFESPAN;
    #[doc = "`DURABILITYSERVICE_QOS_POLICY_ID`"]
    pub const DURABILITYSERVICE_QOS_POLICY_ID: u32 = DURABILITY_SERVICE;
    #[doc = "`DATA_REPRESENTATION_QOS_POLICY_ID`"]
    pub const DATA_REPRESENTATION_QOS_POLICY_ID: u32 = DATA_REPRESENTATION;
    #[doc = "`TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID`"]
    pub const TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID: u32 = TYPE_CONSISTENCY_ENFORCEMENT;
}

/// The names of the QoS policies: the 22 of DDS 1.4 (§2.3.3,
/// `USERDATA_QOS_POLICY_NAME` and the others, from `zerodds_qos`) and the two
/// DDS-XTypes 1.3 adds (Annex D).
pub mod qos_policy_name {
    pub use zerodds_qos::policy_name::*;

    /// `DATA_REPRESENTATION_QOS_POLICY_NAME`.
    pub const DATA_REPRESENTATION: &str = "DataRepresentation";
    /// `TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_NAME`.
    pub const TYPE_CONSISTENCY_ENFORCEMENT: &str = "TypeConsistencyEnforcement";
}

/// `DataRepresentationId_t` values (DDS-XTypes 1.3 Annex D).
pub mod data_representation_id {
    /// `XCDR_DATA_REPRESENTATION`.
    pub const XCDR: i16 = 0;
    /// `XML_DATA_REPRESENTATION`.
    pub const XML: i16 = 1;
    /// `XCDR2_DATA_REPRESENTATION`.
    pub const XCDR2: i16 = 2;
}

/// `Sample/View/InstanceState` bitmask constants (Spec §2.2.2.5.1
/// Tab. 2.6/2.7/2.8 + IDL-PSM).
pub mod state {
    // SampleStateKind
    pub const READ_SAMPLE_STATE: u32 = 1 << 0;
    pub const NOT_READ_SAMPLE_STATE: u32 = 1 << 1;
    pub const ANY_SAMPLE_STATE: u32 = 0xffff;

    // ViewStateKind
    pub const NEW_VIEW_STATE: u32 = 1 << 0;
    pub const NOT_NEW_VIEW_STATE: u32 = 1 << 1;
    pub const ANY_VIEW_STATE: u32 = 0xffff;

    // InstanceStateKind
    pub const ALIVE_INSTANCE_STATE: u32 = 1 << 0;
    pub const NOT_ALIVE_DISPOSED_INSTANCE_STATE: u32 = 1 << 1;
    pub const NOT_ALIVE_NO_WRITERS_INSTANCE_STATE: u32 = 1 << 2;
    pub const NOT_ALIVE_INSTANCE_STATE: u32 =
        NOT_ALIVE_DISPOSED_INSTANCE_STATE | NOT_ALIVE_NO_WRITERS_INSTANCE_STATE;
    pub const ANY_INSTANCE_STATE: u32 = 0xffff;
}

/// Resource-limit sentinels (Spec §2.2.2.5.3).
///
/// `LENGTH_UNLIMITED = -1` is the convention for "no limit" in
/// `max_samples`, `max_instances`, `max_samples_per_instance`.
pub const LENGTH_UNLIMITED: i32 = -1;

use crate::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, EndpointQosArg, PublisherQos, QosArg,
    SubscriberQos, TopicQos,
};

/// The QoS placeholders of §2.3.3 (`#define PARTICIPANT_QOS_DEFAULT` …),
/// as the argument values [`QosArg`] and [`EndpointQosArg`].
pub const PARTICIPANT_QOS_DEFAULT: QosArg<DomainParticipantQos> = QosArg::Default;
/// `TOPIC_QOS_DEFAULT` (§2.3.3).
pub const TOPIC_QOS_DEFAULT: QosArg<TopicQos> = QosArg::Default;
/// `PUBLISHER_QOS_DEFAULT` (§2.3.3).
pub const PUBLISHER_QOS_DEFAULT: QosArg<PublisherQos> = QosArg::Default;
/// `SUBSCRIBER_QOS_DEFAULT` (§2.3.3).
pub const SUBSCRIBER_QOS_DEFAULT: QosArg<SubscriberQos> = QosArg::Default;
/// `DATAWRITER_QOS_DEFAULT` (§2.3.3).
pub const DATAWRITER_QOS_DEFAULT: QosArg<DataWriterQos> = QosArg::Default;
/// `DATAREADER_QOS_DEFAULT` (§2.3.3).
pub const DATAREADER_QOS_DEFAULT: QosArg<DataReaderQos> = QosArg::Default;
/// `DATAWRITER_QOS_USE_TOPIC_QOS` (§2.3.3), for `create_datawriter`.
pub const DATAWRITER_QOS_USE_TOPIC_QOS: EndpointQosArg<DataWriterQos> = EndpointQosArg::UseTopicQos;
/// `DATAREADER_QOS_USE_TOPIC_QOS` (§2.3.3), for `create_datareader`.
pub const DATAREADER_QOS_USE_TOPIC_QOS: EndpointQosArg<DataReaderQos> = EndpointQosArg::UseTopicQos;

#[cfg(test)]
mod tests {
    use super::*;

    /// The two QoS policy ids XTypes 1.3 adds, as Annex D (p. 327–328) has
    /// them: `DATA_REPRESENTATION_QOS_POLICY_ID = 23`,
    /// `TYPE_CONSISTENCY_ENFORCEMENT_QOS_POLICY_ID = 24`. They
    /// were the other way round, and the incompatible-QoS status named a
    /// representation that did not match by the id of the other policy.
    #[test]
    fn the_xtypes_policy_ids_are_those_of_its_idl() {
        assert_eq!(qos_policy_id::DATA_REPRESENTATION, 23);
        assert_eq!(qos_policy_id::TYPE_CONSISTENCY_ENFORCEMENT, 24);
    }

    #[test]
    fn status_mask_any_covers_13_bits() {
        let bits = status::ANY.count_ones();
        assert_eq!(bits, 13);
    }

    #[test]
    fn status_mask_none_is_zero() {
        assert_eq!(status::NONE, 0);
    }

    /// DDS 1.4 §2.3.3: "const SampleStateKind READ_SAMPLE_STATE = 0x0001
    /// << 0; const SampleStateKind NOT_READ_SAMPLE_STATE = 0x0001 << 1;"
    /// and the three ANY masks 0xffff. The masks `read`/`take` take are
    /// these: `sample_info::sample_state_mask` had READ and NOT_READ the
    /// other way round, and the ANY masks were the OR of the kinds.
    #[test]
    fn the_state_masks_are_those_of_the_idl() {
        use crate::sample_info::{instance_state_mask, sample_state_mask, view_state_mask};
        assert_eq!(state::READ_SAMPLE_STATE, 0x0001);
        assert_eq!(state::NOT_READ_SAMPLE_STATE, 0x0002);
        assert_eq!(state::ANY_SAMPLE_STATE, 0xffff);
        assert_eq!(state::NEW_VIEW_STATE, 0x0001);
        assert_eq!(state::NOT_NEW_VIEW_STATE, 0x0002);
        assert_eq!(state::ANY_VIEW_STATE, 0xffff);
        assert_eq!(state::ALIVE_INSTANCE_STATE, 0x0001);
        assert_eq!(state::NOT_ALIVE_DISPOSED_INSTANCE_STATE, 0x0002);
        assert_eq!(state::NOT_ALIVE_NO_WRITERS_INSTANCE_STATE, 0x0004);
        assert_eq!(state::ANY_INSTANCE_STATE, 0xffff);
        assert_eq!(sample_state_mask::READ, state::READ_SAMPLE_STATE);
        assert_eq!(sample_state_mask::NOT_READ, state::NOT_READ_SAMPLE_STATE);
        assert_eq!(sample_state_mask::ANY, state::ANY_SAMPLE_STATE);
        assert_eq!(view_state_mask::NEW, state::NEW_VIEW_STATE);
        assert_eq!(view_state_mask::NOT_NEW, state::NOT_NEW_VIEW_STATE);
        assert_eq!(view_state_mask::ANY, state::ANY_VIEW_STATE);
        assert_eq!(instance_state_mask::ALIVE, state::ALIVE_INSTANCE_STATE);
        assert_eq!(
            instance_state_mask::NOT_ALIVE_DISPOSED,
            state::NOT_ALIVE_DISPOSED_INSTANCE_STATE
        );
        assert_eq!(
            instance_state_mask::NOT_ALIVE_NO_WRITERS,
            state::NOT_ALIVE_NO_WRITERS_INSTANCE_STATE
        );
        assert_eq!(instance_state_mask::ANY, state::ANY_INSTANCE_STATE);
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

    /// Annex D: the names and the representation identifiers the XTypes
    /// policies carry.
    #[test]
    fn the_xtypes_policy_names_and_representation_ids_are_those_of_annex_d() {
        assert_eq!(qos_policy_name::DATA_REPRESENTATION, "DataRepresentation");
        assert_eq!(
            qos_policy_name::TYPE_CONSISTENCY_ENFORCEMENT,
            "TypeConsistencyEnforcement"
        );
        assert_eq!(data_representation_id::XCDR, 0);
        assert_eq!(data_representation_id::XML, 1);
        assert_eq!(data_representation_id::XCDR2, 2);
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
        assert_eq!(seen.len(), 22); // 22 standard policies
    }
}
