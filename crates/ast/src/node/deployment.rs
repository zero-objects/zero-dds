// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Deployment nodes. Spec §3.7.
//!
//! A QoS profile holds typed policy values, not XML text. Every field a
//! DDS-XML document can leave unset is `Option`, so "not written" stays
//! distinguishable from "written as the default" — a distinction a
//! profile that inherits from another one depends on.

use crate::facet::Facets;
use crate::node::AnyId;
use crate::node::type_::TypeNode;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast_core::{Id, NodeId, Ref};

/// A deployment library: the container DDS-XML declares profiles,
/// domains and participants in. Spec §3.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Library name.
    pub name: String,
    /// Everything declared inside, in declaration order.
    pub children: Vec<AnyId>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// A DDS domain. Spec §3.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Domain name.
    pub name: String,
    /// Numeric domain id.
    pub domain_id: u32,
    /// Topics declared in this domain.
    pub topics: Vec<Id<TopicNode>>,
    /// Enclosing library.
    pub parent: Id<LibraryNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// A topic. Spec §3.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Topic name as it appears on the wire.
    pub name: String,
    /// The type published on it.
    pub type_: Ref<TypeNode>,
    /// QoS profile, if one is bound.
    pub qos: Option<Ref<QosProfileNode>>,
    /// Enclosing domain.
    pub parent: Id<DomainNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// A QoS profile. Spec §3.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QosProfileNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Profile name.
    pub name: String,
    /// Profile this one inherits from.
    pub base: Option<Ref<QosProfileNode>>,
    /// Policy values, in declaration order.
    pub policies: Vec<QosPolicy>,
    /// Enclosing library.
    pub parent: Id<LibraryNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// A domain participant. Spec §3.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Participant name.
    pub name: String,
    /// Domain it joins.
    pub domain: Ref<DomainNode>,
    /// QoS profile, if one is bound.
    pub qos: Option<Ref<QosProfileNode>>,
    /// Endpoints it owns.
    pub endpoints: Vec<Id<EndpointNode>>,
    /// Enclosing library.
    pub parent: Id<LibraryNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// Whether an endpoint publishes or subscribes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndpointRole {
    /// Publishing side.
    DataWriter,
    /// Subscribing side.
    DataReader,
}

impl EndpointRole {
    /// Both roles.
    pub const ALL: [Self; 2] = [Self::DataWriter, Self::DataReader];
}

/// A reader or writer endpoint. Spec §3.7.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointNode {
    /// Stable identity.
    pub node_id: NodeId,
    /// Endpoint name.
    pub name: String,
    /// Which side.
    pub role: EndpointRole,
    /// Topic it is bound to.
    pub topic: Ref<TopicNode>,
    /// QoS profile, if one is bound.
    pub qos: Option<Ref<QosProfileNode>>,
    /// Owning participant.
    pub parent: Id<ParticipantNode>,
    /// Per-source information. Spec §4.
    pub facets: Facets,
}

/// A duration, as DDS-DCPS 1.4 §2.2.1.1 defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Duration {
    /// Whole seconds.
    pub sec: i32,
    /// Additional nanoseconds.
    pub nanosec: u32,
}

impl Duration {
    /// `DURATION_INFINITE`, DDS-DCPS 1.4 §2.2.3.
    pub const INFINITE: Self = Self {
        sec: 0x7fff_ffff,
        nanosec: 0xffff_ffff,
    };

    /// Zero duration.
    pub const ZERO: Self = Self { sec: 0, nanosec: 0 };
}

