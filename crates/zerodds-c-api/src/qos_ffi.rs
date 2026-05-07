// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! QoS C-FFI Strukturen + Konvertierungen (Spec §2.2.3 + DDS-PSM-Cxx §7.2.4).
//!
//! Alle 22 normativen QoS-Policies sind als `#[repr(C)]`-Strukturen mit
//! exaktem Field-Layout exponiert. Caller fuellt sie direkt in C aus
//! (kein Builder-Boilerplate).
//!
//! `Vec<u8>`/`Vec<String>`-Felder (UserData, TopicData, GroupData, Partition)
//! werden als `(ptr, len)`-Paare gefuehrt. Die FFI-Strukturen besitzen die
//! gepuntete Daten **nicht** — Caller bleibt Owner; bei `set_qos` kopiert
//! der Konverter die Bytes/Strings in den Rust-Heap.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::{c_char, c_int};
use core::ptr;
use core::slice;
use std::ffi::CStr;

use zerodds_dcps::factory::DomainParticipantFactoryQos;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos,
};
use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy, DurabilityKind,
    DurabilityQosPolicy, DurabilityServiceQosPolicy, Duration, EntityFactoryQosPolicy,
    GroupDataQosPolicy, HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy,
    LivelinessKind, LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy,
    OwnershipStrengthQosPolicy, PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy,
    ReaderDataLifecycleQosPolicy, ReliabilityKind, ReliabilityQosPolicy, ResourceLimitsQosPolicy,
    TimeBasedFilterQosPolicy, TopicDataQosPolicy, TransportPriorityQosPolicy, UserDataQosPolicy,
    WriterDataLifecycleQosPolicy,
};

use crate::ZeroDdsStatus;

// ---------------------------------------------------------------------------
// Time / Duration
// ---------------------------------------------------------------------------

/// Duration (Spec §2.2.3.5 + IDL §9.3.2). seconds + nanoseconds form.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDuration {
    /// Seconds.
    pub sec: i32,
    /// Nanoseconds.
    pub nanosec: u32,
}

impl ZeroDdsDuration {
    /// INFINITE-Marker.
    pub const INFINITE: Self = Self {
        sec: i32::MAX,
        nanosec: u32::MAX,
    };
}

impl From<Duration> for ZeroDdsDuration {
    fn from(d: Duration) -> Self {
        if d.is_infinite() {
            return Self::INFINITE;
        }
        // fraction (2^-32 s) → nanoseconds
        let nanosec = ((d.fraction as u64 * 1_000_000_000) >> 32) as u32;
        Self {
            sec: d.seconds,
            nanosec,
        }
    }
}

impl From<ZeroDdsDuration> for Duration {
    fn from(d: ZeroDdsDuration) -> Self {
        if d.sec == i32::MAX && d.nanosec == u32::MAX {
            return Duration::INFINITE;
        }
        let fraction = ((d.nanosec as u64) << 32) / 1_000_000_000;
        Duration {
            seconds: d.sec,
            fraction: fraction as u32,
        }
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Reliability-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsReliabilityKind {
    /// Best-effort.
    BestEffort = 1,
    /// Reliable.
    Reliable = 2,
}

/// Durability-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsDurabilityKind {
    /// Volatile.
    Volatile = 0,
    /// Transient-local.
    TransientLocal = 1,
    /// Transient.
    Transient = 2,
    /// Persistent.
    Persistent = 3,
}

/// Liveliness-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsLivelinessKind {
    /// Automatic.
    Automatic = 0,
    /// Manual by participant.
    ManualByParticipant = 1,
    /// Manual by topic.
    ManualByTopic = 2,
}

/// History-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsHistoryKind {
    /// Keep last N.
    KeepLast = 0,
    /// Keep all.
    KeepAll = 1,
}

/// Ownership-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsOwnershipKind {
    /// Shared.
    Shared = 0,
    /// Exclusive.
    Exclusive = 1,
}

/// DestinationOrder-Kind.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsDestinationOrderKind {
    /// By reception timestamp.
    ByReceptionTimestamp = 0,
    /// By source timestamp.
    BySourceTimestamp = 1,
}

/// Presentation-AccessScope.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ZeroDdsPresentationAccessScope {
    /// Instance.
    Instance = 0,
    /// Topic.
    Topic = 1,
    /// Group.
    Group = 2,
}

// ---------------------------------------------------------------------------
// Policies (22)
// ---------------------------------------------------------------------------

/// ReliabilityQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsReliabilityQosPolicy {
    /// Kind (1=BestEffort, 2=Reliable).
    pub kind: u32,
    /// max_blocking_time.
    pub max_blocking_time: ZeroDdsDuration,
}

/// DurabilityQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDurabilityQosPolicy {
    /// Kind (0=Volatile, 1=TransientLocal, 2=Transient, 3=Persistent).
    pub kind: u32,
}

/// HistoryQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsHistoryQosPolicy {
    /// Kind (0=KeepLast, 1=KeepAll).
    pub kind: u32,
    /// Depth (für KeepLast).
    pub depth: i32,
}

/// DeadlineQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDeadlineQosPolicy {
    /// period.
    pub period: ZeroDdsDuration,
}

/// LifespanQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsLifespanQosPolicy {
    /// duration.
    pub duration: ZeroDdsDuration,
}

/// LatencyBudgetQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsLatencyBudgetQosPolicy {
    /// duration.
    pub duration: ZeroDdsDuration,
}

/// TimeBasedFilterQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTimeBasedFilterQosPolicy {
    /// minimum_separation.
    pub minimum_separation: ZeroDdsDuration,
}

/// LivelinessQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsLivelinessQosPolicy {
    /// Kind.
    pub kind: u32,
    /// lease_duration.
    pub lease_duration: ZeroDdsDuration,
}

/// OwnershipQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsOwnershipQosPolicy {
    /// Kind (0=Shared, 1=Exclusive).
    pub kind: u32,
}

/// OwnershipStrengthQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsOwnershipStrengthQosPolicy {
    /// strength.
    pub value: i32,
}

/// DestinationOrderQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDestinationOrderQosPolicy {
    /// Kind (0=ByReception, 1=BySource).
    pub kind: u32,
}

/// PresentationQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPresentationQosPolicy {
    /// access_scope.
    pub access_scope: u32,
    /// coherent_access.
    pub coherent_access: bool,
    /// ordered_access.
    pub ordered_access: bool,
}

/// ResourceLimitsQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsResourceLimitsQosPolicy {
    /// max_samples.
    pub max_samples: i32,
    /// max_instances.
    pub max_instances: i32,
    /// max_samples_per_instance.
    pub max_samples_per_instance: i32,
}

/// TransportPriorityQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTransportPriorityQosPolicy {
    /// priority.
    pub value: i32,
}

/// EntityFactoryQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsEntityFactoryQosPolicy {
    /// autoenable_created_entities.
    pub autoenable_created_entities: bool,
}

/// WriterDataLifecycleQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsWriterDataLifecycleQosPolicy {
    /// autodispose_unregistered_instances.
    pub autodispose_unregistered_instances: bool,
}

/// ReaderDataLifecycleQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsReaderDataLifecycleQosPolicy {
    /// autopurge_nowriter_samples_delay.
    pub autopurge_nowriter_samples_delay: ZeroDdsDuration,
    /// autopurge_disposed_samples_delay.
    pub autopurge_disposed_samples_delay: ZeroDdsDuration,
}

/// DurabilityServiceQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDurabilityServiceQosPolicy {
    /// service_cleanup_delay.
    pub service_cleanup_delay: ZeroDdsDuration,
    /// history_kind (0=KeepLast, 1=KeepAll).
    pub history_kind: u32,
    /// history_depth.
    pub history_depth: i32,
    /// max_samples.
    pub max_samples: i32,
    /// max_instances.
    pub max_instances: i32,
    /// max_samples_per_instance.
    pub max_samples_per_instance: i32,
}

/// UserDataQosPolicy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsUserDataQosPolicy {
    /// Pointer to bytes (Caller-owned).
    pub value: *const u8,
    /// Length.
    pub value_len: usize,
}

/// TopicDataQosPolicy.
pub type ZeroDdsTopicDataQosPolicy = ZeroDdsUserDataQosPolicy;
/// GroupDataQosPolicy.
pub type ZeroDdsGroupDataQosPolicy = ZeroDdsUserDataQosPolicy;

/// PartitionQosPolicy. Liste C-string-Pointer + Anzahl.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPartitionQosPolicy {
    /// Array von C-String-Pointern (Caller-owned).
    pub names: *const *const c_char,
    /// Anzahl.
    pub names_len: usize,
}

// ---------------------------------------------------------------------------
// QoS-Sets (6)
// ---------------------------------------------------------------------------

/// DomainParticipantFactoryQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDomainParticipantFactoryQos {
    /// EntityFactory.
    pub entity_factory: ZeroDdsEntityFactoryQosPolicy,
}

/// DomainParticipantQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDomainParticipantQos {
    /// UserData.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// EntityFactory.
    pub entity_factory: ZeroDdsEntityFactoryQosPolicy,
}

/// TopicQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsTopicQos {
    /// Durability.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// DurabilityService.
    pub durability_service: ZeroDdsDurabilityServiceQosPolicy,
    /// Deadline.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LatencyBudget.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// Liveliness.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// Reliability.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// DestinationOrder.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// History.
    pub history: ZeroDdsHistoryQosPolicy,
    /// ResourceLimits.
    pub resource_limits: ZeroDdsResourceLimitsQosPolicy,
    /// TransportPriority.
    pub transport_priority: ZeroDdsTransportPriorityQosPolicy,
    /// Lifespan.
    pub lifespan: ZeroDdsLifespanQosPolicy,
    /// Ownership.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// TopicData.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
}

/// PublisherQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsPublisherQos {
    /// Presentation.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// Partition.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// GroupData.
    pub group_data: ZeroDdsGroupDataQosPolicy,
    /// EntityFactory.
    pub entity_factory: ZeroDdsEntityFactoryQosPolicy,
}

/// SubscriberQos.
pub type ZeroDdsSubscriberQos = ZeroDdsPublisherQos;

/// DataWriterQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDataWriterQos {
    /// Reliability.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// Durability.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// DurabilityService.
    pub durability_service: ZeroDdsDurabilityServiceQosPolicy,
    /// Deadline.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LatencyBudget.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// Liveliness.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// DestinationOrder.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// Lifespan.
    pub lifespan: ZeroDdsLifespanQosPolicy,
    /// Ownership.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// OwnershipStrength.
    pub ownership_strength: ZeroDdsOwnershipStrengthQosPolicy,
    /// Partition.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// Presentation.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// History.
    pub history: ZeroDdsHistoryQosPolicy,
    /// ResourceLimits.
    pub resource_limits: ZeroDdsResourceLimitsQosPolicy,
    /// TransportPriority.
    pub transport_priority: ZeroDdsTransportPriorityQosPolicy,
    /// WriterDataLifecycle.
    pub writer_data_lifecycle: ZeroDdsWriterDataLifecycleQosPolicy,
    /// UserData.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// TopicData.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
    /// GroupData.
    pub group_data: ZeroDdsGroupDataQosPolicy,
}

/// DataReaderQos.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZeroDdsDataReaderQos {
    /// Reliability.
    pub reliability: ZeroDdsReliabilityQosPolicy,
    /// Durability.
    pub durability: ZeroDdsDurabilityQosPolicy,
    /// Deadline.
    pub deadline: ZeroDdsDeadlineQosPolicy,
    /// LatencyBudget.
    pub latency_budget: ZeroDdsLatencyBudgetQosPolicy,
    /// Liveliness.
    pub liveliness: ZeroDdsLivelinessQosPolicy,
    /// DestinationOrder.
    pub destination_order: ZeroDdsDestinationOrderQosPolicy,
    /// Ownership.
    pub ownership: ZeroDdsOwnershipQosPolicy,
    /// Partition.
    pub partition: ZeroDdsPartitionQosPolicy,
    /// Presentation.
    pub presentation: ZeroDdsPresentationQosPolicy,
    /// History.
    pub history: ZeroDdsHistoryQosPolicy,
    /// ResourceLimits.
    pub resource_limits: ZeroDdsResourceLimitsQosPolicy,
    /// TimeBasedFilter.
    pub time_based_filter: ZeroDdsTimeBasedFilterQosPolicy,
    /// ReaderDataLifecycle.
    pub reader_data_lifecycle: ZeroDdsReaderDataLifecycleQosPolicy,
    /// UserData.
    pub user_data: ZeroDdsUserDataQosPolicy,
    /// TopicData.
    pub topic_data: ZeroDdsTopicDataQosPolicy,
    /// GroupData.
    pub group_data: ZeroDdsGroupDataQosPolicy,
}

// ---------------------------------------------------------------------------
// Konvertierung (C → Rust + Rust → C)
// ---------------------------------------------------------------------------

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn slice_or_empty<'a>(p: *const u8, n: usize) -> &'a [u8] {
    if p.is_null() || n == 0 {
        &[]
    } else {
        // SAFETY: Caller-Kontrakt ((p,n) gueltiger Bereich).
        unsafe { slice::from_raw_parts(p, n) }
    }
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe fn cstr_vec(arr: *const *const c_char, n: usize) -> Vec<String> {
    if arr.is_null() || n == 0 {
        return Vec::new();
    }
    // SAFETY: Caller-Kontrakt: arr[0..n] valide.
    let slc = unsafe { slice::from_raw_parts(arr, n) };
    let mut out = Vec::with_capacity(n);
    for &p in slc {
        if p.is_null() {
            continue;
        }
        // SAFETY: Caller-Kontrakt: p ist NUL-terminierter String.
        let cs = unsafe { CStr::from_ptr(p) };
        if let Ok(s) = cs.to_str() {
            out.push(s.to_string());
        }
    }
    out
}

impl From<ZeroDdsReliabilityQosPolicy> for ReliabilityQosPolicy {
    fn from(c: ZeroDdsReliabilityQosPolicy) -> Self {
        Self {
            kind: ReliabilityKind::from_u32(c.kind),
            max_blocking_time: c.max_blocking_time.into(),
        }
    }
}
impl From<ReliabilityQosPolicy> for ZeroDdsReliabilityQosPolicy {
    fn from(r: ReliabilityQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
            max_blocking_time: r.max_blocking_time.into(),
        }
    }
}

impl From<ZeroDdsDurabilityQosPolicy> for DurabilityQosPolicy {
    fn from(c: ZeroDdsDurabilityQosPolicy) -> Self {
        Self {
            kind: DurabilityKind::from_u32(c.kind),
        }
    }
}
impl From<DurabilityQosPolicy> for ZeroDdsDurabilityQosPolicy {
    fn from(r: DurabilityQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
        }
    }
}

impl From<ZeroDdsHistoryQosPolicy> for HistoryQosPolicy {
    fn from(c: ZeroDdsHistoryQosPolicy) -> Self {
        Self {
            kind: HistoryKind::from_u32(c.kind),
            depth: c.depth,
        }
    }
}
impl From<HistoryQosPolicy> for ZeroDdsHistoryQosPolicy {
    fn from(r: HistoryQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
            depth: r.depth,
        }
    }
}

impl From<ZeroDdsDeadlineQosPolicy> for DeadlineQosPolicy {
    fn from(c: ZeroDdsDeadlineQosPolicy) -> Self {
        Self {
            period: c.period.into(),
        }
    }
}
impl From<DeadlineQosPolicy> for ZeroDdsDeadlineQosPolicy {
    fn from(r: DeadlineQosPolicy) -> Self {
        Self {
            period: r.period.into(),
        }
    }
}

impl From<ZeroDdsLifespanQosPolicy> for LifespanQosPolicy {
    fn from(c: ZeroDdsLifespanQosPolicy) -> Self {
        Self {
            duration: c.duration.into(),
        }
    }
}
impl From<LifespanQosPolicy> for ZeroDdsLifespanQosPolicy {
    fn from(r: LifespanQosPolicy) -> Self {
        Self {
            duration: r.duration.into(),
        }
    }
}

