// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Publisher + DataWriter — the send end of the DCPS API.
//!
//! Spec reference: OMG DDS 1.4 §2.2.2.4 `Publisher`, §2.2.2.4.2
//! `DataWriter`.
//!
//! # Routing
//!
//! - `Publisher::create_datawriter<T>(topic, qos)` → `DataWriter<T>`.
//! - `DataWriter::write(&sample)` uses the topic's static codec adapter and
//!   hands live samples to the runtime, or uses the offline in-memory queue.
//! - Discovery/matching and transport belong to the runtime; endpoint creation
//!   keeps representation, history and entity policy wiring here.
//!
//! # Thread safety
//!
//! `DataWriter` is `Send`+`Sync` via `Arc<Mutex<_>>`. Multiple
//! application threads may call `write()` in parallel.

extern crate alloc;
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
use crate::qos::{DataWriterQos, EndpointQosArg, PublisherQos};
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
    /// The QoS `create_datawriter` takes for `DATAWRITER_QOS_DEFAULT`, and
    /// `set_qos` of a DataWriter of this publisher (DDS 1.4
    /// §2.2.2.4.1.15).
    #[cfg(feature = "std")]
    default_datawriter_qos: std::sync::Mutex<DataWriterQos>,
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
    /// `suspend_publications` calls not yet matched by a
    /// `resume_publications` (DDS 1.4 §2.2.2.4.1.8, §2.2.2.4.1.9). Above
    /// zero, the publisher has hinted that writes should be buffered — the
    /// writer may use that as an optimization hint (e.g. coalescing). Not
    /// strictly enforced, because the spec explicitly defines it as a
    /// "hint to the Service".
    suspended: core::sync::atomic::AtomicU32,
    /// The DataWriters this publisher created, held weakly, for
    /// `lookup_datawriter`, `delete_contained_entities` and the
    /// participant's `contains_entity` (DDS 1.4 §2.2.2.4.1.7, .14,
    /// §2.2.2.2.1.10).
    #[cfg(feature = "std")]
    pub(crate) writers: crate::contained::ContainedList<dyn crate::contained::ContainedWriter>,
    /// Runtime `EntityId`s of the live DataWriters of this publisher.
    /// `begin_coherent_changes` / `end_coherent_changes` (Spec §2.2.2.4.1.8/.9)
    /// address the whole publisher, while the wire marker
    /// (`PID_COHERENT_SET`, DDSI-RTPS 2.5 §8.7.6) names a `SequenceNumber` in
    /// one writer's own SN space — so the transaction has to be opened and
    /// closed on every writer separately. A writer removes its id here on drop.
    #[cfg(feature = "std")]
    pub(crate) writer_entity_ids: std::sync::Mutex<alloc::vec::Vec<EntityId>>,
    /// A `begin_coherent_changes` transaction is open (Spec §2.2.2.4.1.8).
    /// A DataWriter created while it is open joins the transaction.
    #[cfg(feature = "std")]
    pub(crate) coherent_changes_open: core::sync::atomic::AtomicBool,
    /// `begin_coherent_changes` calls not yet matched by an
    /// `end_coherent_changes`: DDS 1.4 §2.2.2.4.1.10 "These calls can be
    /// nested. In that case, the coherent set terminates only with the last
    /// call to end_coherent_changes."
    #[cfg(feature = "std")]
    coherent_depth: core::sync::atomic::AtomicU32,
    /// With `PRESENTATION.access_scope = GROUP`, what the writers share
    /// (DDSI-RTPS 2.5 §8.7.5): created when the first writer is enabled,
    /// PRESENTATION being fixed from then on.
    #[cfg(feature = "std")]
    group: std::sync::Mutex<Option<Arc<zerodds_rtps::writer_group::WriterGroup>>>,
    /// The Publisher's `EntityId`, a Writer Group (DDSI-RTPS 2.5 Table
    /// 9.1); its writers announce it as `PID_GROUP_GUID` (Table 9.19).
    #[cfg(feature = "std")]
    pub(crate) entity_id: Option<EntityId>,
}

#[cfg(feature = "std")]
impl PublisherInner {
    /// `PRESENTATION.access_scope` is GROUP.
    fn group_scope(&self) -> bool {
        self.qos.lock().is_ok_and(|q| {
            matches!(
                q.presentation.access_scope,
                zerodds_qos::PresentationAccessScope::Group
            )
        })
    }

    /// The group of a GROUP-scope publisher, created on first use; `None`
    /// for INSTANCE and TOPIC scope.
    pub(crate) fn writer_group(&self) -> Option<Arc<zerodds_rtps::writer_group::WriterGroup>> {
        if !self.group_scope() {
            return None;
        }
        let mut group = self.group.lock().ok()?;
        Some(Arc::clone(group.get_or_insert_with(|| {
            Arc::new(zerodds_rtps::writer_group::WriterGroup::new())
        })))
    }

    /// The group if one was created.
    fn existing_writer_group(&self) -> Option<Arc<zerodds_rtps::writer_group::WriterGroup>> {
        self.group.lock().ok()?.as_ref().map(Arc::clone)
    }

    /// Records the publisher's writers in its group, if it has one.
    pub(crate) fn update_group_writers(&self) {
        if let (Some(group), Ok(ids)) =
            (self.existing_writer_group(), self.writer_entity_ids.lock())
        {
            group.set_writers(&ids);
        }
    }

    /// DDSI-RTPS 2.5 §8.7.6: "Writers are not allowed to be added or
    /// removed from a Publisher from the time that a coherent set begins
    /// until after it ends" — for a GROUP-scope publisher, whose set spans
    /// its writers.
    pub(crate) fn check_writers_may_change(&self) -> Result<()> {
        if self.group_scope()
            && self
                .coherent_changes_open
                .load(core::sync::atomic::Ordering::Acquire)
        {
            return Err(DdsError::PreconditionNotMet {
                reason: "a GROUP-scope publisher's writers do not change while a coherent set is open",
            });
        }
        Ok(())
    }
}