/// Which policy a [`QosPolicy`] carries.
///
/// Kept separate from the payload so a test can assert the set is
/// complete against DDS-DCPS 1.4 §2.2.3 without constructing payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QosPolicyKind {
    /// §2.2.3.4
    UserData,
    /// §2.2.3.5
    TopicData,
    /// §2.2.3.6
    GroupData,
    /// §2.2.3.7
    Durability,
    /// §2.2.3.8
    DurabilityService,
    /// §2.2.3.9
    Presentation,
    /// §2.2.3.10
    Deadline,
    /// §2.2.3.11
    LatencyBudget,
    /// §2.2.3.12
    Ownership,
    /// §2.2.3.13
    OwnershipStrength,
    /// §2.2.3.14
    Liveliness,
    /// §2.2.3.15
    TimeBasedFilter,
    /// §2.2.3.16
    Partition,
    /// §2.2.3.17
    Reliability,
    /// §2.2.3.18
    TransportPriority,
    /// §2.2.3.19
    Lifespan,
    /// §2.2.3.20
    DestinationOrder,
    /// §2.2.3.21
    History,
    /// §2.2.3.22
    ResourceLimits,
    /// §2.2.3.23
    EntityFactory,
    /// §2.2.3.24
    WriterDataLifecycle,
    /// §2.2.3.25
    ReaderDataLifecycle,
    /// DDS-XTypes 1.3 §7.6.3
    DataRepresentation,
    /// DDS-XTypes 1.3 §7.6.4
    TypeConsistencyEnforcement,
}

impl QosPolicyKind {
    /// Every policy of DDS-DCPS 1.4 §2.2.3 plus the two XTypes 1.3 adds.
    pub const ALL: [Self; 24] = [
        Self::UserData,
        Self::TopicData,
        Self::GroupData,
        Self::Durability,
        Self::DurabilityService,
        Self::Presentation,
        Self::Deadline,
        Self::LatencyBudget,
        Self::Ownership,
        Self::OwnershipStrength,
        Self::Liveliness,
        Self::TimeBasedFilter,
        Self::Partition,
        Self::Reliability,
        Self::TransportPriority,
        Self::Lifespan,
        Self::DestinationOrder,
        Self::History,
        Self::ResourceLimits,
        Self::EntityFactory,
        Self::WriterDataLifecycle,
        Self::ReaderDataLifecycle,
        Self::DataRepresentation,
        Self::TypeConsistencyEnforcement,
    ];
}

/// `DurabilityQosPolicyKind`, DDS-DCPS 1.4 §2.2.3.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurabilityKind {
    /// `VOLATILE_DURABILITY_QOS`
    Volatile,
    /// `TRANSIENT_LOCAL_DURABILITY_QOS`
    TransientLocal,
    /// `TRANSIENT_DURABILITY_QOS`
    Transient,
    /// `PERSISTENT_DURABILITY_QOS`
    Persistent,
}

/// `ReliabilityQosPolicyKind`, DDS-DCPS 1.4 §2.2.3.17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReliabilityKind {
    /// `BEST_EFFORT_RELIABILITY_QOS`
    BestEffort,
    /// `RELIABLE_RELIABILITY_QOS`
    Reliable,
}

/// `HistoryQosPolicyKind`, DDS-DCPS 1.4 §2.2.3.21.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryKind {
    /// `KEEP_LAST_HISTORY_QOS`
    KeepLast,
    /// `KEEP_ALL_HISTORY_QOS`
    KeepAll,
}

/// `OwnershipQosPolicyKind`, DDS-DCPS 1.4 §2.2.3.12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnershipKind {
    /// `SHARED_OWNERSHIP_QOS`
    Shared,
    /// `EXCLUSIVE_OWNERSHIP_QOS`
    Exclusive,
}

/// `LivelinessQosPolicyKind`, DDS-DCPS 1.4 §2.2.3.14.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LivelinessKind {
    /// `AUTOMATIC_LIVELINESS_QOS`
    Automatic,
    /// `MANUAL_BY_PARTICIPANT_LIVELINESS_QOS`
    ManualByParticipant,
    /// `MANUAL_BY_TOPIC_LIVELINESS_QOS`
    ManualByTopic,
}

/// `DestinationOrderQosPolicyKind`, DDS-DCPS 1.4 §2.2.3.20.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DestinationOrderKind {
    /// `BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS`
    ByReceptionTimestamp,
    /// `BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS`
    BySourceTimestamp,
}

