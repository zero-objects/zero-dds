// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Publisher + DataWriter — the send end of the DCPS API.
//!
//! Spec reference: OMG DDS 1.4 §2.2.2.4 `Publisher`, §2.2.2.4.2
//! `DataWriter`.
//!
//! # Scope v1.2
//!
//! - `Publisher::create_datawriter<T>(topic, qos)` → `DataWriter<T>`.
//! - `DataWriter::write(&sample)` encodes via `T::encode` and hands off
//!   to an **in-memory queue** (wiring to the ReliableWriter happens in
//!   the runtime).
//! - No matching against remote readers yet.
//! - No QoS conflict check yet.
//!
//! # Thread safety
//!
//! `DataWriter` is `Send`+`Sync` via `Arc<Mutex<_>>`. Multiple
//! application threads may call `write()` in parallel.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::sync::Mutex;

use crate::dds_type::DdsType;
use crate::entity::StatusMask;
use crate::error::{DdsError, Result};
#[cfg(feature = "std")]
use crate::instance_handle::{HANDLE_NIL, InstanceHandle};
#[cfg(feature = "std")]
use crate::instance_tracker::InstanceTracker;
use crate::listener::{ArcDataWriterListener, ArcPublisherListener};
use crate::qos::{DataWriterQos, PublisherQos};
#[cfg(feature = "std")]
use crate::time::{Time, get_current_time};
use crate::topic::Topic;

#[cfg(feature = "std")]
use crate::runtime::DcpsRuntime;
#[cfg(feature = "std")]
use zerodds_qos::ReliabilityKind;
#[cfg(feature = "std")]
use zerodds_rtps::wire_types::EntityId;

/// Publisher — entity group for DataWriters.
///
/// In DDS 1.4 the publisher has its own QoS (Partition, GroupData,
/// Presentation). v1.2 implements only the API shape without partition
/// matching.
#[derive(Debug)]
pub struct Publisher {
    pub(crate) inner: Arc<PublisherInner>,
}

pub(crate) struct PublisherInner {
    /// Mutable QoS. .1 (entity lifecycle): set_qos checks immutability
    /// after enable().
    #[cfg(feature = "std")]
    pub(crate) qos: std::sync::Mutex<PublisherQos>,
    #[cfg(not(feature = "std"))]
    #[allow(dead_code)]
    pub(crate) qos: PublisherQos,
    /// Entity lifecycle (DCPS §2.2.2.1).
    pub(crate) entity_state: alloc::sync::Arc<crate::entity::EntityState>,
    /// Runtime handle (when the publisher was created by a live
    /// participant). None in offline mode → DataWriters fall back to the
    /// in-memory queue.
    #[cfg(feature = "std")]
    pub(crate) runtime: Option<Arc<DcpsRuntime>>,
    /// optionaler [`ArcPublisherListener`] + [`StatusMask`]
    /// (Spec §2.2.2.4.3.x set_listener / Bubble-Up §2.2.4.2.3).
    #[cfg(feature = "std")]
    pub(crate) listener: std::sync::Mutex<Option<(ArcPublisherListener, StatusMask)>>,
    /// Weak back-pointer to the participant — for bubble-up from the
    /// publisher to the participant. Set by
    /// `DomainParticipant::create_publisher`. `Weak` avoids a refcount
    /// cycle participant↔publisher.
    #[cfg(feature = "std")]
    pub(crate) participant:
        std::sync::Mutex<Option<alloc::sync::Weak<crate::participant::ParticipantInner>>>,
    /// `suspend_publications` flag (Spec §2.2.2.4.1.10). When `true`, the
    /// publisher has hinted that writes should be buffered — the writer
    /// may use that as an optimization hint (e.g. coalescing). Not
    /// strictly enforced, because the spec explicitly defines it as a
    /// "hint to the Service".
    suspended: core::sync::atomic::AtomicBool,
    /// DataWriter handles (tracked per `create_datawriter`) for recursive
    /// `DomainParticipant::contains_entity` (Spec §2.2.2.2.1.10).
    #[cfg(feature = "std")]
    pub(crate) datawriters:
        std::sync::Mutex<alloc::vec::Vec<crate::instance_handle::InstanceHandle>>,
}

// Manual Debug impl, because `dyn PublisherListener` does not implement
// Debug. We only print "Some/None" and the mask.
impl core::fmt::Debug for PublisherInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let listener_present = self.listener.lock().map(|s| s.is_some()).unwrap_or(false);
        f.debug_struct("PublisherInner")
            .field("entity_state", &self.entity_state)
            .field("listener_present", &listener_present)
            .finish_non_exhaustive()
    }
}

impl Publisher {
    #[cfg(feature = "std")]
    pub(crate) fn new(qos: PublisherQos, runtime: Option<Arc<DcpsRuntime>>) -> Self {
        Self {
            inner: Arc::new(PublisherInner {
                qos: std::sync::Mutex::new(qos),
                entity_state: crate::entity::EntityState::new(),
                runtime,
                listener: std::sync::Mutex::new(None),
                participant: std::sync::Mutex::new(None),
                suspended: core::sync::atomic::AtomicBool::new(false),
                datawriters: std::sync::Mutex::new(alloc::vec::Vec::new()),
            }),
        }
    }

    #[cfg(not(feature = "std"))]
    pub(crate) fn new(qos: PublisherQos) -> Self {
        Self {
            inner: Arc::new(PublisherInner {
                qos,
                entity_state: crate::entity::EntityState::new(),
                suspended: core::sync::atomic::AtomicBool::new(false),
            }),
        }
    }