impl From<ZeroDdsLatencyBudgetQosPolicy> for LatencyBudgetQosPolicy {
    fn from(c: ZeroDdsLatencyBudgetQosPolicy) -> Self {
        Self {
            duration: c.duration.into(),
        }
    }
}
impl From<LatencyBudgetQosPolicy> for ZeroDdsLatencyBudgetQosPolicy {
    fn from(r: LatencyBudgetQosPolicy) -> Self {
        Self {
            duration: r.duration.into(),
        }
    }
}

impl From<ZeroDdsTimeBasedFilterQosPolicy> for TimeBasedFilterQosPolicy {
    fn from(c: ZeroDdsTimeBasedFilterQosPolicy) -> Self {
        Self {
            minimum_separation: c.minimum_separation.into(),
        }
    }
}
impl From<TimeBasedFilterQosPolicy> for ZeroDdsTimeBasedFilterQosPolicy {
    fn from(r: TimeBasedFilterQosPolicy) -> Self {
        Self {
            minimum_separation: r.minimum_separation.into(),
        }
    }
}

impl From<ZeroDdsLivelinessQosPolicy> for LivelinessQosPolicy {
    fn from(c: ZeroDdsLivelinessQosPolicy) -> Self {
        Self {
            kind: LivelinessKind::from_u32(c.kind),
            lease_duration: c.lease_duration.into(),
        }
    }
}
impl From<LivelinessQosPolicy> for ZeroDdsLivelinessQosPolicy {
    fn from(r: LivelinessQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
            lease_duration: r.lease_duration.into(),
        }
    }
}

impl From<ZeroDdsOwnershipQosPolicy> for OwnershipQosPolicy {
    fn from(c: ZeroDdsOwnershipQosPolicy) -> Self {
        Self {
            kind: OwnershipKind::from_u32(c.kind),
        }
    }
}
impl From<OwnershipQosPolicy> for ZeroDdsOwnershipQosPolicy {
    fn from(r: OwnershipQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
        }
    }
}

impl From<ZeroDdsOwnershipStrengthQosPolicy> for OwnershipStrengthQosPolicy {
    fn from(c: ZeroDdsOwnershipStrengthQosPolicy) -> Self {
        Self { value: c.value }
    }
}
impl From<OwnershipStrengthQosPolicy> for ZeroDdsOwnershipStrengthQosPolicy {
    fn from(r: OwnershipStrengthQosPolicy) -> Self {
        Self { value: r.value }
    }
}

impl From<ZeroDdsDestinationOrderQosPolicy> for DestinationOrderQosPolicy {
    fn from(c: ZeroDdsDestinationOrderQosPolicy) -> Self {
        Self {
            kind: DestinationOrderKind::from_u32(c.kind),
        }
    }
}
impl From<DestinationOrderQosPolicy> for ZeroDdsDestinationOrderQosPolicy {
    fn from(r: DestinationOrderQosPolicy) -> Self {
        Self {
            kind: r.kind as u32,
        }
    }
}

impl From<ZeroDdsPresentationQosPolicy> for PresentationQosPolicy {
    fn from(c: ZeroDdsPresentationQosPolicy) -> Self {
        Self {
            access_scope: PresentationAccessScope::from_u32(c.access_scope),
            coherent_access: c.coherent_access,
            ordered_access: c.ordered_access,
        }
    }
}
impl From<PresentationQosPolicy> for ZeroDdsPresentationQosPolicy {
    fn from(r: PresentationQosPolicy) -> Self {
        Self {
            access_scope: r.access_scope as u32,
            coherent_access: r.coherent_access,
            ordered_access: r.ordered_access,
        }
    }
}

impl From<ZeroDdsResourceLimitsQosPolicy> for ResourceLimitsQosPolicy {
    fn from(c: ZeroDdsResourceLimitsQosPolicy) -> Self {
        Self {
            max_samples: c.max_samples,
            max_instances: c.max_instances,
            max_samples_per_instance: c.max_samples_per_instance,
        }
    }
}
impl From<ResourceLimitsQosPolicy> for ZeroDdsResourceLimitsQosPolicy {
    fn from(r: ResourceLimitsQosPolicy) -> Self {
        Self {
            max_samples: r.max_samples,
            max_instances: r.max_instances,
            max_samples_per_instance: r.max_samples_per_instance,
        }
    }
}

impl From<ZeroDdsTransportPriorityQosPolicy> for TransportPriorityQosPolicy {
    fn from(c: ZeroDdsTransportPriorityQosPolicy) -> Self {
        Self { value: c.value }
    }
}
impl From<TransportPriorityQosPolicy> for ZeroDdsTransportPriorityQosPolicy {
    fn from(r: TransportPriorityQosPolicy) -> Self {
        Self { value: r.value }
    }
}

impl From<ZeroDdsEntityFactoryQosPolicy> for EntityFactoryQosPolicy {
    fn from(c: ZeroDdsEntityFactoryQosPolicy) -> Self {
        Self {
            autoenable_created_entities: c.autoenable_created_entities,
        }
    }
}
impl From<EntityFactoryQosPolicy> for ZeroDdsEntityFactoryQosPolicy {
    fn from(r: EntityFactoryQosPolicy) -> Self {
        Self {
            autoenable_created_entities: r.autoenable_created_entities,
        }
    }
}

