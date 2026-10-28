// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DCPS QoS — complete 22-policy sets for DataWriter/DataReader/Topic/
//! Publisher/Subscriber/DomainParticipant per DDS 1.4 §2.2.3.
//!
//! Re-exports the policy data types from the `zerodds_qos` crate; the
//! aggregates defined here are the DCPS-API view (what DataReader/Writer
//! hold internally and what the runtime path accesses).

pub use zerodds_qos::{
    DataTagQosPolicy, DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy,
    DurabilityKind, DurabilityQosPolicy, DurabilityServiceQosPolicy, EntityFactoryQosPolicy,
    GroupDataQosPolicy, HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy,
    LivelinessKind, LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy,
    OwnershipStrengthQosPolicy, PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy,
    PropertyQosPolicy, ReaderDataLifecycleQosPolicy, ReliabilityKind, ReliabilityQosPolicy,
    ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy, TopicDataQosPolicy,
    TransportPriorityQosPolicy, UserDataQosPolicy, WriterDataLifecycleQosPolicy,
};
pub use zerodds_qos::{QosPolicy, policy_name};

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
    /// LatencyBudget — RxO-matched (Spec §2.2.3.10.4): the runtime matcher
    /// rejects a pairing where offered.duration > requested.duration
    /// (OFFERED_INCOMPATIBLE_QOS). See
    /// `zerodds_qos::LatencyBudgetQosPolicy::is_compatible_with`.
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
    /// Property (DDS-Security 1.2 §7.3.21): name/value configuration of this
    /// writer. `dds.sec.crypto.symmetric_cipher_algorithm` (§10.5.1.1)
    /// sets the key width of the writer's key material; "AUTO" or none
    /// leaves the default of the DomainParticipant. The entries with
    /// `propagate = TRUE` go out in the `PublicationBuiltinTopicData`.
    /// Immutable once the writer is enabled.
    pub property: PropertyQosPolicy,
    /// DataTags (DDS-Security 1.2 §7.3.21, §9.7.2): the tags of this endpoint,
    /// announced as `PID_DATA_TAGS` and immutable.
    pub data_tags: DataTagQosPolicy,
}

/// DDS 1.4 §2.2.3.18 and §2.2.3.19: the HISTORY depth and the
/// RESOURCE_LIMITS of an entity must agree — "depth <=
/// max_samples_per_instance" and "max_samples >= max_samples_per_instance"
/// (LENGTH_UNLIMITED agrees with everything). A KEEP_LAST history keeps at
/// least one sample.
fn check_history_and_limits(
    history: &HistoryQosPolicy,
    limits: &ResourceLimitsQosPolicy,
) -> crate::error::Result<()> {
    if !limits.is_consistent() {
        return Err(crate::error::DdsError::InconsistentPolicy {
            what: "RESOURCE_LIMITS max_samples below max_samples_per_instance",
        });
    }
    if history.kind == zerodds_qos::HistoryKind::KeepLast {
        if history.depth <= 0 {
            return Err(crate::error::DdsError::InconsistentPolicy {
                what: "HISTORY KEEP_LAST with a depth below 1",
            });
        }
        if limits.max_samples_per_instance >= 0 && history.depth > limits.max_samples_per_instance {
            return Err(crate::error::DdsError::InconsistentPolicy {
                what: "HISTORY depth above RESOURCE_LIMITS max_samples_per_instance",
            });
        }
    }
    Ok(())
}

impl DataWriterQos {
    /// Whether the policies agree with each other, as `create_datawriter`
    /// and `set_qos` require (DDS 1.4 §2.2.3: "If the resulting QoS is
    /// inconsistent, the change of QoS operation fails").
    ///
    /// # Errors
    /// `InconsistentPolicy` naming the rule that fails.
    pub fn check_consistency(&self) -> crate::error::Result<()> {
        check_history_and_limits(&self.history, &self.resource_limits)
    }
}

impl DataReaderQos {
    /// [`DataWriterQos::check_consistency`] for a reader, and DDS 1.4
    /// §2.2.3.12: "period >= minimum_separation" of DEADLINE and
    /// TIME_BASED_FILTER.
    ///
    /// # Errors
    /// `InconsistentPolicy` naming the rule that fails.
    pub fn check_consistency(&self) -> crate::error::Result<()> {
        check_history_and_limits(&self.history, &self.resource_limits)?;
        if self.time_based_filter.minimum_separation > self.deadline.period {
            return Err(crate::error::DdsError::InconsistentPolicy {
                what: "TIME_BASED_FILTER minimum_separation above DEADLINE period",
            });
        }
        Ok(())
    }
}

