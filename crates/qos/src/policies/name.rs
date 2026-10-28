// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The `name` of a QosPolicy (DDS 1.4 §2.2.2.1.3: "It has an attribute name
//! that is used to identify uniquely each QoS policy") and the name
//! constants of the IDL PSM (§2.3.3, `USERDATA_QOS_POLICY_NAME` and the
//! others).
//!
//! The IDL gives `GROUPDATA_QOS_POLICY_NAME` the value `"TransportPriority"`
//! and has no constant for the name of the TransportPriority policy; that is
//! a slip in the spec. Here `GROUP_DATA` is `"GroupData"` and
//! `TRANSPORT_PRIORITY` is `"TransportPriority"`.

/// The names of the 22 QoS policies of DDS 1.4 (§2.3.3), by constant name
/// without the suffix `_QOS_POLICY_NAME`.
pub mod policy_name {
    /// `USERDATA_QOS_POLICY_NAME`.
    pub const USER_DATA: &str = "UserData";
    /// `DURABILITY_QOS_POLICY_NAME`.
    pub const DURABILITY: &str = "Durability";
    /// `PRESENTATION_QOS_POLICY_NAME`.
    pub const PRESENTATION: &str = "Presentation";
    /// `DEADLINE_QOS_POLICY_NAME`.
    pub const DEADLINE: &str = "Deadline";
    /// `LATENCYBUDGET_QOS_POLICY_NAME`.
    pub const LATENCY_BUDGET: &str = "LatencyBudget";
    /// `OWNERSHIP_QOS_POLICY_NAME`.
    pub const OWNERSHIP: &str = "Ownership";
    /// `OWNERSHIPSTRENGTH_QOS_POLICY_NAME`.
    pub const OWNERSHIP_STRENGTH: &str = "OwnershipStrength";
    /// `LIVELINESS_QOS_POLICY_NAME`.
    pub const LIVELINESS: &str = "Liveliness";
    /// `TIMEBASEDFILTER_QOS_POLICY_NAME`.
    pub const TIME_BASED_FILTER: &str = "TimeBasedFilter";
    /// `PARTITION_QOS_POLICY_NAME`.
    pub const PARTITION: &str = "Partition";
    /// `RELIABILITY_QOS_POLICY_NAME`.
    pub const RELIABILITY: &str = "Reliability";
    /// `DESTINATIONORDER_QOS_POLICY_NAME`.
    pub const DESTINATION_ORDER: &str = "DestinationOrder";
    /// `HISTORY_QOS_POLICY_NAME`.
    pub const HISTORY: &str = "History";
    /// `RESOURCELIMITS_QOS_POLICY_NAME`.
    pub const RESOURCE_LIMITS: &str = "ResourceLimits";
    /// `ENTITYFACTORY_QOS_POLICY_NAME`.
    pub const ENTITY_FACTORY: &str = "EntityFactory";
    /// `WRITERDATALIFECYCLE_QOS_POLICY_NAME`.
    pub const WRITER_DATA_LIFECYCLE: &str = "WriterDataLifecycle";
    /// `READERDATALIFECYCLE_QOS_POLICY_NAME`.
    pub const READER_DATA_LIFECYCLE: &str = "ReaderDataLifecycle";
    /// `TOPICDATA_QOS_POLICY_NAME`.
    pub const TOPIC_DATA: &str = "TopicData";
    /// `GROUPDATA_QOS_POLICY_NAME`.
    pub const GROUP_DATA: &str = "GroupData";
    /// `TRANSPORTPRIORITY_QOS_POLICY_NAME`.
    pub const TRANSPORT_PRIORITY: &str = "TransportPriority";
    /// `LIFESPAN_QOS_POLICY_NAME`.
    pub const LIFESPAN: &str = "Lifespan";
    /// `DURABILITYSERVICE_QOS_POLICY_NAME`.
    pub const DURABILITY_SERVICE: &str = "DurabilityService";
}

/// A QoS policy and its name (DDS 1.4 §2.2.2.1.3).
pub trait QosPolicy {
    /// The name that identifies the policy, as the constant of
    /// [`policy_name`] has it.
    const NAME: &'static str;

    /// The `name` attribute of the policy.
    #[must_use]
    fn name(&self) -> &'static str {
        Self::NAME
    }
}

impl QosPolicy for crate::policies::UserDataQosPolicy {
    const NAME: &'static str = policy_name::USER_DATA;
}

impl QosPolicy for crate::policies::DurabilityQosPolicy {
    const NAME: &'static str = policy_name::DURABILITY;
}

impl QosPolicy for crate::policies::PresentationQosPolicy {
    const NAME: &'static str = policy_name::PRESENTATION;
}

impl QosPolicy for crate::policies::DeadlineQosPolicy {
    const NAME: &'static str = policy_name::DEADLINE;
}

impl QosPolicy for crate::policies::LatencyBudgetQosPolicy {
    const NAME: &'static str = policy_name::LATENCY_BUDGET;
}

impl QosPolicy for crate::policies::OwnershipQosPolicy {
    const NAME: &'static str = policy_name::OWNERSHIP;
}