    /// Spec §2.2.2.2.1.10 — `true` if `handle` is a DataWriter created
    /// through this publisher.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn contains_writer(&self, handle: crate::instance_handle::InstanceHandle) -> bool {
        self.inner
            .datawriters
            .lock()
            .map(|v| v.contains(&handle))
            .unwrap_or(false)
    }

    /// Sets the `PublisherListener` + StatusMask. `None` clears the slot.
    /// Spec §2.2.2.4.3.x.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcPublisherListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
    }

    /// Current listener clone, if present.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcPublisherListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Sets the weak back-pointer to the participant. Called by
    /// `DomainParticipant::create_publisher`.
    #[cfg(feature = "std")]
    pub(crate) fn attach_participant(
        &self,
        participant: alloc::sync::Weak<crate::participant::ParticipantInner>,
    ) {
        if let Ok(mut slot) = self.inner.participant.lock() {
            *slot = Some(participant);
        }
    }

    /// Returns the [`crate::listener_dispatch::WriterListenerChain`] for
    /// a writer of this publisher — the reader-path counterpart in
    /// Subscriber. Clones all three listener stages under their mutexes
    /// and returns the bundle lock-free (lock discipline).
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn snapshot_writer_chain(
        &self,
        writer_listener: Option<(ArcDataWriterListener, StatusMask)>,
    ) -> crate::listener_dispatch::WriterListenerChain {
        let publisher = self
            .inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
        let participant = {
            let weak = self.inner.participant.lock().ok().and_then(|s| s.clone());
            weak.and_then(|w| w.upgrade()).and_then(|inner| {
                inner
                    .listener
                    .lock()
                    .ok()
                    .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)))
            })
        };
        crate::listener_dispatch::WriterListenerChain {
            writer: writer_listener,
            publisher,
            participant,
        }
    }

    /// Spec §2.2.2.4.1.10 `suspend_publications` — a hint to the Service
    /// that subsequent `write()` calls may be buffered (e.g. for
    /// coalescing). Has no mandatory semantics for the caller; the flag
    /// is readable via `is_suspended()` for the writer implementation.
    ///
    /// Idempotent: a second `suspend_publications()` without a
    /// `resume_publications()` in between is allowed.
    pub fn suspend_publications(&self) {
        self.inner
            .suspended
            .store(true, core::sync::atomic::Ordering::Release);
    }

    /// Spec §2.2.2.4.1.11 `resume_publications` — counterpart to
    /// `suspend_publications`. With the suspend flag active the behavior
    /// is "Service can stop coalescing"; with the flag inactive it is a
    /// no-op.
    pub fn resume_publications(&self) {
        self.inner
            .suspended
            .store(false, core::sync::atomic::Ordering::Release);
    }

    /// `true` if `suspend_publications()` is active and
    /// `resume_publications()` has not yet been called. Read by the
    /// writer send path as a coalescing hint.
    #[must_use]
    pub fn is_suspended(&self) -> bool {
        self.inner
            .suspended
            .load(core::sync::atomic::Ordering::Acquire)
    }

    /// Spec §2.2.2.4.1.13 `copy_from_topic_qos` — copies the policies
    /// shareable between topic and DataWriter QoS from `topic_qos` into
    /// `dw_qos`. The spec's list of common policies (DCPS 1.4
    /// §2.2.2.4.1.13): DURABILITY, DEADLINE, LATENCY_BUDGET, LIVELINESS,
    /// RELIABILITY, DESTINATION_ORDER, HISTORY, RESOURCE_LIMITS,
    /// TRANSPORT_PRIORITY, LIFESPAN, OWNERSHIP.
    ///
    /// # Errors
    /// `DdsError::BadParameter` if the result yields an inconsistent QoS
    /// combination — validated by the caller DataWriter's
    /// QoS-compatibility check (analogous to `set_qos`).
    pub fn copy_from_topic_qos(
        dw_qos: &mut DataWriterQos,
        topic_qos: &crate::qos::TopicQos,
    ) -> Result<()> {
        // The following fields are present in both QoS structs and are
        // overwritten 1:1. DataWriter-only policies (OWNERSHIP_STRENGTH,
        // PARTITION, RESOURCE_LIMITS, HISTORY, LIFESPAN, DEADLINE,
        // LIVELINESS, OWNERSHIP) are left untouched, because TopicQos
        // currently does not carry them. If TopicQos is extended with one
        // of these fields, this list MUST be extended too — Spec
        // §2.2.2.4.1.13.
        dw_qos.durability = topic_qos.durability;
        dw_qos.reliability = topic_qos.reliability;
        Ok(())
    }

    /// Creates a typed `DataWriter<T>`. Spec §2.2.2.4.1.5
    /// `create_datawriter`.
    ///
    /// # Errors
    /// - `BadParameter` if `topic.type_name() != T::TYPE_NAME` (should be
    ///   statically guaranteed, but we check defensively).
    pub fn create_datawriter<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: DataWriterQos,
    ) -> Result<DataWriter<T>> {
        if topic.type_name() != T::TYPE_NAME {
            return Err(DdsError::BadParameter {
                what: "topic.type_name mismatch",
            });
        }
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            // Live mode: register a real user writer with the runtime.
            // Matching and user-data flow run over SEDP + UDP from now on.
            let reliable = qos.reliability.kind == ReliabilityKind::Reliable;
            // Derive the entityKind from the type keyedness (Spec §9.3.1.2:
            // 0x02=WithKey / 0x03=NoKey). A keyless type (`HAS_KEY=false`)
            // MUST produce a NoKey writer, otherwise cross-vendor readers
            // (CycloneDDS/ROS 2) reject the endpoint match due to an
            // entityKind mismatch (keyed vs no-key) — silently, without a
            // log.
            let eid = rt.register_user_writer_kind(
                crate::runtime::UserWriterConfig {
                    topic_name: topic.name().into(),
                    type_name: T::TYPE_NAME.into(),
                    reliable,
                    durability: qos.durability.kind,
                    deadline: qos.deadline,
                    lifespan: qos.lifespan,
                    liveliness: qos.liveliness,
                    ownership: qos.ownership.kind,
                    ownership_strength: qos.ownership_strength.value,
                    partition: qos.partition.names.clone(),
                    user_data: qos.user_data.value.clone(),
                    topic_data: qos.topic_data.value.clone(),
                    group_data: qos.group_data.value.clone(),
                    // F-TYPES-3: pass on the topic type identifier.
                    type_identifier: T::TYPE_IDENTIFIER.clone(),
                    // Per-writer DataRepresentation override from the QoS
                    // (`None` = runtime default). XTypes 1.3 §7.6.3.1.2.
                    data_representation_offer: qos.data_representation.clone(),
                },
                T::HAS_KEY,
            )?;
            let dw =
                DataWriter::new_live(topic.clone(), qos, self.inner.clone(), Arc::clone(rt), eid);
            // Spec §2.2.3.5 — with Durability=Transient/Persistent, pass
            // the writer's own backend to the runtime so that the match
            // path re-injects the backend samples into the HistoryCache on
            // the first late-joiner match (see
            // `DcpsRuntime::attach_durability_backend`).
            if let Some(backend) = dw.durability_backend() {
                let _ = rt.attach_durability_backend(eid, backend);
            }
            self.track_writer(dw.entity_state.instance_handle());
            return Ok(dw);
        }
        let dw = DataWriter::new_offline(topic.clone(), qos, self.inner.clone());
        #[cfg(feature = "std")]
        self.track_writer(dw.entity_state.instance_handle());
        Ok(dw)
    }

    #[cfg(feature = "std")]
    fn track_writer(&self, handle: crate::instance_handle::InstanceHandle) {
        if let Ok(mut list) = self.inner.datawriters.lock() {
            list.push(handle);
        }
        // Propagate to the participant for recursive contains_entity.
        if let Ok(slot) = self.inner.participant.lock() {
            if let Some(weak) = slot.as_ref() {
                if let Some(p_inner) = weak.upgrade() {
                    if let Ok(mut dws) = p_inner.datawriters.lock() {
                        dws.push(handle);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Entity trait (DCPS §2.2.2.1) —
// ============================================================================

#[cfg(feature = "std")]
impl crate::entity::Entity for Publisher {
    type Qos = PublisherQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    fn set_qos(&self, qos: Self::Qos) -> Result<()> {
        // PublisherQos has no immutable fields per DDS spec §2.2.3 —
        // Partition / GroupData / Presentation are all Changeable=YES.
        // So we can simply adopt them both pre- and post-enable.
        if let Ok(mut current) = self.inner.qos.lock() {
            *current = qos;
        }
        Ok(())
    }

    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable();
        Ok(())
    }

    fn entity_state(&self) -> alloc::sync::Arc<crate::entity::EntityState> {
        alloc::sync::Arc::clone(&self.inner.entity_state)
    }
}

/// Typed DataWriter — sends samples to all matched readers of the topic.
///
/// Two modes:
/// - **Live** (`runtime: Some`, `entity_id: Some`): write() delegates to
///   the runtime → ReliableWriter → UDP.
/// - **Offline** (offline fallback, runtime=None): write() queues
///   in-memory; for unit tests without a network.
pub struct DataWriter<T: DdsType> {
    topic: Topic<T>,
    qos: Mutex<DataWriterQos>,
    /// Entity lifecycle (DCPS §2.2.2.1).
    entity_state: Arc<crate::entity::EntityState>,
    /// Parent publisher (clone of the `Arc`) — for bubble-up to the
    /// publisher and participant listeners.
    publisher: Arc<PublisherInner>,
    /// Optional `DataWriterListener` + `StatusMask`.
    #[cfg(feature = "std")]
    listener: Mutex<Option<(ArcDataWriterListener, StatusMask)>>,
    /// Last seen number of matched readers. Used by `poll_status_changes`
    /// (called lazily from public-API paths) to drive delta detection for
    /// `on_publication_matched` — Spec §2.2.4.2.4.4.
    #[cfg(feature = "std")]
    last_match_count: std::sync::atomic::AtomicI64,
    /// Last seen offered_deadline_missed counter.
    #[cfg(feature = "std")]
    last_offered_deadline_missed: std::sync::atomic::AtomicU64,
    /// Last seen liveliness_lost counter.
    #[cfg(feature = "std")]
    last_liveliness_lost: std::sync::atomic::AtomicU64,
    /// Last seen offered_incompatible_qos.total_count.
    #[cfg(feature = "std")]
    last_offered_incompatible_qos: std::sync::atomic::AtomicI64,
    /// Offline fallback queue.
    queue: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Drain notify pair (Spec §2.2.3.19 RESOURCE_LIMITS reliable block).
    /// `write()` blocks on the condvar when the queue is full + RELIABLE +
    /// `max_blocking_time > 0`; `__drain_pending` notifies all waiting
    /// writer threads.
    #[cfg(feature = "std")]
    drain_signal: Arc<std::sync::Condvar>,
    #[cfg(feature = "std")]
    runtime: Option<Arc<DcpsRuntime>>,
    #[cfg(feature = "std")]
    entity_id: Option<EntityId>,
    /// Instance bookkeeping.
    #[cfg(feature = "std")]
    instances: InstanceTracker,
    /// Local publication handle — entered into
    /// `SampleInfo.publication_handle` on the reader side once live wiring
    /// takes effect.
    #[cfg(feature = "std")]
    publication_handle: InstanceHandle,
    /// Spec §2.2.3.5 DurabilityServiceQosPolicy: with
    /// Durability=Transient/Persistent the writer additionally stores
    /// samples in a backend so that late-joiner readers can still obtain
    /// them after the writer's history cleanup.
    #[cfg(feature = "std")]
    durability_backend: Option<Arc<dyn crate::durability_service::DurabilityBackend>>,
    /// Monotonically increasing writer sequence for durability-backend
    /// storage (DDS 1.4 §2.2.3.5 + backend replay order).
    #[cfg(feature = "std")]
    durability_seq: std::sync::atomic::AtomicU64,
    /// Optionally configured Flatdata SlotBackend for the same-host
    /// zero-copy path (`zerodds-flatdata-1.0` §4.1 + §8.1). Set via
    /// `set_flat_backend` (see the `flatdata_integration` module); with
    /// `None`, `write_flat()` falls back to the classic UDP path.
    #[cfg(all(feature = "std", feature = "flatdata-integration"))]
    pub(crate) flat_backend: Mutex<
        Option<(
            Arc<dyn zerodds_flatdata::SlotBackend>,
            u32, // active_readers_mask
        )>,
    >,
    _t: PhantomData<fn() -> T>,
}

impl<T: DdsType> core::fmt::Debug for DataWriter<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DataWriter")
            .field("topic", &self.topic.name())
            .field("type", &T::TYPE_NAME)
            .field("qos", &self.qos)
            .finish_non_exhaustive()
    }
}

impl<T: DdsType> DataWriter<T> {
    #[cfg(feature = "std")]
    fn new_offline(topic: Topic<T>, qos: DataWriterQos, publisher: Arc<PublisherInner>) -> Self {
        let tracker = InstanceTracker::new();
        let pub_handle = InstanceHandle::from_raw(0xFFFF_0000_0000_0001);
        let backend = Self::build_durability_backend(&qos);
        Self {
            topic,
            qos: Mutex::new(qos),
            entity_state: crate::entity::EntityState::new(),
            publisher,
            listener: Mutex::new(None),
            last_match_count: std::sync::atomic::AtomicI64::new(-1),
            last_offered_deadline_missed: std::sync::atomic::AtomicU64::new(0),
            last_liveliness_lost: std::sync::atomic::AtomicU64::new(0),
            last_offered_incompatible_qos: std::sync::atomic::AtomicI64::new(-1),
            queue: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            drain_signal: Arc::new(std::sync::Condvar::new()),
            runtime: None,
            entity_id: None,
            instances: tracker,
            publication_handle: pub_handle,
            durability_backend: backend,
            durability_seq: std::sync::atomic::AtomicU64::new(1),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            _t: PhantomData,
        }
    }

    /// Returns the durability backend (None for Volatile/TransientLocal).
    /// Test/inspection helper — Spec §2.2.3.5.
    #[doc(hidden)]
    #[cfg(feature = "std")]
    #[must_use]
    pub fn durability_backend(
        &self,
    ) -> Option<Arc<dyn crate::durability_service::DurabilityBackend>> {
        self.durability_backend.clone()
    }

    /// Spec §2.2.3.5: with Durability=Transient the writer creates an
    /// in-memory backend. Persistent without a root path is not
    /// auto-configured — the caller must `set_durability_backend`.
    /// Default path for Persistent: the `ZERODDS_DURABILITY_DIR` env var,
    /// otherwise `std::env::temp_dir().join("zerodds-durability")`. The
    /// caller can override the path via the env var for production
    /// deployments (e.g. `/var/lib/zerodds/durability`).
    #[cfg(feature = "std")]
    fn build_durability_backend(
        qos: &DataWriterQos,
    ) -> Option<Arc<dyn crate::durability_service::DurabilityBackend>> {
        match qos.durability.kind {
            zerodds_qos::DurabilityKind::Transient => Some(Arc::new(
                crate::durability_service::InMemoryDurabilityBackend::new(qos.durability_service),
            )),
            zerodds_qos::DurabilityKind::Persistent => {
                let root = std::env::var_os("ZERODDS_DURABILITY_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::temp_dir().join("zerodds-durability"));
                match crate::durability_service::OnDiskDurabilityBackend::new(
                    root,
                    qos.durability_service,
                ) {
                    Ok(b) => Some(Arc::new(b)),
                    Err(_) => None,
                }
            }
            _ => None,
        }
    }

    #[cfg(feature = "std")]
    fn new_live(
        topic: Topic<T>,
        qos: DataWriterQos,
        publisher: Arc<PublisherInner>,
        runtime: Arc<DcpsRuntime>,
        entity_id: EntityId,
    ) -> Self {
        let tracker = InstanceTracker::new();
        // We derive the publication handle from the EntityId — that makes it
        // reproducible across test runs and avoids a pool collision with
        // instance handles. The spec only says it is an opaque u64 anyway.
        let key = entity_id.entity_key;
        let pub_handle = InstanceHandle::from_raw(
            0xFFFF_0000_0000_0000
                | (u64::from(key[0]) << 16)
                | (u64::from(key[1]) << 8)
                | u64::from(key[2]),
        );
        let backend = Self::build_durability_backend(&qos);
        Self {
            topic,
            qos: Mutex::new(qos),
            entity_state: crate::entity::EntityState::new(),
            publisher,
            listener: Mutex::new(None),
            last_match_count: std::sync::atomic::AtomicI64::new(-1),
            last_offered_deadline_missed: std::sync::atomic::AtomicU64::new(0),
            last_liveliness_lost: std::sync::atomic::AtomicU64::new(0),
            last_offered_incompatible_qos: std::sync::atomic::AtomicI64::new(-1),
            queue: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            drain_signal: Arc::new(std::sync::Condvar::new()),
            runtime: Some(runtime),
            entity_id: Some(entity_id),
            instances: tracker,
            publication_handle: pub_handle,
            durability_backend: backend,
            durability_seq: std::sync::atomic::AtomicU64::new(1),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            _t: PhantomData,
        }
    }

    #[cfg(not(feature = "std"))]
    fn new(topic: Topic<T>, qos: DataWriterQos, publisher: Arc<PublisherInner>) -> Self {
        Self {
            topic,
            qos,
            publisher,
            queue: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            drain_signal: Arc::new(std::sync::Condvar::new()),
            _t: PhantomData,
        }
    }

    /// The topic that is sent to.
    #[must_use]
    pub fn topic(&self) -> &Topic<T> {
        &self.topic
    }

    /// Sets the `DataWriterListener` + StatusMask. `None` clears the slot.
    /// Spec §2.2.2.4.2.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcDataWriterListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.entity_state.set_listener_mask(mask);
    }

    /// Current listener clone, if present.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDataWriterListener> {
        self.listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Snapshot of the bubble-up chain (writer → publisher → participant).
    /// Used for hot-path listener dispatch.
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn listener_chain(&self) -> crate::listener_dispatch::WriterListenerChain {
        let writer = self
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
        // We pass the writer clone on to the publisher snapshot, which
        // fills in the publisher and participant stages.
        let pub_handle = Publisher {
            inner: Arc::clone(&self.publisher),
        };
        pub_handle.snapshot_writer_chain(writer)
    }

    /// Current QoS (cloned, .1).
    #[must_use]
    pub fn qos(&self) -> DataWriterQos {
        self.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Sends a sample to all matched readers.
    ///
    /// **Spec §2.2.3.19 RESOURCE_LIMITS reliable block:** If the local
    /// writer cache has reached `max_samples` AND Reliability=RELIABLE AND
    /// `max_blocking_time > 0`, `write()` blocks until a reader ACK frees
    /// the slot or the timeout expires. In best-effort mode or with
    /// `max_blocking_time = 0`, `write()` fails immediately with
    /// `OutOfResources`.
    ///
    /// # Errors
    /// - `WireError` if `T::encode` fails.
    /// - `OutOfResources` if the queue is full + best-effort/no blocking
    ///   time, or if the block timeout expired before a drain.
    /// - `PreconditionNotMet` on lock poisoning.
    pub fn write(&self, sample: &T) -> Result<()> {
        let mut buf = Vec::new();
        sample.encode(&mut buf).map_err(|e| DdsError::WireError {
            message: e.to_string(),
        })?;
        #[cfg(feature = "metrics")]
        crate::metrics::inc_sample_written(self.topic.name());
        #[cfg(feature = "metrics")]
        crate::metrics::record_sample_size(self.topic.name(), buf.len());
        // Spec §2.2.3.5 DurabilityServiceQosPolicy: also store the sample
        // in the backend (Transient/Persistent) so that late-joiner
        // readers can still obtain it after the writer's history cleanup.
        #[cfg(feature = "std")]
        if let Some(backend) = self.durability_backend.as_ref() {
            let key_bytes = Self::keyhash_and_holder(sample)
                .map(|(kh, _)| kh)
                .unwrap_or([0u8; 16]);
            // Monotonic writer sequence for backend replay order
            // (DDS 1.4 §2.2.3.5).
            let seq = self
                .durability_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let _ = backend.store(crate::durability_service::DurabilitySample {
                topic: self.topic.name().to_string(),
                instance_key: key_bytes,
                sequence: seq,
                payload: buf.clone(),
                created_at: std::time::SystemTime::now(),
            });
        }
        // Live mode: delegate to the runtime → ReliableWriter → UDP.
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            return rt.write_user_sample(eid, buf);
        }
        // Offline fallback: in-memory queue with RESOURCE_LIMITS block.
        #[cfg(feature = "std")]
        {
            let qos = self.qos.lock().map(|q| q.clone()).unwrap_or_default();
            let max_samples = qos.resource_limits.max_samples;
            let reliable = qos.reliability.kind == ReliabilityKind::Reliable;
            let max_block = qos.reliability.max_blocking_time;
            let max_block_dur = core::time::Duration::from_nanos(
                u64::try_from(max_block.seconds).unwrap_or(0) * 1_000_000_000
                    + u64::from(max_block.fraction),
            );

            let mut q = self
                .queue
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datawriter queue poisoned",
                })?;
            if max_samples > 0 && q.len() >= max_samples as usize {
                if !reliable || max_block_dur.is_zero() {
                    return Err(DdsError::OutOfResources {
                        what: "datawriter queue full (best-effort or no max_blocking_time)",
                    });
                }
                // Reliable + max_blocking_time > 0 → wait_timeout on the condvar.
                let deadline = std::time::Instant::now() + max_block_dur;
                loop {
                    let now = std::time::Instant::now();
                    if now >= deadline {
                        return Err(DdsError::Timeout);
                    }
                    let remaining = deadline - now;
                    let (g, _) = self.drain_signal.wait_timeout(q, remaining).map_err(|_| {
                        DdsError::PreconditionNotMet {
                            reason: "datawriter queue poisoned",
                        }
                    })?;
                    q = g;
                    if q.len() < max_samples as usize {
                        break;
                    }
                    // otherwise spurious wakeup → keep waiting.
                }
            }
            q.push(buf);
            Ok(())
        }
        #[cfg(not(feature = "std"))]
        {
            let mut q = self
                .queue
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datawriter queue poisoned",
                })?;
            q.push(buf);
            Ok(())
        }
    }

    /// Number of samples written so far. Test helper, replaced by real
    /// HistoryCache counters in the runtime.
    #[must_use]
    pub fn samples_pending(&self) -> usize {
        self.queue.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Number of matched remote readers. Always 0 in offline mode.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.4.2.11 `get_matched_subscriptions`.
    /// That returns a list; this returns only the count, the full list
    /// comes with listener callbacks.
    ///
    /// Side effect — on a change of the matched count relative to the
    /// last call, `on_publication_matched` is fired via the bubble-up
    /// chain (Spec §2.2.4.2.4.4).
    #[must_use]
    pub fn matched_subscription_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let n = rt.user_writer_matched_count(eid);
            self.poll_publication_matched(n);
            return n;
        }
        0
    }

    /// Compares the `current` count with `last_match_count` and fires
    /// `on_publication_matched` if the value has changed. Initially
    /// `last_match_count == -1`, i.e. the first call with n>=0 always
    /// triggers.
    #[cfg(feature = "std")]
    pub(crate) fn poll_publication_matched(&self, current: usize) {
        let curr = current as i64;
        let prev = self
            .last_match_count
            .swap(curr, std::sync::atomic::Ordering::AcqRel);
        if prev == curr {
            return;
        }
        let total = if curr > prev.max(0) {
            curr
        } else {
            prev.max(0)
        };
        let delta = curr - prev.max(0);
        let status = crate::status::PublicationMatchedStatus {
            total_count: total as i32,
            total_count_change: delta.max(0) as i32,
            current_count: curr as i32,
            current_count_change: delta as i32,
            last_subscription_handle: crate::instance_handle::HANDLE_NIL,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_publication_matched(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta detection for `on_offered_deadline_missed`. Reads the
    /// counter from the runtime and fires the listener on a delta. Spec
    /// §2.2.4.2.4.1.
    #[cfg(feature = "std")]
    pub(crate) fn poll_offered_deadline_missed(&self, current: u64) {
        let prev = self
            .last_offered_deadline_missed
            .swap(current, std::sync::atomic::Ordering::AcqRel);
        if current == prev {
            return;
        }
        let total_change = current.saturating_sub(prev);
        let status = crate::status::OfferedDeadlineMissedStatus {
            total_count: current as i32,
            total_count_change: total_change as i32,
            last_instance_handle: crate::instance_handle::HANDLE_NIL,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_offered_deadline_missed(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta detection for `on_liveliness_lost`. Spec §2.2.4.2.4.3.
    #[cfg(feature = "std")]
    pub(crate) fn poll_liveliness_lost(&self, current: u64) {
        let prev = self
            .last_liveliness_lost
            .swap(current, std::sync::atomic::Ordering::AcqRel);
        if current == prev {
            return;
        }
        let total_change = current.saturating_sub(prev);
        let status = crate::status::LivelinessLostStatus {
            total_count: current as i32,
            total_count_change: total_change as i32,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_liveliness_lost(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta detection for `on_offered_incompatible_qos`.
    /// Spec §2.2.4.2.4.2.
    #[cfg(feature = "std")]
    pub(crate) fn poll_offered_incompatible_qos(
        &self,
        snapshot: crate::status::OfferedIncompatibleQosStatus,
    ) {
        let curr = i64::from(snapshot.total_count);
        let prev = self
            .last_offered_incompatible_qos
            .swap(curr, std::sync::atomic::Ordering::AcqRel);
        if curr == prev {
            return;
        }
        let delta = curr - prev.max(0);
        let status = crate::status::OfferedIncompatibleQosStatus {
            total_count: curr as i32,
            total_count_change: delta.max(0) as i32,
            last_policy_id: snapshot.last_policy_id,
            policies: snapshot.policies,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_offered_incompatible_qos(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Blocks until at least `min_count` remote readers are matched or
    /// `timeout` elapses. Event-driven via a runtime condvar (D.5e
    /// phase 1) — wakes up directly when SEDP propagates a match, no more
    /// 20-ms polling.
    ///
    /// Related to OMG DDS 1.4 §2.2.2.4.2.22 `wait_for_acknowledgments`,
    /// but focused on matching rather than ACK. Covers the typical
    /// producer pattern "first create the writer, then wait for
    /// subscribers, then write".
    ///
    /// # Errors
    /// [`DdsError::Timeout`] if `min_count` is not reached within the
    /// time window.
    #[cfg(feature = "std")]
    pub fn wait_for_matched_subscription(
        &self,
        min_count: usize,
        timeout: core::time::Duration,
    ) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.matched_subscription_count() >= min_count {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(DdsError::Timeout);
            }
            if let Some(rt) = self.runtime.as_ref() {
                let _ = rt.wait_match_event(deadline - now);
            } else {
                std::thread::sleep(core::time::Duration::from_millis(20));
            }
        }
    }

    /// Counter for offered-deadline violations (Spec §2.2.4.2.9
    /// `OFFERED_DEADLINE_MISSED_STATUS`). Monotonically increasing;
    /// increments by 1 per expired deadline window without a write.
    /// Always 0 in offline mode or with `deadline=INFINITE`.
    ///
    /// Fires `on_offered_deadline_missed` over the bubble-up chain on a
    /// delta relative to the last call, if applicable.
    #[must_use]
    pub fn offered_deadline_missed_count(&self) -> u64 {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let n = rt.user_writer_offered_deadline_missed(eid);
            self.poll_offered_deadline_missed(n);
            return n;
        }
        0
    }

    /// Counter for LivelinessLost detections (Spec §2.2.4.2.10).
    /// Triggers `on_liveliness_lost` via bubble-up, if applicable.
    #[must_use]
    pub fn liveliness_lost_count(&self) -> u64 {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let n = rt.user_writer_liveliness_lost(eid);
            self.poll_liveliness_lost(n);
            return n;
        }
        0
    }

    /// Current `OfferedIncompatibleQosStatus` (Spec §2.2.4.2.4.2).
    /// Triggers `on_offered_incompatible_qos`, if applicable.
    #[must_use]
    pub fn offered_incompatible_qos_status(&self) -> crate::status::OfferedIncompatibleQosStatus {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let s = rt.user_writer_offered_incompatible_qos(eid);
            self.poll_offered_incompatible_qos(s.clone());
            return s;
        }
        crate::status::OfferedIncompatibleQosStatus::default()
    }

    /// Polls all statuses once and fires pending listeners. Convenience
    /// helper for tests + periodic tick callers.
    #[cfg(feature = "std")]
    pub fn drive_listeners(&self) {
        let _ = self.matched_subscription_count();
        let _ = self.offered_deadline_missed_count();
        let _ = self.liveliness_lost_count();
        let _ = self.offered_incompatible_qos_status();
    }

    /// Manual liveliness assert. Spec §2.2.2.4.2.20 `assert_liveliness`.
    /// Sets the `last_liveliness_assert` timestamp; a no-op with
    /// automatic liveliness (every write asserts anyway).
    #[cfg(feature = "std")]
    pub fn assert_liveliness(&self) {
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            rt.assert_writer_liveliness_eid(eid);
        }
    }

    /// Blocks until all matched remote readers have acknowledged all
    /// samples written so far, or `timeout` elapses.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.4.2.22 `wait_for_acknowledgments`.
    /// Returns `Ok(())` immediately in offline mode and without matched
    /// readers.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] if not all samples are acknowledged within
    /// the time window.
    #[cfg(feature = "std")]
    pub fn wait_for_acknowledgments(&self, timeout: core::time::Duration) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let all_acked = match (&self.runtime, self.entity_id) {
                (Some(rt), Some(eid)) => rt.user_writer_all_acknowledged(eid),
                _ => true, // offline: nothing to acknowledge
            };
            if all_acked {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(DdsError::Timeout);
            }
            // D.5e phase 1: event-driven via the runtime ack-event condvar.
            if let Some(rt) = self.runtime.as_ref() {
                let _ = rt.wait_ack_event(deadline - now);
            } else {
                std::thread::sleep(core::time::Duration::from_millis(20));
            }
        }
    }

    /// Takes all pending samples out of the offline queue. For tests
    /// only; removed with live-mode wiring.
    #[doc(hidden)]
    pub fn __drain_pending(&self) -> Vec<Vec<u8>> {
        let drained = self
            .queue
            .lock()
            .map(|mut q| core::mem::take(&mut *q))
            .unwrap_or_default();
        // Spec §2.2.3.19: drain signal to waiting `write()` threads.
        #[cfg(feature = "std")]
        self.drain_signal.notify_all();
        drained
    }

    // ========================================================================
    // Instance-API.4 / DDS 1.4 §2.2.2.4.2.{5,7,10,13,14}
    // ========================================================================

    /// Local publication handle of this DataWriter (Spec §2.2.2.5.1.11).
    /// Passed along in the `publication_handle` field of `SampleInfo`.
    /// **Note**: this is NOT the same handle as the entity
    /// `InstanceHandle` (Spec §2.2.2.1.1) — see [`Self::instance_handle`].
    #[cfg(feature = "std")]
    #[must_use]
    pub fn publication_handle(&self) -> InstanceHandle {
        self.publication_handle
    }

    /// Spec §2.2.2.1.1 `get_instance_handle` — entity identifier of this
    /// DataWriter for comparisons via
    /// `DomainParticipant::contains_entity`.
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.entity_state.instance_handle()
    }

    /// Returns the current [`InstanceTracker`] (shared with the internal
    /// bookkeeping). Mainly for tests / inspection.
    #[cfg(feature = "std")]
    #[must_use]
    /// Returns (runtime, EntityId) if the writer runs in live mode.
    /// Cross-crate hook for the FFI layer (zerodds-c-api), which needs to
    /// call rt.write_user_lifecycle directly.
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn runtime_handle(&self) -> Option<(Arc<DcpsRuntime>, EntityId)> {
        match (&self.runtime, self.entity_id) {
            (Some(rt), Some(eid)) => Some((Arc::clone(rt), eid)),
            _ => None,
        }
    }

    /// Returns the writer's shared instance tracker (test and inspection
    /// helper, Spec §2.2.2.4.2.5+ lifecycle bookkeeping).
    pub fn instance_tracker(&self) -> InstanceTracker {
        self.instances.clone()
    }

    /// Computes the KeyHash + the PLAIN_CDR2-BE key holder for a sample.
    /// Returns `None` for non-keyed topics.
    #[cfg(feature = "std")]
    fn keyhash_and_holder(sample: &T) -> Option<(crate::instance_tracker::KeyHash, Vec<u8>)> {
        if !T::HAS_KEY {
            return None;
        }
        let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
        sample.encode_key_holder_be(&mut holder);
        let bytes = holder.as_bytes().to_vec();
        let max = T::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
        let kh = crate::dds_type::compute_key_hash(&bytes, max);
        Some((kh, bytes))
    }

    /// Registers an instance with the DataWriter and returns its stable
    /// [`InstanceHandle`]. Spec §2.2.2.4.2.5 `register_instance`.
    ///
    /// For non-keyed topics the call returns [`HANDLE_NIL`] (each sample
    /// is its own "instance"; the spec explicitly says register/
    /// unregister/dispose are optional here).
    ///
    /// # Errors
    /// Currently the call cannot fail. Later (live mode) resource limits
    /// may return an `OutOfResources` error here.
    #[cfg(feature = "std")]
    pub fn register_instance(&self, instance: &T) -> Result<InstanceHandle> {
        self.register_instance_w_timestamp(instance, get_current_time())
    }

    /// Like `register_instance`, but with an explicit timestamp.
    /// Spec §2.2.2.4.2.6.
    #[cfg(feature = "std")]
    pub fn register_instance_w_timestamp(
        &self,
        instance: &T,
        timestamp: Time,
    ) -> Result<InstanceHandle> {
        let Some((kh, holder)) = Self::keyhash_and_holder(instance) else {
            return Ok(HANDLE_NIL);
        };
        Ok(self.instances.register(kh, holder, Some(timestamp)))
    }

    /// Turns a sample value into its corresponding local
    /// [`InstanceHandle`], or [`HANDLE_NIL`] if unknown / non-keyed.
    /// Spec §2.2.2.4.2.14 `lookup_instance`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_instance(&self, instance: &T) -> InstanceHandle {
        let Some((kh, _)) = Self::keyhash_and_holder(instance) else {
            return HANDLE_NIL;
        };
        self.instances.lookup(&kh).unwrap_or(HANDLE_NIL)
    }

    /// Removes the instance from the writer set (Spec §2.2.2.4.2.7).
    /// Sets the lifecycle state to `NOT_ALIVE_NO_WRITERS` once the last
    /// writer has unregistered.
    ///
    /// # Errors
    /// `BadParameter` if `handle` does not match the instance of
    /// `instance` (the spec requires this consistency check). If
    /// `handle == HANDLE_NIL`, the handle is derived from `instance`.
    #[cfg(feature = "std")]
    pub fn unregister_instance(&self, instance: &T, handle: InstanceHandle) -> Result<()> {
        self.unregister_instance_w_timestamp(instance, handle, get_current_time())
    }

    /// Like `unregister_instance`, but with a timestamp. Spec §2.2.2.4.2.8.
    ///
    /// Spec §2.2.3.21 WriterDataLifecycle: if
    /// `autodispose_unregistered_instances=true` (default), the instance
    /// is also disposed in addition to being unregistered — readers then
    /// see both `NOT_ALIVE_DISPOSED` and `NOT_ALIVE_NO_WRITERS`.
    #[cfg(feature = "std")]
    pub fn unregister_instance_w_timestamp(
        &self,
        instance: &T,
        handle: InstanceHandle,
        timestamp: Time,
    ) -> Result<()> {
        let resolved = self.resolve_handle(instance, handle)?;
        let autodispose = self
            .qos
            .lock()
            .map(|q| q.writer_data_lifecycle.autodispose_unregistered_instances)
            .unwrap_or(true);
        if autodispose && !self.instances.dispose(resolved, Some(timestamp)) {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        }
        if !self.instances.unregister(resolved, Some(timestamp)) {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        }
        // Wire side (Spec §9.6.3.9 PID_STATUS_INFO): send a lifecycle
        // marker to all matched readers. With autodispose=true we set both
        // bits, otherwise only UNREGISTERED.
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid), Some((kh, _))) = (
            &self.runtime,
            self.entity_id,
            Self::keyhash_and_holder(instance),
        ) {
            let mut bits = zerodds_rtps::inline_qos::status_info::UNREGISTERED;
            if autodispose {
                bits |= zerodds_rtps::inline_qos::status_info::DISPOSED;
            }
            let _ = rt.write_user_lifecycle(eid, kh, bits);
        }
        Ok(())
    }

    /// Disposes an instance (Spec §2.2.2.4.2.10). Marks it as
    /// `NOT_ALIVE_DISPOSED`; readers then see a sample with
    /// `valid_data == false`.
    ///
    /// # Errors
    /// Like `unregister_instance`.
    #[cfg(feature = "std")]
    pub fn dispose(&self, instance: &T, handle: InstanceHandle) -> Result<()> {
        self.dispose_w_timestamp(instance, handle, get_current_time())
    }

    /// Like `dispose`, but with a timestamp. Spec §2.2.2.4.2.11.
    #[cfg(feature = "std")]
    pub fn dispose_w_timestamp(
        &self,
        instance: &T,
        handle: InstanceHandle,
        timestamp: Time,
    ) -> Result<()> {
        let resolved = self.resolve_handle(instance, handle)?;
        if !self.instances.dispose(resolved, Some(timestamp)) {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        }
        // Wire side (Spec §9.6.3.9 PID_STATUS_INFO).
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid), Some((kh, _))) = (
            &self.runtime,
            self.entity_id,
            Self::keyhash_and_holder(instance),
        ) {
            let _ =
                rt.write_user_lifecycle(eid, kh, zerodds_rtps::inline_qos::status_info::DISPOSED);
        }
        Ok(())
    }

    /// Returns the sample value with only the `@key` fields populated
    /// (Spec §2.2.2.4.2.13 `get_key_value`). Implementation: we
    /// reconstruct `T` via `decode` from the stored PLAIN_CDR2-BE key
    /// holder. For this to work, `T::decode` must accept a key-only
    /// stream — for simple records this is trivially the case.
    ///
    /// # Errors
    /// * `BadParameter` if the handle is unknown.
    /// * `WireError` if the key holder cannot be reconstructed via
    ///   `T::decode`.
    #[cfg(feature = "std")]
    pub fn get_key_value(&self, handle: InstanceHandle) -> Result<T> {
        let Some(bytes) = self.instances.get_key_holder(handle) else {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        };
        T::decode(&bytes).map_err(|e| DdsError::WireError {
            message: alloc::string::ToString::to_string(&e),
        })
    }

    /// Helper: `handle == HANDLE_NIL` → derive from `instance`.
    /// Otherwise: check that `handle` matches the instance of `instance`.
    #[cfg(feature = "std")]
    fn resolve_handle(&self, instance: &T, handle: InstanceHandle) -> Result<InstanceHandle> {
        let derived = self.lookup_instance(instance);
        if handle.is_nil() {
            if derived.is_nil() {
                return Err(DdsError::BadParameter {
                    what: "instance not registered",
                });
            }
            return Ok(derived);
        }
        if !derived.is_nil() && derived != handle {
            return Err(DdsError::BadParameter {
                what: "handle does not match instance key",
            });
        }
        Ok(handle)
    }

    /// Writes a sample with an explicit timestamp (Spec §2.2.2.4.2.16
    /// `write_w_timestamp`) and updates the instance bookkeeping.
    ///
    /// # Errors
    /// Like [`Self::write`].
    #[cfg(feature = "std")]
    pub fn write_w_timestamp(&self, sample: &T, timestamp: Time) -> Result<()> {
        // Auto-register: if the instance is not yet known, we register it
        // implicitly (Spec §2.2.2.4.2.16 allows that).
        if let Some((kh, holder)) = Self::keyhash_and_holder(sample) {
            if self.instances.lookup(&kh).is_none() {
                self.instances.register(kh, holder, Some(timestamp));
            } else {
                // On re-activation after Dispose / NoWriters, register
                // bumps the generation counter but also adds a writer
                // count that we don't want — so decrement it again right
                // away.
                let prev = self.instances.get_by_keyhash(&kh);
                if let Some(state) = prev {
                    if !matches!(state.kind, crate::sample_info::InstanceStateKind::Alive) {
                        self.instances.register(kh, holder, Some(timestamp));
                        self.instances.unregister(state.handle, Some(timestamp));
                    }
                }
            }
        }
        self.write(sample)
    }
}