impl TopicQos {
    /// [`DataWriterQos::check_consistency`] for a topic's HISTORY and
    /// RESOURCE_LIMITS.
    ///
    /// # Errors
    /// `InconsistentPolicy` naming the rule that fails.
    pub fn check_consistency(&self) -> crate::error::Result<()> {
        check_history_and_limits(&self.history, &self.resource_limits)
    }
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
            // §2.2.3.19: every limit defaults to LENGTH_UNLIMITED.
            resource_limits: ResourceLimitsQosPolicy::default(),
            transport_priority: TransportPriorityQosPolicy::default(),
            writer_data_lifecycle: WriterDataLifecycleQosPolicy::default(),
            user_data: UserDataQosPolicy::default(),
            topic_data: TopicDataQosPolicy::default(),
            group_data: GroupDataQosPolicy::default(),
            data_representation: None,
            property: PropertyQosPolicy::default(),
            data_tags: DataTagQosPolicy::default(),
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
    /// LatencyBudget — RxO-matched (Spec §2.2.3.10.4): rejected when
    /// offered.duration > requested.duration (REQUESTED_INCOMPATIBLE_QOS).
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
    /// Property (DDS-Security 1.2 §7.3.21): name/value configuration of this
    /// reader. `dds.sec.crypto.symmetric_cipher_algorithm` (§10.5.1.1)
    /// sets the key width of the reader's key material; "AUTO" or none
    /// leaves the default of the DomainParticipant. The entries with
    /// `propagate = TRUE` go out in the `SubscriptionBuiltinTopicData`.
    /// Immutable once the reader is enabled.
    pub property: PropertyQosPolicy,
    /// DataTags (DDS-Security 1.2 §7.3.21, §9.7.2): the tags of this endpoint,
    /// announced as `PID_DATA_TAGS` and immutable.
    pub data_tags: DataTagQosPolicy,
    /// TypeConsistencyEnforcement (XTypes 1.3 §7.6.3.4):
    /// controls how strictly this reader validates a remote writer's type
    /// against its own at match time. Spec default for a DataReader is
    /// `ALLOW_TYPE_COERCION`. Set `kind = DisallowTypeCoercion` or
    /// `force_type_validation = true` to reject any structurally
    /// non-identical / non-assignable writer at match time
    /// (`REQUESTED_INCOMPATIBLE_QOS`, TYPE_CONSISTENCY_ENFORCEMENT policy)
    /// instead of matching and failing to decode. Authoritative on the
    /// reader side (`wire_reader_to_remote_writer`).
    pub type_consistency: zerodds_types::qos::TypeConsistencyEnforcement,
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
            // §2.2.3.19: every limit defaults to LENGTH_UNLIMITED.
            resource_limits: ResourceLimitsQosPolicy::default(),
            time_based_filter: TimeBasedFilterQosPolicy::default(),
            reader_data_lifecycle: ReaderDataLifecycleQosPolicy::default(),
            user_data: UserDataQosPolicy::default(),
            topic_data: TopicDataQosPolicy::default(),
            group_data: GroupDataQosPolicy::default(),
            data_representation: None,
            property: PropertyQosPolicy::default(),
            data_tags: DataTagQosPolicy::default(),
            // Spec default for a DataReader: ALLOW_TYPE_COERCION
            // (XTypes 1.3 §7.6.3.4.4).
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        }
    }
}

