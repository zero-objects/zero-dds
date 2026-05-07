// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS QoS-Policies (DDS 1.4 §2.2.3).
//!
//! Jede Policy liegt in einem eigenen Sub-Modul und wird hier
//! reexportiert. Die Submodule enthalten Kind-Enums, Policy-Structs,
//! Wire-Encoder/Decoder und Spec-Referenzen.

pub use self::data_lifecycle::{ReaderDataLifecycleQosPolicy, WriterDataLifecycleQosPolicy};
pub use self::deadline::DeadlineQosPolicy;
pub use self::destination_order::{DestinationOrderKind, DestinationOrderQosPolicy};
pub use self::durability::{DurabilityKind, DurabilityQosPolicy};
pub use self::durability_service::DurabilityServiceQosPolicy;
pub use self::entity_factory::EntityFactoryQosPolicy;
pub use self::generic_data::{GroupDataQosPolicy, TopicDataQosPolicy, UserDataQosPolicy};
pub use self::history::{HistoryKind, HistoryQosPolicy};
pub use self::latency_budget::LatencyBudgetQosPolicy;
pub use self::lifespan::LifespanQosPolicy;
pub use self::liveliness::{LivelinessKind, LivelinessQosPolicy};
pub use self::ownership::{OwnershipKind, OwnershipQosPolicy};
pub use self::ownership_strength::OwnershipStrengthQosPolicy;
pub use self::partition::PartitionQosPolicy;
pub use self::presentation::{PresentationAccessScope, PresentationQosPolicy};
pub use self::qos_set::{InconsistentReason, ReaderQos, WriterQos, check_compatibility};
pub use self::reliability::{ReliabilityKind, ReliabilityQosPolicy};
pub use self::resource_limits::ResourceLimitsQosPolicy;
pub use self::time_based_filter::TimeBasedFilterQosPolicy;
pub use self::transport_priority::TransportPriorityQosPolicy;

/// Reader-/Writer-Data-Lifecycle-Policies.
pub mod data_lifecycle;
/// Deadline.
pub mod deadline;
/// Destination-Order.
pub mod destination_order;
/// Durability.
pub mod durability;
/// Durability-Service.
pub mod durability_service;
/// Entity-Factory.
pub mod entity_factory;
/// User/Topic/Group-Data (opaque bytes).
pub mod generic_data;
/// History.
pub mod history;
/// Latency-Budget.
pub mod latency_budget;
/// Lifespan.
pub mod lifespan;
/// Liveliness.
pub mod liveliness;
/// Ownership.
pub mod ownership;
/// Ownership-Strength.
pub mod ownership_strength;
/// Partition + fnmatch-Glob-Helper.
pub mod partition;
/// Presentation.
pub mod presentation;
/// WriterQos/ReaderQos-Aggregate + Compatibility + Consistency.
pub mod qos_set;
/// Reliability.
pub mod reliability;
/// Resource-Limits.
pub mod resource_limits;
/// Time-Based-Filter.
pub mod time_based_filter;
/// Transport-Priority.
pub mod transport_priority;