#[cfg(feature = "std")]
impl<T: DdsType> crate::entity::Entity for DataWriter<T> {
    type Qos = DataWriterQos;

    fn get_qos(&self) -> Self::Qos {
        self.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Spec §2.2.3 / §2.2.2.4.2: DURABILITY, RELIABILITY, HISTORY,
    /// RESOURCE_LIMITS, OWNERSHIP, LIVELINESS are Changeable=NO post-enable.
    fn set_qos(&self, qos: Self::Qos) -> Result<()> {
        let enabled = self.entity_state.is_enabled();
        if let Ok(mut current) = self.qos.lock() {
            if enabled {
                if current.durability != qos.durability {
                    return Err(crate::entity::immutable_if_enabled("DURABILITY"));
                }
                if current.reliability != qos.reliability {
                    return Err(crate::entity::immutable_if_enabled("RELIABILITY"));
                }
                if current.history != qos.history {
                    return Err(crate::entity::immutable_if_enabled("HISTORY"));
                }
                if current.resource_limits != qos.resource_limits {
                    return Err(crate::entity::immutable_if_enabled("RESOURCE_LIMITS"));
                }
                if current.ownership != qos.ownership {
                    return Err(crate::entity::immutable_if_enabled("OWNERSHIP"));
                }
                if current.liveliness != qos.liveliness {
                    return Err(crate::entity::immutable_if_enabled("LIVELINESS"));
                }
            }
            *current = qos;
        }
        Ok(())
    }

    fn enable(&self) -> Result<()> {
        self.entity_state.enable();
        Ok(())
    }

    fn entity_state(&self) -> Arc<crate::entity::EntityState> {
        Arc::clone(&self.entity_state)
    }
}

// ---- Boxed type-mapped variant, so the publisher can hold a
// heterogeneous writer list (live-mode preparation) ----
#[allow(dead_code)]
pub(crate) trait AnyDataWriter: Send + Sync + core::fmt::Debug {
    fn topic_name(&self) -> &str;
    fn type_name(&self) -> &'static str;
}

impl<T: DdsType + Send + 'static> AnyDataWriter for DataWriter<T>
where
    T: Send + Sync,
{
    fn topic_name(&self) -> &str {
        self.topic.name()
    }
    fn type_name(&self) -> &'static str {
        T::TYPE_NAME
    }
}

