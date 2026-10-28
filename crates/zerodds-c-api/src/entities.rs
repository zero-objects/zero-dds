// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Opaque entity handle types for the spec-conformant C-FFI surface
//! (DDS spec §2.2.2 + DDS-PSM-Cxx 1.0 §7.2.1).
//!
//! The handles are `Box<...>`-allocated wrappers around the Rust DCPS
//! entities. Per `*_create()` the box is converted into a pointer;
//! `*_destroy()` calls `Box::from_raw` and thereby also drops the
//! held `Arc<DcpsRuntime>` clone, so that the runtime cleans up
//! when the last handle is removed.
//!
//! ## Architecture note
//!
//! At the C-FFI level the spec hierarchy (factory → participant →
//! publisher/subscriber/topic → DataWriter/DataReader) is modeled as an opaque
//! handle chain. Internally DataWriter and DataReader collapse
//! onto the `DcpsRuntime::create_user_writer/reader` path with a
//! caller-supplied `type_name` string, because `DataWriter<T>` generics
//! do not allow a runtime `type_name` (`T::TYPE_NAME` is `const`).
//!
//! Publisher/subscriber are organizational containers that track QoS defaults
//! plus a list of the contained writers/readers — they remain
//! fully spec-conformant at the API surface (incl. presentation,
//! partition, group_data, suspend/resume).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::{Mutex, mpsc};

use zerodds_dcps::factory::DomainParticipantFactoryQos;
use zerodds_dcps::instance_tracker::InstanceTracker;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, OwnershipKind, PublisherQos, SubscriberQos,
    TopicQos,
};
use zerodds_dcps::runtime::{DcpsRuntime, UserSample};
use zerodds_dcps::sample_info::SampleInfo as DcpsSampleInfo;
use zerodds_rtps::wire_types::EntityId;

// ============================================================================
// Entities a StatusCondition looks at
// ============================================================================

/// The kinds of entity that have communication statuses of their own (DDS
/// 1.4 §2.2.4.1): a StatusCondition of one of them triggers on its
/// StatusChangedFlags. A participant and a publisher have none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEntityKind {
    /// INCONSISTENT_TOPIC.
    Topic,
    /// DATA_ON_READERS.
    Subscriber,
    /// The four writer statuses.
    DataWriter,
    /// The seven reader statuses.
    DataReader,
}

/// The live entities of [`StatusEntityKind`] by address. A StatusCondition
/// is created from an untyped entity pointer; this tells what it points
/// at. Each handle enters at its creation and leaves in its `Drop`.
static STATUS_ENTITIES: Mutex<alloc::collections::BTreeMap<usize, StatusEntityKind>> =
    Mutex::new(alloc::collections::BTreeMap::new());

/// `entity` was created as a `kind`.
pub(crate) fn note_status_entity<T>(entity: *const T, kind: StatusEntityKind) {
    if let Ok(mut live) = STATUS_ENTITIES.lock() {
        live.insert(entity as usize, kind);
    }
}

/// `entity` is being freed.
pub(crate) fn forget_status_entity<T>(entity: *const T) {
    if let Ok(mut live) = STATUS_ENTITIES.lock() {
        live.remove(&(entity as usize));
    }
}

/// What `entity` was created as, if it is a live entity with statuses.
pub(crate) fn status_entity_kind(entity: *const core::ffi::c_void) -> Option<StatusEntityKind> {
    STATUS_ENTITIES
        .lock()
        .ok()
        .and_then(|live| live.get(&(entity as usize)).copied())
}

// ============================================================================
// DomainParticipantFactory — Singleton
// ============================================================================

/// Singleton DomainParticipantFactory (Spec §2.2.2.2.1).
pub struct ZeroDdsDomainParticipantFactory {
    /// Default QoS for participants to be created.
    pub default_participant_qos: Mutex<DomainParticipantQos>,
    /// Factory-owned QoS (entity_factory.autoenable_created_entities).
    pub factory_qos: Mutex<DomainParticipantFactoryQos>,
    /// Registry of all active participants (for lookup_participant).
    pub participants: Mutex<Vec<*mut ZeroDdsDomainParticipant>>,
}

