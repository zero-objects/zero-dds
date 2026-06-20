// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPS QoS — complete 22-policy sets for DataWriter/DataReader/Topic/
//! Publisher/Subscriber/DomainParticipant per DDS 1.4 §2.2.3.
//!
//! Re-exports the policy data types from the `zerodds_qos` crate; the
//! aggregates defined here are the DCPS-API view (what DataReader/Writer
//! hold internally and what the runtime path accesses).

pub use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy, DurabilityKind,
    DurabilityQosPolicy, DurabilityServiceQosPolicy, EntityFactoryQosPolicy, GroupDataQosPolicy,
    HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy, LivelinessKind,
    LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy, OwnershipStrengthQosPolicy,
    PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy, PropertyQosPolicy,
    ReaderDataLifecycleQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
    TimeBasedFilterQosPolicy, TopicDataQosPolicy, TransportPriorityQosPolicy, UserDataQosPolicy,
    WriterDataLifecycleQosPolicy,
};

use zerodds_qos::Duration;

/// QoS set for a `DataWriter` — Spec §2.2.2.4.2 (all policies settable
/// on the DataWriter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataWriterQos {
    /// Reliability — default Reliable.
    pub reliability: ReliabilityQosPolicy,
    /// Durability — default Volatile.
    pub durability: DurabilityQosPolicy,
    /// Configures the persistence service (Spec §2.2.3.5).
    pub durability_service: DurabilityServiceQosPolicy,
    /// Deadline — default INFINITE.
    pub deadline: DeadlineQosPolicy,
    /// LatencyBudget — hint, no match effect (Spec §2.2.3.10).
    pub latency_budget: LatencyBudgetQosPolicy,
    /// Liveliness — default Automatic / INFINITE.
    pub liveliness: LivelinessQosPolicy,
    /// DestinationOrder — default ByReceptionTimestamp.
    pub destination_order: DestinationOrderQosPolicy,
    /// Lifespan — default INFINITE.
    pub lifespan: LifespanQosPolicy,
    /// Ownership — default Shared.
    pub ownership: OwnershipQosPolicy,
    /// Ownership strength — only with Exclusive.
    pub ownership_strength: OwnershipStrengthQosPolicy,
    /// Partition — default empty (matches only the ""-default partition).
    pub partition: PartitionQosPolicy,
    /// Presentation — default Instance/false/false.
    pub presentation: PresentationQosPolicy,
    /// History — default KeepLast(1).
    pub history: HistoryQosPolicy,
    /// Resource limits.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// TransportPriority — hint, no match effect (Spec §2.2.3.15).
    pub transport_priority: TransportPriorityQosPolicy,
    /// WriterDataLifecycle — autodispose_unregistered_instances.
    pub writer_data_lifecycle: WriterDataLifecycleQosPolicy,
    /// UserData — opaque `sequence<octet>`, propagated via discovery.
    pub user_data: UserDataQosPolicy,
    /// TopicData — opaque `sequence<octet>`, propagated via discovery.
    pub topic_data: TopicDataQosPolicy,
    /// GroupData — opaque `sequence<octet>`, propagated via discovery.
    pub group_data: GroupDataQosPolicy,
    /// DataRepresentation (XTypes 1.3 §7.6.3.1.2): the offered
    /// representation list (`XCDR=0`, `XCDR2=2`). `None` = runtime default
    /// (`ZERODDS_DATA_REPR_OFFER` / `DEFAULT_OFFER`). Per-writer override
    /// for targeted cross-vendor interop (e.g. `[XCDR1, XCDR2]` for
    /// XCDR1 readers of ROS/RTI shapes) without a global default change.
    pub data_representation: Option<alloc::vec::Vec<i16>>,
}

impl Default for DataWriterQos {
    fn default() -> Self {
        Self {
            reliability: ReliabilityQosPolicy {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: Duration::from_millis(100_i32),
            },
            durability: DurabilityQosPolicy {
                kind: DurabilityKind::Volatile,
            },
            durability_service: DurabilityServiceQosPolicy::default(),
            deadline: DeadlineQosPolicy::default(),
            latency_budget: LatencyBudgetQosPolicy::default(),
            liveliness: LivelinessQosPolicy::default(),
            destination_order: DestinationOrderQosPolicy::default(),
            lifespan: LifespanQosPolicy::default(),
            ownership: OwnershipQosPolicy::default(),
            ownership_strength: OwnershipStrengthQosPolicy::default(),
            partition: PartitionQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            history: HistoryQosPolicy {
                kind: HistoryKind::KeepLast,
                depth: 1,
            },
            resource_limits: ResourceLimitsQosPolicy {
                max_samples: 1000_i32,
                max_instances: 10_i32,
                max_samples_per_instance: 100_i32,
            },
            transport_priority: TransportPriorityQosPolicy::default(),
            writer_data_lifecycle: WriterDataLifecycleQosPolicy::default(),
            user_data: UserDataQosPolicy::default(),
            topic_data: TopicDataQosPolicy::default(),
            group_data: GroupDataQosPolicy::default(),
            data_representation: None,
        }
    }
}

