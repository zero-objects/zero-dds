// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Subscriber + DataReader — the receive end of the DCPS API.
//!
//! Spec reference: OMG DDS 1.4 §2.2.2.5 `Subscriber`, §2.2.2.5.2
//! `DataReader`.
//!
//! # Scope v1.2
//!
//! - `Subscriber::create_datareader<T>(topic, qos)` → `DataReader<T>`.
//! - `DataReader::take()` removes all cached samples.
//! - `DataReader::read()` peeks without removing (offline: identical to
//!   take, no state change — spec §2.2.2.5.3.4 sample-state is
//!   implemented in live mode).
//! - Listener / WaitSet: live mode.

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::sync::Mutex;
#[cfg(feature = "std")]
use std::sync::mpsc;

use crate::dds_type::DdsType;
use crate::entity::StatusMask;
use crate::error::{DdsError, Result};
#[cfg(feature = "std")]
use crate::instance_handle::{HANDLE_NIL, InstanceHandle};
#[cfg(feature = "std")]
use crate::instance_tracker::InstanceTracker;
use crate::listener::{ArcDataReaderListener, ArcSubscriberListener};
use crate::qos::{DataReaderQos, SubscriberQos};
#[cfg(feature = "std")]
use crate::sample::Sample;
#[cfg(feature = "std")]
use crate::sample_info::{
    InstanceStateKind, SampleInfo, SampleStateKind, ViewStateKind, instance_state_mask,
    sample_state_mask, view_state_mask,
};
#[cfg(feature = "std")]
use crate::time::{Time, get_current_time};
use crate::topic::Topic;

#[cfg(feature = "std")]
use crate::runtime::DcpsRuntime;
#[cfg(feature = "std")]
use zerodds_qos::ReliabilityKind;
#[cfg(feature = "std")]
use zerodds_rtps::wire_types::EntityId;

/// Decodes a received sample body with the encoder that matches its
/// **encapsulation** — both the XCDR version (`representation`: `0` = XCDR1 /
/// classic CDR, `1` = XCDR2) and the byte order (`big_endian`), as extracted
/// from the RTPS encapsulation header (RTPS 2.5 §10.5) when the sample was
/// staged (`UserSample::Alive`).
///
/// This is essential for cross-vendor interop: CycloneDDS (and legacy RTI /
/// OpenDDS) default their writers to **XCDR1** for `@final` types, so a ZeroDDS
/// reader must decode XCDR1 with the classic-CDR alignment rule (max-align 8, no
/// DHEADER) rather than the XCDR2 rule (max-align 4) — otherwise every body with
/// an 8-byte member or an `@appendable` DHEADER would be mis-aligned. The
/// per-representation framing lives in `DdsType::{decode, decode_be,
/// decode_xcdr1, decode_xcdr1_be}` (XTypes 1.3 §7.4.3.4.2).
#[inline]
fn decode_for_encap<T: DdsType>(
    bytes: &[u8],
    representation: u8,
    big_endian: bool,
) -> core::result::Result<T, crate::dds_type::DecodeError> {
    match (representation, big_endian) {
        (0, false) => T::decode_xcdr1(bytes),
        (0, true) => T::decode_xcdr1_be(bytes),
        (_, false) => T::decode(bytes),
        (_, true) => T::decode_be(bytes),
    }
}

/// Builds a diagnostic `WireError` for a failed sample decode. Beyond the inner
/// error it records the received encapsulation (representation + byte order) and
/// this reader type's own extensibility, and — because the most common
/// cross-vendor cause is an extensibility/framing mismatch — names that as a
/// *plausible* cause. It is deliberately not asserted: without the remote type
/// the true cause cannot be confirmed here. See issue #27.
fn decode_wire_error<T: DdsType>(
    inner: &crate::dds_type::DecodeError,
    representation: u8,
    big_endian: bool,
) -> DdsError {
    use crate::dds_type::Extensibility;
    let repr = if representation == 0 {
        "XCDR1"
    } else {
        "XCDR2"
    };
    let endian = if big_endian {
        "big-endian"
    } else {
        "little-endian"
    };
    let ext = match T::EXTENSIBILITY {
        Extensibility::Final => "final",
        Extensibility::Appendable => "appendable",
        Extensibility::Mutable => "mutable",
    };
    DdsError::WireError {
        message: alloc::format!(
            "decode error: {inner} (received {repr} {endian}; this reader's type '{}' is @{ext}. \
             A plausible cross-vendor cause is an extensibility mismatch — in XCDR2 \
             @appendable/@mutable carry a DHEADER length prefix and @final does not, so a peer \
             whose type has a different extensibility fails to decode. Not confirmed: the remote \
             type is not available here to verify.)",
            T::TYPE_NAME,
        ),
    }
}

/// Subscriber — entity group for DataReaders.
#[derive(Debug)]
pub struct Subscriber {
    pub(crate) inner: Arc<SubscriberInner>,
}

pub(crate) struct SubscriberInner {
    #[cfg(feature = "std")]
    pub(crate) qos: std::sync::Mutex<SubscriberQos>,
    #[cfg(not(feature = "std"))]
    #[allow(dead_code)]
    pub(crate) qos: SubscriberQos,
    pub(crate) entity_state: alloc::sync::Arc<crate::entity::EntityState>,
    #[cfg(feature = "std")]
    pub(crate) runtime: Option<Arc<DcpsRuntime>>,
    /// Optional `SubscriberListener` + StatusMask.
    /// Bubble-up target for reader events.
    #[cfg(feature = "std")]
    pub(crate) listener: std::sync::Mutex<Option<(ArcSubscriberListener, StatusMask)>>,
    /// Weak back-pointer to the participant (bubble-up, cycle avoidance
    /// via Weak).
    #[cfg(feature = "std")]
    pub(crate) participant:
        std::sync::Mutex<Option<alloc::sync::Weak<crate::participant::ParticipantInner>>>,
    /// Group access scope for §2.2.2.5.2.8/.9 begin/end_access.
    /// Counter-based (recursively nestable per spec).
    pub(crate) access_scope: Arc<crate::coherent_set::GroupAccessScope>,
    /// DataReader handles (tracked per `create_datareader`) for recursive
    /// `DomainParticipant::contains_entity` (spec §2.2.2.2.1.10).
    #[cfg(feature = "std")]
    pub(crate) datareaders:
        std::sync::Mutex<alloc::vec::Vec<crate::instance_handle::InstanceHandle>>,
}

impl core::fmt::Debug for SubscriberInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let listener_present = self.listener.lock().map(|s| s.is_some()).unwrap_or(false);
        f.debug_struct("SubscriberInner")
            .field("entity_state", &self.entity_state)
            .field("listener_present", &listener_present)
            .finish_non_exhaustive()
    }
}

impl Subscriber {
    #[cfg(feature = "std")]
    pub(crate) fn new(qos: SubscriberQos, runtime: Option<Arc<DcpsRuntime>>) -> Self {
        Self {
            inner: Arc::new(SubscriberInner {
                qos: std::sync::Mutex::new(qos),
                entity_state: crate::entity::EntityState::new(),
                runtime,
                listener: std::sync::Mutex::new(None),
                participant: std::sync::Mutex::new(None),
                access_scope: crate::coherent_set::GroupAccessScope::new(),
                datareaders: std::sync::Mutex::new(alloc::vec::Vec::new()),
            }),
        }
    }

    /// Spec §2.2.2.2.1.10 — `true` if `handle` is a DataReader created
    /// via this Subscriber.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn contains_reader(&self, handle: crate::instance_handle::InstanceHandle) -> bool {
        self.inner
            .datareaders
            .lock()
            .map(|v| v.contains(&handle))
            .unwrap_or(false)
    }

    #[cfg(feature = "std")]
    fn track_reader(&self, handle: crate::instance_handle::InstanceHandle) {
        if let Ok(mut list) = self.inner.datareaders.lock() {
            list.push(handle);
        }
        // Propagate to the participant for recursive contains_entity.
        if let Ok(slot) = self.inner.participant.lock() {
            if let Some(weak) = slot.as_ref() {
                if let Some(p_inner) = weak.upgrade() {
                    if let Ok(mut drs) = p_inner.datareaders.lock() {
                        drs.push(handle);
                    }
                }
            }
        }
    }
    #[cfg(not(feature = "std"))]
    pub(crate) fn new(qos: SubscriberQos) -> Self {
        Self {
            inner: Arc::new(SubscriberInner {
                qos,
                entity_state: crate::entity::EntityState::new(),
                access_scope: crate::coherent_set::GroupAccessScope::new(),
            }),
        }
    }

    /// Spec §2.2.2.5.2.8 `begin_access` — marks the start of a coherent
    /// read set. Nesting is allowed; each call increments an internal
    /// counter, each `end_access` decrements it.
    pub fn begin_access(&self) {
        self.inner.access_scope.begin();
    }

    /// Spec §2.2.2.5.2.9 `end_access` — counterpart to `begin_access`.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` if `end_access` is called without a
    /// preceding `begin_access`.
    pub fn end_access(&self) -> Result<()> {
        self.inner.access_scope.end()
    }

    /// `true` if a group access is currently open.
    #[must_use]
    pub fn is_access_open(&self) -> bool {
        self.inner.access_scope.is_active()
    }

    /// Sets the `SubscriberListener` + StatusMask. `None` clears the
    /// slot. Spec §2.2.2.5.6.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcSubscriberListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
    }

    /// Current listener clone.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcSubscriberListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Sets the weak back-pointer to the participant.
    #[cfg(feature = "std")]
    pub(crate) fn attach_participant(
        &self,
        participant: alloc::sync::Weak<crate::participant::ParticipantInner>,
    ) {
        if let Ok(mut slot) = self.inner.participant.lock() {
            *slot = Some(participant);
        }
    }

    /// Snapshot of the reader bubble-up chain: the given
    /// `reader_listener` tuple + subscriber stage + participant stage.
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn snapshot_reader_chain(
        &self,
        reader_listener: Option<(ArcDataReaderListener, StatusMask)>,
    ) -> crate::listener_dispatch::ReaderListenerChain {
        let subscriber = self
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
        crate::listener_dispatch::ReaderListenerChain {
            reader: reader_listener,
            subscriber,
            participant,
        }
    }

    /// Creates a typed `DataReader<T>`.
    ///
    /// # Errors
    /// `BadParameter` on a type-name mismatch.
    pub fn create_datareader<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: DataReaderQos,
    ) -> Result<DataReader<T>> {
        if topic.type_name() != T::TYPE_NAME {
            return Err(DdsError::BadParameter {
                what: "topic.type_name mismatch",
            });
        }
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            let reliable = qos.reliability.kind == ReliabilityKind::Reliable;
            // Derive entityKind from the type's keyedness (spec §9.3.1.2:
            // 0x04=NoKey / 0x07=WithKey). A keyless type (`HAS_KEY=false`)
            // MUST produce a NoKey reader; otherwise cross-vendor writers
            // (CycloneDDS/ROS 2) reject the endpoint match due to an
            // entityKind mismatch (keyed vs no-key) — silently, with no log.
            let (eid, rx) = rt.register_user_reader_kind(
                crate::runtime::UserReaderConfig {
                    topic_name: topic.name().into(),
                    type_name: T::TYPE_NAME.into(),
                    reliable,
                    durability: qos.durability.kind,
                    deadline: qos.deadline,
                    latency_budget: qos.latency_budget,
                    destination_order: qos.destination_order,
                    liveliness: qos.liveliness,
                    ownership: qos.ownership.kind,
                    presentation: qos.presentation,
                    partition: qos.partition.names.clone(),
                    user_data: qos.user_data.value.clone(),
                    topic_data: qos.topic_data.value.clone(),
                    group_data: qos.group_data.value.clone(),
                    // F-TYPES-3: pass through the topic type identifier + TCE QoS.
                    type_identifier: T::TYPE_IDENTIFIER.clone(),
                    type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                    // Per-reader DataRepresentation override from the QoS
                    // (`None` = runtime default). XTypes 1.3 §7.6.3.1.2.
                    data_representation_offer: qos.data_representation.clone(),
                },
                T::HAS_KEY,
            )?;
            // Gap 2 (#24): auto-register the local TypeObject (if codegen/impl
            // provides one) so the runtime's TypeLookup server can answer a
            // peer's getTypes and the local match sites can resolve this
            // reader's TYPE_IDENTIFIER structurally against the registry.
            // Default `type_object()` is `None` → no-op; object emitter is a
            // follow-up.
            if let Some(obj) = T::type_object() {
                let _ = rt.register_type_object(obj);
            }
            let dr = DataReader::new_live(
                topic.clone(),
                qos,
                self.inner.clone(),
                Arc::clone(rt),
                eid,
                rx,
            );
            self.track_reader(dr.entity_state.instance_handle());
            return Ok(dr);
        }
        let dr = DataReader::new_offline(topic.clone(), qos, self.inner.clone());
        #[cfg(feature = "std")]
        self.track_reader(dr.entity_state.instance_handle());
        Ok(dr)
    }
}