impl From<ZeroDdsWriterDataLifecycleQosPolicy> for WriterDataLifecycleQosPolicy {
    fn from(c: ZeroDdsWriterDataLifecycleQosPolicy) -> Self {
        let mut p = WriterDataLifecycleQosPolicy::default();
        p.autodispose_unregistered_instances = c.autodispose_unregistered_instances;
        p
    }
}
impl From<WriterDataLifecycleQosPolicy> for ZeroDdsWriterDataLifecycleQosPolicy {
    fn from(r: WriterDataLifecycleQosPolicy) -> Self {
        Self {
            autodispose_unregistered_instances: r.autodispose_unregistered_instances,
        }
    }
}

impl From<ZeroDdsReaderDataLifecycleQosPolicy> for ReaderDataLifecycleQosPolicy {
    fn from(c: ZeroDdsReaderDataLifecycleQosPolicy) -> Self {
        let mut p = ReaderDataLifecycleQosPolicy::default();
        p.autopurge_nowriter_samples_delay = c.autopurge_nowriter_samples_delay.into();
        p.autopurge_disposed_samples_delay = c.autopurge_disposed_samples_delay.into();
        p
    }
}
impl From<ReaderDataLifecycleQosPolicy> for ZeroDdsReaderDataLifecycleQosPolicy {
    fn from(r: ReaderDataLifecycleQosPolicy) -> Self {
        Self {
            autopurge_nowriter_samples_delay: r.autopurge_nowriter_samples_delay.into(),
            autopurge_disposed_samples_delay: r.autopurge_disposed_samples_delay.into(),
        }
    }
}

impl From<ZeroDdsDurabilityServiceQosPolicy> for DurabilityServiceQosPolicy {
    fn from(c: ZeroDdsDurabilityServiceQosPolicy) -> Self {
        Self {
            service_cleanup_delay: c.service_cleanup_delay.into(),
            history_kind: HistoryKind::from_u32(c.history_kind),
            history_depth: c.history_depth,
            max_samples: c.max_samples,
            max_instances: c.max_instances,
            max_samples_per_instance: c.max_samples_per_instance,
        }
    }
}
impl From<DurabilityServiceQosPolicy> for ZeroDdsDurabilityServiceQosPolicy {
    fn from(r: DurabilityServiceQosPolicy) -> Self {
        Self {
            service_cleanup_delay: r.service_cleanup_delay.into(),
            history_kind: r.history_kind as u32,
            history_depth: r.history_depth,
            max_samples: r.max_samples,
            max_instances: r.max_instances,
            max_samples_per_instance: r.max_samples_per_instance,
        }
    }
}

// ---------------------------------------------------------------------------
// QoS-Set Konvertierungen
// ---------------------------------------------------------------------------

/// Konvertiert FFI-Pointer (NULL = Default) in `DataWriterQos`.
///
/// # Safety
/// `c` darf NULL sein. Wenn nicht NULL, muss er auf eine valide
/// `ZeroDdsDataWriterQos`-Struktur zeigen.
pub unsafe fn dw_qos_from_c(c: *const ZeroDdsDataWriterQos) -> DataWriterQos {
    if c.is_null() {
        return DataWriterQos::default();
    }
    // SAFETY: NULL-Check oben.
    let q = unsafe { &*c };
    DataWriterQos {
        reliability: q.reliability.into(),
        durability: q.durability.into(),
        durability_service: q.durability_service.into(),
        deadline: q.deadline.into(),
        latency_budget: q.latency_budget.into(),
        liveliness: q.liveliness.into(),
        destination_order: q.destination_order.into(),
        lifespan: q.lifespan.into(),
        ownership: q.ownership.into(),
        ownership_strength: q.ownership_strength.into(),
        partition: PartitionQosPolicy {
            // SAFETY: q.partition.names valider Bereich oder NULL.
            names: unsafe { cstr_vec(q.partition.names, q.partition.names_len) },
        },
        presentation: q.presentation.into(),
        history: q.history.into(),
        resource_limits: q.resource_limits.into(),
        transport_priority: q.transport_priority.into(),
        writer_data_lifecycle: q.writer_data_lifecycle.into(),
        user_data: UserDataQosPolicy {
            // SAFETY: q.user_data.value valider Bereich oder NULL.
            value: unsafe { slice_or_empty(q.user_data.value, q.user_data.value_len) }.to_vec(),
        },
        topic_data: TopicDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.topic_data.value, q.topic_data.value_len) }.to_vec(),
        },
        group_data: GroupDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.group_data.value, q.group_data.value_len) }.to_vec(),
        },
    }
}

/// Konvertiert FFI-Pointer in `DataReaderQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn dr_qos_from_c(c: *const ZeroDdsDataReaderQos) -> DataReaderQos {
    if c.is_null() {
        return DataReaderQos::default();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let q = unsafe { &*c };
    DataReaderQos {
        reliability: q.reliability.into(),
        durability: q.durability.into(),
        deadline: q.deadline.into(),
        latency_budget: q.latency_budget.into(),
        liveliness: q.liveliness.into(),
        destination_order: q.destination_order.into(),
        ownership: q.ownership.into(),
        partition: PartitionQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            names: unsafe { cstr_vec(q.partition.names, q.partition.names_len) },
        },
        presentation: q.presentation.into(),
        history: q.history.into(),
        resource_limits: q.resource_limits.into(),
        time_based_filter: q.time_based_filter.into(),
        reader_data_lifecycle: q.reader_data_lifecycle.into(),
        user_data: UserDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.user_data.value, q.user_data.value_len) }.to_vec(),
        },
        topic_data: TopicDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.topic_data.value, q.topic_data.value_len) }.to_vec(),
        },
        group_data: GroupDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.group_data.value, q.group_data.value_len) }.to_vec(),
        },
    }
}