/// `PresentationQosPolicyAccessScopeKind`, DDS-DCPS 1.4 §2.2.3.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentationAccessScope {
    /// `INSTANCE_PRESENTATION_QOS`
    Instance,
    /// `TOPIC_PRESENTATION_QOS`
    Topic,
    /// `GROUP_PRESENTATION_QOS`
    Group,
}

/// `DataRepresentationId_t`, DDS-XTypes 1.3 §7.6.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataRepresentationId {
    /// `XCDR_DATA_REPRESENTATION`
    Xcdr1,
    /// `XML_DATA_REPRESENTATION`
    Xml,
    /// `XCDR2_DATA_REPRESENTATION`
    Xcdr2,
}

/// `TypeConsistencyEnforcementQosPolicyKind`, DDS-XTypes 1.3 §7.6.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeConsistencyKind {
    /// `DISALLOW_TYPE_COERCION`
    DisallowTypeCoercion,
    /// `ALLOW_TYPE_COERCION`
    AllowTypeCoercion,
}

/// One QoS policy value.
///
/// Every field a DDS-XML document can omit is `Option`, so an unwritten
/// field stays distinguishable from one written at its default. A
/// profile that inherits from another needs exactly that distinction to
/// know which fields to take from its base. Spec §3.7.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QosPolicy {
    /// §2.2.3.4
    UserData {
        /// Opaque payload.
        value: Vec<u8>,
    },
    /// §2.2.3.5
    TopicData {
        /// Opaque payload.
        value: Vec<u8>,
    },
    /// §2.2.3.6
    GroupData {
        /// Opaque payload.
        value: Vec<u8>,
    },
    /// §2.2.3.7
    Durability {
        /// Durability kind.
        kind: Option<DurabilityKind>,
    },
    /// §2.2.3.8
    DurabilityService {
        /// How long a sample is kept before cleanup.
        service_cleanup_delay: Option<Duration>,
        /// History kind of the durability service.
        history_kind: Option<HistoryKind>,
        /// History depth.
        history_depth: Option<i32>,
        /// Sample limit.
        max_samples: Option<i32>,
        /// Instance limit.
        max_instances: Option<i32>,
        /// Per-instance sample limit.
        max_samples_per_instance: Option<i32>,
    },
    /// §2.2.3.9
    Presentation {
        /// Access scope.
        access_scope: Option<PresentationAccessScope>,
        /// Whether samples are delivered coherently.
        coherent_access: Option<bool>,
        /// Whether ordered access is requested.
        ordered_access: Option<bool>,
    },
    /// §2.2.3.10
    Deadline {
        /// Deadline period.
        period: Option<Duration>,
    },
    /// §2.2.3.11
    LatencyBudget {
        /// Acceptable delay.
        duration: Option<Duration>,
    },
    /// §2.2.3.12
    Ownership {
        /// Ownership kind.
        kind: Option<OwnershipKind>,
    },
    /// §2.2.3.13
    OwnershipStrength {
        /// Strength value.
        value: Option<i32>,
    },
    /// §2.2.3.14
    Liveliness {
        /// Liveliness kind.
        kind: Option<LivelinessKind>,
        /// Lease duration.
        lease_duration: Option<Duration>,
    },
    /// §2.2.3.15
    TimeBasedFilter {
        /// Minimum separation between samples.
        minimum_separation: Option<Duration>,
    },
    /// §2.2.3.16
    Partition {
        /// Partition names.
        name: Vec<String>,
    },
    /// §2.2.3.17
    Reliability {
        /// Reliability kind.
        kind: Option<ReliabilityKind>,
        /// How long a write may block.
        max_blocking_time: Option<Duration>,
    },
    /// §2.2.3.18
    TransportPriority {
        /// Priority value.
        value: Option<i32>,
    },
    /// §2.2.3.19
    Lifespan {
        /// Sample validity.
        duration: Option<Duration>,
    },
    /// §2.2.3.20
    DestinationOrder {
        /// Ordering kind.
        kind: Option<DestinationOrderKind>,
    },
    /// §2.2.3.21
    History {
        /// History kind.
        kind: Option<HistoryKind>,
        /// History depth.
        depth: Option<i32>,
    },
    /// §2.2.3.22
    ResourceLimits {
        /// Sample limit.
        max_samples: Option<i32>,
        /// Instance limit.
        max_instances: Option<i32>,
        /// Per-instance sample limit.
        max_samples_per_instance: Option<i32>,
    },
    /// §2.2.3.23
    EntityFactory {
        /// Whether created entities are enabled automatically.
        autoenable_created_entities: Option<bool>,
    },
    /// §2.2.3.24
    WriterDataLifecycle {
        /// Whether unregistered instances are disposed automatically.
        autodispose_unregistered_instances: Option<bool>,
    },
    /// §2.2.3.25
    ReaderDataLifecycle {
        /// Delay before purging no-writer samples.
        autopurge_nowriter_samples_delay: Option<Duration>,
        /// Delay before purging disposed samples.
        autopurge_disposed_samples_delay: Option<Duration>,
    },
    /// DDS-XTypes 1.3 §7.6.3
    DataRepresentation {
        /// Accepted representations, in preference order.
        value: Vec<DataRepresentationId>,
    },
    /// DDS-XTypes 1.3 §7.6.4
    TypeConsistencyEnforcement {
        /// Coercion kind.
        kind: Option<TypeConsistencyKind>,
        /// Whether sequence bounds may be ignored.
        ignore_sequence_bounds: Option<bool>,
        /// Whether string bounds may be ignored.
        ignore_string_bounds: Option<bool>,
        /// Whether member names may be ignored.
        ignore_member_names: Option<bool>,
        /// Whether type coercion may widen.
        prevent_type_widening: Option<bool>,
        /// Whether a complete type object is required.
        force_type_validation: Option<bool>,
    },
}