// ============================================================================
// Entity-Trait (DCPS §2.2.2.1) —
// ============================================================================

#[cfg(feature = "std")]
impl crate::entity::Entity for Subscriber {
    type Qos = SubscriberQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    fn set_qos(&self, qos: Self::Qos) -> Result<()> {
        // SubscriberQos: Partition / GroupData / Presentation are all
        // Changeable=YES per spec §2.2.3 — no immutable check needed.
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

/// Typed DataReader — removes samples that the RTPS reader has received
/// for the topic.
///
/// Live mode: `rx: Some` delivers samples from the runtime mpsc.
/// Offline mode: in-memory `inbox` for unit tests.
pub struct DataReader<T: DdsType> {
    topic: Topic<T>,
    qos: Mutex<DataReaderQos>,
    /// Entity lifecycle (DCPS §2.2.2.1).
    entity_state: Arc<crate::entity::EntityState>,
    /// Parent subscriber — for bubble-up to the subscriber and
    /// participant listeners.
    subscriber: Arc<SubscriberInner>,
    /// Optional `DataReaderListener` + StatusMask.
    #[cfg(feature = "std")]
    listener: Mutex<Option<(ArcDataReaderListener, StatusMask)>>,
    /// Last seen number of matched writers (for delta detection in
    /// poll_subscription_matched).
    #[cfg(feature = "std")]
    last_match_count: std::sync::atomic::AtomicI64,
    /// Last seen requested_deadline_missed counter.
    #[cfg(feature = "std")]
    last_requested_deadline_missed: std::sync::atomic::AtomicU64,
    /// Last seen (alive_count, not_alive_count).
    #[cfg(feature = "std")]
    last_liveliness_alive: std::sync::atomic::AtomicI64,
    /// Last seen not_alive counter.
    #[cfg(feature = "std")]
    last_liveliness_not_alive: std::sync::atomic::AtomicI64,
    /// Last seen requested_incompatible_qos.total_count.
    #[cfg(feature = "std")]
    last_requested_incompatible_qos: std::sync::atomic::AtomicI64,
    /// Last seen sample_lost counter.
    #[cfg(feature = "std")]
    last_sample_lost: std::sync::atomic::AtomicU64,
    /// Last seen sample_rejected.total_count.
    #[cfg(feature = "std")]
    last_sample_rejected: std::sync::atomic::AtomicI64,
    /// Offline fallback inbox. Stores full [`UserSample`] values
    /// (including writer_guid + writer_strength for Alive) so that
    /// `take()`/`read()` can apply the exclusive-ownership filter in a
    /// spec-compliant way.
    inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    runtime: Option<Arc<DcpsRuntime>>,
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    entity_id: Option<EntityId>,
    /// Runtime channel for incoming samples (live mode).
    #[cfg(feature = "std")]
    rx: Option<Mutex<mpsc::Receiver<crate::runtime::UserSample>>>,
    /// Optional content-filter closure. Applied to each sample in
    /// `take()` after decoding; returns `true` → sample is delivered,
    /// `false` → discarded.
    ///
    /// Spec reference: OMG DDS 1.4 §2.2.2.5.4 `ContentFilteredTopic`.
    /// This Rust closure variant is more idiomatic than the spec's SQL
    /// expression syntax and is sufficient for all in-process use cases.
    /// SQL parser + cross-vendor SEDP propagation follow later.
    #[allow(clippy::type_complexity)]
    filter: Option<Arc<dyn Fn(&T) -> bool + Send + Sync>>,
    /// Instance bookkeeping (spec §2.2.2.5.1).
    #[cfg(feature = "std")]
    instances: InstanceTracker,
    /// Sample cache with resolved [`SampleInfo`]. The cache is filled on
    /// arrival via `ingest_bytes`; `take`/`read`/`take_with_info`/
    /// `read_with_info` read from it.
    #[cfg(feature = "std")]
    cache: Arc<Mutex<Vec<CachedSample>>>,
    /// Optionally configured Flatdata SlotBackend for the same-host
    /// zero-copy read path (`zerodds-flatdata-1.0` §4.1 + §9.1). Set via
    /// `set_flat_backend`; `read_flat()` falls back to classic `take()`
    /// when `None`.
    #[cfg(all(feature = "std", feature = "flatdata-integration"))]
    #[allow(clippy::type_complexity)]
    pub(crate) flat_backend: Mutex<
        Option<(
            Arc<dyn zerodds_flatdata::SlotBackend>,
            u8, // reader_index (0..31)
            std::sync::atomic::AtomicU32,
        )>,
    >,
    _t: PhantomData<fn() -> T>,
}

/// Internal: a decoded sample in the reader cache.
///
/// We carry the bytes (instead of `T`) so the reader cache is not bound
/// to `T: Clone` and so `T::decode` can happen lazily. Lifecycle markers
/// (dispose/unregister) have `bytes == None`.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct CachedSample {
    /// Zero-copy container: SampleBytes holds an Arc<[u8]> slice onto the
    /// RTPS wire datagram. None for lifecycle markers (spec §2.2.2.5.1.13).
    pub bytes: Option<crate::sample_bytes::SampleBytes>,
    pub info: SampleInfo,
}

impl<T: DdsType> core::fmt::Debug for DataReader<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DataReader")
            .field("topic", &self.topic.name())
            .field("type", &T::TYPE_NAME)
            .field("qos", &self.qos)
            .finish_non_exhaustive()
    }
}

/// One drained inbox entry awaiting decode in `ingest_into_cache`:
/// `(bytes, writer_guid, writer_strength, source_timestamp, key_only, flags)`.
/// `bytes` is the refcounted zero-copy `SampleBytes` (Wave 2.1).
#[cfg(feature = "std")]
type RawIngestSample = (
    crate::sample_bytes::SampleBytes,
    [u8; 16],
    i32,
    Option<zerodds_rtps::header_extension::HeTimestamp>,
    bool,
    u8,
);

/// RAII teardown (Spec §2.2.2.5.1.2 — deleting a `DataReader`). Dropping the
/// user's handle deregisters the reader from the runtime: it removes the slot,
/// rebuilds the intra-runtime route, and sends an SEDP dispose so remote peers
/// drop the matched reader at once. Offline readers (no runtime) are no-ops.
#[cfg(feature = "std")]
impl<T: DdsType> Drop for DataReader<T> {
    fn drop(&mut self) {
        if let (Some(rt), Some(eid)) = (self.runtime.as_ref(), self.entity_id) {
            rt.remove_user_reader(eid);
        }
    }
}

impl<T: DdsType> DataReader<T> {
    #[cfg(feature = "std")]
    fn new_offline(topic: Topic<T>, qos: DataReaderQos, subscriber: Arc<SubscriberInner>) -> Self {
        Self {
            topic,
            qos: Mutex::new(qos),
            entity_state: crate::entity::EntityState::new(),
            subscriber,
            listener: Mutex::new(None),
            last_match_count: std::sync::atomic::AtomicI64::new(-1),
            last_requested_deadline_missed: std::sync::atomic::AtomicU64::new(0),
            last_liveliness_alive: std::sync::atomic::AtomicI64::new(-1),
            last_liveliness_not_alive: std::sync::atomic::AtomicI64::new(-1),
            last_requested_incompatible_qos: std::sync::atomic::AtomicI64::new(-1),
            last_sample_lost: std::sync::atomic::AtomicU64::new(0),
            last_sample_rejected: std::sync::atomic::AtomicI64::new(-1),
            inbox: Arc::new(Mutex::new(Vec::new())),
            runtime: None,
            entity_id: None,
            rx: None,
            filter: None,
            instances: InstanceTracker::new(),
            cache: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            _t: PhantomData,
        }
    }