/// Konvertiert FFI-Pointer in `TopicQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn topic_qos_from_c(c: *const ZeroDdsTopicQos) -> TopicQos {
    if c.is_null() {
        return TopicQos::default();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let q = unsafe { &*c };
    TopicQos {
        durability: q.durability.into(),
        durability_service: q.durability_service.into(),
        deadline: q.deadline.into(),
        latency_budget: q.latency_budget.into(),
        liveliness: q.liveliness.into(),
        reliability: q.reliability.into(),
        destination_order: q.destination_order.into(),
        history: q.history.into(),
        resource_limits: q.resource_limits.into(),
        transport_priority: q.transport_priority.into(),
        lifespan: q.lifespan.into(),
        ownership: q.ownership.into(),
        topic_data: TopicDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.topic_data.value, q.topic_data.value_len) }.to_vec(),
        },
    }
}

/// Konvertiert FFI-Pointer in `PublisherQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn pub_qos_from_c(c: *const ZeroDdsPublisherQos) -> PublisherQos {
    if c.is_null() {
        return PublisherQos::default();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let q = unsafe { &*c };
    PublisherQos {
        presentation: q.presentation.into(),
        partition: PartitionQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            names: unsafe { cstr_vec(q.partition.names, q.partition.names_len) },
        },
        group_data: GroupDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.group_data.value, q.group_data.value_len) }.to_vec(),
        },
        entity_factory: q.entity_factory.into(),
    }
}

/// Konvertiert FFI-Pointer in `SubscriberQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn sub_qos_from_c(c: *const ZeroDdsSubscriberQos) -> SubscriberQos {
    if c.is_null() {
        return SubscriberQos::default();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let q = unsafe { &*c };
    SubscriberQos {
        presentation: q.presentation.into(),
        partition: PartitionQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            names: unsafe { cstr_vec(q.partition.names, q.partition.names_len) },
        },
        group_data: GroupDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.group_data.value, q.group_data.value_len) }.to_vec(),
        },
        entity_factory: q.entity_factory.into(),
    }
}

/// Konvertiert FFI-Pointer in `DomainParticipantQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn dp_qos_from_c(c: *const ZeroDdsDomainParticipantQos) -> DomainParticipantQos {
    if c.is_null() {
        return DomainParticipantQos::default();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let q = unsafe { &*c };
    DomainParticipantQos {
        user_data: UserDataQosPolicy {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            value: unsafe { slice_or_empty(q.user_data.value, q.user_data.value_len) }.to_vec(),
        },
        entity_factory: q.entity_factory.into(),
    }
}

/// Konvertiert FFI-Pointer in `DomainParticipantFactoryQos`.
///
/// # Safety
/// Wie `dw_qos_from_c`.
pub unsafe fn dpf_qos_from_c(
    c: *const ZeroDdsDomainParticipantFactoryQos,
) -> DomainParticipantFactoryQos {
    if c.is_null() {
        return DomainParticipantFactoryQos::default();
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let q = unsafe { &*c };
    DomainParticipantFactoryQos {
        autoenable_created_entities: q.entity_factory.autoenable_created_entities,
    }
}

/// Schreibt einen `DomainParticipantQos`-Snapshot in den Caller-Buffer.
/// Variable-Laenge-Felder (UserData) werden in einen vom Caller
/// allokierten Buffer kopiert wenn `out.user_data.value` non-NULL ist
/// und `out.user_data.value_len` ausreichend gross. Bei zu kleinem
/// Buffer wird `value_len` mit der erforderlichen Groesse beschrieben
/// und `OUT_OF_RESOURCES` zurueckgeliefert.
///
/// # Safety
/// `out` valide.
pub unsafe fn dp_qos_to_c(
    r: &DomainParticipantQos,
    out: *mut ZeroDdsDomainParticipantQos,
) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let needed = r.user_data.value.len();
    // SAFETY: NULL-Check oben.
    let cap = unsafe { (*out).user_data.value_len };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dst_ptr = unsafe { (*out).user_data.value as *mut u8 };
    if needed > 0 {
        // Buffer-Pruefung nur wenn echte Bytes zu kopieren sind.
        if dst_ptr.is_null() || cap < needed {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { (*out).user_data.value_len = needed };
            return ZeroDdsStatus::OutOfResources as c_int;
        }
        // SAFETY: Caller hat einen ausreichend grossen Buffer bereitgestellt.
        unsafe {
            ptr::copy_nonoverlapping(r.user_data.value.as_ptr(), dst_ptr, needed);
        }
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).user_data.value_len = needed;
        (*out).entity_factory = r.entity_factory.into();
    }
    ZeroDdsStatus::Ok as c_int
}