/// The first policy `next` changes against `current` that an enabled
/// DataWriter may not change (DDS 1.4 §2.2.3, the "Changeable" column;
/// XTypes 1.3 §7.6.3.1.2 for DATA_REPRESENTATION). The Rust and the C
/// `set_qos` refuse it with IMMUTABLE_POLICY.
#[must_use]
pub fn immutable_writer_change(
    current: &DataWriterQos,
    next: &DataWriterQos,
) -> Option<&'static str> {
    [
        (current.durability != next.durability, "DURABILITY"),
        (current.reliability != next.reliability, "RELIABILITY"),
        (current.history != next.history, "HISTORY"),
        (
            current.resource_limits != next.resource_limits,
            "RESOURCE_LIMITS",
        ),
        (current.ownership != next.ownership, "OWNERSHIP"),
        (current.liveliness != next.liveliness, "LIVELINESS"),
        (
            current.data_representation != next.data_representation,
            "DATA_REPRESENTATION",
        ),
        (
            current.durability_service != next.durability_service,
            "DURABILITY_SERVICE",
        ),
        (
            current.destination_order != next.destination_order,
            "DESTINATION_ORDER",
        ),
        (current.presentation != next.presentation, "PRESENTATION"),
        (current.property != next.property, "PROPERTY"),
        (current.data_tags != next.data_tags, "DATA_TAGS"),
    ]
    .into_iter()
    .find_map(|(changed, policy)| changed.then_some(policy))
}

/// The reader half of [`immutable_writer_change`]; TYPE_CONSISTENCY_ENFORCEMENT
/// is not changeable either (XTypes 1.3 §7.6.3.4.4).
#[must_use]
pub fn immutable_reader_change(
    current: &DataReaderQos,
    next: &DataReaderQos,
) -> Option<&'static str> {
    [
        (current.durability != next.durability, "DURABILITY"),
        (current.reliability != next.reliability, "RELIABILITY"),
        (current.history != next.history, "HISTORY"),
        (
            current.resource_limits != next.resource_limits,
            "RESOURCE_LIMITS",
        ),
        (current.ownership != next.ownership, "OWNERSHIP"),
        (current.liveliness != next.liveliness, "LIVELINESS"),
        (
            current.destination_order != next.destination_order,
            "DESTINATION_ORDER",
        ),
        (current.presentation != next.presentation, "PRESENTATION"),
        (
            current.data_representation != next.data_representation,
            "DATA_REPRESENTATION",
        ),
        (
            current.type_consistency != next.type_consistency,
            "TYPE_CONSISTENCY_ENFORCEMENT",
        ),
        (current.property != next.property, "PROPERTY"),
        (current.data_tags != next.data_tags, "DATA_TAGS"),
    ]
    .into_iter()
    .find_map(|(changed, policy)| changed.then_some(policy))
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
    /// DataRepresentation (XTypes 1.3 §7.6.3.1.1): "When representations
    /// are specified in the TopicQos, the first element of the sequence
    /// applies to writers of the Topic, and the whole sequence applies to
    /// readers of the Topic." `None` is the default, an empty list of
    /// preferences, which is equivalent to the single element `XCDR`.
    /// Not changeable once the topic is enabled.
    pub data_representation: Option<alloc::vec::Vec<i16>>,
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

/// The `PropertyQosPolicy` of an endpoint as it goes out in its
/// `PublicationBuiltinTopicData`/`SubscriptionBuiltinTopicData`
/// (`PID_PROPERTY_LIST`): only the entries with `propagate = TRUE`
/// (DDS-Security 1.2 §7.3.21).
#[must_use]
pub fn propagated_properties(
    property: &PropertyQosPolicy,
) -> zerodds_rtps::property_list::WirePropertyList {
    use zerodds_rtps::property_list::{WireBinaryProperty, WireProperty, WirePropertyList};
    let mut list = WirePropertyList::new();
    for (name, value) in property.propagated() {
        list.push(WireProperty::new(name, value));
    }
    for (name, value) in property.propagated_binary() {
        list = list.with_binary(WireBinaryProperty::new(name, value));
    }
    list
}

/// The `DataTagQosPolicy` of an endpoint as it goes out in its discovery data
/// (`PID_DATA_TAGS`, DDS-Security 1.2 §7.5.1.7, §7.5.1.8).
#[must_use]
pub fn announced_data_tags(tags: &DataTagQosPolicy) -> zerodds_rtps::property_list::WireDataTags {
    zerodds_rtps::property_list::WireDataTags {
        tags: tags
            .tags
            .iter()
            .map(|t| {
                zerodds_rtps::property_list::WireProperty::new(t.name.as_str(), t.value.as_str())
            })
            .collect(),
    }
}

/// The inverse of [`announced_data_tags`]: the `DataTagQosPolicy` an endpoint's
/// discovery data carried (`PID_DATA_TAGS`).
#[must_use]
pub fn data_tags_from_announced(
    tags: &zerodds_rtps::property_list::WireDataTags,
) -> DataTagQosPolicy {
    DataTagQosPolicy {
        tags: tags
            .tags
            .iter()
            .map(|t| zerodds_qos::Tag::new(t.name.as_str(), t.value.as_str()))
            .collect(),
    }
}

