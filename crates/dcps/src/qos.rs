// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPS-QoS — komplette 22-Policy-Sets fuer DataWriter/DataReader/Topic/
//! Publisher/Subscriber/DomainParticipant gemaess DDS 1.4 §2.2.3.
//!
//! Re-Exports der Policy-Datentypen aus dem `zerodds_qos`-Crate; die hier
//! definierten Aggregate sind die DCPS-API-Sicht (was DataReader/Writer
//! intern halten und worauf der Runtime-Pfad zugreift).

pub use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy, DurabilityKind,
    DurabilityQosPolicy, DurabilityServiceQosPolicy, EntityFactoryQosPolicy, GroupDataQosPolicy,
    HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy, LivelinessKind,
    LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy, OwnershipStrengthQosPolicy,
    PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy,
    ReaderDataLifecycleQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
    TimeBasedFilterQosPolicy, TopicDataQosPolicy, TransportPriorityQosPolicy, UserDataQosPolicy,
    WriterDataLifecycleQosPolicy,
};

use zerodds_qos::Duration;

/// QoS-Set fuer einen `DataWriter` — Spec §2.2.2.4.2 (alle Policies, die
/// am DataWriter setzbar sind).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataWriterQos {
    /// Reliability — Default Reliable.
    pub reliability: ReliabilityQosPolicy,
    /// Durability — Default Volatile.
    pub durability: DurabilityQosPolicy,
    /// Konfiguriert den Persistence-Service (Spec §2.2.3.5).
    pub durability_service: DurabilityServiceQosPolicy,
    /// Deadline — Default INFINITE.
    pub deadline: DeadlineQosPolicy,
    /// LatencyBudget — Hint, kein Match-Effekt (Spec §2.2.3.10).
    pub latency_budget: LatencyBudgetQosPolicy,
    /// Liveliness — Default Automatic / INFINITE.
    pub liveliness: LivelinessQosPolicy,
    /// DestinationOrder — Default ByReceptionTimestamp.
    pub destination_order: DestinationOrderQosPolicy,
    /// Lifespan — Default INFINITE.
    pub lifespan: LifespanQosPolicy,
    /// Ownership — Default Shared.
    pub ownership: OwnershipQosPolicy,
    /// Ownership-Strength — nur bei Exclusive.
    pub ownership_strength: OwnershipStrengthQosPolicy,
    /// Partition — Default leer (matched only ""-Default-Partition).
    pub partition: PartitionQosPolicy,
    /// Presentation — Default Instance/false/false.
    pub presentation: PresentationQosPolicy,
    /// History — Default KeepLast(1).
    pub history: HistoryQosPolicy,
    /// Resource-Limits.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// TransportPriority — Hint, kein Match-Effekt (Spec §2.2.3.15).
    pub transport_priority: TransportPriorityQosPolicy,
    /// WriterDataLifecycle — autodispose_unregistered_instances.
    pub writer_data_lifecycle: WriterDataLifecycleQosPolicy,
    /// UserData — opaque `sequence<octet>`, ueber Discovery propagiert.
    pub user_data: UserDataQosPolicy,
    /// TopicData — opaque `sequence<octet>`, ueber Discovery propagiert.
    pub topic_data: TopicDataQosPolicy,
    /// GroupData — opaque `sequence<octet>`, ueber Discovery propagiert.
    pub group_data: GroupDataQosPolicy,
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
        }
    }
}