/// The Publisher's group id is the participant's no longer once it is gone
/// (DDSI-RTPS 2.5 §8.2.4.4): the runtime may hand its key out again.
#[cfg(feature = "std")]
impl Drop for PublisherInner {
    fn drop(&mut self) {
        if let (Some(rt), Some(id)) = (&self.runtime, self.entity_id) {
            rt.release_entity_key(id);
        }
    }
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
        let entity_id = runtime.as_ref().map(|rt| {
            rt.new_group_entity_id(zerodds_rtps::wire_types::EntityKind::UserWriterGroup)
        });
        let entity_state = crate::entity::EntityState::new();
        entity_state.set_autoenable_children(qos.entity_factory.autoenable_created_entities);
        let publisher = Self {
            inner: Arc::new(PublisherInner {
                qos: std::sync::Mutex::new(qos),
                entity_state,
                default_datawriter_qos: std::sync::Mutex::new(DataWriterQos::default()),
                runtime,
                listener: std::sync::Mutex::new(None),
                participant: std::sync::Mutex::new(None),
                suspended: core::sync::atomic::AtomicU32::new(0),
                writers: crate::contained::ContainedList::new(),
                writer_entity_ids: std::sync::Mutex::new(alloc::vec::Vec::new()),
                coherent_changes_open: core::sync::atomic::AtomicBool::new(false),
                #[cfg(feature = "std")]
                coherent_depth: core::sync::atomic::AtomicU32::new(0),
                group: std::sync::Mutex::new(None),
                entity_id,
            }),
        };
        // `StatusCondition::get_entity` (DDS 1.4 §2.2.2.1.9.3).
        let weak = Arc::downgrade(&publisher.inner);
        publisher.inner.entity_state.set_entity_ref(move || {
            weak.upgrade()
                .map(|inner| crate::entity::EntityRef::Publisher(Publisher { inner }))
        });
        publisher
    }

    #[cfg(not(feature = "std"))]
    pub(crate) fn new(qos: PublisherQos) -> Self {
        Self {
            inner: Arc::new(PublisherInner {
                qos,
                entity_state: crate::entity::EntityState::new(),
                suspended: core::sync::atomic::AtomicU32::new(0),
            }),
        }
    }

    /// DDS 1.4 §2.2.2.4.1.13 `get_participant`: the DomainParticipant this
    /// publisher belongs to. `None` once every handle of that participant
    /// is gone.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_participant(&self) -> Option<crate::participant::DomainParticipant> {
        let participant = self.inner.participant.lock().ok()?;
        participant
            .as_ref()?
            .upgrade()
            .map(crate::participant::DomainParticipant::from_inner)
    }

    /// Spec §2.2.2.2.1.10 — `true` if `handle` is a DataWriter created
    /// through this publisher.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn contains_writer(&self, handle: crate::instance_handle::InstanceHandle) -> bool {
        self.inner.writers.contains(handle)
    }

    /// Sets the `PublisherListener` + StatusMask. `None` clears the slot.
    /// Spec §2.2.2.4.3.x.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcPublisherListener>, mask: StatusMask) {
        let listener = listener.or_else(|| {
            (mask != 0).then(|| {
                alloc::sync::Arc::new(crate::listener::NilListener) as ArcPublisherListener
            })
        });
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
        crate::listener_dispatcher::ListenerDispatcher::listener_set(self.inner.runtime.as_ref());
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
            .filter(|l| !l.is_nil())
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

    /// DDS 1.4 §2.2.2.4.1.8 `suspend_publications` — a hint to the Service
    /// that subsequent `write()` calls may be buffered (e.g. for
    /// coalescing). Has no mandatory semantics for the caller; the flag
    /// is readable via `is_suspended()` for the writer implementation.
    ///
    /// "The use of this operation must be matched by a corresponding call
    /// to resume_publications": each call wants its own
    /// `resume_publications`.
    ///
    /// # Errors
    /// None today; the operation returns a ReturnCode_t in the spec.
    pub fn suspend_publications(&self) -> Result<()> {
        // §2.2.2.1.1.7: not an operation a disabled publisher takes.
        self.inner.entity_state.check_enabled()?;
        self.inner
            .suspended
            .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        Ok(())
    }

    /// DDS 1.4 §2.2.2.4.1.9 `resume_publications` — counterpart to
    /// `suspend_publications`.
    ///
    /// # Errors
    /// `PreconditionNotMet` without a `suspend_publications` to match: "The
    /// call to resume_publications must match a previous call to
    /// suspend_publications. Otherwise the operation will return the error
    /// PRECONDITION_NOT_MET." It returned nothing and did nothing.
    pub fn resume_publications(&self) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        self.inner
            .suspended
            .fetch_update(
                core::sync::atomic::Ordering::AcqRel,
                core::sync::atomic::Ordering::Acquire,
                |n| n.checked_sub(1),
            )
            .map(|_| ())
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "resume_publications without suspend_publications",
            })
    }

    /// `true` while a `suspend_publications()` is not matched by a
    /// `resume_publications()`. Read by the writer send path as a
    /// coalescing hint.
    #[must_use]
    pub fn is_suspended(&self) -> bool {
        self.inner
            .suspended
            .load(core::sync::atomic::Ordering::Acquire)
            > 0
    }

    /// Spec §2.2.2.4.1.8 `begin_coherent_changes` — opens a coherent set on
    /// every DataWriter of this Publisher. Until `end_coherent_changes`, each
    /// `write()` carries `PID_COHERENT_SET` in its inline QoS (DDSI-RTPS 2.5
    /// §8.7.6), naming the `SequenceNumber` of the first sample its writer
    /// contributed to the set. A subscriber with
    /// `PRESENTATION.coherent_access` and an `access_scope` above INSTANCE
    /// sees the set only once it is closed — all of it or none of it.
    ///
    /// The transaction spans the publisher; the marker names a per-writer
    /// `SequenceNumber`, so a publisher with several writers opens one set per
    /// writer. A writer created while the transaction is open joins it.
    ///
    /// "These calls can be nested. In that case, the coherent set
    /// terminates only with the last call to end_coherent_changes": a call
    /// inside an open transaction opens nothing, and the set ends with the
    /// `end_coherent_changes` that matches the first call.
    ///
    /// # Errors
    /// `PreconditionNotMet` on a poisoned writer list.
    #[cfg(feature = "std")]
    pub fn begin_coherent_changes(&self) -> Result<()> {
        // §2.2.2.1.1.7: not an operation a disabled publisher takes.
        self.inner.entity_state.check_enabled()?;
        if self
            .inner
            .coherent_depth
            .fetch_add(1, core::sync::atomic::Ordering::AcqRel)
            > 0
        {
            return Ok(());
        }
        let opened = self.open_coherent_set();
        if opened.is_err() {
            self.inner
                .coherent_depth
                .fetch_sub(1, core::sync::atomic::Ordering::AcqRel);
        }
        opened
    }

    /// The outermost [`Self::begin_coherent_changes`].
    #[cfg(feature = "std")]
    fn open_coherent_set(&self) -> Result<()> {
        if let Some(group) = self.inner.writer_group() {
            group.begin_coherent_set();
        }
        self.inner
            .coherent_changes_open
            .store(true, core::sync::atomic::Ordering::Release);
        let Some(rt) = self.inner.runtime.as_ref() else {
            // Offline publisher: no runtime writer slots, the flag is all
            // there is to keep.
            return Ok(());
        };
        let ids = self
            .inner
            .writer_entity_ids
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "publisher writer list poisoned",
            })?
            .clone();
        for eid in ids {
            rt.begin_user_writer_coherent_set(eid)?;
        }
        Ok(())
    }

    /// Spec §2.2.2.4.1.9 `end_coherent_changes` — closes the coherent set on
    /// every DataWriter of this Publisher and sends the end-of-coherent-set
    /// marker (DDSI-RTPS 2.5 §8.7.6), which releases the set on every matched
    /// subscriber. A writer that contributed no sample sends nothing.
    ///
    /// # Errors
    /// `PreconditionNotMet` if no `begin_coherent_changes` is open, or on a
    /// poisoned writer list; `WireError` on an encode failure.
    #[cfg(feature = "std")]
    pub fn end_coherent_changes(&self) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        let depth = self
            .inner
            .coherent_depth
            .fetch_update(
                core::sync::atomic::Ordering::AcqRel,
                core::sync::atomic::Ordering::Acquire,
                |n| n.checked_sub(1),
            )
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "end_coherent_changes without begin_coherent_changes",
            })?;
        if depth > 1 {
            // An inner end: the set goes on (§2.2.2.4.1.10).
            return Ok(());
        }
        let was_open = self
            .inner
            .coherent_changes_open
            .swap(false, core::sync::atomic::Ordering::AcqRel);
        if !was_open {
            return Err(DdsError::PreconditionNotMet {
                reason: "end_coherent_changes without begin_coherent_changes",
            });
        }
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Ok(());
        };
        let ids = self
            .inner
            .writer_entity_ids
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "publisher writer list poisoned",
            })?
            .clone();
        // GROUP scope: the set spans the writers, and each of them sends an
        // end-of-coherent-set DATA at the same group sequence number
        // (DDSI-RTPS 2.5 §8.7.6). A set that holds no change ends without.
        let group_end = self
            .inner
            .existing_writer_group()
            .and_then(|group| group.end_coherent_set());
        for eid in ids {
            match group_end {
                Some(end) => rt.end_user_writer_group_coherent_set(eid, end)?,
                None => rt.end_user_writer_coherent_set(eid)?,
            }
        }
        Ok(())
    }

    /// `true` between `begin_coherent_changes` and `end_coherent_changes`
    /// (Spec §2.2.2.4.1.8/.9).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn coherent_changes_open(&self) -> bool {
        self.inner
            .coherent_changes_open
            .load(core::sync::atomic::Ordering::Acquire)
    }

    /// DDS 1.4 §2.2.2.4.1.15 `set_default_datawriter_qos`: the QoS
    /// `create_datawriter` takes for [`DATAWRITER_QOS_DEFAULT`]. That value
    /// itself resets the default to "the values that would be used if the
    /// set_default_datawriter_qos operation had never been called". "This
    /// operation will check that the resulting policies are self
    /// consistent; if they are not, the operation will have no effect and
    /// return INCONSISTENT_POLICY."
    ///
    /// [`DATAWRITER_QOS_DEFAULT`]: crate::psm_constants::DATAWRITER_QOS_DEFAULT
    ///
    /// # Errors
    /// `AlreadyDeleted` after the publisher's deletion;
    /// `InconsistentPolicy`.
    #[cfg(feature = "std")]
    pub fn set_default_datawriter_qos(
        &self,
        qos: impl Into<crate::qos::QosArg<DataWriterQos>>,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        let qos = qos.into().resolve(DataWriterQos::default);
        qos.check_consistency()?;
        crate::participant::set_default(
            &self.inner.default_datawriter_qos,
            crate::qos::QosArg::Value(qos),
        )
    }

    /// DDS 1.4 §2.2.2.4.1.16 `get_default_datawriter_qos`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_default_datawriter_qos(&self) -> DataWriterQos {
        crate::participant::get_default(&self.inner.default_datawriter_qos)
    }

    /// Spec §2.2.2.4.1.13 `copy_from_topic_qos` — "copies the policies in
    /// the a_topic_qos to the corresponding policies in the
    /// a_datawriter_qos (replacing values in the a_datawriter_qos, if
    /// present)". The policies both carry: DURABILITY, DURABILITY_SERVICE,
    /// DEADLINE, LATENCY_BUDGET, LIVELINESS, RELIABILITY,
    /// DESTINATION_ORDER, HISTORY, RESOURCE_LIMITS, TRANSPORT_PRIORITY,
    /// LIFESPAN and OWNERSHIP. TOPIC_DATA is not copied: on a DataWriter it
    /// is the ZeroDDS per-writer override, and a writer that leaves it
    /// empty announces the topic's and follows a later change of it.
    ///
    /// # Errors
    /// None today; the `Result` is the spec's `ReturnCode_t`.
    pub fn copy_from_topic_qos(
        dw_qos: &mut DataWriterQos,
        topic_qos: &crate::qos::TopicQos,
    ) -> Result<()> {
        dw_qos.durability = topic_qos.durability;
        dw_qos.durability_service = topic_qos.durability_service;
        dw_qos.deadline = topic_qos.deadline;
        dw_qos.latency_budget = topic_qos.latency_budget;
        dw_qos.liveliness = topic_qos.liveliness;
        dw_qos.reliability = topic_qos.reliability;
        dw_qos.destination_order = topic_qos.destination_order;
        dw_qos.history = topic_qos.history;
        dw_qos.resource_limits = topic_qos.resource_limits;
        dw_qos.transport_priority = topic_qos.transport_priority;
        dw_qos.lifespan = topic_qos.lifespan;
        dw_qos.ownership = topic_qos.ownership;
        // §7.6.3.1.1: the first element of the Topic's list applies to writers.
        dw_qos.data_representation =
            crate::qos::writer_representation_of_topic(topic_qos.data_representation.as_deref());
        Ok(())
    }

    /// DDS 1.4 §2.2.2.4.1.5 `create_datawriter(a_topic, qos, a_listener,
    /// mask)`: the writer with its listener. `None` with a non-empty `mask` is
    /// the 'nil' listener (§2.2.2.1.1.3). What happened before the listener was
    /// set is heard at its first look: the status flags are kept until read or
    /// taken by a listener.
    ///
    /// # Errors
    /// As [`Self::create_datawriter`].
    #[cfg(feature = "std")]
    pub fn create_datawriter_with_listener<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: impl Into<EndpointQosArg<DataWriterQos>>,
        listener: Option<ArcDataWriterListener>,
        mask: StatusMask,
    ) -> Result<DataWriter<T>> {
        let writer = self.create_datawriter(topic, qos)?;
        writer.set_listener(listener, mask);
        Ok(writer)
    }

    /// Creates a typed `DataWriter<T>`. Spec §2.2.2.4.1.5
    /// `create_datawriter`. With [`DATAWRITER_QOS_DEFAULT`] the writer gets
    /// [`Self::get_default_datawriter_qos`]; with
    /// [`DATAWRITER_QOS_USE_TOPIC_QOS`], that QoS with the Topic QoS copied
    /// over it ([`Self::copy_from_topic_qos`]).
    ///
    /// [`DATAWRITER_QOS_DEFAULT`]: crate::psm_constants::DATAWRITER_QOS_DEFAULT
    /// [`DATAWRITER_QOS_USE_TOPIC_QOS`]: crate::psm_constants::DATAWRITER_QOS_USE_TOPIC_QOS
    ///
    /// # Errors
    /// - `PreconditionNotMet` if the current `DdsType` metadata differs
    ///   from the topic's creation-time snapshot.
    pub fn create_datawriter<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: impl Into<EndpointQosArg<DataWriterQos>>,
    ) -> Result<DataWriter<T>> {
        #[cfg(feature = "std")]
        self.inner.entity_state.check_not_deleted()?;
        let qos = match qos.into() {
            EndpointQosArg::Value(qos) => qos,
            EndpointQosArg::Default => self.get_default_datawriter_qos(),
            EndpointQosArg::UseTopicQos => {
                let mut qos = self.get_default_datawriter_qos();
                Self::copy_from_topic_qos(&mut qos, &crate::entity::Entity::get_qos(topic))?;
                qos
            }
        };
        #[cfg(feature = "std")]
        self.inner.check_writers_may_change()?;
        // DDS 1.4 §2.2.2.4.1.5: "If the Topic was created from a different
        // DomainParticipant, the operation will fail and return a nil
        // result."
        #[cfg(feature = "std")]
        if topic.created_by_another_than(&self.inner.participant) {
            return Err(DdsError::PreconditionNotMet {
                reason: "the Topic was created by another DomainParticipant",
            });
        }
        qos.check_consistency()?;
        let metadata = topic.checked_type_metadata()?;
        #[cfg(not(feature = "std"))]
        let _ = metadata;
        // Reject unknown DataRepresentation offers up
        // front (only XCDR1/XCDR2/XML are emitted) so the body encoder and encap
        // header cannot end up on a representation ZeroDDS does not serialize.
        #[cfg(feature = "std")]
        validate_writer_data_representation(&qos)?;
        // Before the runtime knows the writer: a PERSISTENT writer whose store
        // cannot be created is refused here, not run without its store.
        #[cfg(feature = "std")]
        let backend = self.durability_store_for::<T>(
            topic.name(),
            &crate::entity::Entity::get_qos(topic),
            &qos,
        )?;
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            // Live mode: a real user writer of the runtime. Matching and
            // user-data flow run over SEDP + UDP once it is enabled.
            // Gap 2 (#24): auto-register the local TypeObject (if codegen or
            // a hand impl provides one) so the runtime's TypeLookup server
            // can answer a peer's getTypes and the local match sites can
            // resolve this writer's TYPE_IDENTIFIER structurally against the
            // registry. Default `type_object()` is `None` → no-op.
            //
            // This runs BEFORE the endpoint is registered because the
            // endpoint's SEDP announcement is built and sent inside
            // `register_user_writer_kind`, and issue #42 has it carry
            // PID_TYPE_INFORMATION derived from exactly this registry entry
            // (XTypes 1.3 §7.6.3.2). Registering afterwards would leave the
            // FIRST announcement — the one a peer already present matches on
            // — without the type identity, and nothing re-sends it.
            for obj in topic
                .codec()
                .dependent_type_objects()
                .iter()
                .chain(metadata.type_object.iter())
            {
                rt.register_type_object(obj.clone())
                    .map_err(|error| DdsError::WireError {
                        message: format!("type registration: {error:?}"),
                    })?;
            }
            // A type in a strongly connected component is registered as
            // that component (XTypes 1.3 §7.3.4.9): the hash of a member's
            // object is not the key it is looked up by.
            if let Some(component) = topic.codec().type_component() {
                rt.register_type_component(component)
                    .map_err(|error| DdsError::WireError {
                        message: format!("type registration: {error:?}"),
                    })?;
            }
            // Derive the entityKind from the type keyedness (Spec §9.3.1.2:
            // 0x02=WithKey / 0x03=NoKey). A keyless type (`HAS_KEY=false`)
            // MUST produce a NoKey writer, otherwise cross-vendor readers
            // (CycloneDDS/ROS 2) reject the endpoint match due to an
            // entityKind mismatch (keyed vs no-key) — silently, without a
            // log.
            // DDS 1.4 §2.2.3.6/§2.2.3.13/§2.2.3.3: PRESENTATION, PARTITION and
            // GROUP_DATA are Publisher-level policies, and the value the RxO
            // check compares travels in this writer's
            // `PublicationBuiltinTopicData`. Resolve the group's value here so
            // it reaches the wire; the ZeroDDS-only per-writer field stays an
            // override (see `qos::effective_presentation`).
            // DDS 1.4 §2.2.2.1.1.7: the writer exists, with its entity id
            // and handle, from here; the runtime registers it — and so
            // announces and matches it — at its first `enable`, with the
            // QoS it has then ([`WriterRegistration`]). A writer this
            // publisher enables on creation is registered right below.
            let eid = rt.reserve_user_writer_id(metadata.keyed);
            let dw = DataWriter::new_live(
                topic.clone(),
                qos,
                self.inner.clone(),
                Arc::clone(rt),
                eid,
                backend,
            );
            let registration = WriterRegistration {
                runtime: Arc::clone(rt),
                eid,
                topic_name: topic.name().into(),
                type_name: metadata.type_name.to_string(),
                type_identifier: metadata.type_identifier.clone(),
                extensibility: metadata.extensibility,
                max_serialized_size: metadata.max_serialized_size,
                qos: Arc::clone(&dw.inner.qos),
                publisher: self.inner.clone(),
                topic: topic.inner(),
                big_endian: dw.inner.big_endian,
                backend: dw.durability_backend(),
                store_refused: Arc::clone(&dw.inner.store_refused),
            };
            if let Some(backend) = dw.durability_backend() {
                dw.inner.status.set_store(topic.name(), backend);
            }
            dw.inner
                .entity_state
                .set_enable_work(move || registration.register());
            dw.inner.entity_state.set_factory(&self.inner.entity_state);
            self.autoenable_child(&dw.inner.entity_state)?;
            self.track_writer(&dw);
            Self::register_writer_ref(&dw);
            topic.attach_user(crate::topic::TopicUserKind::DataWriter);
            return Ok(dw);
        }
        let dw = DataWriter::new_offline(topic.clone(), qos, self.inner.clone(), backend);
        #[cfg(feature = "std")]
        dw.inner.entity_state.set_factory(&self.inner.entity_state);
        self.autoenable_child(&dw.inner.entity_state)?;
        #[cfg(feature = "std")]
        {
            self.track_writer(&dw);
            Self::register_writer_ref(&dw);
        }
        topic.attach_user(crate::topic::TopicUserKind::DataWriter);
        Ok(dw)
    }

    /// `StatusCondition::get_entity` of the writer (DDS 1.4 §2.2.2.1.9.3).
    #[cfg(feature = "std")]
    fn register_writer_ref<T: DdsType + Send + 'static>(dw: &DataWriter<T>) {
        let weak = Arc::downgrade(&dw.inner);
        dw.inner.entity_state.set_entity_ref(move || {
            weak.upgrade().map(|inner| {
                crate::entity::EntityRef::DataWriter(AnyDataWriter {
                    inner: inner as Arc<dyn crate::contained::Contained>,
                })
            })
        });
    }

    /// The TRANSIENT or PERSISTENT store a writer of `topic` uses, `None`
    /// for any other DURABILITY. The store belongs to the participant's
    /// topic, so it outlives this writer and is the one the next writer of
    /// the topic finds (DDS 1.4 §2.2.3.4); a publisher without a participant
    /// builds one of its own.
    #[cfg(feature = "std")]
    fn durability_store_for<T: DdsType + Send + 'static>(
        &self,
        topic: &str,
        topic_qos: &crate::qos::TopicQos,
        qos: &DataWriterQos,
    ) -> Result<Option<Arc<dyn crate::durability_service::DurabilityBackend>>> {
        let persistent = match qos.durability.kind {
            zerodds_qos::DurabilityKind::Transient => false,
            zerodds_qos::DurabilityKind::Persistent => true,
            _ => return Ok(None),
        };
        // DDS 1.4 §2.2.3.5: DURABILITY_SERVICE "is used to configure the
        // HISTORY QoS and the RESOURCE_LIMITS QoS used by the fictitious
        // DataReader and DataWriter", and §2.2.3.4 gives those their QoS "from
        // the Topic QoS of the corresponding Topic": the store, which is the
        // topic's, follows the Topic's DURABILITY_SERVICE. A Topic that leaves
        // it at the default leaves the setting to the writer (the first one
        // that creates the store), whose value then is the topic's in effect.
        // A writer that sets another value than a non-default Topic is
        // overridden, and that is reported.
        let default = zerodds_qos::DurabilityServiceQosPolicy::default();
        let mut effective = qos.clone();
        if topic_qos.durability_service != default {
            if qos.durability_service != default
                && qos.durability_service != topic_qos.durability_service
            {
                if let Some(rt) = self.inner.runtime.as_ref() {
                    rt.config.observability.record(
                        &zerodds_foundation::observability::Event::new(
                            zerodds_foundation::observability::Level::Warn,
                            zerodds_foundation::observability::Component::Dcps,
                            "writer.durability_service.overridden_by_topic",
                        )
                        .with_attr("topic", topic),
                    );
                }
            }
            effective.durability_service = topic_qos.durability_service;
        }
        let qos = &effective;
        let dir = self
            .inner
            .runtime
            .as_ref()
            .and_then(|rt| rt.config.durability_dir.clone());
        let build = || {
            DataWriter::<T>::build_durability_backend(qos, dir.as_deref())?.ok_or(
                DdsError::PreconditionNotMet {
                    reason: "no durability store for this DURABILITY kind",
                },
            )
        };
        match self.inner.participant_inner() {
            Some(participant) => participant
                .durability_store(topic, persistent, build)
                .map(Some),
            None => build().map(Some),
        }
    }

    /// DDS 1.4 §2.2.2.4.1.6 `delete_datawriter`: deletes a DataWriter this
    /// publisher created. Every handle of it returns
    /// [`DdsError::AlreadyDeleted`] from then on. The deletion unregisters
    /// the writer's instances and, with WRITER_DATA_LIFECYCLE
    /// `autodispose_unregistered_instances`, disposes them.
    ///
    /// # Errors
    /// `PreconditionNotMet` if another publisher created `writer`, which is
    /// left as it was; `AlreadyDeleted` if `writer` or this publisher is
    /// deleted.
    #[cfg(feature = "std")]
    pub fn delete_datawriter<T: DdsType>(&self, writer: &DataWriter<T>) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        if !Arc::ptr_eq(&writer.inner.publisher, &self.inner) {
            return Err(DdsError::PreconditionNotMet {
                reason: "the DataWriter belongs to another publisher",
            });
        }
        writer.inner.entity_state.check_not_deleted()?;
        self.inner.check_writers_may_change()?;
        writer.inner.delete();
        Ok(())
    }

    /// DDS 1.4 §2.2.2.4.1.7 `lookup_datawriter`: a handle to a DataWriter
    /// of this publisher attached to the topic named `topic_name`, or
    /// `None`. Of several, the oldest; the specification leaves it open.
    /// A writer of another type on a topic of that name is not a `T`
    /// writer and is not returned.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_datawriter<T: DdsType + Send + 'static>(
        &self,
        topic_name: &str,
    ) -> Option<DataWriter<T>> {
        self.inner
            .writers
            .find::<WriterInner<T>>(topic_name)
            .map(DataWriter::from_inner_arc)
    }

    /// DDS 1.4 §2.2.2.4.1.14 `delete_contained_entities`: deletes every
    /// DataWriter this publisher created, as
    /// [`Self::delete_datawriter`] does.
    ///
    /// # Errors
    /// `AlreadyDeleted` if this publisher is deleted; `PreconditionNotMet`
    /// while a GROUP-scope coherent set is open (DDSI-RTPS 2.5 §8.7.6).
    #[cfg(feature = "std")]
    pub fn delete_contained_entities(&self) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        self.inner.check_writers_may_change()?;
        self.inner.writers.delete_all()
    }

    /// DDS 1.4 §2.2.2.4.1.12 `wait_for_acknowledgments`: blocks until all
    /// data written by the reliable DataWriters of this publisher is
    /// acknowledged by all their matched reliable DataReaders, or `timeout`
    /// elapses. "This operation is intended to be used only if the
    /// DataWriter has RELIABILITY QoS kind set to RELIABLE. Otherwise the
    /// operation will return immediately with RETCODE_OK": a BEST_EFFORT
    /// writer has no reliable reader to wait for, and so returns at once.
    ///
    /// # Errors
    /// `Timeout` if not all data is acknowledged within `timeout`;
    /// `AlreadyDeleted` after delete; `NotEnabled` while the publisher is
    /// not enabled.
    #[cfg(feature = "std")]
    pub fn wait_for_acknowledgments(&self, timeout: core::time::Duration) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        let deadline = acknowledgment_deadline(timeout);
        for writer in self.inner.writers.live() {
            if !writer.await_acknowledged(deadline) {
                return Err(DdsError::Timeout);
            }
        }
        Ok(())
    }

    /// `true` if this publisher holds a DataWriter that is not deleted;
    /// `DomainParticipant::delete_publisher` refuses such a publisher
    /// (DDS 1.4 §2.2.2.2.1.2).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn has_datawriters(&self) -> bool {
        !self.inner.writers.is_empty()
    }

    /// Spec §2.2.3.20 + §2.2.2.1.1.7: `create_datawriter` enables the new
    /// writer when this publisher's
    /// `entity_factory.autoenable_created_entities` is TRUE. A disabled
    /// publisher creates disabled writers regardless of the setting.
    fn autoenable_child(&self, child: &alloc::sync::Arc<crate::entity::EntityState>) -> Result<()> {
        let autoenable = self
            .inner
            .qos
            .lock()
            .map(|q| q.entity_factory.autoenable_created_entities)
            .unwrap_or(true);
        if self.inner.entity_state.is_enabled() && autoenable {
            #[cfg(feature = "std")]
            return child.enable_entity();
            #[cfg(not(feature = "std"))]
            child.enable();
        }
        Ok(())
    }

    #[cfg(feature = "std")]
    fn track_writer<T: DdsType + Send + 'static>(&self, writer: &DataWriter<T>) {
        let weak: alloc::sync::Weak<dyn crate::contained::ContainedWriter> =
            Arc::downgrade(&writer.inner) as _;
        self.inner
            .writers
            .add(writer.inner.entity_state.instance_handle(), weak);
    }
}

#[cfg(feature = "std")]
impl PublisherInner {
    /// The participant this publisher came from, while it is still alive.
    pub(crate) fn participant_inner(&self) -> Option<Arc<crate::participant::ParticipantInner>> {
        self.participant
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().and_then(alloc::sync::Weak::upgrade))
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

    /// DDS 1.4 §2.2.3: PARTITION, GROUP_DATA and ENTITY_FACTORY are
    /// Changeable=YES, PRESENTATION is not. A new PARTITION or GROUP_DATA
    /// reaches every DataWriter of this publisher that does not set its
    /// own, in its SEDP publication and in the matching.
    fn set_qos(&self, qos: impl Into<crate::qos::QosArg<Self::Qos>>) -> Result<()> {
        let qos = qos.into().try_resolve(|| {
            self.get_participant()
                .map(|p| p.get_default_publisher_qos())
                .ok_or(DdsError::PreconditionNotMet {
                    reason: "the participant of this publisher is gone",
                })
        })?;
        let enabled = self.inner.entity_state.is_enabled();
        let Ok(mut current) = self.inner.qos.lock() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "publisher qos poisoned",
            });
        };
        if enabled && current.presentation != qos.presentation {
            return Err(crate::entity::immutable_if_enabled("PRESENTATION"));
        }
        // DDS 1.4 §2.2.2.1.1.1: "In all other cases, none of the policies is
        // modified." ENTITY_FACTORY changed before the PARTITION check, and
        // stayed changed when that refused.
        if let Some(rt) = self.inner.runtime.as_ref() {
            if current.partition != qos.partition || current.group_data != qos.group_data {
                let writers = self
                    .inner
                    .writer_entity_ids
                    .lock()
                    .map(|ids| ids.clone())
                    .unwrap_or_default();
                // DDS-Security 1.2 §7.4.5: an enabled publisher keeps its
                // PARTITION while it holds a durable writer of a read
                // protected topic.
                if enabled && current.partition != qos.partition {
                    for eid in &writers {
                        rt.check_user_writer_partition_change(*eid)?;
                    }
                }
                // Every writer may move into the new partitions before any
                // does: a refusal leaves the publisher as it was.
                for eid in &writers {
                    rt.check_user_group_partition(*eid, &qos.partition.names)?;
                }
                for eid in writers {
                    rt.update_user_group_qos(eid, &qos.partition.names, &qos.group_data.value)?;
                }
            }
        }
        self.inner
            .entity_state
            .set_autoenable_children(qos.entity_factory.autoenable_created_entities);
        *current = qos;
        Ok(())
    }

    /// DDS 1.4 §2.2.2.1.1.7; with ENTITY_FACTORY.autoenable_created_entities
    /// it enables the writers this publisher created.
    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable_entity()
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
///
/// A `DataWriter` is a handle: [`Clone`] gives another handle to the same
/// writer, as [`Publisher::lookup_datawriter`] does. The writer is deleted
/// by [`Publisher::delete_datawriter`], by the publisher's or participant's
/// `delete_contained_entities`, or when its last handle is dropped; an
/// operation on a handle of a deleted writer returns
/// [`DdsError::AlreadyDeleted`].
pub struct DataWriter<T: DdsType> {
    pub(crate) inner: Arc<WriterInner<T>>,
}