// ============================================================================
// The *_QOS_DEFAULT and *_QOS_USE_TOPIC_QOS placeholders (DDS 1.4 §2.3.3)
// ============================================================================

/// The QoS argument of `create_participant`, `create_topic`,
/// `create_publisher`, `create_subscriber`, of every `set_qos` and of every
/// `set_default_*_qos`: a QoS, or the placeholder `*_QOS_DEFAULT` (DDS 1.4
/// §2.3.3, the constants in [`crate::psm_constants`]). A QoS converts into
/// it, so a call with a QoS value reads as before.
///
/// What `Default` means depends on the operation (DDS 1.4 §2.2.2.1.1.1,
/// §2.2.2.2.1.1, §2.2.2.2.1.20): `create_*` and `set_qos` take the default
/// the entity's factory holds at the call; `set_default_*_qos` goes back to
/// "the values that would be used if the set_default_… operation had never
/// been called".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QosArg<Q> {
    /// This QoS.
    Value(Q),
    /// `*_QOS_DEFAULT`.
    Default,
}

impl<Q> From<Q> for QosArg<Q> {
    fn from(qos: Q) -> Self {
        Self::Value(qos)
    }
}

impl<Q> QosArg<Q> {
    /// The QoS, with `default` standing for [`QosArg::Default`].
    pub fn resolve(self, default: impl FnOnce() -> Q) -> Q {
        match self {
            Self::Value(qos) => qos,
            Self::Default => default(),
        }
    }

    /// [`QosArg::resolve`] with a `default` that can fail.
    ///
    /// # Errors
    /// The error of `default`.
    pub fn try_resolve(
        self,
        default: impl FnOnce() -> crate::error::Result<Q>,
    ) -> crate::error::Result<Q> {
        match self {
            Self::Value(qos) => Ok(qos),
            Self::Default => default(),
        }
    }
}

/// The QoS argument of `create_datawriter` and `create_datareader`:
/// [`QosArg`] and `DATAWRITER_QOS_USE_TOPIC_QOS` /
/// `DATAREADER_QOS_USE_TOPIC_QOS` (DDS 1.4 §2.2.2.4.1.5, §2.2.2.5.2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointQosArg<Q> {
    /// This QoS.
    Value(Q),
    /// `DATAWRITER_QOS_DEFAULT` / `DATAREADER_QOS_DEFAULT`: the default the
    /// Publisher or Subscriber holds at the call.
    Default,
    /// `*_QOS_USE_TOPIC_QOS`: "a combination of the default … QoS and the
    /// Topic QoS … whereby any policy that is set on the Topic QoS
    /// 'overrides' the corresponding policy on the default QoS"
    /// (`copy_from_topic_qos`).
    UseTopicQos,
}

impl<Q> From<Q> for EndpointQosArg<Q> {
    fn from(qos: Q) -> Self {
        Self::Value(qos)
    }
}

impl From<QosArg<DataWriterQos>> for EndpointQosArg<DataWriterQos> {
    fn from(arg: QosArg<DataWriterQos>) -> Self {
        match arg {
            QosArg::Value(qos) => Self::Value(qos),
            QosArg::Default => Self::Default,
        }
    }
}

impl From<QosArg<DataReaderQos>> for EndpointQosArg<DataReaderQos> {
    fn from(arg: QosArg<DataReaderQos>) -> Self {
        match arg {
            QosArg::Value(qos) => Self::Value(qos),
            QosArg::Default => Self::Default,
        }
    }
}

// ============================================================================
// Publisher/Subscriber-level policies on an endpoint announcement
// ============================================================================