/// QoS-Set fuer einen `DataReader` — Spec §2.2.2.5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataReaderQos {
    /// Reliability — Reader-Default BestEffort (Spec §2.2.3.14.3).
    pub reliability: ReliabilityQosPolicy,
    /// Durability — Default Volatile.
    pub durability: DurabilityQosPolicy,
    /// Deadline — Default INFINITE.
    pub deadline: DeadlineQosPolicy,
    /// LatencyBudget — Hint, kein Match-Effekt.
    pub latency_budget: LatencyBudgetQosPolicy,
    /// Liveliness — Default Automatic / INFINITE.
    pub liveliness: LivelinessQosPolicy,
    /// DestinationOrder — Default ByReceptionTimestamp.
    pub destination_order: DestinationOrderQosPolicy,
    /// Ownership — Default Shared.
    pub ownership: OwnershipQosPolicy,
    /// Partition — Default leer.
    pub partition: PartitionQosPolicy,
    /// Presentation.
    pub presentation: PresentationQosPolicy,
    /// History — Default KeepLast(1).
    pub history: HistoryQosPolicy,
    /// Resource-Limits.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// TimeBasedFilter — minimum_separation pro Instanz.
    pub time_based_filter: TimeBasedFilterQosPolicy,
    /// ReaderDataLifecycle — autopurge-Delays.
    pub reader_data_lifecycle: ReaderDataLifecycleQosPolicy,
    /// UserData (Discovery-propagiert).
    pub user_data: UserDataQosPolicy,
    /// TopicData (Discovery-propagiert).
    pub topic_data: TopicDataQosPolicy,
    /// GroupData (Discovery-propagiert).
    pub group_data: GroupDataQosPolicy,
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
        }
    }
}

/// QoS-Set fuer einen `Topic` — Spec §2.2.2.3.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TopicQos {
    /// Default-Durability des Topics.
    pub durability: DurabilityQosPolicy,
    /// DurabilityService-Default des Topics.
    pub durability_service: DurabilityServiceQosPolicy,
    /// Default-Deadline.
    pub deadline: DeadlineQosPolicy,
    /// Default-LatencyBudget.
    pub latency_budget: LatencyBudgetQosPolicy,
    /// Default-Liveliness.
    pub liveliness: LivelinessQosPolicy,
    /// Default-Reliability.
    pub reliability: ReliabilityQosPolicy,
    /// Default-DestinationOrder.
    pub destination_order: DestinationOrderQosPolicy,
    /// Default-History.
    pub history: HistoryQosPolicy,
    /// Default-Resource-Limits.
    pub resource_limits: ResourceLimitsQosPolicy,
    /// Default-TransportPriority.
    pub transport_priority: TransportPriorityQosPolicy,
    /// Default-Lifespan.
    pub lifespan: LifespanQosPolicy,
    /// Default-Ownership.
    pub ownership: OwnershipQosPolicy,
    /// TopicData (Discovery-propagiert).
    pub topic_data: TopicDataQosPolicy,
}

/// QoS-Set fuer den `DomainParticipant` — Spec §2.2.2.2.2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainParticipantQos {
    /// UserData (Discovery-propagiert).
    pub user_data: UserDataQosPolicy,
    /// EntityFactory — Auto-Enable von Child-Entities.
    pub entity_factory: EntityFactoryQosPolicy,
}

/// QoS-Set fuer den `Publisher` — Spec §2.2.2.4.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublisherQos {
    /// Presentation — Coherent/Ordered Access.
    pub presentation: PresentationQosPolicy,
    /// Partition — Default leer.
    pub partition: PartitionQosPolicy,
    /// GroupData (Discovery-propagiert).
    pub group_data: GroupDataQosPolicy,
    /// EntityFactory — Auto-Enable von DataWritern.
    pub entity_factory: EntityFactoryQosPolicy,
}

/// QoS-Set fuer den `Subscriber` — Spec §2.2.2.5.1.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriberQos {
    /// Presentation — Coherent/Ordered Access.
    pub presentation: PresentationQosPolicy,
    /// Partition — Default leer.
    pub partition: PartitionQosPolicy,
    /// GroupData (Discovery-propagiert).
    pub group_data: GroupDataQosPolicy,
    /// EntityFactory — Auto-Enable von DataReadern.
    pub entity_factory: EntityFactoryQosPolicy,
}