/// QoS set for a `DataReader` — Spec §2.2.2.5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataReaderQos {
    /// Reliability — reader default BestEffort (Spec §2.2.3.14.3).
    pub reliability: ReliabilityQosPolicy,
    /// Durability — default Volatile.
    pub durability: DurabilityQosPolicy,
    /// Deadline — default INFINITE.
    pub deadline: DeadlineQosPolicy,
    /// LatencyBudget — hint, no match effect.
    pub latency_budget: LatencyBudgetQosPolicy,
    /// Liveliness — default Automatic / INFINITE.
    pub liveliness: LivelinessQosPolicy,
    /// DestinationOrder — default ByReceptionTimestamp.
    pub destination_order: DestinationOrderQosPolicy,
    /// Ownership — default Shared.
    pub ownership: OwnershipQosPolicy,
    /// Partition — default empty.
    pub partition: PartitionQosPolicy,
    /// Presentation.
    pub presentation: PresentationQosPolicy,
    /// History — default KeepLast(1).
    pub history: HistoryQosPolicy,
    /// Resource limits.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// TimeBasedFilter — minimum_separation per instance.
    pub time_based_filter: TimeBasedFilterQosPolicy,
    /// ReaderDataLifecycle — autopurge delays.
    pub reader_data_lifecycle: ReaderDataLifecycleQosPolicy,
    /// UserData (propagated via discovery).
    pub user_data: UserDataQosPolicy,
    /// TopicData (propagated via discovery).
    pub topic_data: TopicDataQosPolicy,
    /// GroupData (propagated via discovery).
    pub group_data: GroupDataQosPolicy,
    /// DataRepresentation (XTypes 1.3 §7.6.3.1.2): the accepted
    /// representation list (`XCDR=0`, `XCDR2=2`). `None` = runtime default.
    /// Per-reader override so that a reader specifically matches XCDR1
    /// writers (ROS/RTI shapes) (`[XCDR1, XCDR2]`) without a global default.
    pub data_representation: Option<alloc::vec::Vec<i16>>,
}

impl Default for DataReaderQos {
    fn default() -> Self {
        Self {
            reliability: ReliabilityQosPolicy {
                kind: ReliabilityKind::BestEffort,
                max_blocking_time: Duration::from_millis(100_i32),
            },
            durability: DurabilityQosPolicy {
                kind: DurabilityKind::Volatile,
            },
            deadline: DeadlineQosPolicy::default(),
            latency_budget: LatencyBudgetQosPolicy::default(),
            liveliness: LivelinessQosPolicy::default(),
            destination_order: DestinationOrderQosPolicy::default(),
            ownership: OwnershipQosPolicy::default(),
            partition: PartitionQosPolicy::default(),
            presentation: PresentationQosPolicy::default(),
            history: HistoryQosPolicy {
                kind: HistoryKind::KeepLast,
                depth: 1,
            },
            resource_limits: ResourceLimitsQosPolicy {
                max_samples: 1000_i32,
                max_instances: 10_i32,
                max_samples_per_instance: 100_i32,
            },
            time_based_filter: TimeBasedFilterQosPolicy::default(),
            reader_data_lifecycle: ReaderDataLifecycleQosPolicy::default(),
            user_data: UserDataQosPolicy::default(),
            topic_data: TopicDataQosPolicy::default(),
            group_data: GroupDataQosPolicy::default(),
            data_representation: None,
        }
    }
}

/// QoS set for a `Topic` — Spec §2.2.2.3.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicQos {
    /// Default durability of the topic.
    pub durability: DurabilityQosPolicy,
    /// DurabilityService default of the topic.
    pub durability_service: DurabilityServiceQosPolicy,
    /// Default deadline.
    pub deadline: DeadlineQosPolicy,
    /// Default LatencyBudget.
    pub latency_budget: LatencyBudgetQosPolicy,
    /// Default liveliness.
    pub liveliness: LivelinessQosPolicy,
    /// Default reliability.
    pub reliability: ReliabilityQosPolicy,
    /// Default DestinationOrder.
    pub destination_order: DestinationOrderQosPolicy,
    /// Default history.
    pub history: HistoryQosPolicy,
    /// Default resource limits.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// Default TransportPriority.
    pub transport_priority: TransportPriorityQosPolicy,
    /// Default lifespan.
    pub lifespan: LifespanQosPolicy,
    /// Default ownership.
    pub ownership: OwnershipQosPolicy,
    /// TopicData (propagated via discovery).
    pub topic_data: TopicDataQosPolicy,
}

/// QoS set for the `DomainParticipant` — Spec §2.2.2.2.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainParticipantQos {
    /// UserData (propagated via discovery).
    pub user_data: UserDataQosPolicy,
    /// EntityFactory — auto-enable of child entities.
    pub entity_factory: EntityFactoryQosPolicy,
    /// Property — name/value configuration carried on the participant
    /// (DDS-Security `dds.sec.*` plugin config, incl. the security logger).
    pub property: PropertyQosPolicy,
}