impl<T: DdsType> Clone for DataWriter<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// The writer the handles of a [`DataWriter`] share.
pub(crate) struct WriterInner<T: DdsType> {
    topic: Topic<T>,
    /// Shared with the writer's registration ([`WriterRegistration`]), which
    /// registers it with the QoS it has at its `enable`.
    qos: Arc<Mutex<DataWriterQos>>,
    /// Entity lifecycle (DCPS §2.2.2.1).
    entity_state: Arc<crate::entity::EntityState>,
    /// Parent publisher (clone of the `Arc`) — for bubble-up to the
    /// publisher and participant listeners.
    publisher: Arc<PublisherInner>,
    /// The listener and the last seen value of every status it is told
    /// about, shared with the runtime's listener dispatcher.
    #[cfg(feature = "std")]
    status: Arc<WriterStatus>,
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
    /// Set when the writer's QoS is incompatible with the Topic's fictitious
    /// persistence-service reader (DDS 1.4 §2.2.3.4): "will not send its
    /// data to the transient/persistent service".
    #[cfg(feature = "std")]
    store_refused: Arc<core::sync::atomic::AtomicBool>,
    /// Monotonically increasing writer sequence for durability-backend
    /// storage (DDS 1.4 §2.2.3.5 + backend replay order).
    #[cfg(feature = "std")]
    /// The last written key holder and its hash; see
    /// [`crate::topic::KeyMaterialCache`].
    #[cfg(feature = "std")]
    last_key: crate::topic::KeyMaterialCache,
    /// The encoded first sample of a writer of a topic without a key. The
    /// topic has no key members, so `get_key_value` has no key to rebuild a
    /// value from and returns this one (DDS 1.4 §2.2.1.2.2).
    #[cfg(feature = "std")]
    keyless_value: std::sync::OnceLock<Vec<u8>>,
    /// The length of the last encoded body, the capacity the next write's
    /// buffer starts with.
    encoded_size_hint: core::sync::atomic::AtomicUsize,
    /// The series every `write` updates, looked up at the first write;
    /// see [`crate::metrics::WriterMetrics`].
    #[cfg(feature = "metrics")]
    metrics: std::sync::OnceLock<crate::metrics::WriterMetrics>,
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
    /// Emit the user payload big-endian (encapsulation `*_BE` id + BE body via
    /// `DdsType::encode_be`) instead of the little-endian default. Set from
    /// `wire.big_endian` (env `ZERODDS_WIRE_BIG_ENDIAN`) at creation. The matching `*_BE` encap header is
    /// stamped by wiring the runtime slot's byte-order override in
    /// `create_datawriter`. Standard DDS readers decode per-sample endianness
    /// from the encap, so this stays cross-vendor interoperable.
    #[cfg(feature = "std")]
    big_endian: bool,
    /// Emit the user payload as **XCDR1** (classic CDR, `DdsType::encode_xcdr1`
    /// / `encode_xcdr1_be`) instead of the XCDR2 default. Set at creation from
    /// the runtime's resolved *effective first* DataRepresentation
    /// ([`DcpsRuntime::user_writer_offer_first`]) — the same source that drives
    /// the SEDP announcement and the encapsulation header — so the body matches
    /// the declared encap. Without it a `-x 1` / XCDR1-offer writer would stamp
    /// a CDR_LE (`0x01`) encap but ship an XCDR2 body (e.g. an @appendable
    /// DHEADER), which a strict XCDR1 reader (CycloneDDS) mis-frames.
    #[cfg(feature = "std")]
    emit_xcdr1: bool,
    _t: PhantomData<fn() -> T>,
}

impl<T: DdsType> core::fmt::Debug for DataWriter<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DataWriter")
            .field("topic", &self.inner.topic.name())
            .field("type", &T::type_name())
            .field("qos", &self.inner.qos)
            .finish_non_exhaustive()
    }
}

/// The part of a `DataWriter` its status listeners need: the listener,
/// the parents of the bubble-up chain and the last seen value of every
/// communication status (DDS 1.4 §2.2.4.1). Shared with the runtime's
/// listener dispatcher, which runs [`WriterStatus::drive`] when a status
/// moves.
#[cfg(feature = "std")]
pub(crate) struct WriterStatus {
    runtime: Option<Arc<DcpsRuntime>>,
    entity_id: Option<EntityId>,
    entity_state: Arc<crate::entity::EntityState>,
    publisher: Arc<PublisherInner>,
    /// Optional `DataWriterListener` + `StatusMask`.
    listener: Mutex<Option<(ArcDataWriterListener, StatusMask)>>,
    /// The writer's instances: the runtime names a missed deadline's
    /// instance by key hash, this turns it into the handle.
    instances: InstanceTracker,
    /// The topic name and the store of a TRANSIENT or PERSISTENT writer, for
    /// the loss of its liveliness (DDS 1.4 §2.2.3.4 condition 2).
    store: Mutex<
        Option<(
            String,
            Arc<dyn crate::durability_service::DurabilityBackend>,
        )>,
    >,
}

#[cfg(feature = "std")]
impl WriterStatus {
    fn new(
        runtime: Option<Arc<DcpsRuntime>>,
        entity_id: Option<EntityId>,
        entity_state: &Arc<crate::entity::EntityState>,
        publisher: &Arc<PublisherInner>,
        instances: &InstanceTracker,
    ) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            entity_id,
            entity_state: Arc::clone(entity_state),
            publisher: Arc::clone(publisher),
            listener: Mutex::new(None),
            instances: instances.clone(),
            store: Mutex::new(None),
        })
    }

    /// Names the store this writer feeds, so that the loss of its liveliness
    /// can take it out of the live writers of its instances.
    pub(crate) fn set_store(
        &self,
        topic: &str,
        backend: Arc<dyn crate::durability_service::DurabilityBackend>,
    ) {
        if let Ok(mut slot) = self.store.lock() {
            *slot = Some((topic.to_string(), backend));
        }
    }

    /// DDS 1.4 §2.2.3.4 condition 2 counts "live" DataWriters: a writer that
    /// lost its liveliness (§2.2.3.11) is not one until it asserts it again
    /// by writing. The `service_cleanup_delay` of an instance that is
    /// disposed and that only this writer still wrote starts now.
    fn leave_store_on_lost_liveliness(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        if !rt.user_writer_liveliness_is_lost(eid) {
            return;
        }
        let Some((topic, backend)) = self.store.lock().ok().and_then(|s| s.clone()) else {
            return;
        };
        let Some(participant) = self.publisher.participant_inner() else {
            return;
        };
        let writer = self.entity_state.instance_handle().as_raw();
        for handle in self.instances.ordered_handles() {
            let Some(key) = self.instances.key_hash_for_handle(handle) else {
                continue;
            };
            if participant.store_writers.is_live(&topic, key, writer)
                && participant.store_writers.left(&topic, key, writer, false)
            {
                let _ = backend.unregister_instance(&topic, key, std::time::SystemTime::now());
            }
        }
    }

    /// Snapshot of the bubble-up chain (writer → publisher → participant).
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

    /// Calls `on_publication_matched` while PUBLICATION_MATCHED changed
    /// since it was last read or handed to a listener (DDS 1.4 §2.2.4.1),
    /// with the status as the runtime keeps it: `total_count` cumulative,
    /// the last reader that joined or left. A listener that took the status
    /// acknowledges its changes; a listener set after the change hears it
    /// at its first look.
    #[cfg(feature = "std")]
    pub(crate) fn poll_publication_matched(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || rt.observe_user_writer_publication_matched_status(eid),
            &self.entity_state,
            crate::psm_constants::status::PUBLICATION_MATCHED,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::PUBLICATION_MATCHED)
            },
            |status| {
                crate::listener_dispatch::dispatch_publication_matched(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| {
                rt.acknowledge_user_writer_publication_matched_status(eid, status, reads)
            },
        );
    }

    /// `on_offered_deadline_missed` while the status is unread; see
    /// [`Self::poll_publication_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_offered_deadline_missed(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || {
                rt.observe_user_writer_offered_deadline_missed_status(eid)
                    .map(|(unread, status, reads)| {
                        (unread, self.with_missed_instance(status), reads)
                    })
            },
            &self.entity_state,
            crate::psm_constants::status::OFFERED_DEADLINE_MISSED,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::OFFERED_DEADLINE_MISSED)
            },
            |status| {
                crate::listener_dispatch::dispatch_offered_deadline_missed(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| {
                rt.acknowledge_user_writer_offered_deadline_missed_status(eid, status, reads)
            },
        );
    }

    /// `status` with the handle of the instance whose deadline was missed
    /// last; the runtime knows it by key hash only.
    #[cfg(feature = "std")]
    pub(crate) fn with_missed_instance(
        &self,
        mut status: crate::status::OfferedDeadlineMissedStatus,
    ) -> crate::status::OfferedDeadlineMissedStatus {
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            if let Some(handle) = rt
                .user_writer_missed_deadline_instance(eid)
                .and_then(|key| self.instances.lookup(&key))
            {
                status.last_instance_handle = handle;
            }
        }
        status
    }

    /// `on_liveliness_lost` while the status is unread; see
    /// [`Self::poll_publication_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_liveliness_lost(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || rt.observe_user_writer_liveliness_lost_status(eid),
            &self.entity_state,
            crate::psm_constants::status::LIVELINESS_LOST,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::LIVELINESS_LOST)
            },
            |status| {
                crate::listener_dispatch::dispatch_liveliness_lost(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| rt.acknowledge_user_writer_liveliness_lost_status(eid, status, reads),
        );
    }

    /// `on_offered_incompatible_qos` while the status is unread; see
    /// [`Self::poll_publication_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_offered_incompatible_qos(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || rt.observe_user_writer_offered_incompatible_qos_status(eid),
            &self.entity_state,
            crate::psm_constants::status::OFFERED_INCOMPATIBLE_QOS,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::OFFERED_INCOMPATIBLE_QOS)
            },
            |status| {
                crate::listener_dispatch::dispatch_offered_incompatible_qos(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| {
                rt.acknowledge_user_writer_offered_incompatible_qos_status(eid, status, reads)
            },
        );
    }
}

#[cfg(feature = "std")]
impl crate::listener_dispatcher::StatusDriver for WriterStatus {
    /// Every writer status against its last seen value, read from the
    /// runtime as the getters read it. Offline there is nothing to read.
    fn drive(&self) {
        self.poll_publication_matched();
        self.poll_offered_deadline_missed();
        self.poll_liveliness_lost();
        self.leave_store_on_lost_liveliness();
        self.poll_offered_incompatible_qos();
    }
}

/// The end of the last handle deletes a writer that is not deleted yet.
#[cfg(feature = "std")]
impl<T: DdsType> Drop for WriterInner<T> {
    fn drop(&mut self) {
        self.delete();
    }
}

#[cfg(feature = "std")]
impl<T: DdsType> WriterInner<T> {
    /// Waits until the reliable readers of the writer have acknowledged
    /// everything it wrote, or `deadline` passes; `true` when they have.
    #[cfg(feature = "std")]
    fn await_acknowledged(&self, deadline: std::time::Instant) -> bool {
        match (&self.runtime, self.entity_id) {
            (Some(rt), Some(eid)) => rt.await_user_writer_acknowledged(eid, deadline),
            // Offline: nothing to acknowledge.
            _ => true,
        }
    }

    /// Deletes the writer (DDS 1.4 §2.2.2.4.1.6); the first call does it,
    /// a later one finds it deleted and returns.
    ///
    /// "The deletion of the DataWriter will automatically unregister all
    /// instances", and with WRITER_DATA_LIFECYCLE
    /// `autodispose_unregistered_instances` it disposes them (§2.2.3.21):
    /// every instance the writer holds registered goes out as one
    /// lifecycle marker before the runtime slot is removed, unless
    /// `writer.unregister_on_delete` is off
    /// ([`crate::runtime::RuntimeConfig::writer_unregister_on_delete`]).
    /// The writer then waits for its reliable readers to acknowledge what
    /// it sent, at most `writer.linger_ms`
    /// ([`crate::runtime::DcpsRuntime::linger_user_writer`]). Removing the
    /// slot stops the heartbeats and sends the SEDP dispose of the
    /// publication, so remote peers drop the match at once instead of
    /// waiting for a liveliness timeout. Offline writers have no slot.
    fn delete(&self) {
        if !self.entity_state.mark_deleted() {
            return;
        }
        self.publisher
            .writers
            .remove(self.entity_state.instance_handle());
        if let (Some(rt), Some(eid)) = (self.runtime.as_ref(), self.entity_id) {
            // `writer.unregister_on_delete` off: no markers, the SEDP
            // dispose below ends the instances as NOT_ALIVE_NO_WRITERS.
            if rt.config.writer_unregister_on_delete {
                self.unregister_all_instances(rt, eid);
            }
            // What it sent reaches its reliable readers before it goes.
            rt.linger_user_writer(eid);
            if let Ok(mut ids) = self.publisher.writer_entity_ids.lock() {
                ids.retain(|e| *e != eid);
            }
            self.publisher.update_group_writers();
            // DDS 1.4 §2.2.3.4: the persistence service of a TRANSIENT or
            // PERSISTENT topic outlives the DataWriter. While its store holds
            // samples, the endpoint stays as the fictitious DataWriter for
            // readers that come later.
            let kept = match self.durability_backend.as_ref() {
                Some(backend)
                    if backend
                        .replay_for_topic(self.topic.name())
                        .is_ok_and(|samples| !samples.is_empty()) =>
                {
                    let (backend, topic) = (Arc::clone(backend), self.topic.name().to_string());
                    rt.retain_user_writer_as_service(self.topic.name(), eid, move || {
                        backend
                            .replay_for_topic(&topic)
                            .map_or(true, |samples| samples.is_empty())
                    });
                    true
                }
                _ => false,
            };
            if !kept {
                rt.remove_user_writer(eid);
            }
        } else {
            self.leave_store_of_all_instances();
        }
        // A deleted writer is one user of its topic less: DDS 1.4
        // §2.2.2.2.1.6 refuses `delete_topic` only while one exists.
        self.topic
            .detach_user(crate::topic::TopicUserKind::DataWriter);
    }

    /// A writer without a runtime sends no markers when it is deleted, but it
    /// is no live writer of its instances any more: the store hears of it
    /// (DDS 1.4 §2.2.3.4).
    fn leave_store_of_all_instances(&self) {
        if self.durability_backend.is_none() {
            return;
        }
        let autodispose = self
            .qos
            .lock()
            .map(|q| q.writer_data_lifecycle.autodispose_unregistered_instances)
            .unwrap_or(true);
        let now = get_current_time();
        for handle in self.instances.ordered_handles() {
            let Some(state) = self.instances.get_by_handle(handle) else {
                continue;
            };
            if state.writer_count == 0 {
                continue;
            }
            let Some(key_hash) = self.instances.key_hash_for_handle(handle) else {
                continue;
            };
            let disposes =
                autodispose && state.kind == crate::sample_info::InstanceStateKind::Alive;
            if disposes {
                let _ = self.instances.dispose(handle, Some(now));
            }
            let _ = self.instances.unregister(handle, Some(now));
            self.leave_store(
                key_hash,
                disposes || state.kind == crate::sample_info::InstanceStateKind::NotAliveDisposed,
            );
        }
    }

    /// This writer stops writing the instance `key_hash` (it unregistered
    /// it, or is deleted); `disposed` says it disposed it too. The store
    /// starts the `service_cleanup_delay` of the instance when it is disposed
    /// and no live writer of this participant writes it any more (DDS 1.4
    /// §2.2.3.4).
    fn leave_store(&self, key_hash: crate::instance_tracker::KeyHash, disposed: bool) {
        let Some(backend) = self.durability_backend.as_ref() else {
            return;
        };
        let last = match self.publisher.participant_inner() {
            Some(participant) => participant.store_writers.left(
                self.topic.name(),
                key_hash,
                self.entity_state.instance_handle().as_raw(),
                disposed,
            ),
            None => disposed,
        };
        if !last {
            return;
        }
        if let Err(error) =
            backend.unregister_instance(self.topic.name(), key_hash, std::time::SystemTime::now())
        {
            if let Some(rt) = self.runtime.as_ref() {
                rt.config.observability.record(
                    &zerodds_foundation::observability::Event::new(
                        zerodds_foundation::observability::Level::Warn,
                        zerodds_foundation::observability::Component::Dcps,
                        "writer.durability.store_failed",
                    )
                    .with_attr("topic", self.topic.name())
                    .with_attr("error", alloc::format!("{error}").as_str()),
                );
            }
        }
    }

    /// The lifecycle markers of [`Self::delete`]: UNREGISTERED, with
    /// DISPOSED when the writer's QoS autodisposes, for every instance
    /// still registered (`writer_count > 0` on the writer side).
    fn unregister_all_instances(&self, rt: &DcpsRuntime, eid: EntityId) {
        use zerodds_rtps::inline_qos::status_info;
        let autodispose = self
            .qos
            .lock()
            .map(|q| q.writer_data_lifecycle.autodispose_unregistered_instances)
            .unwrap_or(true);
        let now = get_current_time();
        for handle in self.instances.ordered_handles() {
            let Some(state) = self.instances.get_by_handle(handle) else {
                continue;
            };
            if state.writer_count == 0 {
                continue;
            }
            let Some(key_hash) = self.instances.key_hash_for_handle(handle) else {
                continue;
            };
            let disposes =
                autodispose && state.kind == crate::sample_info::InstanceStateKind::Alive;
            let mut bits = status_info::UNREGISTERED;
            if disposes {
                bits |= status_info::DISPOSED;
                let _ = self.instances.dispose(handle, Some(now));
            }
            let _ = self.instances.unregister(handle, Some(now));
            self.leave_store(
                key_hash,
                disposes || state.kind == crate::sample_info::InstanceStateKind::NotAliveDisposed,
            );
            // The deletion goes on: DDS 1.4 §2.2.2.4.1.6 gives
            // delete_datawriter no TIMEOUT. A marker that could not be
            // sent is named, not dropped.
            if let Err(e) = rt.write_user_lifecycle(
                eid,
                key_hash,
                state.key_holder.as_deref(),
                bits,
                Some(crate::time::time_to_he_timestamp(now)),
            ) {
                rt.config.observability.record(
                    &zerodds_foundation::observability::Event::new(
                        zerodds_foundation::observability::Level::Warn,
                        zerodds_foundation::observability::Component::Dcps,
                        "writer.delete.lifecycle_not_sent",
                    )
                    .with_attr("topic", self.topic.name())
                    .with_attr("reason", alloc::format!("{e}").as_str()),
                );
            }
        }
    }
}

/// A DataWriter whatever its type: what `StatusCondition::get_entity`
/// returns for the StatusCondition of a writer. [`Self::typed`] gives the
/// [`DataWriter`] handle of its type.
#[cfg(feature = "std")]
#[derive(Clone)]
pub struct AnyDataWriter {
    inner: Arc<dyn crate::contained::Contained>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for AnyDataWriter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnyDataWriter")
            .field("topic", &self.inner.topic_name())
            .field("handle", &self.inner.handle())
            .finish()
    }
}