impl QosPolicy {
    /// Which policy this value carries.
    #[must_use]
    pub const fn kind(&self) -> QosPolicyKind {
        match self {
            Self::UserData { .. } => QosPolicyKind::UserData,
            Self::TopicData { .. } => QosPolicyKind::TopicData,
            Self::GroupData { .. } => QosPolicyKind::GroupData,
            Self::Durability { .. } => QosPolicyKind::Durability,
            Self::DurabilityService { .. } => QosPolicyKind::DurabilityService,
            Self::Presentation { .. } => QosPolicyKind::Presentation,
            Self::Deadline { .. } => QosPolicyKind::Deadline,
            Self::LatencyBudget { .. } => QosPolicyKind::LatencyBudget,
            Self::Ownership { .. } => QosPolicyKind::Ownership,
            Self::OwnershipStrength { .. } => QosPolicyKind::OwnershipStrength,
            Self::Liveliness { .. } => QosPolicyKind::Liveliness,
            Self::TimeBasedFilter { .. } => QosPolicyKind::TimeBasedFilter,
            Self::Partition { .. } => QosPolicyKind::Partition,
            Self::Reliability { .. } => QosPolicyKind::Reliability,
            Self::TransportPriority { .. } => QosPolicyKind::TransportPriority,
            Self::Lifespan { .. } => QosPolicyKind::Lifespan,
            Self::DestinationOrder { .. } => QosPolicyKind::DestinationOrder,
            Self::History { .. } => QosPolicyKind::History,
            Self::ResourceLimits { .. } => QosPolicyKind::ResourceLimits,
            Self::EntityFactory { .. } => QosPolicyKind::EntityFactory,
            Self::WriterDataLifecycle { .. } => QosPolicyKind::WriterDataLifecycle,
            Self::ReaderDataLifecycle { .. } => QosPolicyKind::ReaderDataLifecycle,
            Self::DataRepresentation { .. } => QosPolicyKind::DataRepresentation,
            Self::TypeConsistencyEnforcement { .. } => QosPolicyKind::TypeConsistencyEnforcement,
        }
    }
}