/// Resolves the PRESENTATION value a DataWriter/DataReader announces in SEDP.
///
/// DDS 1.4 §2.2.3.6 puts PRESENTATION on the **Publisher/Subscriber**, not on
/// the endpoint: "the values of PRESENTATION access_scope are considered
/// ordered such that INSTANCE < TOPIC < GROUP", and the offered/requested
/// comparison runs when a DataWriter and a DataReader are matched. The value
/// that comparison sees travels in `PublicationBuiltinTopicData` /
/// `SubscriptionBuiltinTopicData` (DDSI-RTPS 2.5 §8.5.3.2/§8.5.3.3, PID
/// 0x0021), so the endpoint has to announce its group's value — otherwise a
/// `Publisher` created with `access_scope = INSTANCE` and a `Subscriber`
/// created with `access_scope = TOPIC` both announce the default, compare
/// equal, and match where §2.2.3.6 requires `REQUESTED_INCOMPATIBLE_QOS`.
///
/// ZeroDDS additionally carries the policy on `DataWriterQos`/`DataReaderQos`
/// as a per-endpoint override (the spec has no such field). A value left at
/// its default therefore inherits the group's; an explicitly set one wins.
#[must_use]
pub fn effective_presentation(
    group: PresentationQosPolicy,
    endpoint: PresentationQosPolicy,
) -> PresentationQosPolicy {
    if endpoint == PresentationQosPolicy::default() {
        group
    } else {
        endpoint
    }
}

/// Resolves the PARTITION names a DataWriter/DataReader announces in SEDP.
///
/// Same group-level origin as [`effective_presentation`] — DDS 1.4 §2.2.3.13
/// defines PARTITION on the Publisher/Subscriber. An empty endpoint list is
/// the DDS default (the single `""` partition) and inherits the group's.
#[must_use]
pub fn effective_partition(
    group: &PartitionQosPolicy,
    endpoint: &PartitionQosPolicy,
) -> alloc::vec::Vec<alloc::string::String> {
    if endpoint.names.is_empty() {
        group.names.clone()
    } else {
        endpoint.names.clone()
    }
}

/// Resolves the GROUP_DATA octets a DataWriter/DataReader announces in SEDP.
///
/// DDS 1.4 §2.2.3.3 defines GROUP_DATA on the Publisher/Subscriber and has it
/// propagated with the endpoint's builtin-topic data. An empty endpoint value
/// inherits the group's.
#[must_use]
pub fn effective_group_data(
    group: &GroupDataQosPolicy,
    endpoint: &GroupDataQosPolicy,
) -> alloc::vec::Vec<u8> {
    if endpoint.value.is_empty() {
        group.value.clone()
    } else {
        endpoint.value.clone()
    }
}

#[cfg(test)]
mod group_policy_tests {
    use super::{
        GroupDataQosPolicy, PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy,
        effective_group_data, effective_partition, effective_presentation,
    };
    use alloc::string::ToString;

    fn topic_scope() -> PresentationQosPolicy {
        PresentationQosPolicy {
            access_scope: PresentationAccessScope::Topic,
            coherent_access: false,
            ordered_access: true,
        }
    }

    /// DDS 1.4 §2.2.3.6: the policy lives on the Publisher/Subscriber, so an
    /// endpoint that never set it announces the group's value.
    #[test]
    fn endpoint_default_presentation_inherits_the_group_value() {
        assert_eq!(
            effective_presentation(topic_scope(), PresentationQosPolicy::default()),
            topic_scope()
        );
    }

    /// The ZeroDDS-only per-endpoint field stays an override.
    #[test]
    fn endpoint_presentation_overrides_the_group_value() {
        let group = PresentationQosPolicy {
            access_scope: PresentationAccessScope::Group,
            coherent_access: true,
            ordered_access: true,
        };
        assert_eq!(effective_presentation(group, topic_scope()), topic_scope());
    }

    #[test]
    fn empty_endpoint_partition_inherits_the_group_names() {
        let group = PartitionQosPolicy {
            names: alloc::vec!["A".to_string()],
        };
        assert_eq!(
            effective_partition(&group, &PartitionQosPolicy::default()),
            alloc::vec!["A".to_string()]
        );
        let endpoint = PartitionQosPolicy {
            names: alloc::vec!["B".to_string()],
        };
        assert_eq!(
            effective_partition(&group, &endpoint),
            alloc::vec!["B".to_string()]
        );
    }

    #[test]
    fn empty_endpoint_group_data_inherits_the_group_octets() {
        let group = GroupDataQosPolicy {
            value: alloc::vec![7, 8],
        };
        assert_eq!(
            effective_group_data(&group, &GroupDataQosPolicy::default()),
            alloc::vec![7u8, 8]
        );
    }
}