    #[cfg(feature = "std")]
    fn new_live(
        topic: Topic<T>,
        qos: DataReaderQos,
        subscriber: Arc<SubscriberInner>,
        runtime: Arc<DcpsRuntime>,
        entity_id: EntityId,
        rx: mpsc::Receiver<crate::runtime::UserSample>,
    ) -> Self {
        Self {
            topic,
            qos: Mutex::new(qos),
            entity_state: crate::entity::EntityState::new(),
            subscriber,
            listener: Mutex::new(None),
            last_match_count: std::sync::atomic::AtomicI64::new(-1),
            last_requested_deadline_missed: std::sync::atomic::AtomicU64::new(0),
            last_liveliness_alive: std::sync::atomic::AtomicI64::new(-1),
            last_liveliness_not_alive: std::sync::atomic::AtomicI64::new(-1),
            last_requested_incompatible_qos: std::sync::atomic::AtomicI64::new(-1),
            last_sample_lost: std::sync::atomic::AtomicU64::new(0),
            last_sample_rejected: std::sync::atomic::AtomicI64::new(-1),
            inbox: Arc::new(Mutex::new(Vec::new())),
            runtime: Some(runtime),
            entity_id: Some(entity_id),
            rx: Some(Mutex::new(rx)),
            filter: None,
            instances: InstanceTracker::new(),
            cache: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            _t: PhantomData,
        }
    }

    #[cfg(not(feature = "std"))]
    fn new(topic: Topic<T>, qos: DataReaderQos, subscriber: Arc<SubscriberInner>) -> Self {
        Self {
            topic,
            qos,
            subscriber,
            inbox: Arc::new(Mutex::new(Vec::new())),
            filter: None,
            _t: PhantomData,
        }
    }

    /// Constructor for builtin-topic readers.
    ///
    /// Unlike `new_offline`, this reader shares the inbox with the
    /// `DcpsRuntime` discovery hook: SPDP/SEDP receive pushes an encoded
    /// sample through the same `Arc<Mutex<Vec<crate::runtime::UserSample>>>`,
    /// which is read here via `take()`/`read()`.
    #[cfg(feature = "std")]
    pub(crate) fn new_builtin(
        topic: Topic<T>,
        qos: DataReaderQos,
        subscriber: Arc<SubscriberInner>,
        inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    ) -> Self {
        Self {
            topic,
            qos: Mutex::new(qos),
            entity_state: crate::entity::EntityState::new(),
            subscriber,
            listener: Mutex::new(None),
            last_match_count: std::sync::atomic::AtomicI64::new(-1),
            last_requested_deadline_missed: std::sync::atomic::AtomicU64::new(0),
            last_liveliness_alive: std::sync::atomic::AtomicI64::new(-1),
            last_liveliness_not_alive: std::sync::atomic::AtomicI64::new(-1),
            last_requested_incompatible_qos: std::sync::atomic::AtomicI64::new(-1),
            last_sample_lost: std::sync::atomic::AtomicU64::new(0),
            last_sample_rejected: std::sync::atomic::AtomicI64::new(-1),
            inbox,
            runtime: None,
            entity_id: None,
            rx: None,
            filter: None,
            instances: InstanceTracker::new(),
            cache: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            _t: PhantomData,
        }
    }