#[cfg(feature = "std")]
impl AnyDataWriter {
    /// The writer's `InstanceHandle` (DDS 1.4 §2.2.2.1.1.9).
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.inner.handle()
    }

    /// The name of the topic the writer writes.
    #[must_use]
    pub fn topic_name(&self) -> &str {
        self.inner.topic_name()
    }

    /// The writer as a `DataWriter<T>`, or `None` if it writes another type.
    #[must_use]
    pub fn typed<T: DdsType + Send + 'static>(&self) -> Option<DataWriter<T>> {
        Arc::clone(&self.inner)
            .into_any()
            .downcast::<WriterInner<T>>()
            .ok()
            .map(DataWriter::from_inner_arc)
    }
}

/// The part of a writer its [`Publisher`] holds for `lookup_datawriter`,
/// `delete_contained_entities` and the participant's `contains_entity`.
#[cfg(feature = "std")]
impl<T: DdsType + Send + 'static> crate::contained::Contained for WriterInner<T> {
    fn handle(&self) -> InstanceHandle {
        self.entity_state.instance_handle()
    }

    fn topic_name(&self) -> &str {
        self.topic.name()
    }

    fn is_deleted(&self) -> bool {
        self.entity_state.is_deleted()
    }

    fn delete(&self) -> Result<()> {
        WriterInner::delete(self);
        Ok(())
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }
}

impl<T: DdsType + Send + 'static> crate::contained::ContainedWriter for WriterInner<T> {
    fn await_acknowledged(&self, deadline: std::time::Instant) -> bool {
        WriterInner::await_acknowledged(self, deadline)
    }
}

/// The instant `timeout` from now. A timeout past what the monotonic clock
/// can hold waits as long as it can; adding it would panic.
#[cfg(feature = "std")]
fn acknowledgment_deadline(timeout: core::time::Duration) -> std::time::Instant {
    let now = std::time::Instant::now();
    now.checked_add(timeout)
        .or_else(|| now.checked_add(core::time::Duration::from_secs(u64::from(u32::MAX))))
        .unwrap_or(now)
}

/// A source timestamp an application passes (`*_w_timestamp`): DDS 1.4
/// §2.3.2 "the nanosec field must verify 0 <= nanosec < 1000000000", which
/// also leaves out the INVALID and INFINITE sentinels. Any `nanosec` was
/// taken.
#[cfg(feature = "std")]
fn check_timestamp(timestamp: Time) -> Result<()> {
    if timestamp.nanosec >= 1_000_000_000 {
        return Err(DdsError::BadParameter {
            what: "a timestamp with nanosec of 1000000000 or more",
        });
    }
    Ok(())
}

/// Validates a writer's `DataRepresentation` offer. ZeroDDS emits exactly one
/// wire representation per writer, so only **XCDR1** (`0`), **XCDR2** (`2`)
/// or **XML** (`1`, `{0x00, 0x04}` on the wire; the body handed to the
/// writer is XCDR1, the document is built from it) are accepted as the
/// first (emitted) id. Any **unknown** id is rejected with a clear error rather than silently falling
/// back to some other wire form. `None` (policy left at default) is accepted
/// and resolves to the runtime's offer, whose library default is
/// `DEFAULT_OFFER = [XCDR1]`.
///
/// An **explicitly empty** offer is accepted: DDS-XTypes 1.3 §7.6.3.1.1,
/// "An empty list of preferences shall be taken to be equivalent to a list
/// containing the single element XCDR." It is the offer `[XCDR1]`; see
/// [`crate::qos::effective_data_representation`].
#[cfg(feature = "std")]
fn validate_writer_data_representation(qos: &DataWriterQos) -> Result<()> {
    use zerodds_rtps::publication_data::data_representation as dr;
    if let Some(v) = qos.data_representation.as_ref() {
        match v.first().copied() {
            None => {}
            Some(id) if id == dr::XCDR || id == dr::XCDR2 || id == dr::XML => {}
            Some(_) => {
                return Err(DdsError::Unsupported {
                    feature: "DATA_REPRESENTATION other than XCDR1, XCDR2 and XML (DDS 1.4 §2.2.2.1.1.1)",
                });
            }
        }
    }
    Ok(())
}

/// `true` if the body handed to the runtime is **XCDR1** — the offer's first
/// id is XCDR1, or XML, whose document the runtime builds from an XCDR1 body
/// ([`crate::xml_data`]). Offers are validated at creation
/// ([`validate_writer_data_representation`]) to be XCDR1, XCDR2 or absent, so
/// XML / unknown / empty offers never reach here; there is deliberately no
/// fallback that would emit one of them as something else.
///
/// An absent offer resolves to `DEFAULT_OFFER`'s first element and not to a
/// literal, so an offline writer emits what the library's default announces.
/// Offline means no runtime slot and therefore no `RuntimeConfig` and no
/// `ZERODDS_DATA_REPR_OFFER`; the live path resolves from
/// [`DcpsRuntime::user_writer_offer_first`], which does see both.
#[cfg(feature = "std")]
fn qos_emit_xcdr1(qos: &DataWriterQos) -> bool {
    use zerodds_rtps::publication_data::data_representation as dr;
    qos.data_representation
        .as_ref()
        .and_then(|v| v.first().copied())
        .unwrap_or(dr::DEFAULT_OFFER[0])
        != dr::XCDR2
}

impl<T: DdsType> DataWriter<T> {
    fn from_inner(inner: WriterInner<T>) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    #[cfg(feature = "std")]
    fn from_inner_arc(inner: Arc<WriterInner<T>>) -> Self {
        Self { inner }
    }

    #[cfg(feature = "std")]
    fn new_offline(
        topic: Topic<T>,
        qos: DataWriterQos,
        publisher: Arc<PublisherInner>,
        backend: Option<Arc<dyn crate::durability_service::DurabilityBackend>>,
    ) -> Self {
        let tracker = InstanceTracker::new();
        let pub_handle = InstanceHandle::from_raw(0xFFFF_0000_0000_0001);
        // Resolve XCDR1 from the qos offer before `qos` is moved into the slot
        // (absent → the first element of `DEFAULT_OFFER`).
        let emit_xcdr1 = qos_emit_xcdr1(&qos);
        let entity_state = crate::entity::EntityState::new();
        let status = WriterStatus::new(None, None, &entity_state, &publisher, &tracker);
        Self::from_inner(WriterInner {
            topic,
            qos: Arc::new(Mutex::new(qos)),
            entity_state,
            publisher,
            status,
            queue: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            drain_signal: Arc::new(std::sync::Condvar::new()),
            runtime: None,
            entity_id: None,
            instances: tracker,
            publication_handle: pub_handle,
            durability_backend: backend,
            #[cfg(feature = "std")]
            store_refused: Arc::new(core::sync::atomic::AtomicBool::new(false)),
            last_key: crate::topic::KeyMaterialCache::default(),
            #[cfg(feature = "std")]
            keyless_value: std::sync::OnceLock::new(),
            encoded_size_hint: core::sync::atomic::AtomicUsize::new(0),
            #[cfg(feature = "metrics")]
            metrics: std::sync::OnceLock::new(),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            // Offline: no runtime and so no configuration; the compiled
            // default, little-endian.
            big_endian: false,
            // Offline: no runtime slot to consult, so resolve from the qos offer
            // directly (absent → the first element of `DEFAULT_OFFER`).
            emit_xcdr1,
            _t: PhantomData,
        })
    }

    /// Returns the durability backend (None for Volatile/TransientLocal).
    /// Test/inspection helper — Spec §2.2.3.5.
    #[doc(hidden)]
    #[cfg(feature = "std")]
    #[must_use]
    pub fn durability_backend(
        &self,
    ) -> Option<Arc<dyn crate::durability_service::DurabilityBackend>> {
        self.inner.durability_backend.clone()
    }

    /// Spec §2.2.3.5: with Durability=Transient the writer creates an
    /// in-memory backend; with Persistent an on-disk one under `dir`
    /// (`durability.dir`, env `ZERODDS_DURABILITY_DIR`), and without one
    /// under `std::env::temp_dir().join("zerodds-durability")`.
    ///
    /// # Errors
    /// `PreconditionNotMet` when the Persistent store cannot be created: a
    /// writer that promised PERSISTENT and runs without a store would lose
    /// every sample it was asked to keep.
    #[cfg(feature = "std")]
    fn build_durability_backend(
        qos: &DataWriterQos,
        dir: Option<&std::path::Path>,
    ) -> Result<Option<Arc<dyn crate::durability_service::DurabilityBackend>>> {
        Ok(match qos.durability.kind {
            zerodds_qos::DurabilityKind::Transient => Some(Arc::new(
                crate::durability_service::InMemoryDurabilityBackend::new(qos.durability_service),
            )),
            zerodds_qos::DurabilityKind::Persistent => {
                let root = dir.map_or_else(
                    || std::env::temp_dir().join("zerodds-durability"),
                    std::path::Path::to_path_buf,
                );
                Some(Arc::new(
                    crate::durability_service::OnDiskDurabilityBackend::new(
                        root,
                        qos.durability_service,
                    )?,
                ))
            }
            _ => None,
        })
    }

    #[cfg(feature = "std")]
    fn new_live(
        topic: Topic<T>,
        qos: DataWriterQos,
        publisher: Arc<PublisherInner>,
        runtime: Arc<DcpsRuntime>,
        entity_id: EntityId,
        backend: Option<Arc<dyn crate::durability_service::DurabilityBackend>>,
    ) -> Self {
        let tracker = InstanceTracker::new();
        // Same derivation `SampleInfo::publication_handle`
        // (`subscriber.rs`'s `writer_publication_handle`) and
        // `get_matched_publications()`'s equivalent
        // (`DcpsRuntime::user_reader_matched_publication_handles`) use for
        // this exact writer's GUID: `InstanceHandle::from_guid`. A writer
        // that reported a differently-derived handle for itself than what
        // a reader (even a same-runtime one) observes for it makes any
        // "is this sample mine" comparison against `publication_handle()`
        // silently always false — found the hard way, by
        // `spatial_bridge.rs`'s echo suppression bouncing samples it
        // should have recognized as its own sibling's.
        let pub_handle = InstanceHandle::from_guid(zerodds_rtps::wire_types::Guid::new(
            runtime.guid_prefix,
            entity_id,
        ));
        // Resolve the effective first DataRepresentation from the SAME source
        // the announcement + encap use (the runtime slot), so the body encoder
        // agrees with the declared encapsulation. Emit XCDR1 for any non-XCDR2
        // offer (mirrors `user_payload_encap`), never a silent XCDR2 body under
        // an XCDR1 header. The slot's offer override was set at
        // `register_user_writer_kind` (see `create_datawriter`), so it is
        // populated by now; an unknown eid (None) resolves from the qos the
        // same way an offline writer does, so no path here decides on a
        // literal that `DEFAULT_OFFER` could move away from.
        let big_endian = runtime.config.wire_big_endian;
        let emit_xcdr1 = runtime.user_writer_offer_first(entity_id).map_or_else(
            || qos_emit_xcdr1(&qos),
            |f| f != zerodds_rtps::publication_data::data_representation::XCDR2,
        );
        let entity_state = crate::entity::EntityState::new();
        // The StatusChangedFlags (DDS 1.4 §2.2.4.1) as `get_status_changes`
        // and a StatusCondition see them: from the runtime's ledgers at the
        // call, changed since last read or handed to a listener, not once a
        // listener look raised the flag (see the reader's
        // `install_status_probe`). Weak: the state must not keep the
        // runtime alive.
        let ledgers = Arc::downgrade(&runtime);
        entity_state.set_status_probe(Arc::new(move || {
            ledgers
                .upgrade()
                .map_or(0, |runtime| runtime.user_writer_status_changes(entity_id))
        }));
        let status = WriterStatus::new(
            Some(Arc::clone(&runtime)),
            Some(entity_id),
            &entity_state,
            &publisher,
            &tracker,
        );
        // The dispatcher holds it weakly: dropping the writer ends it.
        runtime
            .listener_dispatcher()
            .register(Arc::downgrade(&status)
                as alloc::sync::Weak<dyn crate::listener_dispatcher::StatusDriver>);
        Self::from_inner(WriterInner {
            topic,
            qos: Arc::new(Mutex::new(qos)),
            entity_state,
            publisher,
            status,
            queue: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            drain_signal: Arc::new(std::sync::Condvar::new()),
            runtime: Some(runtime),
            entity_id: Some(entity_id),
            instances: tracker,
            publication_handle: pub_handle,
            durability_backend: backend,
            #[cfg(feature = "std")]
            store_refused: Arc::new(core::sync::atomic::AtomicBool::new(false)),
            last_key: crate::topic::KeyMaterialCache::default(),
            #[cfg(feature = "std")]
            keyless_value: std::sync::OnceLock::new(),
            encoded_size_hint: core::sync::atomic::AtomicUsize::new(0),
            #[cfg(feature = "metrics")]
            metrics: std::sync::OnceLock::new(),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            big_endian,
            emit_xcdr1,
            _t: PhantomData,
        })
    }

    #[cfg(not(feature = "std"))]
    fn new(topic: Topic<T>, qos: DataWriterQos, publisher: Arc<PublisherInner>) -> Self {
        Self::from_inner(WriterInner {
            topic,
            qos,
            publisher,
            queue: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            drain_signal: Arc::new(std::sync::Condvar::new()),
            encoded_size_hint: core::sync::atomic::AtomicUsize::new(0),
            _t: PhantomData,
        })
    }

    /// The topic that is sent to.
    #[must_use]
    pub fn topic(&self) -> &Topic<T> {
        &self.inner.topic
    }

    /// `get_topic` of DDS 1.4 DataWriter::get_topic (the spelling of the specification).
    #[must_use]
    pub fn get_topic(&self) -> &Topic<T> {
        self.topic()
    }

    /// The Publisher that created this writer (DDS 1.4 §2.2.2.4.2
    /// `get_publisher`).
    #[must_use]
    pub fn get_publisher(&self) -> Publisher {
        Publisher {
            inner: Arc::clone(&self.inner.publisher),
        }
    }