// Silence dead_code on Box<dyn AnyDataWriter> construction helper.
#[allow(dead_code)]
pub(crate) fn boxed_any_writer<T: DdsType + Send + Sync + 'static>(
    w: DataWriter<T>,
) -> Box<dyn AnyDataWriter> {
    Box::new(w)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dds_type::RawBytes;
    use crate::factory::DomainParticipantFactory;
    use crate::qos::{DomainParticipantQos, TopicQos};

    fn mk_topic() -> Topic<RawBytes> {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        Topic::new("Chatter".into(), TopicQos::default(), p)
    }

    #[test]
    fn publisher_creates_datawriter_for_matching_type() {
        let p = Publisher::new(PublisherQos::default(), None);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        assert_eq!(w.topic().name(), "Chatter");
    }

    /// Regression (ROS-2 cross-vendor): create_datawriter MUST derive the
    /// entityKind from `DdsType::HAS_KEY`. `RawBytes` is keyless
    /// (`HAS_KEY=false`), so the live writer MUST get a NoKey entityid
    /// (0x03) — otherwise a keyless CycloneDDS/ROS-2 reader silently
    /// rejects the match (entityKind mismatch). Before the fix, WithKey
    /// (0x02) was hard-coded.
    #[test]
    fn live_datawriter_entity_kind_is_nokey_for_keyless_type() {
        use zerodds_rtps::wire_types::EntityKind;
        let participant = DomainParticipantFactory::instance()
            .create_participant(0, DomainParticipantQos::default())
            .expect("live participant");
        let topic = participant
            .create_topic::<RawBytes>("KindChatter", TopicQos::default())
            .expect("topic");
        let pubr = participant.create_publisher(PublisherQos::default());
        let w = pubr
            .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
            .expect("writer");
        assert_eq!(
            w.entity_id.expect("live writer has entity_id").entity_kind,
            EntityKind::UserWriterNoKey,
            "keyless type must yield a NoKey writer entityid"
        );
    }

    #[test]
    fn datawriter_write_queues_encoded_sample() {
        let p = Publisher::new(PublisherQos::default(), None);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        assert_eq!(w.samples_pending(), 0);
        w.write(&RawBytes::new(vec![1, 2, 3])).unwrap();
        assert_eq!(w.samples_pending(), 1);
        let drained = w.__drain_pending();
        assert_eq!(drained, vec![vec![1u8, 2, 3]]);
    }

    // poll_publication_matched + Listener-Slot-API.

    use core::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn datawriter_set_listener_stores_arc_and_mask() {
        struct L;
        impl crate::listener::DataWriterListener for L {}
        let p = Publisher::new(PublisherQos::default(), None);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        assert!(w.get_listener().is_none());
        w.set_listener(Some(Arc::new(L)), crate::psm_constants::status::ANY);
        assert!(w.get_listener().is_some());
        // The mask is mirrored to the EntityState.
        assert_eq!(
            w.entity_state.listener_mask(),
            crate::psm_constants::status::ANY
        );
    }

    #[test]
    fn poll_publication_matched_fires_on_count_increase() {
        struct Cnt(AtomicU32);
        impl crate::listener::DataWriterListener for Cnt {
            fn on_publication_matched(
                &self,
                _w: crate::InstanceHandle,
                _s: crate::status::PublicationMatchedStatus,
            ) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let p = Publisher::new(PublisherQos::default(), None);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        let cnt = Arc::new(Cnt(AtomicU32::new(0)));
        w.set_listener(Some(cnt.clone()), crate::psm_constants::status::ANY);

        // 0 → 0 (initial call, AtomicI64 is -1, so there is a delta).
        w.poll_publication_matched(0);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 1);
        // 0 → 1 (change).
        w.poll_publication_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 2);
        // 1 → 1 (no delta).
        w.poll_publication_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 2);
        // 1 → 2.
        w.poll_publication_matched(2);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 3);
        // 2 → 1 (reader gone).
        w.poll_publication_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn poll_publication_matched_with_no_listener_is_noop() {
        let p = Publisher::new(PublisherQos::default(), None);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        // No listener set — must neither panic nor corrupt the delta
        // state.
        w.poll_publication_matched(0);
        w.poll_publication_matched(5);
    }

    #[test]
    fn poll_publication_matched_bubbles_to_publisher() {
        struct PubL(AtomicU32);
        impl crate::listener::PublisherListener for PubL {
            fn on_publication_matched(
                &self,
                _w: crate::InstanceHandle,
                _s: crate::status::PublicationMatchedStatus,
            ) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let p = Publisher::new(PublisherQos::default(), None);
        let pl = Arc::new(PubL(AtomicU32::new(0)));
        p.set_listener(Some(pl.clone()), crate::psm_constants::status::ANY);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        // No writer listener → the publisher receives it.
        w.poll_publication_matched(1);
        assert_eq!(pl.0.load(Ordering::Relaxed), 1);
    }

    // ---- §2.2.2.4.1.10 / .11 suspend/resume_publications ----

    #[test]
    fn suspend_publications_sets_flag() {
        let p = Publisher::new(PublisherQos::default(), None);
        assert!(!p.is_suspended());
        p.suspend_publications();
        assert!(p.is_suspended());
    }

    #[test]
    fn resume_publications_clears_flag() {
        let p = Publisher::new(PublisherQos::default(), None);
        p.suspend_publications();
        p.resume_publications();
        assert!(!p.is_suspended());
    }

    #[test]
    fn suspend_publications_is_idempotent() {
        let p = Publisher::new(PublisherQos::default(), None);
        p.suspend_publications();
        p.suspend_publications(); // second call is a no-op
        assert!(p.is_suspended());
    }

    #[test]
    fn resume_without_suspend_is_noop() {
        let p = Publisher::new(PublisherQos::default(), None);
        // Spec §2.2.2.4.1.11 — resume without an active suspend is a no-op.
        p.resume_publications();
        assert!(!p.is_suspended());
    }

    // ---- §2.2.2.4.1.13 copy_from_topic_qos ----

    #[test]
    fn copy_from_topic_qos_copies_durability_and_reliability() {
        use crate::qos::{DurabilityKind, ReliabilityKind, TopicQos};
        let mut topic = TopicQos::default();
        topic.durability.kind = DurabilityKind::TransientLocal;
        topic.reliability.kind = ReliabilityKind::Reliable;

        let mut dw = DataWriterQos::default();
        // Set something different so that we can see the change.
        dw.durability.kind = DurabilityKind::Volatile;
        Publisher::copy_from_topic_qos(&mut dw, &topic).unwrap();
        assert_eq!(dw.durability.kind, DurabilityKind::TransientLocal);
        assert_eq!(dw.reliability.kind, ReliabilityKind::Reliable);
    }

    // ---- §2.2.3.19 RESOURCE_LIMITS Reliable-Block ----

    #[test]
    fn write_blocks_until_drain_when_reliable_max_samples_reached() {
        use crate::qos::{HistoryQosPolicy, ResourceLimitsQosPolicy};
        let p = Publisher::new(PublisherQos::default(), None);
        let qos = DataWriterQos {
            resource_limits: ResourceLimitsQosPolicy {
                max_samples: 2,
                max_instances: -1,
                max_samples_per_instance: -1,
            },
            reliability: crate::qos::ReliabilityQosPolicy {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: zerodds_qos::Duration::from_millis(500_i32),
            },
            ..DataWriterQos::default()
        };
        let _ = qos.history;
        let _ = HistoryQosPolicy::default();
        let w = p.create_datawriter::<RawBytes>(&mk_topic(), qos).unwrap();
        let s = RawBytes::new(b"x".to_vec());
        // First fill both slots (no block).
        w.write(&s).unwrap();
        w.write(&s).unwrap();
        assert_eq!(w.samples_pending(), 2);

        // The third write blocks; in a second thread we drain after 50ms.
        let w_clone_q = w.queue.clone();
        let w_clone_signal = w.drain_signal.clone();
        let drain_handle = std::thread::spawn(move || {
            std::thread::sleep(core::time::Duration::from_millis(50));
            if let Ok(mut q) = w_clone_q.lock() {
                let _ = core::mem::take(&mut *q);
            }
            w_clone_signal.notify_all();
        });

        let start = std::time::Instant::now();
        let res = w.write(&s);
        let elapsed = start.elapsed();
        drain_handle.join().unwrap();

        assert!(res.is_ok(), "write should succeed after drain, got {res:?}");
        assert!(
            elapsed >= core::time::Duration::from_millis(40)
                && elapsed < core::time::Duration::from_millis(450),
            "elapsed = {elapsed:?}, expected ~50ms"
        );
    }

    #[test]
    fn write_returns_timeout_when_reliable_drain_too_slow() {
        use crate::qos::ResourceLimitsQosPolicy;
        let p = Publisher::new(PublisherQos::default(), None);
        let qos = DataWriterQos {
            resource_limits: ResourceLimitsQosPolicy {
                max_samples: 1,
                max_instances: -1,
                max_samples_per_instance: -1,
            },
            reliability: crate::qos::ReliabilityQosPolicy {
                kind: ReliabilityKind::Reliable,
                max_blocking_time: zerodds_qos::Duration::from_millis(50_i32),
            },
            ..DataWriterQos::default()
        };
        let w = p.create_datawriter::<RawBytes>(&mk_topic(), qos).unwrap();
        let s = RawBytes::new(b"x".to_vec());
        w.write(&s).unwrap();
        // The second write has Reliable + 50ms block; without a drain → timeout.
        let res = w.write(&s);
        assert!(matches!(res, Err(DdsError::Timeout)));
    }

    #[test]
    fn write_returns_oor_when_best_effort_queue_full() {
        use crate::qos::ResourceLimitsQosPolicy;
        let p = Publisher::new(PublisherQos::default(), None);
        let qos = DataWriterQos {
            resource_limits: ResourceLimitsQosPolicy {
                max_samples: 1,
                max_instances: -1,
                max_samples_per_instance: -1,
            },
            reliability: crate::qos::ReliabilityQosPolicy {
                kind: ReliabilityKind::BestEffort,
                max_blocking_time: zerodds_qos::Duration::from_millis(0_i32),
            },
            ..DataWriterQos::default()
        };
        let w = p.create_datawriter::<RawBytes>(&mk_topic(), qos).unwrap();
        let s = RawBytes::new(b"x".to_vec());
        w.write(&s).unwrap();
        let res = w.write(&s);
        assert!(matches!(res, Err(DdsError::OutOfResources { .. })));
    }

    #[test]
    fn write_does_not_block_when_max_samples_unlimited() {
        // max_samples = -1 (LENGTH_UNLIMITED) → no cap, no block.
        let p = Publisher::new(PublisherQos::default(), None);
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        let s = RawBytes::new(b"x".to_vec());
        for _ in 0..50 {
            w.write(&s).unwrap();
        }
        assert_eq!(w.samples_pending(), 50);
    }

    #[test]
    fn copy_from_topic_qos_does_not_touch_writer_only_policies() {
        use crate::qos::TopicQos;
        let topic = TopicQos::default();
        let mut dw = DataWriterQos::default();
        // Set ownership_strength to a concrete value; it should remain
        // untouched after the copy (no TopicQos counterpart).
        dw.ownership_strength.value = 42;
        Publisher::copy_from_topic_qos(&mut dw, &topic).unwrap();
        assert_eq!(dw.ownership_strength.value, 42);
    }
}