/// Schreibt einen `TopicQos`-Snapshot in den Caller-Buffer.
/// Variable-Length-Felder (TopicData) wie bei `dp_qos_to_c`.
///
/// # Safety
/// `out` valide.
pub unsafe fn topic_qos_to_c(r: &TopicQos, out: *mut ZeroDdsTopicQos) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let needed = r.topic_data.value.len();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let cap = unsafe { (*out).topic_data.value_len };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dst_ptr = unsafe { (*out).topic_data.value as *mut u8 };
    if needed > 0 {
        if dst_ptr.is_null() || cap < needed {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { (*out).topic_data.value_len = needed };
            return ZeroDdsStatus::OutOfResources as c_int;
        }
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { ptr::copy_nonoverlapping(r.topic_data.value.as_ptr(), dst_ptr, needed) };
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).topic_data.value_len = needed;
        (*out).durability = r.durability.into();
        (*out).durability_service = r.durability_service.into();
        (*out).deadline = r.deadline.into();
        (*out).latency_budget = r.latency_budget.into();
        (*out).liveliness = r.liveliness.into();
        (*out).reliability = r.reliability.into();
        (*out).destination_order = r.destination_order.into();
        (*out).history = r.history.into();
        (*out).resource_limits = r.resource_limits.into();
        (*out).transport_priority = r.transport_priority.into();
        (*out).lifespan = r.lifespan.into();
        (*out).ownership = r.ownership.into();
    }
    ZeroDdsStatus::Ok as c_int
}

/// Schreibt einen `DataWriterQos`-Snapshot. UserData/TopicData/GroupData
/// muessen ausreichende Buffer haben; Partition wird im RC1 als len=0
/// zurueckgegeben (Caller muss separat lesen — Folge-Patch).
///
/// # Safety
/// `out` valide.
pub unsafe fn dw_qos_to_c(r: &DataWriterQos, out: *mut ZeroDdsDataWriterQos) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // Bytes-Buffer-Pruefung pro Feld.
    macro_rules! copy_bytes {
        ($field:ident) => {{
            let needed = r.$field.value.len();
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            let cap = unsafe { (*out).$field.value_len };
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            let dst = unsafe { (*out).$field.value as *mut u8 };
            if needed > 0 {
                if dst.is_null() || cap < needed {
                    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                    unsafe { (*out).$field.value_len = needed };
                    return ZeroDdsStatus::OutOfResources as c_int;
                }
                // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                unsafe { ptr::copy_nonoverlapping(r.$field.value.as_ptr(), dst, needed) };
            }
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { (*out).$field.value_len = needed };
        }};
    }
    copy_bytes!(user_data);
    copy_bytes!(topic_data);
    copy_bytes!(group_data);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).reliability = r.reliability.into();
        (*out).durability = r.durability.into();
        (*out).durability_service = r.durability_service.into();
        (*out).deadline = r.deadline.into();
        (*out).latency_budget = r.latency_budget.into();
        (*out).liveliness = r.liveliness.into();
        (*out).destination_order = r.destination_order.into();
        (*out).lifespan = r.lifespan.into();
        (*out).ownership = r.ownership.into();
        (*out).ownership_strength = r.ownership_strength.into();
        (*out).presentation = r.presentation.into();
        (*out).history = r.history.into();
        (*out).resource_limits = r.resource_limits.into();
        (*out).transport_priority = r.transport_priority.into();
        (*out).writer_data_lifecycle = r.writer_data_lifecycle.into();
        // Partition als len=0 (variable-length, Folge-Patch).
        (*out).partition.names = ptr::null();
        (*out).partition.names_len = 0;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Schreibt einen `DataReaderQos`-Snapshot.
///
/// # Safety
/// `out` valide.
pub unsafe fn dr_qos_to_c(r: &DataReaderQos, out: *mut ZeroDdsDataReaderQos) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    macro_rules! copy_bytes {
        ($field:ident) => {{
            let needed = r.$field.value.len();
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            let cap = unsafe { (*out).$field.value_len };
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            let dst = unsafe { (*out).$field.value as *mut u8 };
            if needed > 0 {
                if dst.is_null() || cap < needed {
                    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                    unsafe { (*out).$field.value_len = needed };
                    return ZeroDdsStatus::OutOfResources as c_int;
                }
                // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
                unsafe { ptr::copy_nonoverlapping(r.$field.value.as_ptr(), dst, needed) };
            }
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { (*out).$field.value_len = needed };
        }};
    }
    copy_bytes!(user_data);
    copy_bytes!(topic_data);
    copy_bytes!(group_data);
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).reliability = r.reliability.into();
        (*out).durability = r.durability.into();
        (*out).deadline = r.deadline.into();
        (*out).latency_budget = r.latency_budget.into();
        (*out).liveliness = r.liveliness.into();
        (*out).destination_order = r.destination_order.into();
        (*out).ownership = r.ownership.into();
        (*out).presentation = r.presentation.into();
        (*out).history = r.history.into();
        (*out).resource_limits = r.resource_limits.into();
        (*out).time_based_filter = r.time_based_filter.into();
        (*out).reader_data_lifecycle = r.reader_data_lifecycle.into();
        (*out).partition.names = ptr::null();
        (*out).partition.names_len = 0;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Schreibt einen `PublisherQos`-Snapshot.