    /// This writer's 16-byte RTPS GUID, when it is bound to a live runtime
    /// (online participant). `None` in offline mode. Used by the iceoryx-cyclone
    /// bridge to stamp the cross-vendor PSMX chunk with the writer's real GUID
    /// so a peer that discovered it over RTPS associates the SHM sample with it.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn rtps_guid(&self) -> Option<[u8; 16]> {
        Some(
            self.inner
                .runtime
                .as_ref()?
                .writer_guid(self.inner.entity_id?),
        )
    }

    /// The RTPS sequence number the next sample written through this writer
    /// will carry (DDSI-RTPS 2.5 §8.3.5.4). `None` in offline mode. Additive
    /// helper for RPC: the request header can name the identity the writer
    /// is about to stamp, so correlation by sample identity matches.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn next_sequence_number(&self) -> Option<i64> {
        self.inner
            .runtime
            .as_ref()?
            .user_writer_next_sequence_number(self.inner.entity_id?)
    }

    /// Announces the DDS-RPC discovery extension of this writer
    /// (DDS-RPC 1.0 §7.6.2.1.1): `service_instance_name`,
    /// `related_datareader_key` and `topic_aliases`. A writer without a
    /// runtime is discovered by nobody and has nothing to announce.
    ///
    /// # Errors
    /// [`DdsError::BadParameter`] for a `string<256>` that is longer;
    /// otherwise as `DcpsRuntime::announce_publication`.
    #[cfg(feature = "std")]
    pub fn set_rpc_discovery(&self, ext: &crate::runtime::RpcDiscoveryExt) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        match (&self.inner.runtime, self.inner.entity_id) {
            (Some(rt), Some(eid)) => rt.set_user_writer_rpc_discovery(eid, ext),
            _ => Ok(()),
        }
    }

    /// The readers this writer is matched with and what each announced of
    /// the DDS-RPC discovery extension (§7.6.2.1.2). Empty offline.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn matched_rpc_readers(&self) -> Vec<crate::runtime::RpcPeer> {
        match (&self.inner.runtime, self.inner.entity_id) {
            (Some(rt), Some(eid)) => rt.user_writer_matched_rpc_readers(eid),
            _ => Vec::new(),
        }
    }

    /// Sets the `DataWriterListener` + StatusMask. `None` clears the slot.
    /// Spec §2.2.2.4.2.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcDataWriterListener>, mask: StatusMask) {
        let listener = listener.or_else(|| {
            (mask != 0).then(|| {
                alloc::sync::Arc::new(crate::listener::NilListener) as ArcDataWriterListener
            })
        });
        if let Ok(mut slot) = self.inner.status.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
        crate::listener_dispatcher::ListenerDispatcher::listener_set(self.inner.runtime.as_ref());
    }

    /// Current listener clone, if present.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDataWriterListener> {
        self.inner
            .status
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
            .filter(|l| !l.is_nil())
    }

    /// Current QoS (cloned, .1).
    #[must_use]
    pub fn qos(&self) -> DataWriterQos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Sends a sample to all matched readers.
    ///
    /// **RESOURCE_LIMITS (DDS 1.4 §2.2.3.19, §2.2.2.4.2.11):** a RELIABLE
    /// writer whose cache holds `max_samples` samples, or under KEEP_ALL
    /// `max_samples_per_instance` of this instance, not yet acknowledged
    /// by every reliable reader waits until an acknowledgement makes room,
    /// at most `max_blocking_time`, then returns `Timeout`. Where waiting
    /// cannot make room it returns `OutOfResources` at once: a sample of a
    /// new instance beyond `max_instances`, or a TRANSIENT_LOCAL writer,
    /// which keeps acknowledged samples for late joiners. A BEST_EFFORT
    /// writer discards its oldest samples instead (§2.2.3.18). See
    /// [`DcpsRuntime::set_user_writer_resource_limits`]. Without a runtime
    /// the in-memory queue applies `max_samples` the same way, and refuses
    /// with `OutOfResources` when best-effort or `max_blocking_time` is 0.
    ///
    /// # Errors
    /// - `WireError` if `T::encode` fails.
    /// - `Timeout` if `max_blocking_time` passed without room.
    /// - `OutOfResources` where waiting cannot make room (above).
    /// - `PreconditionNotMet` on lock poisoning.
    pub fn write(&self, sample: &T) -> Result<()> {
        self.write_with_source_ts_override(sample, None)
    }

    /// DDS 1.4 §2.2.2.4.2.11 `write` with its `handle` parameter.
    /// `HANDLE_NIL` "indicates that the identity of the instance should be
    /// automatically deduced from the instance_data (by means of the key)",
    /// which is [`Self::write`]. Any other value "must correspond to the
    /// value returned by register_instance when the instance (identified by
    /// its key) was registered".
    ///
    /// # Errors
    /// As [`Self::write`]; `PreconditionNotMet` if `handle` names an
    /// existing instance other than the one `sample` belongs to;
    /// `BadParameter` if `handle` names no instance.
    #[cfg(feature = "std")]
    pub fn write_with_handle(&self, sample: &T, handle: InstanceHandle) -> Result<()> {
        self.check_write_handle(sample, handle)?;
        self.write(sample)
    }

    /// [`Self::write_with_handle`] with the source timestamp of
    /// [`Self::write_w_timestamp`] (§2.2.2.4.2.12: "the constraints on the
    /// values of the handle parameter and the corresponding error behavior
    /// are the same").
    ///
    /// # Errors
    /// As [`Self::write_with_handle`] and [`Self::write_w_timestamp`].
    #[cfg(feature = "std")]
    pub fn write_w_timestamp_with_handle(
        &self,
        sample: &T,
        handle: InstanceHandle,
        timestamp: Time,
    ) -> Result<()> {
        self.check_write_handle(sample, handle)?;
        self.write_w_timestamp(sample, timestamp)
    }

    /// The rule of §2.2.2.4.2.11 for the `handle` of a write: nil is
    /// deduced from the key; another value must be the handle the key
    /// registered.
    #[cfg(feature = "std")]
    fn check_write_handle(&self, sample: &T, handle: InstanceHandle) -> Result<()> {
        if handle.is_nil() {
            return Ok(());
        }
        self.inner.topic.codec().validate_sample(sample)?;
        if self.lookup_instance(sample) == handle {
            return Ok(());
        }
        if self.inner.instances.get_by_handle(handle).is_some() {
            return Err(DdsError::PreconditionNotMet {
                reason: "handle names another instance than the key",
            });
        }
        Err(DdsError::BadParameter {
            what: "handle names no instance",
        })
    }

    /// [`Self::write`], with an optional explicit SOURCE_TIMESTAMP
    /// override — [`Self::write_w_timestamp`]'s actual write path. `None`
    /// (plain `write()`) resolves to the configured `time_source` exactly
    /// as before; `Some(ts)` carries `ts` all the way to the wire INFO_TS
    /// and the same-runtime intra-process dispatch (DDS 1.4 §2.2.2.4.2.16:
    /// "the application provides the value for the source_timestamp").
    /// Offline mode (no runtime slot) has no source-timestamp concept on
    /// its queued bytes at all, `write_w_timestamp`'s or plain `write()`'s
    /// alike, so `source_ts_override` only matters in live mode.
    fn write_with_source_ts_override(
        &self,
        sample: &T,
        source_ts_override: Option<Time>,
    ) -> Result<()> {
        self.write_sample(sample, source_ts_override, None)
            .map(|_| ())
    }

    /// [`Self::write`], with the sample the new one answers or belongs to
    /// in the DATA's inline QoS. DDS-RPC 1.0 §7.8.2: the reply to a request
    /// "shall include the PID_RELATED_SAMPLE_IDENTITY", the sample identity
    /// of the request. Goes out as `PID_RELATED_SAMPLE_IDENTITY` (0x0083)
    /// and as eProsima's 0x800f. Returns the RTPS sequence number the
    /// sample went out with.
    ///
    /// # Errors
    /// As [`Self::write`]; `PreconditionNotMet` for a writer without a
    /// runtime, which has no inline QoS to carry the identity.
    pub fn write_related(
        &self,
        sample: &T,
        related: zerodds_rtps::inline_qos::SampleIdentityBytes,
    ) -> Result<i64> {
        self.write_sample(sample, None, Some(related))
    }

    fn write_sample(
        &self,
        sample: &T,
        source_ts_override: Option<Time>,
        related: Option<zerodds_rtps::inline_qos::SampleIdentityBytes>,
    ) -> Result<i64> {
        // DDS 1.4 §2.2.2.1.1.7: an operation not listed for a disabled
        // entity returns NOT_ENABLED.
        self.inner.entity_state.check_enabled()?;
        if let Some(timestamp) = source_ts_override {
            check_timestamp(timestamp)?;
        }
        // Sized from the last write: a body grown from empty reallocated
        // several times per write of a 1 kB sample.
        let mut buf = Vec::with_capacity(
            self.inner
                .encoded_size_hint
                .load(core::sync::atomic::Ordering::Relaxed),
        );
        // Body encoder MUST match the declared encapsulation (representation +
        // byte order). Resolve the representation from the SAME source the
        // runtime uses for the encap header + SEDP announcement
        // ([`DcpsRuntime::user_writer_offer_first`]). Body and encap read the
        // slot under two separate locks; the typed writer locked this slot's
        // representation at creation (`lock_user_writer_representation`), so a
        // runtime override cannot change it between the two reads — the body and
        // encap therefore always resolve the same offer. Offline writers (no
        // runtime slot) use the value resolved from the qos at creation.
        // Emitting an XCDR2 body under an XCDR1 encap (or a LE body under a BE
        // encap) makes a strict peer (CycloneDDS) mis-frame the sample. XCDR1
        // big-endian has no safe default and errors unless the type overrides
        // `encode_xcdr1_be`.
        let emit_xcdr1 = match (self.inner.runtime.as_ref(), self.inner.entity_id) {
            (Some(rt), Some(eid)) => rt
                .user_writer_offer_first(eid)
                .map_or(self.inner.emit_xcdr1, |f| {
                    f != zerodds_rtps::publication_data::data_representation::XCDR2
                }),
            _ => self.inner.emit_xcdr1,
        };
        let enc =
            self.inner
                .topic
                .codec()
                .encode(sample, &mut buf, emit_xcdr1, self.inner.big_endian);
        enc.map_err(|e| DdsError::WireError {
            message: e.to_string(),
        })?;
        self.inner
            .encoded_size_hint
            .store(buf.len(), core::sync::atomic::Ordering::Relaxed);
        // Derive once before publication side effects. Durability and live
        // transport must consume the same key, including a known zero hash.
        // One key materialization per write: the registration, the
        // durability backend and the live write all use it.
        #[cfg(feature = "std")]
        let key_hash = match self
            .inner
            .last_key
            .key_hash(self.inner.topic.codec(), sample)
        {
            Some(kh) => {
                // The tracker is handed the holder only when the instance is
                // new or comes back to life: a copy of the one the hash was
                // taken from.
                self.register_on_write(
                    kh,
                    || {
                        self.inner
                            .last_key
                            .holder_of(kh)
                            .unwrap_or_else(|| self.holder_of(sample))
                    },
                    || source_ts_override.unwrap_or_else(|| self.wall_now()),
                )?;
                Some(kh)
            }
            None => {
                // A topic without a key has one instance; the wire carries
                // no KeyHash for it.
                if !self.inner.topic.is_keyed() {
                    self.register_on_write(crate::topic::KEYLESS_INSTANCE_KEY, Vec::new, || {
                        source_ts_override.unwrap_or_else(|| self.wall_now())
                    })?;
                }
                None
            }
        };
        #[cfg(feature = "metrics")]
        self.inner
            .metrics
            .get_or_init(|| crate::metrics::WriterMetrics::for_topic(self.inner.topic.name()))
            .sample_written(buf.len());
        // Cross-vendor same-host zero-copy: if this writer was
        // `enable_cyclone_iox`-ed, additionally publish a Cyclone-PSMX
        // serialized chunk over iceoryx (best-effort; the RTPS path below
        // still runs for non-iox peers).
        #[cfg(all(feature = "std", feature = "cyclone-iox", target_os = "linux"))]
        crate::cyclone_iox_integration::publish(self.inner.topic.name(), sample);
        // Spec §2.2.3.5 DurabilityServiceQosPolicy: also store the sample
        // in the backend (Transient/Persistent) so that late-joiner
        // readers can still obtain it after the writer's history cleanup.
        #[cfg(feature = "std")]
        if let Some(backend) = self.inner.durability_backend.as_ref().filter(|_| {
            !self
                .inner
                .store_refused
                .load(core::sync::atomic::Ordering::Acquire)
        }) {
            let key_bytes = key_hash.unwrap_or([0u8; 16]);
            // Monotonic sequence of the store for replay order (DDS 1.4
            // §2.2.3.5); the store is shared by every writer of the topic.
            let seq = backend.next_sequence();
            let created_at = std::time::SystemTime::now();
            // DDS 1.4 §2.2.3.16: the sample is "removed … from the
            // transient and persistent information caches" once its
            // LIFESPAN, counted from the source timestamp, has passed.
            let expires_at = self.inner.qos.lock().ok().and_then(|q| {
                let nanos = q.lifespan.duration.to_nanos();
                u64::try_from(nanos)
                    .ok()
                    .and_then(|n| created_at.checked_add(core::time::Duration::from_nanos(n)))
            });
            if let Err(error) = backend.store(crate::durability_service::DurabilitySample {
                topic: self.inner.topic.name().to_string(),
                instance_key: key_bytes,
                sequence: seq,
                payload: buf.clone(),
                created_at,
                expires_at,
            }) {
                // The store refused the sample (DURABILITY_SERVICE limits or
                // I/O): the write goes on, a late joiner will not get it, and
                // that is reported rather than dropped.
                self.report_durability_store_failure(&error);
            }
            if let Some(participant) = self.inner.publisher.participant_inner() {
                participant.store_writers.joined(
                    self.inner.topic.name(),
                    key_bytes,
                    self.inner.entity_state.instance_handle().as_raw(),
                );
            }
        }
        // Live mode: delegate to the runtime → ReliableWriter → UDP.
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            let he_ts_override = source_ts_override.map(crate::time::time_to_he_timestamp);
            if let Some(related) = related {
                return rt.write_user_sample_related(eid, &buf, related);
            }
            if let Some(key) = key_hash {
                return rt
                    .write_user_sample_keyed_with_holder(
                        eid,
                        &buf,
                        key,
                        &|| {
                            self.inner
                                .last_key
                                .holder_of(key)
                                .unwrap_or_else(|| self.holder_of(sample))
                        },
                        he_ts_override,
                    )
                    .map(|()| -1);
            }
            return match he_ts_override {
                Some(ts) => rt.write_user_sample_stamped(eid, &buf, ts),
                None => rt.write_user_sample(eid, buf),
            }
            .map(|()| -1);
        }
        // The inline QoS that carries the identity exists only on the wire.
        if related.is_some() {
            return Err(DdsError::PreconditionNotMet {
                reason: "a related sample identity needs a writer with a runtime",
            });
        }
        // Offline fallback: in-memory queue with RESOURCE_LIMITS block.
        #[cfg(feature = "std")]
        {
            let qos = self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default();
            let max_samples = qos.resource_limits.max_samples;
            let reliable = qos.reliability.kind == ReliabilityKind::Reliable;
            let max_block = qos.reliability.max_blocking_time;
            let max_block_dur = core::time::Duration::from_nanos(
                u64::try_from(max_block.seconds).unwrap_or(0) * 1_000_000_000
                    + u64::from(max_block.fraction),
            );

            let mut q = self
                .inner
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
                    let (g, _) =
                        self.inner
                            .drain_signal
                            .wait_timeout(q, remaining)
                            .map_err(|_| DdsError::PreconditionNotMet {
                                reason: "datawriter queue poisoned",
                            })?;
                    q = g;
                    if q.len() < max_samples as usize {
                        break;
                    }
                    // otherwise spurious wakeup → keep waiting.
                }
            }
            q.push(buf);
            Ok(-1)
        }
        #[cfg(not(feature = "std"))]
        {
            let mut q = self
                .inner
                .queue
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datawriter queue poisoned",
                })?;
            q.push(buf);
            Ok(-1)
        }
    }

    /// Number of samples written so far. Test helper, replaced by real
    /// HistoryCache counters in the runtime.
    #[must_use]
    pub fn samples_pending(&self) -> usize {
        self.inner.queue.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Number of matched readers. Always 0 in offline mode. The list is
    /// [`Self::matched_subscriptions`].
    ///
    /// Side effect — on a change of the matched count relative to the
    /// last call, `on_publication_matched` is fired via the bubble-up
    /// chain (Spec §2.2.4.2.4.4).
    #[must_use]
    pub fn matched_subscription_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_publication_matched();
            return rt.user_writer_matched_count(eid);
        }
        0
    }

    /// The handles of the subscriptions this writer is matched with (DDS
    /// 1.4 §2.2.2.4.2 `get_matched_subscriptions`): discovered readers and
    /// readers of this participant. Empty offline.
    ///
    /// # Errors
    /// [`DdsError::NotEnabled`] before `enable`.
    pub fn get_matched_subscriptions(&self) -> Result<Vec<crate::instance_handle::InstanceHandle>> {
        self.matched_subscriptions()
    }

    /// The handles of the subscriptions this writer is matched with, as
    /// [`Self::get_matched_subscriptions`].
    ///
    /// # Errors
    /// [`DdsError::NotEnabled`] before `enable`.
    pub fn matched_subscriptions(&self) -> Result<Vec<crate::instance_handle::InstanceHandle>> {
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            return Ok(rt.user_writer_matched_subscription_handles(eid));
        }
        Ok(Vec::new())
    }

    /// The DCPSSubscription data of the subscription `handle` names (DDS
    /// 1.4 §2.2.2.4.2 `get_matched_subscription_data`), as it was last
    /// announced.
    ///
    /// # Errors
    /// [`DdsError::NotEnabled`] before `enable`; [`DdsError::BadParameter`]
    /// for a handle that names no subscription this writer is matched
    /// with, as the specification has it.
    pub fn matched_subscription_data(
        &self,
        handle: crate::instance_handle::InstanceHandle,
    ) -> Result<crate::builtin_topics::SubscriptionBuiltinTopicData> {
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            if let Some(data) = rt.user_writer_matched_subscription_data(eid, handle) {
                return Ok(crate::builtin_topics::SubscriptionBuiltinTopicData::from_wire(&data));
            }
        }
        let _ = handle;
        Err(DdsError::BadParameter {
            what: "no subscription matched with this writer has this handle",
        })
    }

    /// See [`WriterStatus::poll_publication_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_publication_matched(&self) {
        self.inner.status.poll_publication_matched();
    }

    /// See [`WriterStatus::poll_offered_deadline_missed`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_offered_deadline_missed(&self) {
        self.inner.status.poll_offered_deadline_missed();
    }

    /// See [`WriterStatus::poll_liveliness_lost`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_liveliness_lost(&self) {
        self.inner.status.poll_liveliness_lost();
    }

    /// See [`WriterStatus::poll_offered_incompatible_qos`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_offered_incompatible_qos(&self) {
        self.inner.status.poll_offered_incompatible_qos();
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
            if let Some(rt) = self.inner.runtime.as_ref() {
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
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_offered_deadline_missed();
            return rt.user_writer_offered_deadline_missed(eid);
        }
        0
    }

    /// Counter for LivelinessLost detections (Spec §2.2.4.2.10).
    /// Triggers `on_liveliness_lost` via bubble-up, if applicable.
    #[must_use]
    pub fn liveliness_lost_count(&self) -> u64 {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_liveliness_lost();
            return rt.user_writer_liveliness_lost(eid);
        }
        0
    }

    /// Current `OfferedIncompatibleQosStatus` (Spec §2.2.4.2.4.2).
    /// Triggers `on_offered_incompatible_qos`, if applicable.
    #[must_use]
    pub fn offered_incompatible_qos_status(&self) -> crate::status::OfferedIncompatibleQosStatus {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_offered_incompatible_qos();
            return rt.user_writer_offered_incompatible_qos(eid);
        }
        crate::status::OfferedIncompatibleQosStatus::default()
    }

    /// `get_publication_matched_status` (DDS 1.4 §2.2.2.4.2, §2.2.4.1):
    /// `total_count` counts every reader that ever matched, `current_count`
    /// those matched now, the changes count since the status was last read
    /// or handed to a listener, and the last handle is the reader that last
    /// joined or left. Reading resets the change fields.
    ///
    /// Offline: the default status.
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_publication_matched_status(
        &self,
    ) -> Result<crate::status::PublicationMatchedStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_publication_matched();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = rt
                .take_user_writer_publication_matched_status(eid)
                .unwrap_or_default();
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::PUBLICATION_MATCHED);
            return Ok(status);
        }
        Ok(crate::status::PublicationMatchedStatus::default())
    }

    /// `get_offered_deadline_missed_status` (DDS 1.4 §2.2.2.4.2); as
    /// [`Self::get_publication_matched_status`].
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_offered_deadline_missed_status(
        &self,
    ) -> Result<crate::status::OfferedDeadlineMissedStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_offered_deadline_missed();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = self.inner.status.with_missed_instance(
                rt.take_user_writer_offered_deadline_missed_status(eid)
                    .unwrap_or_default(),
            );
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::OFFERED_DEADLINE_MISSED);
            return Ok(status);
        }
        Ok(crate::status::OfferedDeadlineMissedStatus::default())
    }

    /// `get_liveliness_lost_status` (DDS 1.4 §2.2.2.4.2); as
    /// [`Self::get_publication_matched_status`].
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_liveliness_lost_status(&self) -> Result<crate::status::LivelinessLostStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_liveliness_lost();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = rt
                .take_user_writer_liveliness_lost_status(eid)
                .unwrap_or_default();
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::LIVELINESS_LOST);
            return Ok(status);
        }
        Ok(crate::status::LivelinessLostStatus::default())
    }

    /// `get_offered_incompatible_qos_status` (DDS 1.4 §2.2.2.4.2); as
    /// [`Self::get_publication_matched_status`].
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_offered_incompatible_qos_status(
        &self,
    ) -> Result<crate::status::OfferedIncompatibleQosStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_offered_incompatible_qos();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = rt
                .take_user_writer_offered_incompatible_qos_status(eid)
                .unwrap_or_default();
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::OFFERED_INCOMPATIBLE_QOS);
            return Ok(status);
        }
        Ok(crate::status::OfferedIncompatibleQosStatus::default())
    }

    /// Polls all statuses once and fires pending listeners. Convenience
    /// helper for tests + periodic tick callers.
    #[cfg(feature = "std")]
    pub fn drive_listeners(&self) {
        crate::listener_dispatcher::StatusDriver::drive(&*self.inner.status);
    }

    /// Manual liveliness assert. Spec §2.2.2.4.2.20 `assert_liveliness`.
    /// Sets the `last_liveliness_assert` timestamp; a no-op with
    /// automatic liveliness (every write asserts anyway).
    #[cfg(feature = "std")]
    pub fn assert_liveliness(&self) {
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            rt.assert_writer_liveliness_eid(eid);
        }
    }

    /// Blocks until all matched reliable readers have acknowledged all
    /// samples written so far, or `timeout` elapses.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.4.2.15 `wait_for_acknowledgments`.
    /// Returns `Ok(())` immediately in offline mode and without matched
    /// reliable readers.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] if not all samples are acknowledged within
    /// the time window.
    #[cfg(feature = "std")]
    pub fn wait_for_acknowledgments(&self, timeout: core::time::Duration) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        let deadline = acknowledgment_deadline(timeout);
        if self.inner.await_acknowledged(deadline) {
            Ok(())
        } else {
            Err(DdsError::Timeout)
        }
    }

    /// Takes all pending samples out of the offline queue. For tests
    /// only; removed with live-mode wiring.
    #[doc(hidden)]
    pub fn __drain_pending(&self) -> Vec<Vec<u8>> {
        let drained = self
            .inner
            .queue
            .lock()
            .map(|mut q| core::mem::take(&mut *q))
            .unwrap_or_default();
        // Spec §2.2.3.19: drain signal to waiting `write()` threads.
        #[cfg(feature = "std")]
        self.inner.drain_signal.notify_all();
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
        self.inner.publication_handle
    }

    /// Spec §2.2.2.1.1 `get_instance_handle` — entity identifier of this
    /// DataWriter for comparisons via
    /// `DomainParticipant::contains_entity`.
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.inner.entity_state.instance_handle()
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
        match (&self.inner.runtime, self.inner.entity_id) {
            (Some(rt), Some(eid)) => Some((Arc::clone(rt), eid)),
            _ => None,
        }
    }

    /// Returns the writer's shared instance tracker (test and inspection
    /// helper, Spec §2.2.2.4.2.5+ lifecycle bookkeeping).
    pub fn instance_tracker(&self) -> InstanceTracker {
        self.inner.instances.clone()
    }

    /// Computes the KeyHash + the PLAIN_CDR2-BE key holder for a sample.
    /// Returns `None` for non-keyed topics.
    #[cfg(feature = "std")]
    fn keyhash_and_holder(
        &self,
        sample: &T,
    ) -> Option<(crate::instance_tracker::KeyHash, Vec<u8>)> {
        self.inner
            .last_key
            .key_material(self.inner.topic.codec(), sample)
    }

    /// Tells the durability store that this writer has disposed and
    /// unregistered the instance `key_hash`, which starts its
    /// `service_cleanup_delay` (DDS 1.4 §2.2.3.4: the information is kept
    /// until the instance is disposed, no live writer writes it, and the
    /// delay has run out). A later write brings the instance back.
    #[cfg(feature = "std")]
    fn start_store_cleanup(&self, key_hash: crate::instance_tracker::KeyHash) {
        self.inner.leave_store(key_hash, true);
    }

    /// Reports that the durability store refused a sample of this writer.
    #[cfg(feature = "std")]
    fn report_durability_store_failure(&self, error: &DdsError) {
        use zerodds_foundation::observability::{Component, Event, Level};
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.config.observability.record(
                &Event::new(
                    Level::Warn,
                    Component::Dcps,
                    "writer.durability.store_failed",
                )
                .with_attr("topic", self.inner.topic.name())
                .with_attr("error", alloc::format!("{error}").as_str()),
            );
        }
    }

    /// The instance key of `sample`: its KeyHash and key holder, or for a
    /// topic without a key the one instance of the topic with no holder
    /// (DDS 1.4 §2.2.1.2.2: "If no key is provided, the data set associated
    /// with the Topic is restricted to a single instance.").
    #[cfg(feature = "std")]
    fn instance_key(&self, sample: &T) -> (crate::instance_tracker::KeyHash, Vec<u8>) {
        if !self.inner.topic.is_keyed() {
            return (crate::topic::KEYLESS_INSTANCE_KEY, Vec::new());
        }
        self.keyhash_and_holder(sample)
            .unwrap_or((crate::topic::KEYLESS_INSTANCE_KEY, Vec::new()))
    }

    /// The key holder of `sample`, for an instance the tracker does not
    /// have yet; empty for a keyless type, which never asks.
    fn holder_of(&self, sample: &T) -> Vec<u8> {
        let codec = self.inner.topic.codec();
        codec
            .key_holder(sample)
            .map(|(holder, _)| holder)
            .or_else(|| codec.key_material(sample).map(|(_, holder)| holder))
            .unwrap_or_default()
    }

    /// The wall clock behind every `register_instance`/`unregister_instance`/
    /// `dispose` default timestamp: [`crate::runtime::RuntimeConfig::time_source`]
    /// (C2(6)) when this writer has a runtime, else the platform clock — same
    /// as an offline writer had before that field existed.
    #[cfg(feature = "std")]
    fn wall_now(&self) -> Time {
        self.inner
            .runtime
            .as_ref()
            .map_or_else(get_current_time, |rt| rt.config.time_source.now())
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
        self.register_instance_w_timestamp(instance, self.wall_now())
    }

    /// Like `register_instance`, but with an explicit timestamp.
    /// Spec §2.2.2.4.2.6.
    #[cfg(feature = "std")]
    pub fn register_instance_w_timestamp(
        &self,
        instance: &T,
        timestamp: Time,
    ) -> Result<InstanceHandle> {
        self.inner.entity_state.check_enabled()?;
        check_timestamp(timestamp)?;
        self.inner.topic.codec().validate_sample(instance)?;
        let (kh, holder) = self.instance_key(instance);
        self.check_instance_access(&holder, false)?;
        if !self.inner.topic.is_keyed() && self.inner.keyless_value.get().is_none() {
            let mut encoded = Vec::new();
            self.inner
                .topic
                .codec()
                .encode(
                    instance,
                    &mut encoded,
                    self.inner.emit_xcdr1,
                    self.inner.big_endian,
                )
                .map_err(|e| DdsError::WireError {
                    message: alloc::string::ToString::to_string(&e),
                })?;
            self.inner.keyless_value.get_or_init(|| encoded);
        }
        Ok(self.inner.instances.register(kh, holder, Some(timestamp)))
    }

    /// Turns a sample value into its corresponding local
    /// [`InstanceHandle`], or [`HANDLE_NIL`] if unknown / non-keyed.
    /// Spec §2.2.2.4.2.14 `lookup_instance`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_instance(&self, instance: &T) -> InstanceHandle {
        if self.inner.topic.codec().validate_sample(instance).is_err() {
            return HANDLE_NIL;
        }
        let (kh, _) = self.instance_key(instance);
        self.inner.instances.lookup(&kh).unwrap_or(HANDLE_NIL)
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
        self.unregister_instance_w_timestamp(instance, handle, self.wall_now())
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
        self.inner.entity_state.check_enabled()?;
        check_timestamp(timestamp)?;
        let resolved = self.resolve_handle(instance, handle)?;
        let Some(state) = self.inner.instances.get_by_handle(resolved) else {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        };
        let autodispose = self
            .inner
            .qos
            .lock()
            .map(|q| q.writer_data_lifecycle.autodispose_unregistered_instances)
            .unwrap_or(true);
        // §9.4.2.9.8: an unregister that disposes needs the right to dispose.
        if autodispose {
            self.check_instance_access(&self.dispose_key(resolved, &state), true)?;
        }
        // Wire side (Spec §9.6.3.9 PID_STATUS_INFO): send a lifecycle
        // marker to all matched readers. With autodispose=true we set both
        // bits, otherwise only UNREGISTERED. First: DDS 1.4 §2.2.2.4.2.7 has
        // the operation return TIMEOUT where `write` would, and an
        // operation that failed changes nothing. Its error was dropped, and
        // the instance was unregistered here with no marker sent.
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid), Some(kh)) = (
            &self.inner.runtime,
            self.inner.entity_id,
            self.inner.instances.key_hash_for_handle(resolved),
        ) {
            let mut bits = zerodds_rtps::inline_qos::status_info::UNREGISTERED;
            if autodispose {
                bits |= zerodds_rtps::inline_qos::status_info::DISPOSED;
            }
            rt.write_user_lifecycle(
                eid,
                kh,
                state.key_holder.as_deref(),
                bits,
                Some(crate::time::time_to_he_timestamp(timestamp)),
            )?;
        }
        if autodispose {
            self.inner.instances.dispose(resolved, Some(timestamp));
        }
        self.inner.instances.unregister(resolved, Some(timestamp));
        if let Some(kh) = self.inner.instances.key_hash_for_handle(resolved) {
            self.inner.leave_store(
                kh,
                autodispose
                    || state.kind == crate::sample_info::InstanceStateKind::NotAliveDisposed,
            );
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
        self.dispose_w_timestamp(instance, handle, self.wall_now())
    }

    /// Like `dispose`, but with a timestamp. Spec §2.2.2.4.2.11.
    #[cfg(feature = "std")]
    pub fn dispose_w_timestamp(
        &self,
        instance: &T,
        handle: InstanceHandle,
        timestamp: Time,
    ) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        check_timestamp(timestamp)?;
        let resolved = self.resolve_handle(instance, handle)?;
        let Some(state) = self.inner.instances.get_by_handle(resolved) else {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        };
        self.check_instance_access(&self.dispose_key(resolved, &state), true)?;
        // Wire side (Spec §9.6.3.9 PID_STATUS_INFO), first: DDS 1.4
        // §2.2.2.4.2.13 has dispose return TIMEOUT and OUT_OF_RESOURCES
        // where `write` would, and an operation that failed changes nothing.
        // Its error was dropped.
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid), Some(kh)) = (
            &self.inner.runtime,
            self.inner.entity_id,
            self.inner.instances.key_hash_for_handle(resolved),
        ) {
            rt.write_user_lifecycle(
                eid,
                kh,
                state.key_holder.as_deref(),
                zerodds_rtps::inline_qos::status_info::DISPOSED,
                Some(crate::time::time_to_he_timestamp(timestamp)),
            )?;
        }
        self.inner.instances.dispose(resolved, Some(timestamp));
        // Disposed after the writer unregistered it: no live writer is left.
        if state.writer_count == 0 {
            if let Some(kh) = self.inner.instances.key_hash_for_handle(resolved) {
                self.start_store_cleanup(kh);
            }
        }
        Ok(())
    }

    /// Returns the sample value with only the `@key` fields populated
    /// (Spec §2.2.2.4.2.9 `get_key_value`). Implementation: we
    /// reconstruct `T` via `decode_key_holder_be` from the stored
    /// PLAIN_CDR2-BE key holder, not the full sample decoder.
    ///
    /// # Errors
    /// * `BadParameter` if the handle is unknown.
    /// * `BadParameter` if only its hash, not its key holder, is known and
    ///   the hash does not spell the key out.
    /// * `WireError` if the key holder cannot be reconstructed via
    ///   `T::decode_key_holder_be`.
    #[cfg(feature = "std")]
    pub fn get_key_value(&self, handle: InstanceHandle) -> Result<T> {
        self.inner.entity_state.check_enabled()?;
        let Some(state) = self.inner.instances.get_by_handle(handle) else {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        };
        let wire_error = |e: crate::dds_type::DecodeError| DdsError::WireError {
            message: alloc::string::ToString::to_string(&e),
        };
        if !self.inner.topic.is_keyed() {
            // No key members to fill: the first sample stands for the
            // instance.
            let Some(encoded) = self.inner.keyless_value.get() else {
                return Err(DdsError::BadParameter {
                    what: "the instance of a topic without a key has no value yet",
                });
            };
            return self
                .inner
                .topic
                .codec()
                .decode(
                    encoded,
                    u8::from(!self.inner.emit_xcdr1),
                    self.inner.big_endian,
                )
                .map_err(wire_error);
        }
        // An instance known by its hash alone: a key of at most sixteen
        // octets is spelled out by it (DDSI-RTPS 2.5 §9.6.4.8 Step 5.1).
        let holder = state
            .key_holder
            .filter(|holder| !holder.is_empty())
            .or_else(|| {
                let hash = self.inner.instances.key_hash_for_handle(handle)?;
                self.inner
                    .last_key
                    .holder_from_hash(self.inner.topic.codec(), hash)
            });
        let Some(bytes) = holder else {
            return Err(DdsError::BadParameter {
                what: "the key of this instance is known by its hash only, which does not spell it out",
            });
        };
        self.inner
            .topic
            .codec()
            .decode_key_holder(&bytes)
            .map_err(wire_error)
    }

    /// Helper: `handle == HANDLE_NIL` → derive from `instance`.
    /// Otherwise: check that `handle` matches the instance of `instance`.
    #[cfg(feature = "std")]
    fn resolve_handle(&self, instance: &T, handle: InstanceHandle) -> Result<InstanceHandle> {
        self.inner.topic.codec().validate_sample(instance)?;
        let derived = self.lookup_instance(instance);
        if handle.is_nil() {
            if derived.is_nil() {
                return Err(DdsError::BadParameter {
                    what: "instance not registered",
                });
            }
            return Ok(derived);
        }
        if derived != handle {
            // DDS 1.4 §2.2.2.4.2.7: a handle of another existing instance
            // is PRECONDITION_NOT_MET, one of no instance BAD_PARAMETER.
            // Both were BAD_PARAMETER.
            if self.inner.instances.get_by_handle(handle).is_some() {
                return Err(DdsError::PreconditionNotMet {
                    reason: "handle names another instance than the key",
                });
            }
            return Err(DdsError::BadParameter {
                what: "handle names no instance",
            });
        }
        Ok(handle)
    }

    /// Writes a sample with an explicit timestamp (Spec §2.2.2.4.2.16
    /// `write_w_timestamp`) and updates the instance bookkeeping.
    ///
    /// `timestamp` is not only local bookkeeping: it is the actual
    /// SOURCE_TIMESTAMP this sample is stamped with on the wire INFO_TS
    /// and the same-runtime intra-process dispatch, exactly as
    /// `DataWriter::dispose_w_timestamp`/`unregister_instance` already do
    /// for a lifecycle marker — a plain `self.write(sample)` here would
    /// have silently discarded it in favour of the configured
    /// `time_source`'s current reading.
    ///
    /// # Errors
    /// Like [`Self::write`].
    #[cfg(feature = "std")]
    pub fn write_w_timestamp(&self, sample: &T, timestamp: Time) -> Result<()> {
        self.write_with_source_ts_override(sample, Some(timestamp))
    }

    /// A write registers the instance it writes when the application did
    /// not, so `lookup_instance`, `unregister_instance` and `dispose` with
    /// `HANDLE_NIL` find it afterwards. `write` and `write_w_timestamp` both
    /// go through here; before, only `write_w_timestamp` registered.
    ///
    /// # Errors
    /// `NotAllowedBySecurity` when the AccessControl plugin refuses the
    /// registration of the instance (DDS-Security 1.2 §9.4.2.9.7); the write
    /// then does not happen.
    #[cfg(feature = "std")]
    fn register_on_write(
        &self,
        kh: crate::instance_tracker::KeyHash,
        holder: impl FnOnce() -> Vec<u8>,
        timestamp: impl FnOnce() -> Time,
    ) -> Result<()> {
        // One look at the tracker, without copying the instance's state:
        // this runs on every write of a keyed sample.
        match self.inner.instances.handle_and_kind(&kh) {
            None => {
                let holder = holder();
                self.check_instance_access(&holder, false)?;
                self.inner.instances.register(kh, holder, Some(timestamp()));
            }
            // On re-activation after Dispose / NoWriters, register brings the
            // instance back to ALIVE, bumps the generation counter and makes
            // this writer a writer of it again, as the first write does. An
            // `unregister` right after took the writer out again, and with it
            // the instance back to NOT_ALIVE_NO_WRITERS.
            Some((_, kind)) if !matches!(kind, crate::sample_info::InstanceStateKind::Alive) => {
                let holder = holder();
                self.check_instance_access(&holder, false)?;
                self.inner.instances.register(kh, holder, Some(timestamp()));
            }
            Some(_) => {}
        }
        Ok(())
    }

    /// The serialized key of the instance `handle` for the dispose check:
    /// its key holder, or its key hash when no holder is known.
    #[cfg(feature = "std")]
    fn dispose_key(
        &self,
        handle: InstanceHandle,
        state: &crate::instance_tracker::InstanceState,
    ) -> Vec<u8> {
        state.key_holder.clone().unwrap_or_else(|| {
            self.inner
                .instances
                .key_hash_for_handle(handle)
                .map(|kh| kh.to_vec())
                .unwrap_or_default()
        })
    }

    /// DDS-Security 1.2 §9.4.2.9.7 `check_local_datawriter_register_instance`
    /// and §9.4.2.9.8 `check_local_datawriter_dispose_instance` (`dispose`)
    /// for the instance with the serialized key `key`.
    #[cfg(all(feature = "std", feature = "security"))]
    fn check_instance_access(&self, key: &[u8], dispose: bool) -> Result<()> {
        match (&self.inner.runtime, self.inner.entity_id) {
            (Some(rt), Some(eid)) => rt.check_local_instance_access(eid, key, dispose),
            _ => Ok(()),
        }
    }

    #[cfg(all(feature = "std", not(feature = "security")))]
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn check_instance_access(&self, _key: &[u8], _dispose: bool) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<T: DdsType> crate::entity::Entity for DataWriter<T> {
    type Qos = DataWriterQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Spec §2.2.3 / §2.2.2.4.2: DURABILITY, RELIABILITY, HISTORY,
    /// RESOURCE_LIMITS, OWNERSHIP, LIVELINESS are Changeable=NO post-enable.
    fn set_qos(&self, qos: impl Into<crate::qos::QosArg<Self::Qos>>) -> Result<()> {
        let qos = qos
            .into()
            .resolve(|| self.get_publisher().get_default_datawriter_qos());
        qos.check_consistency()?;
        let enabled = self.inner.entity_state.is_enabled();
        if let Ok(mut current) = self.inner.qos.lock() {
            if enabled {
                if let Some(policy) = crate::qos::immutable_writer_change(&current, &qos) {
                    return Err(crate::entity::immutable_if_enabled(policy));
                }
            }
            // The changeable policies the runtime holds reach the writer
            // slot and the SEDP publication (DDS 1.4 §2.2.3); a refusal
            // leaves the QoS as it was. A writer not yet enabled is not
            // registered; its `enable` registers it with this QoS.
            if let (true, Some(rt), Some(eid)) =
                (enabled, self.inner.runtime.as_ref(), self.inner.entity_id)
            {
                let update = runtime_qos_update(&qos);
                if update != runtime_qos_update(&current) {
                    rt.update_user_writer_qos(eid, &update)?;
                }
            }
            *current = qos;
        }
        Ok(())
    }

    /// DDS 1.4 §2.2.2.1.1.7: refused while the publisher is disabled; the
    /// first call registers the writer with the runtime, which announces
    /// it (`WriterRegistration`).
    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable_entity()
    }

    fn entity_state(&self) -> Arc<crate::entity::EntityState> {
        Arc::clone(&self.inner.entity_state)
    }
}