// SAFETY: pointers in `participants` are accessed only under the `Mutex`
// and reference `Box`-allocated ZeroDdsDomainParticipants, which are
// `Send + Sync` via the contained `Arc<DcpsRuntime>`.
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDomainParticipantFactory {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDomainParticipantFactory {}

impl ZeroDdsDomainParticipantFactory {
    /// Returns the global singleton instance.
    pub fn instance() -> &'static Self {
        use std::sync::OnceLock;
        static FACTORY: OnceLock<ZeroDdsDomainParticipantFactory> = OnceLock::new();
        FACTORY.get_or_init(|| Self {
            default_participant_qos: Mutex::new(DomainParticipantQos::default()),
            factory_qos: Mutex::new(DomainParticipantFactoryQos::default()),
            participants: Mutex::new(Vec::new()),
        })
    }
}

// ============================================================================
// DomainParticipant
// ============================================================================

/// DomainParticipant (Spec §2.2.2.2.1.1).
pub struct ZeroDdsDomainParticipant {
    /// Live wrapper on the high-level `dcps::DomainParticipant` form.
    /// Allows direct access to `ignore_*`, `contains_entity`,
    /// `assert_liveliness`, discovery listings etc.
    pub dp: DomainParticipant,
    /// Runtime clone (extracted via `dp.runtime()`) for the
    /// user-writer/reader path. None if offline.
    pub rt: Option<Arc<DcpsRuntime>>,
    /// Domain ID (cached, from dp.domain_id()).
    pub domain_id: u32,
    pub default_topic_qos: Mutex<TopicQos>,
    pub default_publisher_qos: Mutex<PublisherQos>,
    pub default_subscriber_qos: Mutex<SubscriberQos>,
    /// Topics created via this participant.
    pub topics: Mutex<Vec<*mut ZeroDdsTopic>>,
    /// ContentFilteredTopics created via this participant
    /// (`zerodds_dp_create_contentfilteredtopic`). `delete_topic` counts them
    /// against the topic they are related to, which is the
    /// ContentFilteredTopic clause of DDS 1.4 §2.2.2.2.1.6.
    pub content_filtered_topics: Mutex<Vec<*mut ZeroDdsContentFilteredTopic>>,
    pub publishers: Mutex<Vec<*mut ZeroDdsPublisher>>,
    pub subscribers: Mutex<Vec<*mut ZeroDdsSubscriber>>,
    /// Stable PARTITION-names backing for `get_default_publisher_qos`
    /// (see `qos_ffi::PartitionOutCache`).
    pub default_pub_partition_out: Mutex<crate::qos_ffi::PartitionOutCache>,
    /// Stable PARTITION-names backing for `get_default_subscriber_qos`.
    pub default_sub_partition_out: Mutex<crate::qos_ffi::PartitionOutCache>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDomainParticipant {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDomainParticipant {}

// ============================================================================
// Topic
// ============================================================================

/// Topic (Spec §2.2.2.3.1).
pub struct ZeroDdsTopic {
    pub participant: *mut ZeroDdsDomainParticipant,
    pub name: String,
    pub type_name: String,
    pub qos: Mutex<TopicQos>,
    /// The topic's type, reconstructed from the `CompleteTypeObject` `zerodds_topic_set_type_objects`
    /// gave: a MultiTopic names the members of the topics of its `FROM` clause through it.
    pub support:
        Mutex<Option<std::sync::Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>>>,
    /// INCONSISTENT_TOPIC's total when the status was last read; its
    /// `total_count_change` counts from it (DDS 1.4 §2.2.4.1).
    pub inconsistent_topic_read: core::sync::atomic::AtomicI32,
    /// Enabled state (DDS 1.4 §2.2.2.1.1.7); see `entity_ffi`.
    pub lifecycle: crate::entity_ffi::Lifecycle,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsTopic {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsTopic {}

impl Drop for ZeroDdsTopic {
    fn drop(&mut self) {
        forget_status_entity(self as *const Self);
    }
}

// ============================================================================
// ContentFilteredTopic
// ============================================================================

/// ContentFilteredTopic (Spec §2.2.2.3.3).
pub struct ZeroDdsContentFilteredTopic {
    pub participant: *mut ZeroDdsDomainParticipant,
    pub related_topic: *mut ZeroDdsTopic,
    pub name: String,
    pub filter_expression: String,
    pub parameters: Mutex<Vec<String>>,
    /// Positional CDR field schema for the untyped C-FFI: set via
    /// `zerodds_cft_set_schema` so the filter expression evaluates against
    /// real decoded members instead of falling back to pass-through.
    pub schema: Mutex<Vec<CftField>>,
    /// Type extensibility of the payload (XTypes 1.3 §7.3.1.2.1). In XCDR2 an
    /// `@appendable`/`@mutable` aggregate is prefixed with a 4-byte DHEADER
    /// (§7.4.3.4.2) before the first member, so the positional schema must
    /// skip those 4 bytes; a `@final` type has no DHEADER (offset 0).
    pub extensibility: Mutex<CftExtensibility>,
    /// The related topic's type, reconstructed from its `CompleteTypeObject`
    /// (`zerodds_cft_set_type_object`). When set, the filter reads every
    /// field by name from the decoded sample, and the positional schema is
    /// not used.
    pub typed: Mutex<Option<std::sync::Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>>>,
    /// `parameters` as the filter reads them (DDS 1.4 Annex B literals),
    /// shared with every reader created on this topic.
    pub values: std::sync::Arc<std::sync::RwLock<Vec<zerodds_sql_filter::Value>>>,
    /// The readers created on this topic, whose SEDP subscriptions carry the
    /// filter and are announced again when the parameters change.
    pub readers: Mutex<Vec<(std::sync::Arc<DcpsRuntime>, EntityId)>>,
    /// How many DataReaders created on this topic exist.
    /// `zerodds_dp_delete_contentfilteredtopic` refuses while it is not 0
    /// (DDS 1.4 §2.2.2.2.1.8).
    pub live_readers: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// One DataReader's hold on its ContentFilteredTopic, given back when the
/// reader is freed by whichever path frees it.
pub struct CftReaderHold {
    live_readers: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl CftReaderHold {
    /// Counts one more reader on `live_readers`.
    #[must_use]
    pub fn new(live_readers: &std::sync::Arc<std::sync::atomic::AtomicUsize>) -> Self {
        live_readers.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Self {
            live_readers: std::sync::Arc::clone(live_readers),
        }
    }
}

impl Drop for CftReaderHold {
    fn drop(&mut self) {
        self.live_readers
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// Payload extensibility for the untyped CFT positional decoder
/// (XTypes 1.3 §7.3.1.2.1). Determines the byte offset at which the first
/// schema member starts in an XCDR2 stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CftExtensibility {
    /// `@final` — no DHEADER; the first member starts at offset 0.
    #[default]
    Final,
    /// `@appendable`/`@mutable` — a 4-byte DHEADER (UInt32 object length,
    /// XCDR2 §7.4.3.4.2) precedes the members; the schema starts at offset 4.
    Appendable,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsContentFilteredTopic {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsContentFilteredTopic {}

// ============================================================================
// Publisher
// ============================================================================

/// Publisher (Spec §2.2.2.4.1).
pub struct ZeroDdsPublisher {
    pub participant: *mut ZeroDdsDomainParticipant,
    pub qos: Mutex<PublisherQos>,
    pub default_dw_qos: Mutex<DataWriterQos>,
    pub datawriters: Mutex<Vec<*mut ZeroDdsDataWriter>>,
    /// `suspend_publications` calls not yet matched by a
    /// `resume_publications` (DDS 1.4 §2.2.2.4.1.8, §2.2.2.4.1.9).
    pub suspended: Mutex<u32>,
    /// `begin_coherent_changes` calls not yet matched by an
    /// `end_coherent_changes` (DDS 1.4 §2.2.2.4.1.10: "These calls can be
    /// nested"); above zero a coherent set is open on every writer.
    pub coherent_depth: Mutex<u32>,
    /// The Publisher's `EntityId`, a Writer Group (DDSI-RTPS 2.5 Table
    /// 9.1); its writers announce it as `PID_GROUP_GUID` (Table 9.19).
    /// `None` without a runtime.
    pub entity_id: Option<EntityId>,
    /// With `PRESENTATION.access_scope = GROUP`, what the writers share
    /// (DDSI-RTPS 2.5 §8.7.5): created with the first writer.
    pub group: Mutex<Option<Arc<zerodds_rtps::writer_group::WriterGroup>>>,
    /// Enabled state (DDS 1.4 §2.2.2.1.1.7); see `entity_ffi`.
    pub lifecycle: crate::entity_ffi::Lifecycle,
    /// Stable backing for PARTITION names emitted by `get_qos` /
    /// `get_default_datawriter_qos` (see `qos_ffi::PartitionOutCache`).
    pub partition_out: Mutex<crate::qos_ffi::PartitionOutCache>,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsPublisher {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsPublisher {}

// ============================================================================
// Subscriber
// ============================================================================

/// Subscriber (Spec §2.2.2.5.1).
pub struct ZeroDdsSubscriber {
    pub participant: *mut ZeroDdsDomainParticipant,
    pub qos: Mutex<SubscriberQos>,
    pub default_dr_qos: Mutex<DataReaderQos>,
    pub datareaders: Mutex<Vec<*mut ZeroDdsDataReader>>,
    /// Stable backing for PARTITION names emitted by `get_qos` /
    /// `get_default_datareader_qos` (see `qos_ffi::PartitionOutCache`).
    pub partition_out: Mutex<crate::qos_ffi::PartitionOutCache>,
    /// The Subscriber's `EntityId`, a Reader Group (DDSI-RTPS 2.5 Table
    /// 9.1); its readers announce it as `PID_GROUP_GUID` (Table 9.19).
    /// `None` without a runtime.
    pub entity_id: Option<EntityId>,
    /// The GROUP order across the readers, shared with the runtime's group
    /// hook.
    pub group: Arc<SubscriberGroup>,
    /// The hook the runtime runs on group progress (a HEARTBEAT, a GAP, the
    /// tick), held here; the runtime holds it weakly.
    pub group_hook: Mutex<Option<zerodds_dcps::runtime::StatusHook>>,
    /// `begin_access` calls not yet matched by an `end_access` (DDS 1.4
    /// §2.2.2.5.2.8, §2.2.2.5.2.9).
    pub access: zerodds_dcps::coherent_set::GroupAccessScope,
    /// Enabled state (DDS 1.4 §2.2.2.1.1.7); see `entity_ffi`.
    pub lifecycle: crate::entity_ffi::Lifecycle,
}

/// The samples a release of a [`SubscriberGroup`] handed one reader.
pub type GroupInbox = Mutex<Vec<UserSample>>;

/// With `PRESENTATION.access_scope = GROUP` and ordered or coherent access,
/// what a Subscriber orders a GROUP-scope publisher's samples across its
/// readers with (DDSI-RTPS 2.5 §8.7.5, §8.7.6), as the typed `Subscriber`
/// does: the order, and the inbox of each reader a release hands its
/// samples to.
#[derive(Debug, Default)]
pub struct SubscriberGroup {
    pub order: zerodds_dcps::presentation::GroupOrdering<EntityId>,
    /// The readers, each with its inbox; a deleted reader's inbox is gone.
    pub inboxes: Mutex<Vec<(EntityId, alloc::sync::Weak<GroupInbox>)>>,
}

impl SubscriberGroup {
    /// Commits what the order allows into the inboxes of the readers.
    pub fn release(&self, rt: &Arc<DcpsRuntime>) {
        let inboxes: Vec<(EntityId, Arc<GroupInbox>)> = match self.inboxes.lock() {
            Ok(mut list) => {
                list.retain(|(_, inbox)| inbox.strong_count() > 0);
                list.iter()
                    .filter_map(|(eid, inbox)| Some((*eid, inbox.upgrade()?)))
                    .collect()
            }
            Err(_) => return,
        };
        let readers: Vec<(EntityId, EntityId)> = inboxes.iter().map(|(e, _)| (*e, *e)).collect();
        self.order.release(rt, &readers, |reader, item| {
            inboxes
                .iter()
                .find(|(eid, _)| eid == reader)
                .is_some_and(|(_, inbox)| match inbox.lock() {
                    Ok(mut inbox) => {
                        inbox.push(item);
                        true
                    }
                    Err(_) => false,
                })
        });
    }
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsSubscriber {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsSubscriber {}

impl Drop for ZeroDdsSubscriber {
    fn drop(&mut self) {
        forget_status_entity(self as *const Self);
    }
}

// ============================================================================
// DataWriter
// ============================================================================

/// DataWriter (Spec §2.2.2.4.2).
pub struct ZeroDdsDataWriter {
    /// Full-key instance identities shared by write and lifecycle entry points.
    pub instances: InstanceTracker,
    pub publisher: *mut ZeroDdsPublisher,
    pub topic: *mut ZeroDdsTopic,
    pub rt: Arc<DcpsRuntime>,
    pub eid: EntityId,
    pub qos: Mutex<DataWriterQos>,
    /// Stable backing for PARTITION names emitted by `dw_get_qos`
    /// (see `qos_ffi::PartitionOutCache`).
    pub partition_out: Mutex<crate::qos_ffi::PartitionOutCache>,
    /// Enabled state (DDS 1.4 §2.2.2.1.1.7); see `entity_ffi`.
    pub lifecycle: crate::entity_ffi::Lifecycle,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDataWriter {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDataWriter {}

/// Deleting the handle deletes the writer: the runtime stops announcing it
/// and sends its SEDP dispose, so a remote reader unmatches it (DDS 1.4
/// `Publisher::delete_datawriter`), as the typed `DataWriter`'s `Drop`
/// does. Every path that frees a `ZeroDdsDataWriter` —
/// `zerodds_pub_delete_datawriter`, `delete_contained_entities` — used to
/// free the handle alone, and the writer lived on in the runtime: still
/// matched, still counted alive by every remote reader.
impl Drop for ZeroDdsDataWriter {
    fn drop(&mut self) {
        forget_status_entity(self as *const Self);
        self.rt.remove_user_writer(self.eid);
    }
}

// ============================================================================
// DataReader
// ============================================================================

/// DataReader (Spec §2.2.2.5.2).
pub struct ZeroDdsDataReader {
    pub subscriber: *mut ZeroDdsSubscriber,
    pub topic: *mut ZeroDdsTopic,
    pub rt: Arc<DcpsRuntime>,
    pub eid: EntityId,
    pub qos: Mutex<DataReaderQos>,
    pub rx: Mutex<mpsc::Receiver<UserSample>>,
    /// Members of open coherent sets, held back until their set ends (DDS
    /// 1.4 §2.2.3.6, DDSI-RTPS 2.5 §8.7.6).
    pub coherent_hold: zerodds_dcps::presentation::CoherentHold,
    /// What the Subscriber's GROUP order released to this reader
    /// (DDSI-RTPS 2.5 §8.7.5), taken after the channel.
    pub group_inbox: Arc<GroupInbox>,
    /// Local read cache for non-destructive `read()` (Spec §2.2.2.5.3).
    /// Per sample `(sample, sample_state, info)` is stored; `take` pulls
    /// from cache+channel and removes; `read` reads from cache+channel
    /// without removing, but marks the sample state as READ.
    ///
    /// `info` is baked exactly once, when the sample is first pulled off
    /// `rx` (by `subscriber_ffi::drain_fresh_into_read_cache` or
    /// `condition_ffi::drain_channel_into_cache`) — the same "resolve at
    /// arrival, never again" rule the typed `DataReader`'s
    /// `ingest_into_cache` follows (`crates/dcps/src/subscriber.rs`).
    /// Re-resolving an already-cached entry on every subsequent
    /// `read()`/`take()` call would re-run `InstanceTracker::note_writer`
    /// /`ensure_registered_limited` against possibly-stale wire state (e.g.
    /// after that instance's last writer has since gone) and could
    /// spuriously reactivate an instance this reader already correctly
    /// declared `NOT_ALIVE_DISPOSED`/`NOT_ALIVE_NO_WRITERS`. Only
    /// `sample_state` (the 2nd element) and `SampleInfo`'s three rank
    /// fields (computed once per returned batch, not per cache entry) are
    /// still mutable after baking.
    pub read_cache: Mutex<Vec<(UserSample, ReadSampleState, DcpsSampleInfo)>>,
    /// Resolved samples a RELIABLE reader's RESOURCE_LIMITS refused (DDS
    /// 1.4 §2.2.3.19, §2.2.4.1: "a rejected sample may be accepted in the
    /// future"), with what arrived after them, in arrival order; the next
    /// ingest takes them first.
    pub held_back: Mutex<Vec<(UserSample, DcpsSampleInfo)>>,
    /// Optional: ContentFilteredTopic filter active. If Some,
    /// `take`/`read` evaluate every sample against the filter (Spec §2.2.2.3.3).
    /// Untyped topics (RawBytes/String) return true for all filters,
    /// because no type info is present (vendor decision: pass-through
    /// instead of block-all).
    pub cft_filter: Option<CftFilter>,
    /// The hold on the ContentFilteredTopic the reader was created on
    /// (DDS 1.4 §2.2.2.2.1.8).
    pub cft_hold: Option<CftReaderHold>,
    /// The reader's type, reconstructed from its `CompleteTypeObject`
    /// (`zerodds_dr_set_type_objects`). A QueryCondition reads the fields its
    /// expression names through it.
    pub query_type:
        Mutex<Option<std::sync::Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>>>,
    /// Reader OWNERSHIP kind (Spec §2.2.3.23). EXCLUSIVE → the take path
    /// arbitrates per instance through [`InstanceTracker`] so only samples
    /// from the strongest writer reach the binding.
    pub ownership: OwnershipKind,
    /// Per-instance state tracker (Spec §2.2.2.5.4 + §2.2.3.23). Holds the
    /// stable `InstanceHandle` per instance (surfaced in `SampleInfo`) and
    /// the current exclusive-ownership owner. Untyped C-FFI derives the
    /// per-instance KeyHash from the sample payload (no IDL key info).
    pub instances: InstanceTracker,
    /// Stable backing for PARTITION names emitted by `dr_get_qos`
    /// (see `qos_ffi::PartitionOutCache`).
    pub partition_out: Mutex<crate::qos_ffi::PartitionOutCache>,
    /// Enabled state (DDS 1.4 §2.2.2.1.1.7); see `entity_ffi`.
    pub lifecycle: crate::entity_ffi::Lifecycle,
}

/// Primitive CDR field kind for the untyped C-FFI ContentFilteredTopic
/// row decoder. The C-FFI carries no IDL type info, so a filter on a real
/// sample payload needs an explicit positional schema declaring how the
/// leading members are encoded (Spec §2.2.2.3.3 — the filter expression
/// references the topic's fields by name; here the name→ordinal mapping is
/// supplied through the schema).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CftFieldKind {
    /// 1-byte boolean (CDR `boolean`, §9.3.2.2).
    Bool,
    /// 4-byte signed integer (`long`).
    Int32,
    /// 8-byte signed integer (`long long`).
    Int64,
    /// 4-byte IEEE float (`float`).
    Float32,
    /// 8-byte IEEE float (`double`).
    Float64,
    /// length-prefixed CDR `string` (§9.3.2.7).
    StringField,
}

/// One named field in the CFT row schema, decoded in declaration order.
#[derive(Debug, Clone)]
pub struct CftField {
    /// Field name as referenced by the filter expression.
    pub name: String,
    /// CDR encoding of the field.
    pub kind: CftFieldKind,
}

/// ContentFilteredTopic filter in the DataReader.
pub struct CftFilter {
    /// Parsed filter expression from `crates/sql-filter`.
    pub expr: zerodds_sql_filter::Expr,
    /// Filter parameters %0..%N as a `Value` vector.
    /// Filter parameters %0..%N, shared with the ContentFilteredTopic so
    /// `zerodds_cft_set_filter_parameters` reaches readers already created.
    pub params: std::sync::Arc<std::sync::RwLock<Vec<zerodds_sql_filter::Value>>>,
    /// Optional positional schema describing the leading CDR members of the
    /// payload. When present, the filter is evaluated against the decoded
    /// fields (genuinely filters); when absent, the untyped pass-through
    /// applies. Encoding follows the wire payload: XCDR (little-endian)
    /// without the encapsulation header (the same-runtime path strips it).
    pub schema: Vec<CftField>,
    /// Payload extensibility (XTypes 1.3 §7.3.1.2.1): for `Appendable` the
    /// positional decoder skips the leading 4-byte XCDR2 DHEADER
    /// (§7.4.3.4.2) so the first schema member resolves at the right offset.
    pub extensibility: CftExtensibility,
    /// The type reconstructed from its `CompleteTypeObject`; takes
    /// precedence over `schema`.
    pub typed: Option<std::sync::Arc<zerodds_dcps::serialized_type::SerializedTypeSupport>>,
}

/// `RowAccess` over a sample decoded through its `CompleteTypeObject`.
pub(crate) struct TypedRow<'a>(pub(crate) &'a zerodds_dcps::serialized_type::SerializedSample);

impl zerodds_sql_filter::RowAccess for TypedRow<'_> {
    fn get(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        use zerodds_dcps::dds_type::DdsType;
        self.0.field_value(path)
    }
}

/// `RowAccess` over a raw CDR payload using a positional `CftField` schema.
/// Decodes the requested field lazily by walking the leading members in
/// declaration order until the named field is reached.
struct CdrRow<'a> {
    payload: &'a [u8],
    schema: &'a [CftField],
    extensibility: CftExtensibility,
    /// The body is XCDR1 (MAXALIGN 8, no DHEADER) rather than XCDR2.
    xcdr1: bool,
    big_endian: bool,
}

impl zerodds_sql_filter::RowAccess for CdrRow<'_> {
    fn get(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        use zerodds_sql_filter::Value;
        use zerodds_wire::{Endianness, Reader};
        // XCDR2 little-endian over the bare body (no encap header). The
        // same-runtime dispatch hands the reader the CDR body directly.
        //
        // `max_align = 4` is XCDR2's MAXALIGN and is passed explicitly.
        // `Reader::new` defaults to 8, which is XCDR1's; the reader that
        // preceded this one carried the 4 in a `.xcdr2()` call, so the
        // number has to be restated here or every eight-byte member of a
        // filtered sample is read from the wrong offset — silently, and
        // only for types that have one.
        // An XCDR1 body aligns eight-byte members to 8 and carries no
        // DHEADER; the byte order is the one its encapsulation header named.
        let endianness = if self.big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        };
        let mut r =
            Reader::with_max_align(self.payload, endianness, if self.xcdr1 { 8 } else { 4 });
        // For an @appendable/@mutable aggregate the XCDR2 stream begins with a
        // 4-byte DHEADER (UInt32 object length, XTypes 1.3 §7.4.3.4.2) before
        // the first member; consume it so the schema offsets line up. A @final
        // type has no DHEADER (offset 0).
        if !self.xcdr1 && matches!(self.extensibility, CftExtensibility::Appendable) {
            r.read_u32().ok()?;
        }
        for field in self.schema {
            let want = field.name == path;
            match field.kind {
                CftFieldKind::Bool => {
                    let v = r.read_u8().ok()?;
                    if want {
                        return Some(Value::Bool(v != 0));
                    }
                }
                CftFieldKind::Int32 => {
                    let v = r.read_u32().ok()? as i32;
                    if want {
                        return Some(Value::Int(v as i64));
                    }
                }
                CftFieldKind::Int64 => {
                    let v = r.read_u64().ok()? as i64;
                    if want {
                        return Some(Value::Int(v));
                    }
                }
                CftFieldKind::Float32 => {
                    let v = f32::from_bits(r.read_u32().ok()?);
                    if want {
                        return Some(Value::Float(v as f64));
                    }
                }
                CftFieldKind::Float64 => {
                    let v = f64::from_bits(r.read_u64().ok()?);
                    if want {
                        return Some(Value::Float(v));
                    }
                }
                CftFieldKind::StringField => {
                    let v = r.read_string().ok()?;
                    if want {
                        return Some(Value::String(v));
                    }
                }
            }
        }
        None
    }
}

impl CftFilter {
    /// The parameters now; empty when their lock is poisoned, and then a
    /// parameter the expression names does not resolve and nothing passes.
    fn current_params(&self) -> Vec<zerodds_sql_filter::Value> {
        self.params.read().map(|p| p.clone()).unwrap_or_default()
    }

    /// Whether an `Alive` sample passes. With the type known from its
    /// `CompleteTypeObject` the body is decoded in the representation and
    /// byte order its encapsulation header named (`representation` 0 =
    /// XCDR1, 1 = XCDR2, as `UserSample::Alive` carries it); a body that
    /// does not decode, or a field the filter cannot resolve, is not
    /// delivered (DDS 1.4 §2.2.2.3.3).
    #[must_use]
    pub fn accepts(&self, payload: &[u8], representation: u8, big_endian: bool) -> bool {
        let Some(typed) = &self.typed else {
            return self.evaluate_positional(payload, representation == 0, big_endian);
        };
        let representation =
            zerodds_dcps::serialized_type::read_body_representation(representation);
        match typed.read_body(payload, representation, big_endian) {
            Ok(sample) => self
                .expr
                .evaluate(&TypedRow(&sample), &self.current_params())
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    /// [`Self::evaluate_positional`] for an XCDR2 little-endian body.
    #[must_use]
    pub fn evaluate(&self, payload: &[u8]) -> bool {
        self.evaluate_positional(payload, false, false)
    }

    /// Evaluates the filter against a sample payload through the positional
    /// `schema` (Spec §2.2.2.3.3): the leading members are decoded in the
    /// representation and byte order the sample came in, and the SQL
    /// expression is applied to them (`seq > 4` rejects a sample whose `seq`
    /// is `<= 4`). A sample the expression cannot be evaluated against
    /// (truncated payload, unknown field, type mismatch) is not delivered.
    /// Without a schema only an expression that reads no field gets here;
    /// `zerodds_sub_create_datareader_with_cft` refuses the others.
    #[must_use]
    pub fn evaluate_positional(&self, payload: &[u8], xcdr1: bool, big_endian: bool) -> bool {
        let row = CdrRow {
            payload,
            schema: &self.schema,
            extensibility: self.extensibility,
            xcdr1,
            big_endian,
        };
        self.expr
            .evaluate(&row, &self.current_params())
            .unwrap_or(false)
    }
}

/// Local sample state in the read cache (Spec §2.2.2.5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadSampleState {
    /// Fresh from the channel — sample_state = NOT_READ (bit 2).
    NotRead,
    /// Already read via `read()` — sample_state = READ (bit 1).
    Read,
}

// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Send for ZeroDdsDataReader {}
// SAFETY: FFI-boundary; pointer validity is the caller's contract per crate-level docs.
unsafe impl Sync for ZeroDdsDataReader {}

/// Deleting the handle deletes the reader in the runtime too (DDS 1.4
/// `Subscriber::delete_datareader`); see the `ZeroDdsDataWriter` drop.
impl Drop for ZeroDdsDataReader {
    fn drop(&mut self) {
        forget_status_entity(self as *const Self);
        self.rt.remove_user_reader(self.eid);
    }
}

// ============================================================================
// Helper functions for FFI pointer validation
// ============================================================================

/// Safe cast `*mut T` → `&T` with a NULL check.
///
/// # Safety
/// The caller must guarantee that `p` is either NULL or points to a
/// valid `Box<T>` allocation that has not yet been freed.
pub unsafe fn handle_ref<T>(p: *mut T) -> Option<&'static T> {
    if p.is_null() {
        None
    } else {
        // SAFETY: NULL check above + caller contract.
        Some(unsafe { &*p })
    }
}