/// The list a `DataRepresentationQosPolicy` stands for on the wire and in
/// matching.
///
/// DDS-XTypes 1.3 §7.6.3.1.1: "The default value of the
/// DataRepresentationQosPolicy shall be an empty list of preferences. An
/// empty list of preferences shall be taken to be equivalent to a list
/// containing the single element XCDR." `None` is the policy left at its
/// default and stays `None`, which the runtime resolves to its own offer;
/// an explicitly empty list is the one element `XCDR` (`0`).
#[must_use]
pub(crate) fn effective_data_representation(
    policy: Option<&[i16]>,
) -> Option<alloc::vec::Vec<i16>> {
    policy.map(|list| {
        if list.is_empty() {
            alloc::vec![zerodds_rtps::publication_data::data_representation::XCDR]
        } else {
            list.to_vec()
        }
    })
}

/// The representation a writer of a topic takes from the topic's
/// DataRepresentation: "the first element of the sequence applies to
/// writers of the Topic" (XTypes 1.3 §7.6.3.1.1), an empty list being the
/// single element `XCDR`.
#[must_use]
pub(crate) fn writer_representation_of_topic(
    policy: Option<&[i16]>,
) -> Option<alloc::vec::Vec<i16>> {
    effective_data_representation(policy)
        .and_then(|list| list.first().copied())
        .map(|first| alloc::vec![first])
}

#[cfg(test)]
mod data_representation_tests {
    use super::{DataReaderQos, DataWriterQos};

    /// DDS-Security 1.2 §7.3.21: `property` is a member of both Qos and,
    /// once the entity is enabled, no longer changeable (the key material
    /// of the endpoint is made from it at creation).
    #[test]
    fn property_is_a_member_and_immutable_after_enable() {
        use super::{immutable_reader_change, immutable_writer_change};
        assert!(DataWriterQos::default().property.is_empty());
        assert!(DataReaderQos::default().property.is_empty());
        let mut w = DataWriterQos::default();
        w.property
            .set("dds.sec.crypto.symmetric_cipher_algorithm", "AES128+GCM");
        assert_eq!(
            immutable_writer_change(&DataWriterQos::default(), &w),
            Some("PROPERTY")
        );
        let mut r = DataReaderQos::default();
        r.property
            .set("dds.sec.crypto.symmetric_cipher_algorithm", "AES128+GCM");
        assert_eq!(
            immutable_reader_change(&DataReaderQos::default(), &r),
            Some("PROPERTY")
        );
        // §9.7.2: the DataTags are immutable as well.
        let tagged = DataWriterQos {
            data_tags: super::DataTagQosPolicy::new().with("level", "secret"),
            ..DataWriterQos::default()
        };
        assert_eq!(
            immutable_writer_change(&DataWriterQos::default(), &tagged),
            Some("DATA_TAGS")
        );
        let tagged = DataReaderQos {
            data_tags: super::DataTagQosPolicy::new().with("level", "secret"),
            ..DataReaderQos::default()
        };
        assert_eq!(
            immutable_reader_change(&DataReaderQos::default(), &tagged),
            Some("DATA_TAGS")
        );
    }

    /// §2.2.3.19: RESOURCE_LIMITS defaults to LENGTH_UNLIMITED on every
    /// member. The DCPS defaults were 1000 / 10 / 100, which the reader
    /// (and the C API writer) enforce: a default reader refused a reliable
    /// instance's 101st not-yet-taken sample and an 11th instance.
    #[test]
    fn resource_limits_default_to_unlimited() {
        // LENGTH_UNLIMITED is -1 (IDL4).
        let unlimited = zerodds_qos::ResourceLimitsQosPolicy {
            max_samples: -1,
            max_instances: -1,
            max_samples_per_instance: -1,
        };
        assert_eq!(DataWriterQos::default().resource_limits, unlimited);
        assert_eq!(DataReaderQos::default().resource_limits, unlimited);
    }

    /// DDS-XTypes 1.3 §7.6.3.1.1: an empty list of preferences is the
    /// single element XCDR; an absent policy stays the runtime's to resolve.
    #[test]
    fn an_empty_list_of_preferences_is_the_single_element_xcdr() {
        use super::effective_data_representation as effective;
        assert_eq!(effective(None), None);
        assert_eq!(effective(Some(&[])), Some(alloc::vec![0_i16]));
        assert_eq!(effective(Some(&[2, 0])), Some(alloc::vec![2_i16, 0]));
    }

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