///
/// # Safety
/// `out` valide.
pub unsafe fn pub_qos_to_c(r: &PublisherQos, out: *mut ZeroDdsPublisherQos) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let needed = r.group_data.value.len();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let cap = unsafe { (*out).group_data.value_len };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dst = unsafe { (*out).group_data.value as *mut u8 };
    if needed > 0 {
        if dst.is_null() || cap < needed {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { (*out).group_data.value_len = needed };
            return ZeroDdsStatus::OutOfResources as c_int;
        }
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { ptr::copy_nonoverlapping(r.group_data.value.as_ptr(), dst, needed) };
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).group_data.value_len = needed;
        (*out).presentation = r.presentation.into();
        (*out).entity_factory = r.entity_factory.into();
        (*out).partition.names = ptr::null();
        (*out).partition.names_len = 0;
    }
    ZeroDdsStatus::Ok as c_int
}

/// Schreibt einen `SubscriberQos`-Snapshot. Identisches Layout wie
/// PublisherQos.
///
/// # Safety
/// `out` valide.
pub unsafe fn sub_qos_to_c(r: &SubscriberQos, out: *mut ZeroDdsSubscriberQos) -> c_int {
    if out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    let needed = r.group_data.value.len();
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let cap = unsafe { (*out).group_data.value_len };
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    let dst = unsafe { (*out).group_data.value as *mut u8 };
    if needed > 0 {
        if dst.is_null() || cap < needed {
            // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
            unsafe { (*out).group_data.value_len = needed };
            return ZeroDdsStatus::OutOfResources as c_int;
        }
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        unsafe { ptr::copy_nonoverlapping(r.group_data.value.as_ptr(), dst, needed) };
    }
    // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
    unsafe {
        (*out).group_data.value_len = needed;
        (*out).presentation = r.presentation.into();
        (*out).entity_factory = r.entity_factory.into();
        (*out).partition.names = ptr::null();
        (*out).partition.names_len = 0;
    }
    ZeroDdsStatus::Ok as c_int
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn duration_roundtrip() {
        let r = Duration::from_millis(250);
        let c: ZeroDdsDuration = r.into();
        let back: Duration = c.into();
        // Konversion via nanosec hat moeglicherweise minimal-Drift —
        // pruefe nur Sekunden + grobe nanosec.
        assert_eq!(back.seconds, r.seconds);
        assert!((back.fraction as i64 - r.fraction as i64).abs() < 1024);
    }

    #[test]
    fn duration_infinite_roundtrip() {
        let r = Duration::INFINITE;
        let c: ZeroDdsDuration = r.into();
        assert_eq!(c.sec, i32::MAX);
        assert_eq!(c.nanosec, u32::MAX);
        let back: Duration = c.into();
        assert!(back.is_infinite());
    }

    #[test]
    fn dw_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dw_qos_from_c(ptr::null()) };
        assert_eq!(q, DataWriterQos::default());
    }

    #[test]
    fn dw_qos_reliability_roundtrip() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let mut c: ZeroDdsDataWriterQos = unsafe { core::mem::zeroed() };
        c.reliability = ZeroDdsReliabilityQosPolicy {
            kind: 1, // BestEffort
            max_blocking_time: ZeroDdsDuration { sec: 1, nanosec: 0 },
        };
        c.history.kind = 0;
        c.history.depth = 1;
        c.resource_limits.max_samples = 1000;
        c.resource_limits.max_instances = 10;
        c.resource_limits.max_samples_per_instance = 100;
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dw_qos_from_c(&c) };
        assert!(matches!(q.reliability.kind, ReliabilityKind::BestEffort));
    }

    #[test]
    fn dr_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dr_qos_from_c(ptr::null()) };
        assert_eq!(q, DataReaderQos::default());
    }

    #[test]
    fn topic_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { topic_qos_from_c(ptr::null()) };
        assert_eq!(q, TopicQos::default());
    }

    #[test]
    fn pub_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { pub_qos_from_c(ptr::null()) };
        assert_eq!(q, PublisherQos::default());
    }

    #[test]
    fn sub_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { sub_qos_from_c(ptr::null()) };
        assert_eq!(q, SubscriberQos::default());
    }

    #[test]
    fn dp_qos_default_from_null() {
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dp_qos_from_c(ptr::null()) };
        assert_eq!(q, DomainParticipantQos::default());
    }

    #[test]
    fn dp_qos_userdata_passthrough() {
        let bytes = b"hello world";
        let c = ZeroDdsDomainParticipantQos {
            user_data: ZeroDdsUserDataQosPolicy {
                value: bytes.as_ptr(),
                value_len: bytes.len(),
            },
            entity_factory: ZeroDdsEntityFactoryQosPolicy {
                autoenable_created_entities: false,
            },
        };
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dp_qos_from_c(&c) };
        assert_eq!(q.user_data.value, bytes.to_vec());
        assert!(!q.entity_factory.autoenable_created_entities);
    }

    #[test]
    fn partition_cstring_array_passthrough() {
        let p1 = c"alpha";
        let p2 = c"beta";
        let arr: [*const c_char; 2] = [p1.as_ptr(), p2.as_ptr()];
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let mut c: ZeroDdsDataWriterQos = unsafe { core::mem::zeroed() };
        c.partition.names = arr.as_ptr();
        c.partition.names_len = 2;
        // Reliability/History/ResourceLimits sind via zeroed() = 0
        // (BestEffort=1 ist nicht 0, aber ReliabilityKind::from_u32 mappt
        // 0 auf BestEffort als Fallback). Pruefe nur Partition.
        // SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
        let q = unsafe { dw_qos_from_c(&c) };
        assert_eq!(
            q.partition.names,
            alloc::vec!["alpha".to_string(), "beta".to_string()]
        );
    }
}