/// QoS set for the `Publisher` — Spec §2.2.2.4.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublisherQos {
    /// Presentation — coherent/ordered access.
    pub presentation: PresentationQosPolicy,
    /// Partition — default empty.
    pub partition: PartitionQosPolicy,
    /// GroupData (propagated via discovery).
    pub group_data: GroupDataQosPolicy,
    /// EntityFactory — auto-enable of DataWriters.
    pub entity_factory: EntityFactoryQosPolicy,
}

/// QoS set for the `Subscriber` — Spec §2.2.2.5.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriberQos {
    /// Presentation — coherent/ordered access.
    pub presentation: PresentationQosPolicy,
    /// Partition — default empty.
    pub partition: PartitionQosPolicy,
    /// GroupData (propagated via discovery).
    pub group_data: GroupDataQosPolicy,
    /// EntityFactory — auto-enable of DataReaders.
    pub entity_factory: EntityFactoryQosPolicy,
}

/// Materializes a DDS-XML-resolved [`zerodds_qos::WriterQos`] (e.g. from
/// `zerodds_xml::QosProfileRegistry::writer_qos`) into the DCPS `DataWriterQos`.
/// All shared policies carry over; DCPS-only policies not expressible in the
/// qos-aggregate (`data_representation`) keep their default. Closes the
/// DDS-XML-1.0 QoS-profile → entity-creation path.
impl From<zerodds_qos::WriterQos> for DataWriterQos {
    fn from(w: zerodds_qos::WriterQos) -> Self {
        Self {
            durability: w.durability,
            durability_service: w.durability_service,
            deadline: w.deadline,
            latency_budget: w.latency_budget,
            liveliness: w.liveliness,
            reliability: w.reliability,
            destination_order: w.destination_order,
            history: w.history,
            resource_limits: w.resource_limits,
            transport_priority: w.transport_priority,
            lifespan: w.lifespan,
            ownership: w.ownership,
            ownership_strength: w.ownership_strength,
            presentation: w.presentation,
            partition: w.partition,
            writer_data_lifecycle: w.writer_data_lifecycle,
            user_data: w.user_data,
            topic_data: w.topic_data,
            group_data: w.group_data,
            ..Self::default()
        }
    }
}

/// Materializes a DDS-XML-resolved [`zerodds_qos::ReaderQos`] into the DCPS
/// `DataReaderQos`. See [`From<zerodds_qos::WriterQos>`] for the writer side.
impl From<zerodds_qos::ReaderQos> for DataReaderQos {
    fn from(r: zerodds_qos::ReaderQos) -> Self {
        Self {
            durability: r.durability,
            deadline: r.deadline,
            latency_budget: r.latency_budget,
            liveliness: r.liveliness,
            reliability: r.reliability,
            destination_order: r.destination_order,
            history: r.history,
            resource_limits: r.resource_limits,
            ownership: r.ownership,
            time_based_filter: r.time_based_filter,
            presentation: r.presentation,
            partition: r.partition,
            reader_data_lifecycle: r.reader_data_lifecycle,
            user_data: r.user_data,
            topic_data: r.topic_data,
            group_data: r.group_data,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod data_representation_tests {
    use super::{DataReaderQos, DataWriterQos};

    #[test]
    fn data_representation_defaults_none_and_settable() {
        // B5: per-endpoint DataRepresentation override (XTypes §7.6.3.1.2).
        // Default None = runtime default; settable for a targeted XCDR1 offer.
        assert_eq!(DataWriterQos::default().data_representation, None);
        assert_eq!(DataReaderQos::default().data_representation, None);
        let r = DataReaderQos {
            data_representation: Some(alloc::vec![0_i16, 2]), // [XCDR1, XCDR2]
            ..Default::default()
        };
        assert_eq!(r.data_representation.as_deref(), Some(&[0_i16, 2][..]));
        let w = DataWriterQos {
            data_representation: Some(alloc::vec![0_i16]), // XCDR1-only
            ..Default::default()
        };
        assert_eq!(w.data_representation.as_deref(), Some(&[0_i16][..]));
    }

    #[test]
    fn from_qos_aggregate_carries_policies() {
        use zerodds_qos::{HistoryKind, ReliabilityKind};
        let mut w = zerodds_qos::WriterQos::default();
        w.history.kind = HistoryKind::KeepLast;
        w.history.depth = 42;
        w.reliability.kind = ReliabilityKind::Reliable;
        let dw: DataWriterQos = w.into();
        assert_eq!(dw.history.depth, 42);
        assert_eq!(dw.reliability.kind, ReliabilityKind::Reliable);
        assert_eq!(dw.data_representation, None); // DCPS-only → default

        let mut rq = zerodds_qos::ReaderQos::default();
        rq.history.depth = 7;
        let dr: DataReaderQos = rq.into();
        assert_eq!(dr.history.depth, 7);
        assert_eq!(dr.data_representation, None);
    }
}