impl QosPolicy for crate::policies::OwnershipStrengthQosPolicy {
    const NAME: &'static str = policy_name::OWNERSHIP_STRENGTH;
}

impl QosPolicy for crate::policies::LivelinessQosPolicy {
    const NAME: &'static str = policy_name::LIVELINESS;
}

impl QosPolicy for crate::policies::TimeBasedFilterQosPolicy {
    const NAME: &'static str = policy_name::TIME_BASED_FILTER;
}

impl QosPolicy for crate::policies::PartitionQosPolicy {
    const NAME: &'static str = policy_name::PARTITION;
}

impl QosPolicy for crate::policies::ReliabilityQosPolicy {
    const NAME: &'static str = policy_name::RELIABILITY;
}

impl QosPolicy for crate::policies::DestinationOrderQosPolicy {
    const NAME: &'static str = policy_name::DESTINATION_ORDER;
}

impl QosPolicy for crate::policies::HistoryQosPolicy {
    const NAME: &'static str = policy_name::HISTORY;
}

impl QosPolicy for crate::policies::ResourceLimitsQosPolicy {
    const NAME: &'static str = policy_name::RESOURCE_LIMITS;
}

impl QosPolicy for crate::policies::EntityFactoryQosPolicy {
    const NAME: &'static str = policy_name::ENTITY_FACTORY;
}

impl QosPolicy for crate::policies::WriterDataLifecycleQosPolicy {
    const NAME: &'static str = policy_name::WRITER_DATA_LIFECYCLE;
}

impl QosPolicy for crate::policies::ReaderDataLifecycleQosPolicy {
    const NAME: &'static str = policy_name::READER_DATA_LIFECYCLE;
}

impl QosPolicy for crate::policies::TopicDataQosPolicy {
    const NAME: &'static str = policy_name::TOPIC_DATA;
}

impl QosPolicy for crate::policies::GroupDataQosPolicy {
    const NAME: &'static str = policy_name::GROUP_DATA;
}

impl QosPolicy for crate::policies::TransportPriorityQosPolicy {
    const NAME: &'static str = policy_name::TRANSPORT_PRIORITY;
}

impl QosPolicy for crate::policies::LifespanQosPolicy {
    const NAME: &'static str = policy_name::LIFESPAN;
}

impl QosPolicy for crate::policies::DurabilityServiceQosPolicy {
    const NAME: &'static str = policy_name::DURABILITY_SERVICE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policies::*;

    /// §2.3.3: the name of every policy is the string of its constant, and
    /// the 22 are distinct.
    #[test]
    fn every_policy_has_its_name() {
        let names = [
            (UserDataQosPolicy::default().name(), "UserData"),
            (DurabilityQosPolicy::default().name(), "Durability"),
            (PresentationQosPolicy::default().name(), "Presentation"),
            (DeadlineQosPolicy::default().name(), "Deadline"),
            (LatencyBudgetQosPolicy::default().name(), "LatencyBudget"),
            (OwnershipQosPolicy::default().name(), "Ownership"),
            (
                OwnershipStrengthQosPolicy::default().name(),
                "OwnershipStrength",
            ),
            (LivelinessQosPolicy::default().name(), "Liveliness"),
            (
                TimeBasedFilterQosPolicy::default().name(),
                "TimeBasedFilter",
            ),
            (PartitionQosPolicy::default().name(), "Partition"),
            (ReliabilityQosPolicy::default().name(), "Reliability"),
            (
                DestinationOrderQosPolicy::default().name(),
                "DestinationOrder",
            ),
            (HistoryQosPolicy::default().name(), "History"),
            (ResourceLimitsQosPolicy::default().name(), "ResourceLimits"),
            (EntityFactoryQosPolicy::default().name(), "EntityFactory"),
            (
                WriterDataLifecycleQosPolicy::default().name(),
                "WriterDataLifecycle",
            ),
            (
                ReaderDataLifecycleQosPolicy::default().name(),
                "ReaderDataLifecycle",
            ),
            (TopicDataQosPolicy::default().name(), "TopicData"),
            (GroupDataQosPolicy::default().name(), "GroupData"),
            (
                TransportPriorityQosPolicy::default().name(),
                "TransportPriority",
            ),
            (LifespanQosPolicy::default().name(), "Lifespan"),
            (
                DurabilityServiceQosPolicy::default().name(),
                "DurabilityService",
            ),
        ];
        for (name, expected) in names {
            assert_eq!(name, expected);
        }
        for (i, (a, _)) in names.iter().enumerate() {
            for (b, _) in &names[i + 1..] {
                assert_ne!(a, b, "a name identifies one policy");
            }
        }
        assert_eq!(names.len(), 22);
        assert_eq!(
            <ReliabilityQosPolicy as QosPolicy>::NAME,
            policy_name::RELIABILITY
        );
        assert_eq!(policy_name::GROUP_DATA, "GroupData");
        assert_eq!(policy_name::TRANSPORT_PRIORITY, "TransportPriority");
    }
}