/// What a DataWriter needs to register with the runtime at its first
/// `enable` (DDS 1.4 §2.2.2.1.1.7): until then it holds a reserved entity
/// id and nothing announces or matches it.
#[cfg(feature = "std")]
struct WriterRegistration {
    runtime: Arc<DcpsRuntime>,
    eid: EntityId,
    topic_name: String,
    type_name: String,
    type_identifier: zerodds_types::TypeIdentifier,
    extensibility: crate::dds_type::Extensibility,
    max_serialized_size: Option<usize>,
    qos: Arc<Mutex<DataWriterQos>>,
    publisher: Arc<PublisherInner>,
    topic: Arc<crate::topic::TopicInner>,
    big_endian: bool,
    backend: Option<Arc<dyn crate::durability_service::DurabilityBackend>>,
    store_refused: Arc<core::sync::atomic::AtomicBool>,
}

#[cfg(feature = "std")]
impl WriterRegistration {
    /// Registers the writer with the QoS it has now, then sets what the
    /// slot takes beside the announcement. A failure after the
    /// registration removes it again, so the next `enable` starts clean.
    fn register(&self) -> Result<()> {
        let rt = &self.runtime;
        let eid = self.eid;
        let qos = self
            .qos
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datawriter qos poisoned",
            })?
            .clone();
        // DDS 1.4 §2.2.3.6/§2.2.3.13/§2.2.3.3: PRESENTATION, PARTITION and
        // GROUP_DATA are Publisher-level policies, and the value the RxO
        // check compares travels in this writer's
        // `PublicationBuiltinTopicData`. The ZeroDDS-only per-writer
        // PRESENTATION stays an override (see `qos::effective_presentation`).
        let group_qos = self
            .publisher
            .qos
            .lock()
            .map(|q| q.clone())
            .unwrap_or_default();
        let eff_presentation =
            crate::qos::effective_presentation(group_qos.presentation, qos.presentation);
        // PARTITION and GROUP_DATA of the publisher, TOPIC_DATA of the
        // topic (DDS 1.4 §2.2.3.2): the runtime keeps them apart from the
        // writer's own, so a later `set_qos` on the publisher or the topic
        // reaches this writer.
        let topic_data = self
            .topic
            .qos
            .lock()
            .map(|q| q.topic_data.value.clone())
            .unwrap_or_default();
        let inherited = crate::runtime::GroupTopicQos {
            partition: group_qos.partition.names.clone(),
            group_data: group_qos.group_data.value.clone(),
            topic_data,
        };
        rt.register_user_writer_as(
            eid,
            crate::runtime::UserWriterConfig {
                topic_name: self.topic_name.clone(),
                type_name: self.type_name.clone(),
                reliable: qos.reliability.kind == ReliabilityKind::Reliable,
                durability: qos.durability.kind,
                durability_service: qos.durability_service,
                deadline: qos.deadline,
                latency_budget: qos.latency_budget,
                destination_order: qos.destination_order,
                lifespan: qos.lifespan,
                liveliness: qos.liveliness,
                ownership: qos.ownership.kind,
                ownership_strength: qos.ownership_strength.value,
                transport_priority: qos.transport_priority.value,
                presentation: eff_presentation,
                partition: qos.partition.names.clone(),
                user_data: qos.user_data.value.clone(),
                topic_data: qos.topic_data.value.clone(),
                group_data: qos.group_data.value.clone(),
                // F-TYPES-3: pass on the topic type identifier.
                type_identifier: self.type_identifier.clone(),
                // Per-writer DataRepresentation override from the QoS
                // (`None` = runtime default). XTypes 1.3 §7.6.3.1.2.
                data_representation_offer: crate::qos::effective_data_representation(
                    qos.data_representation.as_deref(),
                ),
                group_entity_id: self.publisher.entity_id,
                property: qos.property.clone(),
                data_tags: qos.data_tags.clone(),
                // Table 8.53 `dataMaxSizeSerialized`: the payload includes
                // the 4-octet encapsulation header.
                data_max_size_serialized: self
                    .max_serialized_size
                    .and_then(|n| i32::try_from(n.saturating_add(4)).ok()),
                entity_name: None,
            },
            inherited,
        )?;
        if let Err(error) = self.configure_slot(&qos) {
            rt.remove_user_writer(eid);
            return Err(error);
        }
        Ok(())
    }

    fn configure_slot(&self, qos: &DataWriterQos) -> Result<()> {
        let rt = &self.runtime;
        let eid = self.eid;
        // Lock the writer's DataRepresentation as immutable: a typed
        // DataWriter emits exactly the create-time representation, and its
        // body encoder + encap header must never diverge. This rejects any
        // later runtime override on this slot (the durability/routing replay
        // writers use their own unlocked slots).
        let _ = rt.lock_user_writer_representation(eid);
        // Spec §2.2.3.17 HISTORY: wire the writer's KeepLast depth onto
        // the runtime writer slot, which caps per-instance retained
        // samples for TransientLocal late joiners in this runtime and in
        // other participants. KeepAll → unbounded (usize::MAX). A
        // non-positive depth is clamped to 1 by
        // `set_user_writer_history_depth`.
        {
            use crate::qos::HistoryKind;
            let depth = match qos.history.kind {
                HistoryKind::KeepLast => qos.history.depth.max(1) as usize,
                HistoryKind::KeepAll => usize::MAX,
            };
            let _ = rt.set_user_writer_history_depth(eid, depth);
        }
        // §2.2.3.19 RESOURCE_LIMITS with RELIABILITY max_blocking_time:
        // a write that finds no room waits for an acknowledgement, then
        // TIMEOUT (§2.2.2.4.2.11).
        rt.set_user_writer_resource_limits(
            eid,
            qos.resource_limits,
            qos.reliability.max_blocking_time,
        )?;
        // XTypes 1.3 §7.6.3.1.2 / RTPS §10.5 — the user-payload encapsulation
        // header MUST declare the type's extensibility honestly: @final →
        // (PLAIN_)CDR2 0x07, @appendable → D_CDR2 0x09, @mutable → PL_CDR2
        // 0x0b. The generated `encode()` already frames the body correctly
        // (DHEADER for appendable, EMHEADER for mutable), but the writer
        // slot defaults its `wire_extensibility` to Final, so without this
        // it stamps a 0x07 header onto an appendable/mutable body. Lenient
        // readers (Cyclone/FastDDS/RTI) tolerate the mismatch; a strict
        // reader (OpenDDS `EncapsulationHeader::to_encoding`) rejects the
        // sample ("expected APPENDABLE extensibility, but got ..."). Wire the
        // type's declared extensibility through so the header matches.
        {
            use crate::dds_type::Extensibility as E;
            use zerodds_types::qos::ExtensibilityForRepr as Efr;
            let efr = match self.extensibility {
                E::Final => Efr::Final,
                E::Appendable => Efr::Appendable,
                E::Mutable => Efr::Mutable,
            };
            let _ = rt.set_user_writer_wire_extensibility(eid, efr);
        }
        // Match the encapsulation header to the BE body the writer will emit
        // (see `DataWriter::write` / `wire.big_endian`): without this
        // the slot would stamp a little-endian encap id on big-endian bytes.
        if self.big_endian {
            let _ = rt.set_user_writer_byte_order_override(eid, true);
        }
        // Spec §2.2.3.5 — with Durability=Transient/Persistent, pass the
        // writer's own backend to the runtime, which folds its stored
        // history straight into the writer's HistoryCache right here
        // (see `DcpsRuntime::attach_durability_backend`'s doc comment).
        // DDS 1.4 §2.2.3.4: against the fictitious persistence-service reader
        // of a TRANSIENT or PERSISTENT topic, whose QoS is the Topic QoS. An
        // incompatible writer changes its OFFERED_INCOMPATIBLE_QOS and does
        // not send to the service.
        let refused = self
            .topic
            .qos
            .lock()
            .ok()
            .map(|q| q.clone())
            .and_then(|topic_qos| rt.check_writer_against_fictitious_reader(eid, &topic_qos))
            .is_some();
        if refused {
            self.store_refused
                .store(true, core::sync::atomic::Ordering::Release);
        }
        if let Some(backend) = self.backend.as_ref().filter(|_| !refused) {
            let _ = rt.attach_durability_backend(eid, Arc::clone(backend));
        }
        // Spec §2.2.2.4.1.8/.9: the coherent transaction is a publisher
        // operation, the wire marker a per-writer one — so every writer of
        // this publisher is tracked, and a writer enabled inside an open
        // transaction joins it immediately.
        if let Ok(mut ids) = self.publisher.writer_entity_ids.lock() {
            ids.push(eid);
        }
        // GROUP scope (DDSI-RTPS 2.5 §8.7.5): the writer draws its group
        // sequence numbers from the publisher's group, and the writer set
        // every change carries now names it.
        if let Some(group) = self.publisher.writer_group() {
            self.publisher.update_group_writers();
            rt.set_user_writer_group(eid, Some(group))?;
        }
        if self
            .publisher
            .coherent_changes_open
            .load(core::sync::atomic::Ordering::Acquire)
        {
            let _ = rt.begin_user_writer_coherent_set(eid);
        }
        Ok(())
    }
}