    /// Sets a content filter that is evaluated on every sample in the
    /// `take()` path. Returning `false` discards the sample.
    ///
    /// Builder style: `reader.with_filter(|s| s.value > 0)`.
    ///
    /// .7a — SQL expression syntax via `set_filter_expression` follows
    /// later.
    #[must_use]
    pub fn with_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        self.filter = Some(Arc::new(filter));
        self
    }

    /// The topic being read from.
    #[must_use]
    pub fn topic(&self) -> &Topic<T> {
        &self.topic
    }

    /// Spec §2.2.2.5.3.6 / §2.2.2.1.1 — `InstanceHandle` of this
    /// DataReader. A stable identity for
    /// `DomainParticipant::contains_entity`.
    #[must_use]
    pub fn subscription_handle(&self) -> crate::instance_handle::InstanceHandle {
        self.entity_state.instance_handle()
    }

    /// Sets the `DataReaderListener` + StatusMask. `None` clears the
    /// slot. Spec §2.2.2.5.7.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcDataReaderListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.entity_state.set_listener_mask(mask);
    }

    /// Current listener clone, if any.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDataReaderListener> {
        self.listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Snapshot of the bubble-up chain (Reader → Subscriber → Participant)
    /// for hot-path listener dispatch.
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn listener_chain(&self) -> crate::listener_dispatch::ReaderListenerChain {
        let reader = self
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
        let sub_handle = Subscriber {
            inner: Arc::clone(&self.subscriber),
        };
        sub_handle.snapshot_reader_chain(reader)
    }

    /// Current QoS (cloned, .1).
    #[must_use]
    pub fn qos(&self) -> DataReaderQos {
        self.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Takes all cached samples and removes them from the inbox. Returns
    /// an empty Vec if there is nothing.
    ///
    /// # Errors
    /// - `WireError` if a stored payload can no longer be decoded
    ///   (type-eval mismatch).
    pub fn take(&self) -> Result<Vec<T>> {
        // Spec §2.2.3.22 ReaderDataLifecycle.autopurge — on every read/take,
        // check whether expired instances must be removed from the tracker.
        #[cfg(feature = "std")]
        {
            let now = get_current_time();
            let mut empty: Vec<CachedSample> = Vec::new();
            self.run_reader_autopurge(now, &mut empty);
        }
        // Cross-vendor same-host zero-copy: if this reader was
        // `enable_cyclone_iox`-ed, return Cyclone-published samples drained from
        // iceoryx (classic-CDR-decoded) when any are queued.
        #[cfg(all(feature = "std", feature = "cyclone-iox", target_os = "linux"))]
        {
            let iox = crate::cyclone_iox_integration::take::<T>(self.topic.name());
            if !iox.is_empty() {
                return Ok(iox);
            }
        }
        // Live mode: first drain the staging inbox (filled by
        // wait_for_data), then pull all not-yet-polled samples from mpsc.
        #[cfg(feature = "std")]
        if let Some(rx_mu) = self.rx.as_ref() {
            let mut out = Vec::new();
            // Read TimeBasedFilter (spec §2.2.3.13) min_separation from QoS
            // so live mode applies the same filtering as ingest_into_cache.
            let min_sep_nanos = {
                let qos = self.qos.lock().unwrap_or_else(|e| e.into_inner());
                qos.time_based_filter.minimum_separation.to_nanos()
            };
            let staged = {
                let mut inbox = self
                    .inbox
                    .lock()
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "datareader inbox poisoned",
                    })?;
                core::mem::take(&mut *inbox)
            };
            for staged_item in staged {
                match staged_item {
                    crate::runtime::UserSample::Alive {
                        payload: bytes,
                        writer_guid,
                        writer_strength,
                        representation,
                        big_endian,
                        ..
                    } => {
                        let sample = decode_for_encap::<T>(&bytes, representation, big_endian)
                            .map_err(|e| decode_wire_error::<T>(&e, representation, big_endian))?;
                        if !self.sample_passes_filter(&sample) {
                            continue;
                        }
                        if !self.live_mode_time_based_filter_pass(&sample, min_sep_nanos) {
                            continue;
                        }
                        // §2.2.3.23 exclusive-ownership filter.
                        if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                            continue;
                        }
                        out.push(sample);
                    }
                    crate::runtime::UserSample::Lifecycle { .. } => {
                        // Lifecycle in the staging inbox: in the
                        // live-mode take() loop it is handled
                        // immediately below via __push_lifecycle — just
                        // skip it here; it comes around next round.
                    }
                }
            }
            let rx = rx_mu.lock().map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader rx poisoned",
            })?;
            while let Ok(item) = rx.try_recv() {
                match item {
                    crate::runtime::UserSample::Alive {
                        payload: bytes,
                        writer_guid,
                        writer_strength,
                        representation,
                        big_endian,
                        ..
                    } => {
                        let sample = decode_for_encap::<T>(&bytes, representation, big_endian)
                            .map_err(|e| decode_wire_error::<T>(&e, representation, big_endian))?;
                        if !self.sample_passes_filter(&sample) {
                            continue;
                        }
                        if !self.live_mode_time_based_filter_pass(&sample, min_sep_nanos) {
                            continue;
                        }
                        // §2.2.3.23 exclusive-ownership filter.
                        if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                            continue;
                        }
                        out.push(sample);
                    }
                    crate::runtime::UserSample::Lifecycle { key_hash, kind } => {
                        // Feed lifecycle markers into the tracker via
                        // __push_lifecycle (spec §8.2.1.2).
                        let mut holder_bytes = Vec::with_capacity(16);
                        holder_bytes.extend_from_slice(&key_hash);
                        let lc_kind = match kind {
                            zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed
                            | zerodds_rtps::history_cache::ChangeKind::NotAliveDisposedUnregistered => {
                                crate::sample_info::InstanceStateKind::NotAliveDisposed
                            }
                            zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered => {
                                crate::sample_info::InstanceStateKind::NotAliveNoWriters
                            }
                            _ => crate::sample_info::InstanceStateKind::Alive,
                        };
                        let _ = self.__push_lifecycle(key_hash, holder_bytes, lc_kind);
                    }
                }
            }
            return Ok(out);
        }
        // Offline fallback.
        let raw = {
            let mut inbox = self
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            core::mem::take(&mut *inbox)
        };
        let mut out = Vec::with_capacity(raw.len());
        for staged_item in raw {
            let crate::runtime::UserSample::Alive {
                payload: bytes,
                writer_guid,
                writer_strength,
                representation,
                big_endian,
                ..
            } = staged_item
            else {
                continue;
            };
            let sample = decode_for_encap::<T>(&bytes, representation, big_endian)
                .map_err(|e| decode_wire_error::<T>(&e, representation, big_endian))?;
            if !self.sample_passes_filter(&sample) {
                continue;
            }
            // §2.2.3.23 exclusive-ownership filter (also in the offline
            // fallback). The builtin-inject path uses writer_guid=[0;16]
            // with a shared-ownership default; passes_exclusive_ownership
            // then always returns `true`.
            if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                continue;
            }
            out.push(sample);
        }
        Ok(out)
    }

    /// Helper — evaluates the content filter if set.
    fn sample_passes_filter(&self, sample: &T) -> bool {
        match &self.filter {
            Some(f) => f(sample),
            None => true,
        }
    }

    /// Spec §2.2.3.23 / §2.2.2.5.5 — exclusive-ownership filter.
    ///
    /// Returns `true` if the sample may be delivered:
    /// - Reader ownership QoS = Shared → always `true` (no filter).
    /// - Keyless topic → always `true` (no per-instance owner state).
    /// - Otherwise: computes the KeyHash and consults
    ///   [`instance_tracker::InstanceTracker::should_accept_sample_under_exclusive_ownership`],
    ///   which holds the (writer_guid, writer_strength) of the
    ///   currently-winning source per instance and rejects samples from
    ///   weaker writers.
    #[cfg(feature = "std")]
    fn passes_exclusive_ownership(
        &self,
        sample: &T,
        writer_guid: [u8; 16],
        writer_strength: i32,
    ) -> bool {
        let kind = {
            let qos = self.qos.lock().unwrap_or_else(|e| e.into_inner());
            qos.ownership.kind
        };
        if kind != zerodds_qos::OwnershipKind::Exclusive {
            return true;
        }
        // Spec §2.2.3.23: ownership resolution applies per instance; for
        // keyless topics we treat the topic as a single instance with a
        // synthetic all-zero KeyHash.
        let (kh, key_bytes) = if T::HAS_KEY {
            let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
            sample.encode_key_holder_be(&mut holder);
            let kb = holder.as_bytes().to_vec();
            let max = T::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
            (crate::dds_type::compute_key_hash(&kb, max), kb)
        } else {
            ([0u8; 16], Vec::new())
        };
        // The instance must be registered so the owner tracker can
        // create the slot (`should_accept` otherwise returns `true` for
        // an unknown instance, which bypasses the filtering).
        let _ = self.instances.observe_sample(kh, key_bytes, None);
        self.instances
            .should_accept_sample_under_exclusive_ownership(&kh, writer_guid, writer_strength)
    }

    /// Spec §2.2.3.13 TIME_BASED_FILTER for the live-mode path.
    /// Returns `true` if the sample may be delivered.
    /// For keyless types or min_separation=0, always `true`.
    /// For keyed types: compute the keyhash via `encode_key_holder_be`,
    /// check it against instance_tracker, and on `true` call
    /// `record_delivery` directly so subsequent samples of the same
    /// instance are filtered correctly.
    #[cfg(feature = "std")]
    fn live_mode_time_based_filter_pass(&self, sample: &T, min_sep_nanos: u128) -> bool {
        if min_sep_nanos == 0 || !T::HAS_KEY {
            return true;
        }
        let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
        sample.encode_key_holder_be(&mut holder);
        let key_bytes = holder.as_bytes().to_vec();
        let max = T::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
        let kh = crate::dds_type::compute_key_hash(&key_bytes, max);
        let now = get_current_time();
        if !self
            .instances
            .should_deliver_under_time_based_filter(&kh, now, min_sep_nanos)
        {
            return false;
        }
        let _ = self.instances.observe_sample(kh, key_bytes, Some(now));
        self.instances.record_delivery(&kh, now);
        true
    }

    /// Reads all samples without removing them. Currently identical to
    /// `take` minus the removal. Sample state (`ReadCondition`
    /// §2.2.2.5.8) follows during wire-up.
    ///
    /// # Errors
    /// Same as `take`.
    pub fn read(&self) -> Result<Vec<T>> {
        // Live mode: stage any samples already delivered on the mpsc into the
        // inbox first, so read() (non-consuming) reflects all arrived data.
        // They stay in the inbox until take() drains them — without this, live
        // samples sit in the mpsc unseen by read() (and by
        // `data_available_stream`, which polls read()).
        #[cfg(feature = "std")]
        self.stage_pending_rx();

        let raw = {
            let inbox = self
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            inbox.clone()
        };
        let mut out = Vec::with_capacity(raw.len());
        for staged_item in raw {
            let crate::runtime::UserSample::Alive {
                payload: bytes,
                writer_guid,
                writer_strength,
                representation,
                big_endian,
                ..
            } = staged_item
            else {
                continue;
            };
            let sample = decode_for_encap::<T>(&bytes, representation, big_endian)
                .map_err(|e| decode_wire_error::<T>(&e, representation, big_endian))?;
            if !self.sample_passes_filter(&sample) {
                continue;
            }
            // §2.2.3.23 exclusive-ownership filter (also in the offline
            // fallback). The builtin-inject path uses writer_guid=[0;16]
            // with a shared-ownership default; passes_exclusive_ownership
            // then always returns `true`.
            if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                continue;
            }
            out.push(sample);
        }
        Ok(out)
    }

    /// Number of matched remote writers. Always 0 in offline mode.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.5.3.15 `get_matched_publications`.
    ///
    /// Side effect — when the matched count changes versus the last
    /// call, `on_subscription_matched` is fired via the bubble-up chain
    /// (spec §2.2.4.2.6.7).
    #[must_use]
    pub fn matched_publication_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let n = rt.user_reader_matched_count(eid);
            self.poll_subscription_matched(n);
            return n;
        }
        0
    }

    /// Delta-detect helper for `on_subscription_matched`.
    #[cfg(feature = "std")]
    pub(crate) fn poll_subscription_matched(&self, current: usize) {
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
        let status = crate::status::SubscriptionMatchedStatus {
            total_count: total as i32,
            total_count_change: delta.max(0) as i32,
            current_count: curr as i32,
            current_count_change: delta as i32,
            last_publication_handle: crate::instance_handle::HANDLE_NIL,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_subscription_matched(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta-detect for `on_requested_deadline_missed`.
    /// Spec §2.2.4.2.6.4.
    #[cfg(feature = "std")]
    pub(crate) fn poll_requested_deadline_missed(&self, current: u64) {
        let prev = self
            .last_requested_deadline_missed
            .swap(current, std::sync::atomic::Ordering::AcqRel);
        if current == prev {
            return;
        }
        let total_change = current.saturating_sub(prev);
        let status = crate::status::RequestedDeadlineMissedStatus {
            total_count: current as i32,
            total_count_change: total_change as i32,
            last_instance_handle: crate::instance_handle::HANDLE_NIL,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_requested_deadline_missed(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta-detect for `on_liveliness_changed`. Spec §2.2.4.2.6.6.
    /// Considers both counters (alive + not_alive); each change triggers
    /// exactly once.
    #[cfg(feature = "std")]
    pub(crate) fn poll_liveliness_changed(&self, alive_count: u64, not_alive_count: u64) {
        let curr_alive = alive_count as i64;
        let curr_not = not_alive_count as i64;
        let prev_alive = self
            .last_liveliness_alive
            .swap(curr_alive, std::sync::atomic::Ordering::AcqRel);
        let prev_not = self
            .last_liveliness_not_alive
            .swap(curr_not, std::sync::atomic::Ordering::AcqRel);
        // First observation (prev == -1) only counts if the counter is
        // nonzero; otherwise no trigger.
        let alive_changed = if prev_alive < 0 {
            curr_alive != 0
        } else {
            prev_alive != curr_alive
        };
        let not_changed = if prev_not < 0 {
            curr_not != 0
        } else {
            prev_not != curr_not
        };
        if !alive_changed && !not_changed {
            return;
        }
        let alive_delta = if prev_alive < 0 {
            curr_alive
        } else {
            curr_alive - prev_alive
        };
        let not_delta = if prev_not < 0 {
            curr_not
        } else {
            curr_not - prev_not
        };
        let status = crate::status::LivelinessChangedStatus {
            alive_count: curr_alive as i32,
            not_alive_count: curr_not as i32,
            alive_count_change: alive_delta as i32,
            not_alive_count_change: not_delta as i32,
            last_publication_handle: crate::instance_handle::HANDLE_NIL,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_liveliness_changed(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta-detect for `on_requested_incompatible_qos`.
    /// Spec §2.2.4.2.6.5.
    #[cfg(feature = "std")]
    pub(crate) fn poll_requested_incompatible_qos(
        &self,
        snapshot: crate::status::RequestedIncompatibleQosStatus,
    ) {
        let curr = i64::from(snapshot.total_count);
        let prev = self
            .last_requested_incompatible_qos
            .swap(curr, std::sync::atomic::Ordering::AcqRel);
        if curr == prev {
            return;
        }
        let delta = curr - prev.max(0);
        let status = crate::status::RequestedIncompatibleQosStatus {
            total_count: curr as i32,
            total_count_change: delta.max(0) as i32,
            last_policy_id: snapshot.last_policy_id,
            policies: snapshot.policies,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_requested_incompatible_qos(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta-detect for `on_sample_lost`. Spec §2.2.4.2.6.2.
    #[cfg(feature = "std")]
    pub(crate) fn poll_sample_lost(&self, current: u64) {
        let prev = self
            .last_sample_lost
            .swap(current, std::sync::atomic::Ordering::AcqRel);
        if current == prev {
            return;
        }
        let delta = current.saturating_sub(prev);
        let status = crate::status::SampleLostStatus {
            total_count: current as i32,
            total_count_change: delta as i32,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_sample_lost(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Delta-detect for `on_sample_rejected`. Spec §2.2.4.2.6.3.
    #[cfg(feature = "std")]
    pub(crate) fn poll_sample_rejected(&self, snapshot: crate::status::SampleRejectedStatus) {
        let curr = i64::from(snapshot.total_count);
        let prev = self
            .last_sample_rejected
            .swap(curr, std::sync::atomic::Ordering::AcqRel);
        if curr == prev {
            return;
        }
        let delta = curr - prev.max(0);
        let status = crate::status::SampleRejectedStatus {
            total_count: curr as i32,
            total_count_change: delta.max(0) as i32,
            last_reason: snapshot.last_reason,
            last_instance_handle: snapshot.last_instance_handle,
        };
        let chain = self.listener_chain();
        crate::listener_dispatch::dispatch_sample_rejected(
            &chain,
            self.entity_state.instance_handle(),
            status,
        );
    }

    /// Blocks until at least `min_count` remote writers are matched or
    /// `timeout` elapses. Event-driven via the runtime condvar
    /// (D.5e phase 1) — wakeup directly when SEDP propagates a match, no
    /// more 20-ms polling.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] if `min_count` is not reached within the
    /// time window.
    #[cfg(feature = "std")]
    pub fn wait_for_matched_publication(
        &self,
        min_count: usize,
        timeout: core::time::Duration,
    ) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.matched_publication_count() >= min_count {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(DdsError::Timeout);
            }
            // Live mode: park on the runtime match event. Spurious
            // wakeups are fine — we check the count on the next iteration.
            if let Some(rt) = self.runtime.as_ref() {
                let _ = rt.wait_match_event(deadline - now);
            } else {
                // Offline mode: no match events, sleep fallback.
                std::thread::sleep(core::time::Duration::from_millis(20));
            }
        }
    }

    /// Counter for requested-deadline violations (spec §2.2.4.2.11
    /// `REQUESTED_DEADLINE_MISSED_STATUS`). Monotonically increasing;
    /// rises by 1 per expired deadline window without a received sample.
    /// Offline / INFINITE → 0.
    ///
    /// May fire `on_requested_deadline_missed`.
    #[must_use]
    pub fn requested_deadline_missed_count(&self) -> u64 {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let n = rt.user_reader_requested_deadline_missed(eid);
            self.poll_requested_deadline_missed(n);
            return n;
        }
        0
    }

    /// Current `RequestedIncompatibleQosStatus`. Spec §2.2.4.2.6.5.
    /// May trigger `on_requested_incompatible_qos`.
    #[must_use]
    pub fn requested_incompatible_qos_status(
        &self,
    ) -> crate::status::RequestedIncompatibleQosStatus {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let s = rt.user_reader_requested_incompatible_qos(eid);
            self.poll_requested_incompatible_qos(s.clone());
            return s;
        }
        crate::status::RequestedIncompatibleQosStatus::default()
    }

    /// SampleLost counter. Spec §2.2.4.2.6.2.
    #[must_use]
    pub fn sample_lost_count(&self) -> u64 {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let n = rt.user_reader_sample_lost(eid);
            self.poll_sample_lost(n);
            return n;
        }
        0
    }

    /// SampleRejected status. Spec §2.2.4.2.6.3.
    #[must_use]
    pub fn sample_rejected_status(&self) -> crate::status::SampleRejectedStatus {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let s = rt.user_reader_sample_rejected(eid);
            self.poll_sample_rejected(s);
            return s;
        }
        crate::status::SampleRejectedStatus::default()
    }

    /// Polls all reader statuses once and fires pending listeners.
    /// Convenience helper for tests + periodic tick callers.
    #[cfg(feature = "std")]
    pub fn drive_listeners(&self) {
        let _ = self.matched_publication_count();
        let _ = self.requested_deadline_missed_count();
        let (_, alive, not_alive) = self.liveliness_changed_status();
        self.poll_liveliness_changed(alive, not_alive);
        let _ = self.requested_incompatible_qos_status();
        let _ = self.sample_lost_count();
        let _ = self.sample_rejected_status();
    }

    /// Liveliness status of the matched writer (spec §2.2.4.2.14
    /// `LIVELINESS_CHANGED_STATUS`): `(alive, alive_count, not_alive_count)`.
    ///
    /// * `alive`: current state (true = writer delivered a sample within
    ///   its lease duration).
    /// * `alive_count`: counter of "not_alive → alive" transitions.
    /// * `not_alive_count`: counter of "alive → not_alive" transitions.
    ///
    /// Offline / INFINITE lease → `(false, 0, 0)` / `(true, 0, 0)`
    /// depending on init. For v1.3 only `LivelinessKind::Automatic` is
    /// monitored.
    #[must_use]
    pub fn liveliness_changed_status(&self) -> (bool, u64, u64) {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let triple = rt.user_reader_liveliness_status(eid);
            // Listener trigger via delta detection.
            self.poll_liveliness_changed(triple.1, triple.2);
            return triple;
        }
        (false, 0, 0)
    }

    /// Blocks until at least one sample is available or the timeout has
    /// elapsed. The sample is not removed in the process — it is placed
    /// in a staging buffer that the next `take()` reads. This keeps
    /// `wait_for_data` + `take()` the canonical subscriber loop instead
    /// of busy-polling in application code.
    ///
    /// Spec analog: OMG DDS 1.4 §2.2.2.5.8 `ReadCondition` + `WaitSet`.
    /// This API provides the most important semantics (wake-on-data)
    /// without the full WaitSet/Condition infrastructure.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] if nothing arrives within the time window.
    #[cfg(feature = "std")]
    /// Non-blocking drain of all currently-available mpsc samples into the
    /// staging inbox (live mode only). Alive samples are staged; lifecycle
    /// events are applied to the tracker. Used by `read()` so a non-consuming
    /// read reflects data that has already been delivered on the channel.
    #[cfg(feature = "std")]
    fn stage_pending_rx(&self) {
        let Some(rx_mu) = self.rx.as_ref() else {
            return;
        };
        let Ok(rx) = rx_mu.lock() else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(sample @ crate::runtime::UserSample::Alive { .. }) => {
                    if let Ok(mut inbox) = self.inbox.lock() {
                        inbox.push(sample);
                    }
                }
                Ok(crate::runtime::UserSample::Lifecycle { key_hash, kind }) => {
                    let lc_kind = match kind {
                        zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed
                        | zerodds_rtps::history_cache::ChangeKind::NotAliveDisposedUnregistered => {
                            crate::sample_info::InstanceStateKind::NotAliveDisposed
                        }
                        zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered => {
                            crate::sample_info::InstanceStateKind::NotAliveNoWriters
                        }
                        _ => crate::sample_info::InstanceStateKind::Alive,
                    };
                    let mut holder_bytes = Vec::with_capacity(16);
                    holder_bytes.extend_from_slice(&key_hash);
                    let _ = self.__push_lifecycle(key_hash, holder_bytes, lc_kind);
                }
                Err(_) => break, // Empty or Disconnected
            }
        }
    }

    /// Blocks up to `timeout` for at least one sample to be available, staging
    /// one delivered mpsc sample into the inbox (live mode). Returns
    /// `Timeout` if none arrives. Offline mode: `Ok` iff the inbox is non-empty.
    ///
    /// # Errors
    /// `Timeout` on no data; `PreconditionNotMet` if an internal lock is poisoned.
    pub fn wait_for_data(&self, timeout: core::time::Duration) -> Result<()> {
        let Some(rx_mu) = self.rx.as_ref() else {
            // Offline mode: if the inbox already has something, OK,
            // otherwise timeout.
            let inbox_has = self.inbox.lock().map(|i| !i.is_empty()).unwrap_or(false);
            if inbox_has {
                return Ok(());
            }
            return Err(DdsError::Timeout);
        };

        // Anything already in the staging inbox?
        {
            let inbox = self
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            if !inbox.is_empty() {
                return Ok(());
            }
        }

        let rx = rx_mu.lock().map_err(|_| DdsError::PreconditionNotMet {
            reason: "datareader rx poisoned",
        })?;
        let result = match rx.recv_timeout(timeout) {
            Ok(item) => {
                match item {
                    sample @ crate::runtime::UserSample::Alive { .. } => {
                        let mut inbox =
                            self.inbox
                                .lock()
                                .map_err(|_| DdsError::PreconditionNotMet {
                                    reason: "datareader inbox poisoned",
                                })?;
                        inbox.push(sample);
                    }
                    crate::runtime::UserSample::Lifecycle { key_hash, kind } => {
                        let lc_kind = match kind {
                            zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed
                            | zerodds_rtps::history_cache::ChangeKind::NotAliveDisposedUnregistered => {
                                crate::sample_info::InstanceStateKind::NotAliveDisposed
                            }
                            zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered => {
                                crate::sample_info::InstanceStateKind::NotAliveNoWriters
                            }
                            _ => crate::sample_info::InstanceStateKind::Alive,
                        };
                        let mut holder_bytes = Vec::with_capacity(16);
                        holder_bytes.extend_from_slice(&key_hash);
                        let _ = self.__push_lifecycle(key_hash, holder_bytes, lc_kind);
                    }
                }
                Ok(())
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(DdsError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(DdsError::PreconditionNotMet {
                    reason: "datareader rx disconnected",
                })
            }
        };
        // Release the lock first, then fire listeners (lock discipline).
        drop(rx);
        if result.is_ok() {
            self.notify_data_arrived();
        }
        result
    }

    /// Builtin-topic helper: returns the Arc to the shared inbox (reader
    /// clones share the same buffer).
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn __inbox_handle(&self) -> Arc<Mutex<Vec<crate::runtime::UserSample>>> {
        Arc::clone(&self.inbox)
    }

    /// Test helper: inserts an encoded payload into the inbox.
    /// At runtime this is replaced by the ReliableReader delivery path.
    ///
    /// Triggers the listener bubble-up chain `on_data_on_readers`
    /// (subscriber stage) and `on_data_available` (reader stage). Spec
    /// §2.2.4.2.7.1 / §2.2.4.2.6.1.
    #[doc(hidden)]
    pub fn __push_raw(&self, bytes: Vec<u8>) -> Result<()> {
        self.__push_raw_with_writer(bytes, [0u8; 16], 0)
    }

    /// Test hook: pushes a sample with an explicit writer GUID and
    /// `ownership_strength` into the inbox. Used by the Cyclone interop
    /// harness and the exclusive-ownership tests.
    #[doc(hidden)]
    pub fn __push_raw_with_writer(
        &self,
        bytes: Vec<u8>,
        writer_guid: [u8; 16],
        writer_strength: i32,
    ) -> Result<()> {
        {
            let mut inbox = self
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            inbox.push(crate::runtime::UserSample::Alive {
                payload: crate::sample_bytes::SampleBytes::from_vec(bytes),
                writer_guid,
                writer_strength,
                // Test hook: raw-bytes injection, XCDR1 baseline, little-endian.
                representation: 0,
                big_endian: false,
                source_timestamp: None,
                source_sequence_number: -1,
            });
        }
        // Listener notify outside the inbox lock to avoid re-entrancy.
        self.notify_data_arrived();
        Ok(())
    }

    /// Calls the `on_data_on_readers` and `on_data_available` bubble-up
    /// paths. Spec §2.2.4.1: for each new sample, `data_on_readers`
    /// (subscriber level) and `data_available` (reader level) are set as
    /// independent statuses; once the subscriber has consumed
    /// `data_on_readers`, `data_available` must *not* be suppressed — the
    /// two statuses are separate bits in the mask.
    #[cfg(feature = "std")]
    pub(crate) fn notify_data_arrived(&self) {
        let chain = self.listener_chain();
        let reader_handle = self.entity_state.instance_handle();
        crate::listener_dispatch::dispatch_data_on_readers(&chain, reader_handle);
        crate::listener_dispatch::dispatch_data_available(&chain, reader_handle);
    }

    // ========================================================================
    // SampleInfo statechart + instance lifecycle.
    // Spec §2.2.2.5.1, §2.2.2.5.3.{5,27,28}.
    // ========================================================================

    /// Returns the current [`InstanceTracker`] (shared with the internal
    /// bookkeeping). Mainly for tests / inspection.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn instance_tracker(&self) -> InstanceTracker {
        self.instances.clone()
    }

    /// This reader's instance handle (GUID-derived). Lets the application
    /// ignore the reader's own subscription — e.g. a durability service
    /// ignoring its ingest reader on the replay-writer side to avoid an echo
    /// loop. Mirrors [`crate::DataWriter::instance_handle`].
    #[must_use]
    pub fn instance_handle(&self) -> crate::instance_handle::InstanceHandle {
        self.entity_state.instance_handle()
    }

    /// Returns (Runtime, EntityId) when the reader runs in live mode.
    /// Cross-crate hook for the async layer (dcps-async), which must
    /// register the waker slot directly.
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn runtime_handle(
        &self,
    ) -> Option<(alloc::sync::Arc<crate::runtime::DcpsRuntime>, EntityId)> {
        match (&self.runtime, self.entity_id) {
            (Some(rt), Some(eid)) => Some((alloc::sync::Arc::clone(rt), eid)),
            _ => None,
        }
    }

    /// Spec §2.2.3.23 — hook for "writer X lost liveliness". Does two
    /// things:
    ///   1. clears the OWNERSHIP=EXCLUSIVE owner for all instances whose
    ///      owner was this writer (so the next sample from another writer
    ///      can win again via
    ///      `should_accept_sample_under_exclusive_ownership`);
    ///   2. returns the number of affected instances.
    ///
    /// Called from the WLP path once a writer lease has expired (see
    /// `wlp::WlpEndpoint::lost_peers`).
    #[must_use]
    pub fn notify_writer_liveliness_lost(&self, writer_guid: [u8; 16]) -> usize {
        self.instances.clear_owner_for_writer(writer_guid)
    }

    /// Like [`Self::notify_writer_liveliness_lost`], but matches only on
    /// the first 12 bytes (GuidPrefix). Allows failover when only the
    /// participant identity is known (e.g. on SPDP lease expiry).
    #[must_use]
    pub fn notify_participant_liveliness_lost(&self, prefix: [u8; 12]) -> usize {
        self.instances.clear_owner_for_writer_prefix(prefix)
    }

    /// Turns a sample value into its corresponding local
    /// [`InstanceHandle`], or [`HANDLE_NIL`] if unknown / non-keyed.
    /// Spec §2.2.2.5.3.26 `lookup_instance` (reader variant).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_instance(&self, instance: &T) -> InstanceHandle {
        if !T::HAS_KEY {
            return HANDLE_NIL;
        }
        let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
        instance.encode_key_holder_be(&mut holder);
        let bytes = holder.as_bytes();
        let max = T::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
        let kh = crate::dds_type::compute_key_hash(bytes, max);
        self.instances.lookup(&kh).unwrap_or(HANDLE_NIL)
    }

    /// Spec §2.2.2.5.3.25 `get_key_value`. Returns the sample value with
    /// only the `@key` fields filled in (reconstructed from the stored
    /// key holder via `T::decode`).
    ///
    /// # Errors
    /// `BadParameter` if `handle` is unknown; `WireError` if `T::decode`
    /// cannot reconstruct the key stream.
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

    /// Drains all pending bytes from rx + inbox into the internal sample
    /// cache. For each sample the KeyHash is computed, the instance is
    /// registered (if new), and a matching [`SampleInfo`] is created.
    ///
    /// Called automatically by the `*_with_info`/`*_instance` APIs.
    #[cfg(feature = "std")]
    fn ingest_into_cache(&self) -> Result<()> {
        // Step 1: collect all incoming samples. `raw` carries
        // (bytes, writer_guid, writer_strength) so the exclusive-
        // ownership filter (DDS 1.4 §2.2.3.23) is applicable.
        //
        // Wave 2.1 zero-copy: `raw` now carries `SampleBytes` (refcounted
        // Arc<[u8]>) instead of `Vec<u8>`. Decode goes via Deref<[u8]>
        // directly without to_vec. Saves 2 hot-path copies per recv'd
        // Alive sample. Spec: docs/specs/zerodds-zero-copy-1.0.md §6 wave 2.1.
        let mut raw: Vec<RawIngestSample> = Vec::new();
        {
            let mut inbox = self
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            for item in inbox.drain(..) {
                if let crate::runtime::UserSample::Alive {
                    payload,
                    writer_guid,
                    writer_strength,
                    source_timestamp,
                    big_endian,
                    representation,
                    ..
                } = item
                {
                    raw.push((
                        payload,
                        writer_guid,
                        writer_strength,
                        source_timestamp,
                        big_endian,
                        representation,
                    ));
                }
            }
        }
        // Live-mode channel: enqueue Alive samples into `raw`, handle
        // lifecycle markers directly via __push_lifecycle.
        let mut lifecycle_pending: Vec<(
            crate::instance_tracker::KeyHash,
            crate::sample_info::InstanceStateKind,
        )> = Vec::new();
        if let Some(rx_mu) = self.rx.as_ref() {
            let rx = rx_mu.lock().map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader rx poisoned",
            })?;
            while let Ok(item) = rx.try_recv() {
                match item {
                    crate::runtime::UserSample::Alive {
                        payload: bytes,
                        writer_guid,
                        writer_strength,
                        source_timestamp,
                        big_endian,
                        representation,
                        ..
                    } => raw.push((
                        bytes,
                        writer_guid,
                        writer_strength,
                        source_timestamp,
                        big_endian,
                        representation,
                    )),
                    crate::runtime::UserSample::Lifecycle { key_hash, kind } => {
                        let lc_kind = match kind {
                            zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed
                            | zerodds_rtps::history_cache::ChangeKind::NotAliveDisposedUnregistered => {
                                crate::sample_info::InstanceStateKind::NotAliveDisposed
                            }
                            zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered => {
                                crate::sample_info::InstanceStateKind::NotAliveNoWriters
                            }
                            _ => crate::sample_info::InstanceStateKind::Alive,
                        };
                        lifecycle_pending.push((key_hash, lc_kind));
                    }
                }
            }
        }
        // Apply lifecycle markers only AFTER draining so the lock path
        // stays clean (__push_lifecycle takes its own locks).
        for (kh, lc_kind) in lifecycle_pending {
            let mut holder_bytes = Vec::with_capacity(16);
            holder_bytes.extend_from_slice(&kh);
            let _ = self.__push_lifecycle(kh, holder_bytes, lc_kind);
        }
        let now = get_current_time();
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        if raw.is_empty() {
            // Even without new bytes, autopurge must run; otherwise
            // disposed/nowriter instances never expire outside sample inflow.
            self.run_reader_autopurge(now, &mut cache);
            return Ok(());
        }
        for (bytes, writer_guid, writer_strength, src_ts, big_endian, representation) in raw {
            // RTPS-F1: use the writer's INFO_TS source timestamp for the
            // SampleInfo + DESTINATION_ORDER; fall back to reception time
            // (`now`) when the writer sent none.
            let sample_source_ts = src_ts.map_or(now, crate::time::he_timestamp_to_time);
            // Decode T to (a) evaluate the filter and (b) compute the
            // KeyHash.
            let sample = decode_for_encap::<T>(&bytes, representation, big_endian)
                .map_err(|e| decode_wire_error::<T>(&e, representation, big_endian))?;
            if !self.sample_passes_filter(&sample) {
                continue;
            }
            // §2.2.3.23 exclusive-ownership filter: reject samples from
            // weaker writers before they enter the cache.
            if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                continue;
            }
            let info = if T::HAS_KEY {
                let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
                sample.encode_key_holder_be(&mut holder);
                let key_bytes = holder.as_bytes().to_vec();
                let max = T::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
                let kh = crate::dds_type::compute_key_hash(&key_bytes, max);
                // QoS filter BEFORE observe_sample so discarded samples do
                // not affect the sample state.
                let (min_sep_nanos, by_source_ts) = {
                    let qos = self.qos.lock().unwrap_or_else(|e| e.into_inner());
                    (
                        qos.time_based_filter.minimum_separation.to_nanos(),
                        qos.destination_order.kind
                            == zerodds_qos::DestinationOrderKind::BySourceTimestamp,
                    )
                };
                // Spec §2.2.3.13 TIME_BASED_FILTER: drop if less than
                // minimum_separation has elapsed since the last delivered
                // sample of this instance.
                if !self
                    .instances
                    .should_deliver_under_time_based_filter(&kh, now, min_sep_nanos)
                {
                    continue;
                }
                // Spec §2.2.3.18 DESTINATION_ORDER: under BY_SOURCE_TIMESTAMP
                // the ordering key is the writer's source timestamp (from
                // INFO_TS), not the reception time — only deliver samples with a
                // strictly greater source_ts. Under BY_RECEPTION_TIMESTAMP the
                // reception clock (`now`) is the key.
                let order_ts = if by_source_ts { sample_source_ts } else { now };
                if !self.instances.should_deliver_under_destination_order(
                    &kh,
                    order_ts,
                    by_source_ts,
                ) {
                    continue;
                }
                let (handle, _) = self.instances.observe_sample(kh, key_bytes, Some(now));
                self.instances.record_delivery(&kh, order_ts);
                let state = match self.instances.get_by_handle(handle) {
                    Some(s) => s,
                    None => continue, // should never happen — defensive
                };
                SampleInfo {
                    sample_state: SampleStateKind::NotRead,
                    view_state: if state.reader_view_new {
                        ViewStateKind::New
                    } else {
                        ViewStateKind::NotNew
                    },
                    instance_state: state.kind,
                    disposed_generation_count: state.disposed_generation_count,
                    no_writers_generation_count: state.no_writers_generation_count,
                    source_timestamp: sample_source_ts,
                    instance_handle: handle,
                    valid_data: true,
                    ..SampleInfo::default()
                }
            } else {
                // Non-keyed topics: a "pseudo handle" per sample would be
                // overkill — we leave it at HANDLE_NIL (spec §2.2.2.5.1.10
                // allows that, since the instance view for non-keyed
                // topics is formally "everything is one instance").
                SampleInfo {
                    sample_state: SampleStateKind::NotRead,
                    view_state: ViewStateKind::NotNew,
                    instance_handle: HANDLE_NIL,
                    source_timestamp: sample_source_ts,
                    valid_data: true,
                    ..SampleInfo::default()
                }
            };
            cache.push(CachedSample {
                bytes: Some(bytes),
                info,
            });
        }
        // Spec §2.2.3.22 ReaderDataLifecycle: remove instances from the
        // tracker and cache that have been in NotAlive-Disposed or
        // NotAlive-NoWriters longer than autopurge_*_samples_delay.
        self.run_reader_autopurge(now, &mut cache);
        Ok(())
    }

    /// Applies `ReaderDataLifecycle.autopurge_*`: removes expired
    /// instances from the tracker + cache. Called by `ingest_into_cache`
    /// and when reading with no new bytes.
    #[cfg(feature = "std")]
    fn run_reader_autopurge(&self, now: Time, cache: &mut Vec<CachedSample>) {
        let (purge_disp, purge_now) = {
            let qos = self.qos.lock().unwrap_or_else(|e| e.into_inner());
            (
                qos.reader_data_lifecycle
                    .autopurge_disposed_samples_delay
                    .to_nanos(),
                qos.reader_data_lifecycle
                    .autopurge_nowriter_samples_delay
                    .to_nanos(),
            )
        };
        if purge_disp == u128::MAX && purge_now == u128::MAX {
            return;
        }
        let purged = self.instances.autopurge(now, purge_disp, purge_now);
        if purged > 0 {
            cache.retain(|s| {
                s.info.instance_handle.is_nil()
                    || self
                        .instances
                        .get_by_handle(s.info.instance_handle)
                        .is_some()
            });
        }
    }

    /// Pushes a pure lifecycle marker (dispose / unregister) into the
    /// cache. Called by the runtime as soon as a writer sends
    /// `dispose`/`unregister_instance`.
    #[cfg(feature = "std")]
    #[doc(hidden)]
    pub fn __push_lifecycle(
        &self,
        keyhash: crate::instance_tracker::KeyHash,
        key_holder: Vec<u8>,
        kind: InstanceStateKind,
    ) -> Result<()> {
        let now = get_current_time();
        // First bring the instance into the right state in the tracker.
        // observe_sample re-registers it if needed and makes it alive.
        let (handle, _) = self
            .instances
            .observe_sample(keyhash, key_holder, Some(now));
        match kind {
            InstanceStateKind::NotAliveDisposed => {
                self.instances.dispose(handle, Some(now));
            }
            InstanceStateKind::NotAliveNoWriters => {
                self.instances.unregister(handle, Some(now));
            }
            InstanceStateKind::Alive => {}
        }
        let Some(state) = self.instances.get_by_handle(handle) else {
            return Ok(()); // should never happen — defensive
        };
        let info = SampleInfo {
            source_timestamp: now,
            valid_data: false,
            instance_handle: handle,
            instance_state: state.kind,
            disposed_generation_count: state.disposed_generation_count,
            no_writers_generation_count: state.no_writers_generation_count,
            view_state: if state.reader_view_new {
                ViewStateKind::New
            } else {
                ViewStateKind::NotNew
            },
            ..SampleInfo::default()
        };
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        cache.push(CachedSample { bytes: None, info });
        Ok(())
    }

    /// `take` with full [`SampleInfo`]. Spec §2.2.2.5.3.5 `take`.
    /// Consumes the samples from the cache (the `NOT_READ → READ`
    /// transition is moot since they are gone).
    ///
    /// # Errors
    /// Same as [`Self::take`].
    #[cfg(feature = "std")]
    pub fn take_with_info(&self) -> Result<Vec<Sample<T>>> {
        self.take_filtered(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
    }

    /// `read` with full [`SampleInfo`]. Does not consume — only marks
    /// the samples as `READ` (spec §2.2.2.5.3.4).
    ///
    /// # Errors
    /// Same as [`Self::read`].
    #[cfg(feature = "std")]
    pub fn read_with_info(&self) -> Result<Vec<Sample<T>>> {
        self.read_filtered(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
    }

    /// `take` with state masks (spec §2.2.2.5.3.6 `take_w_condition`).
    ///
    /// # Errors
    /// Same as [`Self::take`].
    #[cfg(feature = "std")]
    pub fn take_filtered(
        &self,
        sample_mask: u32,
        view_mask: u32,
        instance_mask: u32,
    ) -> Result<Vec<Sample<T>>> {
        self.ingest_into_cache()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        let mut out = Vec::new();
        let mut keep = Vec::with_capacity(cache.len());
        for s in cache.drain(..) {
            if s.info.matches_states(sample_mask, view_mask, instance_mask) {
                let sample = self.materialize(s)?;
                self.instances.mark_view_seen(sample.info.instance_handle);
                if sample.info.instance_handle != HANDLE_NIL {
                    self.instances.drain_samples(sample.info.instance_handle, 1);
                }
                out.push(sample);
            } else {
                keep.push(s);
            }
        }
        *cache = keep;
        Ok(out)
    }

    /// `read` with state masks (spec §2.2.2.5.3.3 `read_w_condition`).
    ///
    /// # Errors
    /// Same as [`Self::read`].
    #[cfg(feature = "std")]
    pub fn read_filtered(
        &self,
        sample_mask: u32,
        view_mask: u32,
        instance_mask: u32,
    ) -> Result<Vec<Sample<T>>> {
        self.ingest_into_cache()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        let mut out = Vec::with_capacity(cache.len());
        for s in cache.iter_mut() {
            if !s.info.matches_states(sample_mask, view_mask, instance_mask) {
                continue;
            }
            // Build a snapshot (with the current sample-state view).
            let snapshot = Sample::new(
                self.decode_or_keyholder(s.bytes.as_deref(), s.info.instance_handle)?,
                s.info,
            );
            // Sample-state transition NOT_READ → READ (spec §2.2.2.5.3.4).
            s.info.sample_state = SampleStateKind::Read;
            self.instances.mark_view_seen(s.info.instance_handle);
            out.push(snapshot);
        }
        Ok(out)
    }

    /// `read_w_condition` (spec §2.2.2.5.3.7) — in addition to the state
    /// mask, applies the QueryCondition's SQL filter per sample. Samples
    /// stay in the cache (sample state NOT_READ → READ).
    ///
    /// # Errors
    /// `PreconditionNotMet` on lock poisoning or SQL eval error.
    #[cfg(feature = "std")]
    pub fn read_w_condition(
        &self,
        condition: &Arc<crate::condition::QueryCondition>,
    ) -> Result<Vec<Sample<T>>> {
        let base = condition.base();
        let sample_mask = base.get_sample_state_mask();
        let view_mask = base.get_view_state_mask();
        let instance_mask = base.get_instance_state_mask();

        self.ingest_into_cache()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        let mut out = Vec::with_capacity(cache.len());
        for s in cache.iter_mut() {
            if !s.info.matches_states(sample_mask, view_mask, instance_mask) {
                continue;
            }
            let decoded = self.decode_or_keyholder(s.bytes.as_deref(), s.info.instance_handle)?;
            let row = crate::dds_type::DdsTypeRow::new(&decoded);
            // Filter eval error -> sample is rejected (spec: "filter
            // expression false" semantics), but we do not propagate a
            // hard error upward, except for lock poisoning.
            if !condition.evaluate(&row).unwrap_or(false) {
                continue;
            }
            let snapshot = Sample::new(decoded, s.info);
            s.info.sample_state = SampleStateKind::Read;
            self.instances.mark_view_seen(s.info.instance_handle);
            out.push(snapshot);
        }
        Ok(out)
    }

    /// `take_w_condition` (spec §2.2.2.5.3.8) — like `read_w_condition`,
    /// but consumes the samples (removes them from the cache).
    ///
    /// # Errors
    /// `PreconditionNotMet` on lock poisoning or SQL eval error.
    #[cfg(feature = "std")]
    pub fn take_w_condition(
        &self,
        condition: &Arc<crate::condition::QueryCondition>,
    ) -> Result<Vec<Sample<T>>> {
        let base = condition.base();
        let sample_mask = base.get_sample_state_mask();
        let view_mask = base.get_view_state_mask();
        let instance_mask = base.get_instance_state_mask();

        self.ingest_into_cache()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        let mut out = Vec::new();
        let mut keep = Vec::with_capacity(cache.len());
        for s in cache.drain(..) {
            if !s.info.matches_states(sample_mask, view_mask, instance_mask) {
                keep.push(s);
                continue;
            }
            let decoded = self.decode_or_keyholder(s.bytes.as_deref(), s.info.instance_handle)?;
            let row = crate::dds_type::DdsTypeRow::new(&decoded);
            if !condition.evaluate(&row).unwrap_or(false) {
                keep.push(s);
                continue;
            }
            let sample = Sample::new(decoded, s.info);
            self.instances.mark_view_seen(sample.info.instance_handle);
            if sample.info.instance_handle != HANDLE_NIL {
                self.instances.drain_samples(sample.info.instance_handle, 1);
            }
            out.push(sample);
        }
        *cache = keep;
        Ok(out)
    }

    /// `read_instance` (spec §2.2.2.5.3.27). Returns only samples of the
    /// given instance.
    ///
    /// # Errors
    /// `BadParameter` if `handle == HANDLE_NIL`.
    #[cfg(feature = "std")]
    pub fn read_instance(&self, handle: InstanceHandle) -> Result<Vec<Sample<T>>> {
        if handle.is_nil() {
            return Err(DdsError::BadParameter {
                what: "read_instance with HANDLE_NIL",
            });
        }
        self.ingest_into_cache()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        let mut out = Vec::new();
        for s in cache.iter_mut() {
            if s.info.instance_handle != handle {
                continue;
            }
            let snap = Sample::new(
                self.decode_or_keyholder(s.bytes.as_deref(), s.info.instance_handle)?,
                s.info,
            );
            s.info.sample_state = SampleStateKind::Read;
            self.instances.mark_view_seen(handle);
            out.push(snap);
        }
        Ok(out)
    }

    /// `take_instance` (spec §2.2.2.5.3.27, take variant). Consumes.
    ///
    /// # Errors
    /// `BadParameter` if `handle == HANDLE_NIL`.
    #[cfg(feature = "std")]
    pub fn take_instance(&self, handle: InstanceHandle) -> Result<Vec<Sample<T>>> {
        if handle.is_nil() {
            return Err(DdsError::BadParameter {
                what: "take_instance with HANDLE_NIL",
            });
        }
        self.ingest_into_cache()?;
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        let mut out = Vec::new();
        let mut keep = Vec::with_capacity(cache.len());
        for s in cache.drain(..) {
            if s.info.instance_handle == handle {
                out.push(self.materialize(s)?);
            } else {
                keep.push(s);
            }
        }
        *cache = keep;
        if !out.is_empty() {
            self.instances.mark_view_seen(handle);
            self.instances.drain_samples(handle, out.len() as u32);
        }
        Ok(out)
    }

    /// `read_next_instance` (spec §2.2.2.5.3.28). Returns the samples of
    /// the **next** instance (in sort order) after `previous`.
    ///
    /// `previous == HANDLE_NIL` starts at the first handle.
    ///
    /// # Errors
    /// Same as `read`.
    #[cfg(feature = "std")]
    pub fn read_next_instance(&self, previous: InstanceHandle) -> Result<Vec<Sample<T>>> {
        // E1 bug 3 — a fresh reader (empty `InstanceTracker`) must discover
        // instances here too. `next_handle_after` only ever sees instances
        // already registered in `self.instances`; that registration
        // previously happened only inside `read_instance`/`take_instance`,
        // which this function reaches ONLY if `next_handle_after` already
        // succeeded — a reader that has never called a plain
        // `read`/`take`/`*_with_info` first therefore always saw an empty
        // tracker and returned `Ok(vec![])` forever, even with samples
        // sitting unread in the incoming channel. Ingest unconditionally
        // BEFORE the lookup so a fresh reader discovers instances on the
        // very first call.
        self.ingest_into_cache()?;
        let Some(next) = self.instances.next_handle_after(previous) else {
            return Ok(Vec::new());
        };
        self.read_instance(next)
    }

    /// `take_next_instance` (spec §2.2.2.5.3.28). Take variant.
    ///
    /// # Errors
    /// Same as `take`.
    #[cfg(feature = "std")]
    pub fn take_next_instance(&self, previous: InstanceHandle) -> Result<Vec<Sample<T>>> {
        // E1 bug 3 — see `read_next_instance`.
        self.ingest_into_cache()?;
        let Some(next) = self.instances.next_handle_after(previous) else {
            return Ok(Vec::new());
        };
        self.take_instance(next)
    }

    /// Helper: turns a CachedSample into a `Sample<T>`. For lifecycle
    /// markers (`bytes == None`), `T` is reconstructed from the stored
    /// key holder (spec §2.2.2.5.1.13: `data` then contains only the key
    /// portion).
    #[cfg(feature = "std")]
    fn materialize(&self, s: CachedSample) -> Result<Sample<T>> {
        let data = self.decode_or_keyholder(s.bytes.as_deref(), s.info.instance_handle)?;
        #[cfg(feature = "metrics")]
        crate::metrics::add_samples_read(self.topic.name(), 1);
        Ok(Sample::new(data, s.info))
    }

    /// Decode helper: for `Some(bytes)` via `T::decode`, for `None`
    /// (lifecycle marker) via the instance's key holder; if that is also
    /// unavailable, falls back to `T::decode(&[])`.
    #[cfg(feature = "std")]
    fn decode_or_keyholder(&self, bytes: Option<&[u8]>, handle: InstanceHandle) -> Result<T> {
        if let Some(b) = bytes {
            return T::decode(b).map_err(|e| DdsError::WireError {
                message: alloc::string::ToString::to_string(&e),
            });
        }
        if let Some(holder) = self.instances.get_key_holder(handle) {
            return T::decode(&holder).map_err(|e| DdsError::WireError {
                message: alloc::string::ToString::to_string(&e),
            });
        }
        T::decode(&[]).map_err(|e| DdsError::WireError {
            message: alloc::string::ToString::to_string(&e),
        })
    }
}

#[cfg(feature = "std")]
impl<T: DdsType> crate::entity::Entity for DataReader<T> {
    type Qos = DataReaderQos;

    fn get_qos(&self) -> Self::Qos {
        self.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Spec §2.2.3 / §2.2.2.5.3: DURABILITY, RELIABILITY, HISTORY,
    /// RESOURCE_LIMITS, OWNERSHIP are Changeable=NO post-enable.
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

// ---- Boxed type-mapped variant for heterogeneous reader lists ----
#[allow(dead_code)]
pub(crate) trait AnyDataReader: Send + Sync + core::fmt::Debug {
    fn topic_name(&self) -> &str;
    fn type_name(&self) -> &'static str;
}

impl<T: DdsType + Send + 'static> AnyDataReader for DataReader<T>
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

#[allow(dead_code)]
pub(crate) fn boxed_any_reader<T: DdsType + Send + Sync + 'static>(
    r: DataReader<T>,
) -> Box<dyn AnyDataReader> {
    Box::new(r)
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

    /// `decode_for_encap` must pick the decoder matching the sample's
    /// encapsulation — XCDR version *and* byte order — so an XCDR1 writer
    /// (CycloneDDS / legacy RTI default for `@final`) is decoded with the
    /// classic-CDR rule, not the XCDR2 rule. Regression for Bug DR1.
    #[test]
    fn decode_for_encap_dispatches_on_representation_and_endianness() {
        use crate::dds_type::{DecodeError, EncodeError};

        /// Marker whose four decode variants return a distinct value, so the
        /// test can prove which one `decode_for_encap` selected.
        #[derive(Debug, PartialEq)]
        struct Probe(u8);
        impl DdsType for Probe {
            const TYPE_NAME: &'static str = "test::Probe";
            fn encode(&self, _out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(1)) // XCDR2-LE
            }
            fn decode_be(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(2)) // XCDR2-BE
            }
            fn decode_xcdr1(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(10)) // XCDR1-LE
            }
            fn decode_xcdr1_be(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(20)) // XCDR1-BE
            }
        }

        // (representation, big_endian) -> expected marker.
        // representation: 1 = XCDR2, 0 = XCDR1.
        assert_eq!(decode_for_encap::<Probe>(&[], 1, false).unwrap(), Probe(1));
        assert_eq!(decode_for_encap::<Probe>(&[], 1, true).unwrap(), Probe(2));
        assert_eq!(decode_for_encap::<Probe>(&[], 0, false).unwrap(), Probe(10));
        assert_eq!(decode_for_encap::<Probe>(&[], 0, true).unwrap(), Probe(20));
    }

    /// A failed decode carries diagnostic context (encapsulation + this reader's
    /// extensibility) and flags the extensibility/framing mismatch as a
    /// *plausible* — not asserted — cause (issue #27).
    #[test]
    fn decode_wire_error_carries_diagnostic_context() {
        use crate::dds_type::DecodeError;
        let inner = DecodeError::Invalid { what: "boom" };
        // RawBytes is @final by default; received XCDR2 little-endian.
        let msg = match decode_wire_error::<RawBytes>(&inner, 1, false) {
            DdsError::WireError { message } => message,
            _ => alloc::string::String::new(),
        };
        assert!(!msg.is_empty(), "expected a WireError variant");
        assert!(msg.contains("XCDR2"), "{msg}");
        assert!(msg.contains("little-endian"), "{msg}");
        assert!(msg.contains("@final"), "{msg}");
        assert!(msg.contains("DHEADER"), "{msg}");
        assert!(msg.to_lowercase().contains("plausible"), "{msg}");
        assert!(msg.contains("Not confirmed"), "{msg}");
    }

    #[test]
    fn subscriber_creates_datareader_for_matching_type() {
        let s = Subscriber::new(SubscriberQos::default(), None);
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        assert_eq!(r.topic().name(), "Chatter");
    }

    #[test]
    fn datareader_take_returns_decoded_samples() {
        let s = Subscriber::new(SubscriberQos::default(), None);
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        r.__push_raw(vec![1, 2, 3]).unwrap();
        r.__push_raw(vec![4, 5]).unwrap();
        let samples = r.take().unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].data, vec![1, 2, 3]);
        assert_eq!(samples[1].data, vec![4, 5]);
        // The inbox is now empty.
        let again = r.take().unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn datareader_read_preserves_samples() {
        let s = Subscriber::new(SubscriberQos::default(), None);
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        r.__push_raw(vec![0xAA]).unwrap();
        let first = r.read().unwrap();
        let second = r.read().unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
    }

    // poll_subscription_matched + listener-slot API.

    use core::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn datareader_set_listener_stores_arc_and_mask() {
        struct L;
        impl crate::listener::DataReaderListener for L {}
        let s = Subscriber::new(SubscriberQos::default(), None);
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        assert!(r.get_listener().is_none());
        r.set_listener(Some(Arc::new(L)), crate::psm_constants::status::ANY);
        assert!(r.get_listener().is_some());
        assert_eq!(
            r.entity_state.listener_mask(),
            crate::psm_constants::status::ANY
        );
    }

    #[test]
    fn poll_subscription_matched_fires_on_count_increase() {
        struct Cnt(AtomicU32);
        impl crate::listener::DataReaderListener for Cnt {
            fn on_subscription_matched(
                &self,
                _r: crate::InstanceHandle,
                _s: crate::status::SubscriptionMatchedStatus,
            ) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let s = Subscriber::new(SubscriberQos::default(), None);
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        let cnt = Arc::new(Cnt(AtomicU32::new(0)));
        r.set_listener(Some(cnt.clone()), crate::psm_constants::status::ANY);

        r.poll_subscription_matched(0);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 1);
        r.poll_subscription_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 2);
        r.poll_subscription_matched(1);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 2);
        r.poll_subscription_matched(0);
        assert_eq!(cnt.0.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn poll_subscription_matched_with_no_listener_is_noop() {
        let s = Subscriber::new(SubscriberQos::default(), None);
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        r.poll_subscription_matched(0);
        r.poll_subscription_matched(3);
    }

    #[test]
    fn notify_data_arrived_fires_data_available_and_data_on_readers() {
        struct ReadCnt(AtomicU32, AtomicU32);
        impl crate::listener::DataReaderListener for ReadCnt {
            fn on_data_available(&self, _r: crate::InstanceHandle) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_subscription_matched(
                &self,
                _r: crate::InstanceHandle,
                _s: crate::status::SubscriptionMatchedStatus,
            ) {
                self.1.fetch_add(1, Ordering::Relaxed);
            }
        }
        let s = Subscriber::new(SubscriberQos::default(), None);
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        let rc = Arc::new(ReadCnt(AtomicU32::new(0), AtomicU32::new(0)));
        r.set_listener(Some(rc.clone()), crate::psm_constants::status::ANY);
        r.notify_data_arrived();
        assert_eq!(rc.0.load(Ordering::Relaxed), 1);
        // sub_matched counter unchanged (different status bit).
        assert_eq!(rc.1.load(Ordering::Relaxed), 0);
    }

    // ---- §2.2.2.5.2.8/.9 begin/end_access ----

    #[test]
    fn subscriber_begin_end_access_roundtrip() {
        let s = Subscriber::new(SubscriberQos::default(), None);
        assert!(!s.is_access_open());
        s.begin_access();
        assert!(s.is_access_open());
        s.end_access().unwrap();
        assert!(!s.is_access_open());
    }

    #[test]
    fn subscriber_end_access_without_begin_returns_precondition_not_met() {
        // Spec §2.2.2.5.2.9 — end without begin is a spec violation.
        let s = Subscriber::new(SubscriberQos::default(), None);
        let res = s.end_access();
        assert!(matches!(
            res,
            Err(crate::error::DdsError::PreconditionNotMet { .. })
        ));
    }

    #[test]
    fn subscriber_begin_access_is_nestable() {
        // Spec §2.2.2.5.2.8 — nesting allowed; each begin needs its own
        // end.
        let s = Subscriber::new(SubscriberQos::default(), None);
        s.begin_access();
        s.begin_access();
        assert!(s.is_access_open());
        s.end_access().unwrap();
        // Still open after the first end (recursive nesting).
        assert!(s.is_access_open());
        s.end_access().unwrap();
        // Only after the second end is the scope closed again.
        assert!(!s.is_access_open());
    }

    #[test]
    fn subscriber_too_many_ends_after_balanced_returns_error() {
        // Negative: after a balanced begin/end, the next end is an
        // underflow → PreconditionNotMet.
        let s = Subscriber::new(SubscriberQos::default(), None);
        s.begin_access();
        s.end_access().unwrap();
        let res = s.end_access();
        assert!(matches!(
            res,
            Err(crate::error::DdsError::PreconditionNotMet { .. })
        ));
    }
}