/// The policies of `qos` that the runtime holds and `set_qos` may change
/// once the writer is enabled.
#[cfg(feature = "std")]
fn runtime_qos_update(qos: &DataWriterQos) -> crate::runtime::WriterQosUpdate {
    crate::runtime::WriterQosUpdate {
        deadline: qos.deadline,
        latency_budget: qos.latency_budget,
        lifespan: qos.lifespan,
        ownership_strength: qos.ownership_strength.value,
        transport_priority: qos.transport_priority.value,
        user_data: qos.user_data.value.clone(),
        own: crate::runtime::GroupTopicQos {
            partition: qos.partition.names.clone(),
            group_data: qos.group_data.value.clone(),
            topic_data: qos.topic_data.value.clone(),
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A publisher as a participant creates one with the default
    /// ENTITY_FACTORY: enabled, so what it creates is enabled too
    /// (DDS 1.4 §2.2.3.20).
    fn offline_publisher() -> Publisher {
        let p = Publisher::new(PublisherQos::default(), None);
        let _ = p.inner.entity_state.enable();
        p
    }
    use crate::dds_type::RawBytes;
    use crate::factory::DomainParticipantFactory;
    use crate::qos::{DomainParticipantQos, TopicQos};

    fn mk_topic() -> Topic<RawBytes> {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        Topic::new("Chatter".into(), TopicQos::default(), p)
    }

    #[test]
    fn durability_and_live_write_share_one_key_materialization() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct CountingKey(AtomicUsize);
        impl DdsType for CountingKey {
            const TYPE_NAME: &'static str = "test::CountingKey";
            const HAS_KEY: bool = true;
            const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(
                &self,
                out: &mut Vec<u8>,
            ) -> core::result::Result<(), crate::dds_type::EncodeError> {
                out.extend_from_slice(&7u32.to_le_bytes());
                Ok(())
            }
            fn decode(_: &[u8]) -> core::result::Result<Self, crate::dds_type::DecodeError> {
                Ok(Self(AtomicUsize::new(0)))
            }
            fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
                self.0.fetch_add(1, Ordering::Relaxed);
                holder.write_bytes(&7u32.to_be_bytes());
            }
        }
        let domain_lease = zerodds_test_domains::lease_domain();
        let participant = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .unwrap();
        let topic = participant
            .create_topic::<CountingKey>("CountedKey", TopicQos::default())
            .unwrap();
        let publisher = participant.create_publisher(PublisherQos::default());
        let qos = DataWriterQos {
            durability: zerodds_qos::DurabilityQosPolicy {
                kind: zerodds_qos::DurabilityKind::Transient,
            },
            ..DataWriterQos::default()
        };
        let writer = publisher.create_datawriter(&topic, qos).unwrap();
        let (runtime, _) = writer.runtime_handle().unwrap();
        let (_, rx) = runtime
            .register_user_reader_kind(
                crate::runtime::UserReaderConfig {
                    topic_name: topic.name().into(),
                    type_name: CountingKey::TYPE_NAME.into(),
                    ..crate::runtime::UserReaderConfig::default()
                },
                true,
            )
            .unwrap();
        let sample = CountingKey(AtomicUsize::new(0));
        writer.write(&sample).unwrap();
        assert_eq!(sample.0.load(Ordering::Relaxed), 1);
        let mut expected = [0; 16];
        expected[..4].copy_from_slice(&7u32.to_be_bytes());
        assert!(
            matches!(rx.recv_timeout(core::time::Duration::from_millis(200)).unwrap(),
            crate::runtime::UserSample::Alive { key_hash: Some(key), .. } if key == expected)
        );
        let stored = writer
            .durability_backend()
            .unwrap()
            .replay_for_topic(topic.name())
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].instance_key, expected);

        let handle = writer.register_instance(&sample).unwrap();
        sample.0.store(0, Ordering::Relaxed);
        writer.dispose(&sample, handle).unwrap();
        assert_eq!(
            sample.0.load(Ordering::Relaxed),
            1,
            "lifecycle validates once and sends the registered key, not a second encoding"
        );
        assert!(
            matches!(rx.recv_timeout(core::time::Duration::from_millis(200)).unwrap(),
            crate::runtime::UserSample::Lifecycle { key_hash, .. } if key_hash == expected)
        );
        sample.0.store(0, Ordering::Relaxed);
        writer.unregister_instance(&sample, handle).unwrap();
        assert_eq!(sample.0.load(Ordering::Relaxed), 1);
        assert!(
            matches!(rx.recv_timeout(core::time::Duration::from_millis(200)).unwrap(),
            crate::runtime::UserSample::Lifecycle { key_hash, .. } if key_hash == expected)
        );

        // The volatile offline path encodes the key once as well: the write
        // registers the instance, so lookup_instance and the lifecycle calls
        // with HANDLE_NIL find it. It skipped the key before and left the
        // instance unregistered.
        let offline = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = offline
            .create_topic::<CountingKey>("OfflineKey", TopicQos::default())
            .unwrap();
        let writer = offline
            .create_publisher(PublisherQos::default())
            .create_datawriter(&topic, DataWriterQos::default())
            .unwrap();
        sample.0.store(0, Ordering::Relaxed);
        writer.write(&sample).unwrap();
        assert_eq!(sample.0.load(Ordering::Relaxed), 1);
        assert!(!writer.lookup_instance(&sample).is_nil());
    }

    /// The last key's hash is reused only for the same holder octets: a
    /// second instance gets its own hash, and coming back to the first gives
    /// the first's again. `ShapeType`'s `string` key can exceed sixteen
    /// octets, so its hash is the MD5 of XTypes 1.3 §7.6.8 step 5.2.
    #[test]
    fn a_repeated_key_reuses_its_hash_and_another_key_gets_its_own() {
        use crate::dds_type::PlainCdr2BeKeyHolder;
        use crate::interop::ShapeType;
        let participant = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = participant
            .create_topic::<ShapeType>("CachedKey", TopicQos::default())
            .unwrap();
        let writer = participant
            .create_publisher(PublisherQos::default())
            .create_datawriter(&topic, DataWriterQos::default())
            .unwrap();
        let direct = |color: &str| {
            let mut holder = PlainCdr2BeKeyHolder::new();
            holder.write_string(color);
            crate::dds_type::compute_key_hash(holder.as_bytes(), usize::MAX)
        };
        let long_a = "A".repeat(100);
        let long_b = "B".repeat(100);
        for color in [&long_a, &long_a, &long_b, &long_a, &long_b] {
            let sample = ShapeType::new(color.as_str(), 0, 0, 0);
            let (hash, _) = writer.keyhash_and_holder(&sample).unwrap();
            assert_eq!(hash, direct(color), "{}…", &color[..1]);
        }
        assert_ne!(direct(&long_a), direct(&long_b));
    }

    #[test]
    fn lifecycle_refuses_an_unregistered_key_with_another_instances_handle() {
        use crate::interop::ShapeType;
        use crate::sample_info::InstanceStateKind;
        let participant = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = participant
            .create_topic::<ShapeType>("LifecycleKey", TopicQos::default())
            .unwrap();
        let writer = participant
            .create_publisher(PublisherQos::default())
            .create_datawriter(&topic, DataWriterQos::default())
            .unwrap();
        let registered = ShapeType::with_payload("BLUE", 1, 2, 3, vec![]);
        let unknown = ShapeType::with_payload("RED", 1, 2, 3, vec![]);
        let handle = writer.register_instance(&registered).unwrap();
        // DDS 1.4 §2.2.2.4.2.7: a handle of another existing instance is
        // PRECONDITION_NOT_MET.
        assert!(matches!(
            writer.dispose(&unknown, handle),
            Err(DdsError::PreconditionNotMet { .. })
        ));
        assert!(matches!(
            writer.unregister_instance(&unknown, handle),
            Err(DdsError::PreconditionNotMet { .. })
        ));
        let state = writer.inner.instances.get_by_handle(handle).unwrap();
        assert_eq!(state.kind, InstanceStateKind::Alive);
        assert_eq!(state.writer_count, 1);
        writer.dispose(&registered, HANDLE_NIL).unwrap();
        assert_eq!(
            writer.inner.instances.get_by_handle(handle).unwrap().kind,
            InstanceStateKind::NotAliveDisposed
        );
        writer.unregister_instance(&registered, handle).unwrap();
        assert_eq!(
            writer
                .inner
                .instances
                .get_by_handle(handle)
                .unwrap()
                .writer_count,
            0
        );
    }

    #[test]
    fn qos_emit_xcdr1_only_for_explicit_xcdr1() {
        use zerodds_rtps::publication_data::data_representation as dr;
        let mk = |rep: Option<Vec<i16>>| {
            let q = DataWriterQos {
                data_representation: rep,
                ..DataWriterQos::default()
            };
            qos_emit_xcdr1(&q)
        };
        // Only these reach `qos_emit_xcdr1` — XML/unknown offers are
        // rejected at `create_datawriter` (see
        // `create_datawriter_rejects_invalid_data_representation_offers`), so
        // there is no fallback for them here. An empty list is `[XCDR]`.
        assert!(mk(Some(vec![])), "[] is [XCDR1]");
        // Absent (None) → the first element of DEFAULT_OFFER, which is XCDR1.
        // Asserted against the constant and not against a literal: the point is
        // that an offline writer emits what the library announces by default,
        // whichever that is.
        assert_eq!(
            mk(None),
            dr::DEFAULT_OFFER[0] == dr::XCDR,
            "None → DEFAULT_OFFER[0]"
        );
        // Explicit XCDR2 → XCDR2 body.
        assert!(!mk(Some(vec![dr::XCDR2])), "[XCDR2] → XCDR2");
        assert!(!mk(Some(vec![dr::XCDR2, dr::XCDR])), "first=XCDR2 → XCDR2");
        // Explicit XCDR1 → XCDR1 body.
        assert!(mk(Some(vec![dr::XCDR])), "[XCDR1] → XCDR1");
        assert!(mk(Some(vec![dr::XCDR, dr::XCDR2])), "first=XCDR1 → XCDR1");
        // XML: the document is built from an XCDR1 body.
        assert!(mk(Some(vec![dr::XML])), "[XML] → XCDR1 body");
    }

    #[test]
    fn create_datawriter_rejects_invalid_data_representation_offers() {
        use zerodds_rtps::publication_data::data_representation as dr;
        let p = offline_publisher();
        let mk = |rep: Option<Vec<i16>>| {
            let q = DataWriterQos {
                data_representation: rep,
                ..DataWriterQos::default()
            };
            p.create_datawriter::<RawBytes>(&mk_topic(), q)
        };
        // Accepted: default (None), explicit XCDR1, explicit XCDR2.
        assert!(mk(None).is_ok(), "None → default XCDR2");
        assert!(mk(Some(vec![dr::XCDR])).is_ok(), "[XCDR1] ok");
        assert!(mk(Some(vec![dr::XCDR2])).is_ok(), "[XCDR2] ok");
        // An explicitly empty list is the single element XCDR (XTypes 1.3
        // §7.6.3.1.1), not an error.
        assert!(mk(Some(vec![])).is_ok(), "empty offer is [XCDR]");
        // XML is emitted (DDS-XTypes 1.3 §7.4.4); an unknown id is not and
        // has no silent fallback.
        assert!(mk(Some(vec![dr::XML])).is_ok(), "XML accepted");
        assert!(mk(Some(vec![99])).is_err(), "unknown id rejected");
    }

    #[test]
    fn publisher_creates_datawriter_for_matching_type() {
        let p = offline_publisher();
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        assert_eq!(w.topic().name(), "Chatter");
    }

    /// §2.2.2.3.7: the PSM defines a derived `FooDataWriter` /
    /// `FooDataReader` / `FooTypeSupport` per application type. The Rust
    /// PSM does it by monomorphisation instead of by generated classes,
    /// which is only equivalent if the type parameter actually reaches the
    /// endpoint: two writers of two types on one publisher must carry two
    /// different type names. One erased writer would carry one.
    #[test]
    fn datawriter_type_name_follows_the_topic_type() {
        use crate::interop::ShapeType;
        let participant = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let raw =
            Topic::<RawBytes>::new("TypedRaw".into(), TopicQos::default(), participant.clone());
        let shapes = Topic::<ShapeType>::new("TypedShape".into(), TopicQos::default(), participant);
        let publisher = offline_publisher();

        let w_raw = publisher
            .create_datawriter::<RawBytes>(&raw, DataWriterQos::default())
            .unwrap();
        let w_shape = publisher
            .create_datawriter::<ShapeType>(&shapes, DataWriterQos::default())
            .unwrap();

        assert_eq!(w_raw.topic().type_name(), RawBytes::TYPE_NAME);
        assert_eq!(w_shape.topic().type_name(), ShapeType::TYPE_NAME);
        assert_ne!(
            w_raw.topic().type_name(),
            w_shape.topic().type_name(),
            "two application types must not share one writer type"
        );
    }

    /// DDS-RPC 1.0 §7.8.2: the related sample identity travels in the
    /// DATA's inline QoS, which a writer without a runtime does not have.
    /// The write is refused, not sent without the identity.
    #[test]
    fn write_related_without_a_runtime_is_refused() {
        let writer = offline_publisher()
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        let related = zerodds_rtps::inline_qos::SampleIdentityBytes::new([1; 16], 3);
        let err = writer
            .write_related(&RawBytes::new(alloc::vec![1]), related)
            .unwrap_err();
        assert!(
            matches!(err, DdsError::PreconditionNotMet { .. }),
            "{err:?}"
        );
        assert_eq!(writer.samples_pending(), 0, "nothing was queued");
    }

    /// §2.2.2.4.5 Concurrency Behavior: "several DataWriter may operate in
    /// different threads. If they share the same Publisher, the middleware
    /// guarantees that its operations are thread-safe."
    ///
    /// Two writers of one publisher, written from two threads at once.
    /// Every sample has to be queued — a lost one, or a panic out of a
    /// poisoned lock, is the failure this states is impossible.
    #[test]
    fn two_writers_of_one_publisher_write_concurrently() {
        const PER_THREAD: usize = 200;
        let publisher = Arc::new(offline_publisher());
        let a = Arc::new(
            publisher
                .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
                .unwrap(),
        );
        let b = Arc::new(
            publisher
                .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
                .unwrap(),
        );

        let mut threads = Vec::new();
        for writer in [Arc::clone(&a), Arc::clone(&b)] {
            threads.push(std::thread::spawn(move || {
                for i in 0..PER_THREAD {
                    writer.write(&RawBytes::new(alloc::vec![i as u8])).unwrap();
                }
            }));
        }
        for t in threads {
            t.join().expect("a writer thread panicked");
        }

        assert_eq!(a.samples_pending(), PER_THREAD);
        assert_eq!(b.samples_pending(), PER_THREAD);
        // The publisher itself is still usable after the concurrent run.
        assert!(!publisher.is_suspended());
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
        let domain_lease = zerodds_test_domains::lease_domain();
        let participant = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .expect("live participant");
        let topic = participant
            .create_topic::<RawBytes>("KindChatter", TopicQos::default())
            .expect("topic");
        let pubr = participant.create_publisher(PublisherQos::default());
        let w = pubr
            .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
            .expect("writer");
        assert_eq!(
            w.inner
                .entity_id
                .expect("live writer has entity_id")
                .entity_kind,
            EntityKind::UserWriterNoKey,
            "keyless type must yield a NoKey writer entityid"
        );
    }

    /// QB-cluster regression: `create_datawriter` MUST wire the History
    /// keep-last DEPTH onto the runtime writer slot (DDS 1.4 §2.2.3.18).
    /// With TransientLocal + KeepLast(2), writing 5 samples of the same
    /// (keyless ⇒ single default) instance must leave only the last 2
    /// retained for late-join replay. Before the fix the slot kept the
    /// default depth (1) regardless of QoS — and the retain path would not
    /// cap at the requested 2.
    #[test]
    fn create_datawriter_wires_history_keep_last_depth() {
        use crate::qos::{DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy};
        let domain_lease = zerodds_test_domains::lease_domain();
        let participant = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .expect("live participant");
        let topic = participant
            .create_topic::<RawBytes>("HistDepth", TopicQos::default())
            .expect("topic");
        let pubr = participant.create_publisher(PublisherQos::default());
        let qos = DataWriterQos {
            durability: DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            history: HistoryQosPolicy {
                kind: HistoryKind::KeepLast,
                depth: 2,
            },
            ..DataWriterQos::default()
        };
        let w = pubr
            .create_datawriter::<RawBytes>(&topic, qos)
            .expect("writer");
        let (rt, eid) = w.runtime_handle().expect("live writer");
        for i in 0u8..5 {
            w.write(&RawBytes::new(vec![i])).expect("write");
        }
        assert_eq!(
            rt.user_writer_retained_len(eid),
            2,
            "KeepLast(2) must cap the retained set at 2 for the default instance"
        );
    }

    /// The closure goes in beside the object: `DdsType::type_object_dependencies`
    /// is registered by `create_datawriter` too, so the endpoint's own
    /// identity resolves locally and a peer's `getTypeDependencies` is
    /// answerable (XTypes 1.3 §7.3.4.6, §7.6.3.3.4). Measured on the OMG
    /// `dds-xtypes` suite before this: an inherited type whose base was
    /// never registered resolved to nothing and was wired against every
    /// type on its topic.
    #[test]
    fn create_datawriter_registers_the_type_object_closure() {
        use crate::dds_type::{DecodeError, EncodeError};
        use zerodds_types::builder::{Extensibility, TypeObjectBuilder};

        fn base() -> zerodds_types::TypeObject {
            zerodds_types::TypeObject::Minimal(zerodds_types::MinimalTypeObject::Struct(
                TypeObjectBuilder::struct_type("::closure::Base")
                    .extensibility(Extensibility::Appendable)
                    .member(
                        "a",
                        zerodds_types::TypeIdentifier::Primitive(
                            zerodds_types::PrimitiveKind::Int32,
                        ),
                        |m| m,
                    )
                    .build_minimal(),
            ))
        }

        struct Derived;
        impl DdsType for Derived {
            const TYPE_NAME: &'static str = "closure::Derived";
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(&self, _out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Derived)
            }
            fn type_object() -> Option<zerodds_types::type_object::TypeObject> {
                Some(zerodds_types::TypeObject::Minimal(
                    zerodds_types::MinimalTypeObject::Struct(
                        TypeObjectBuilder::struct_type("::closure::Derived")
                            .extensibility(Extensibility::Appendable)
                            .member(
                                "angle",
                                zerodds_types::TypeIdentifier::Primitive(
                                    zerodds_types::PrimitiveKind::Float32,
                                ),
                                |m| m,
                            )
                            .build_minimal(),
                    ),
                ))
            }
            fn type_object_dependencies() -> Vec<zerodds_types::type_object::TypeObject> {
                vec![base()]
            }
        }

        let base_hash = zerodds_types::compute_hash(&base()).expect("hash");
        let domain_lease = zerodds_test_domains::lease_domain();
        let participant = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .expect("live participant");
        let topic = participant
            .create_topic::<Derived>("ClosureTopic", TopicQos::default())
            .expect("topic");
        let pubr = participant.create_publisher(PublisherQos::default());
        let w = pubr
            .create_datawriter::<Derived>(&topic, DataWriterQos::default())
            .expect("writer");
        let (rt, _eid) = w.runtime_handle().expect("live writer");
        let server = rt.type_lookup_server.lock().expect("server lock");
        assert!(
            server.registry.get_minimal(&base_hash).is_some(),
            "the dependency object must be registered beside the type's own"
        );
    }

    /// Gap 2 (#24): `create_datawriter` auto-registers the local
    /// `TypeObject` (when `DdsType::type_object()` provides one) with the
    /// runtime's TypeLookup server — so peers can `getTypes` it and the
    /// local match sites can resolve this writer's identifier structurally.
    #[test]
    fn create_datawriter_auto_registers_type_object() {
        use crate::dds_type::{DecodeError, EncodeError};

        struct RegType;
        impl DdsType for RegType {
            const TYPE_NAME: &'static str = "gap2::RegType";
            /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(&self, _out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(RegType)
            }
            fn type_object() -> Option<zerodds_types::type_object::TypeObject> {
                use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
                Some(zerodds_types::TypeObject::Minimal(
                    zerodds_types::MinimalTypeObject::Struct(
                        TypeObjectBuilder::struct_type("::gap2::RegType")
                            .extensibility(Extensibility::Appendable)
                            .member(
                                "a",
                                zerodds_types::TypeIdentifier::Primitive(
                                    zerodds_types::PrimitiveKind::Int32,
                                ),
                                |m| m,
                            )
                            .build_minimal(),
                    ),
                ))
            }
        }

        let expected_hash = zerodds_types::compute_hash(
            &RegType::type_object().expect("RegType provides a type object"),
        )
        .expect("hash");

        let domain_lease = zerodds_test_domains::lease_domain();
        let participant = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .expect("live participant");
        let topic = participant
            .create_topic::<RegType>("Gap2RegTopic", TopicQos::default())
            .expect("topic");
        let pubr = participant.create_publisher(PublisherQos::default());
        let w = pubr
            .create_datawriter::<RegType>(&topic, DataWriterQos::default())
            .expect("writer");
        let (rt, _eid) = w.runtime_handle().expect("live writer");

        let server = rt.type_lookup_server.lock().expect("server lock");
        assert!(
            server.registry.get_minimal(&expected_hash).is_some(),
            "create_datawriter must auto-register the local TypeObject in the \
             TypeLookup registry"
        );
    }

    /// Contrast: KeepAll retains every written sample (depth = unbounded).
    #[test]
    fn create_datawriter_keep_all_retains_every_sample() {
        use crate::qos::{DurabilityKind, DurabilityQosPolicy, HistoryKind, HistoryQosPolicy};
        let domain_lease = zerodds_test_domains::lease_domain();
        let participant = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .expect("live participant");
        let topic = participant
            .create_topic::<RawBytes>("HistAll", TopicQos::default())
            .expect("topic");
        let pubr = participant.create_publisher(PublisherQos::default());
        let qos = DataWriterQos {
            durability: DurabilityQosPolicy {
                kind: DurabilityKind::TransientLocal,
            },
            history: HistoryQosPolicy {
                kind: HistoryKind::KeepAll,
                depth: 1,
            },
            ..DataWriterQos::default()
        };
        let w = pubr
            .create_datawriter::<RawBytes>(&topic, qos)
            .expect("writer");
        let (rt, eid) = w.runtime_handle().expect("live writer");
        for i in 0u8..4 {
            w.write(&RawBytes::new(vec![i])).expect("write");
        }
        assert_eq!(
            rt.user_writer_retained_len(eid),
            4,
            "KeepAll must retain every sample of the default instance"
        );
    }

    #[test]
    fn datawriter_write_queues_encoded_sample() {
        let p = offline_publisher();
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        assert_eq!(w.samples_pending(), 0);
        w.write(&RawBytes::new(vec![1, 2, 3])).unwrap();
        assert_eq!(w.samples_pending(), 1);
        let drained = w.__drain_pending();
        assert_eq!(drained, vec![vec![1u8, 2, 3]]);
    }

    /// §2.2.2.4.5 Concurrency Behavior: "several DataWriter may operate
    /// in different threads. If they share the same Publisher, the
    /// middleware guarantees that its operations are thread-safe."
    /// Two writers off one publisher, written from two threads at once;
    /// every sample of both must be there afterwards and none may cross
    /// into the other writer's queue.
    #[test]
    fn two_datawriters_on_one_publisher_write_concurrently() {
        const N: usize = 200;
        let p = offline_publisher();
        let w1 = Arc::new(
            p.create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
                .unwrap(),
        );
        let w2 = Arc::new(
            p.create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
                .unwrap(),
        );

        let (a, b) = (Arc::clone(&w1), Arc::clone(&w2));
        std::thread::scope(|s| {
            s.spawn(move || {
                for i in 0..N {
                    a.write(&RawBytes::new(alloc::vec![0xA1, i as u8])).unwrap();
                }
            });
            s.spawn(move || {
                for i in 0..N {
                    b.write(&RawBytes::new(alloc::vec![0xB2, i as u8])).unwrap();
                }
            });
        });

        let d1 = w1.__drain_pending();
        let d2 = w2.__drain_pending();
        assert_eq!(d1.len(), N, "no sample lost on writer 1");
        assert_eq!(d2.len(), N, "no sample lost on writer 2");
        assert!(d1.iter().all(|s| s[0] == 0xA1), "no crossover into w1");
        assert!(d2.iter().all(|s| s[0] == 0xB2), "no crossover into w2");
    }

    // poll_publication_matched + Listener-Slot-API.

    use core::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn datawriter_set_listener_stores_arc_and_mask() {
        struct L;
        impl crate::listener::DataWriterListener for L {}
        let p = offline_publisher();
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        assert!(w.get_listener().is_none());
        w.set_listener(Some(Arc::new(L)), crate::psm_constants::status::ANY);
        assert!(w.get_listener().is_some());
        // The mask is mirrored to the EntityState.
        assert_eq!(
            w.inner.entity_state.listener_mask(),
            crate::psm_constants::status::ANY
        );
    }

    /// Offline there is no runtime: a poll reads nothing and calls no
    /// listener, and the getters answer the default status.
    #[test]
    fn an_offline_writer_polls_nothing_and_reads_the_default_status() {
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
        let p = offline_publisher();
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        let cnt = Arc::new(Cnt(AtomicU32::new(0)));
        w.set_listener(Some(cnt.clone()), crate::psm_constants::status::ANY);
        w.poll_publication_matched();
        w.drive_listeners();
        assert_eq!(cnt.0.load(Ordering::Relaxed), 0);
        assert_eq!(
            w.get_publication_matched_status().unwrap(),
            crate::status::PublicationMatchedStatus::default()
        );
        assert_eq!(w.get_liveliness_lost_status().unwrap().total_count, 0);
    }

    // ---- §2.2.2.4.1.8 / .9 suspend/resume_publications ----

    #[test]
    fn suspend_publications_sets_flag() {
        let p = offline_publisher();
        assert!(!p.is_suspended());
        p.suspend_publications().unwrap();
        assert!(p.is_suspended());
    }

    #[test]
    fn resume_publications_clears_flag() {
        let p = offline_publisher();
        p.suspend_publications().unwrap();
        p.resume_publications().unwrap();
        assert!(!p.is_suspended());
    }

    /// Each suspend wants its own resume (§2.2.2.4.1.8: "must be matched by
    /// a corresponding call to resume_publications").
    #[test]
    fn two_suspends_want_two_resumes() {
        let p = offline_publisher();
        p.suspend_publications().unwrap();
        p.suspend_publications().unwrap();
        p.resume_publications().unwrap();
        assert!(p.is_suspended());
        p.resume_publications().unwrap();
        assert!(!p.is_suspended());
    }

    /// DDS 1.4 §2.2.2.4.1.9: "The call to resume_publications must match a
    /// previous call to suspend_publications. Otherwise the operation will
    /// return the error PRECONDITION_NOT_MET." It did nothing and returned
    /// nothing.
    #[test]
    fn resume_without_suspend_is_precondition_not_met() {
        let p = offline_publisher();
        assert!(matches!(
            p.resume_publications(),
            Err(DdsError::PreconditionNotMet { .. })
        ));
        assert!(!p.is_suspended());
    }

    // ---- §2.2.2.4.1.10 / .11 begin/end_coherent_changes ----

    /// DDS 1.4 §2.2.2.4.1.10: "These calls can be nested. In that case, the
    /// coherent set terminates only with the last call to
    /// end_coherent_changes." The inner end used to close the set, and the
    /// outer one returned PRECONDITION_NOT_MET.
    #[test]
    fn nested_coherent_changes_end_with_the_last_end() {
        let p = offline_publisher();
        p.begin_coherent_changes().unwrap();
        p.begin_coherent_changes().unwrap();
        p.end_coherent_changes().unwrap();
        assert!(p.coherent_changes_open(), "the inner end closed the set");
        p.end_coherent_changes().unwrap();
        assert!(!p.coherent_changes_open());
        assert!(matches!(
            p.end_coherent_changes(),
            Err(DdsError::PreconditionNotMet { .. })
        ));
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
        let p = offline_publisher();
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
        let w_clone_q = w.inner.queue.clone();
        let w_clone_signal = w.inner.drain_signal.clone();
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
        let p = offline_publisher();
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
        let p = offline_publisher();
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
        let p = offline_publisher();
        let w = p
            .create_datawriter::<RawBytes>(&mk_topic(), DataWriterQos::default())
            .unwrap();
        let s = RawBytes::new(b"x".to_vec());
        for _ in 0..50 {
            w.write(&s).unwrap();
        }
        assert_eq!(w.samples_pending(), 50);
    }

    /// Every policy TopicQos and DataWriterQos both carry is copied, not
    /// only the two the operation used to copy.
    #[test]
    fn copy_from_topic_qos_copies_every_policy_both_carry() {
        use crate::qos::TopicQos;
        let mut topic = TopicQos::default();
        topic.deadline.period = zerodds_qos::Duration::from_millis(250);
        topic.latency_budget.duration = zerodds_qos::Duration::from_millis(5);
        topic.lifespan.duration = zerodds_qos::Duration::from_millis(900);
        topic.transport_priority.value = 7;
        topic.history.depth = 9;
        topic.resource_limits.max_samples = 40;
        topic.durability_service.history_depth = 3;
        topic.liveliness.kind = crate::qos::LivelinessKind::ManualByTopic;
        topic.destination_order.kind = crate::qos::DestinationOrderKind::BySourceTimestamp;
        topic.ownership.kind = crate::qos::OwnershipKind::Exclusive;
        let mut dw = DataWriterQos::default();
        Publisher::copy_from_topic_qos(&mut dw, &topic).unwrap();
        assert_eq!(dw.deadline, topic.deadline);
        assert_eq!(dw.latency_budget, topic.latency_budget);
        assert_eq!(dw.lifespan, topic.lifespan);
        assert_eq!(dw.transport_priority, topic.transport_priority);
        assert_eq!(dw.history, topic.history);
        assert_eq!(dw.resource_limits, topic.resource_limits);
        assert_eq!(dw.durability_service, topic.durability_service);
        assert_eq!(dw.liveliness, topic.liveliness);
        assert_eq!(dw.destination_order, topic.destination_order);
        assert_eq!(dw.ownership, topic.ownership);
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
