// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Subscriber + DataReader — das Empfangs-Ende der DCPS-API.
//!
//! Spec-Referenz: OMG DDS 1.4 §2.2.2.5 `Subscriber`, §2.2.2.5.2
//! `DataReader`.
//!
//! # Scope v1.2
//!
//! - `Subscriber::create_datareader<T>(topic, qos)` → `DataReader<T>`.
//! - `DataReader::take()` entnimmt alle zwischengespeicherten Samples.
//! - `DataReader::read()` peekt ohne zu entfernen (Offline: identisch
//!   zu take, kein Statement-Wechsel — Spec §2.2.2.5.3.4 sample-state
//!   wird in Live-Mode implementiert).
//! - Listener / WaitSet: Live-Mode.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::ToString;
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

/// Subscriber — Entity-Gruppe fuer DataReader.
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
    /// optionaler `SubscriberListener` + StatusMask.
    /// Bubble-Up-Target fuer Reader-Events.
    #[cfg(feature = "std")]
    pub(crate) listener: std::sync::Mutex<Option<(ArcSubscriberListener, StatusMask)>>,
    /// Schwacher Back-Pointer auf den Participant (Bubble-Up,
    /// Cycle-Vermeidung via Weak).
    #[cfg(feature = "std")]
    pub(crate) participant:
        std::sync::Mutex<Option<alloc::sync::Weak<crate::participant::ParticipantInner>>>,
    /// Group-Access-Scope fuer §2.2.2.5.2.8/.9 begin/end_access.
    /// Counter-basiert (rekursiv nestable per Spec).
    pub(crate) access_scope: Arc<crate::coherent_set::GroupAccessScope>,
    /// DataReader-Handles (per `create_datareader` getrackt) fuer
    /// rekursives `DomainParticipant::contains_entity`
    /// (Spec §2.2.2.2.1.10).
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

    /// Spec §2.2.2.2.1.10 — `true` wenn `handle` ein DataReader ist,
    /// der ueber diesen Subscriber erzeugt wurde.
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
        // Propagiere zum Participant fuer rekursives contains_entity.
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

    /// Spec §2.2.2.5.2.8 `begin_access` — markiert den Beginn eines
    /// kohaerenten Read-Sets. Verschachtelung ist erlaubt; jeder
    /// Aufruf erhoeht einen internen Counter, jedes `end_access`
    /// erniedrigt ihn.
    pub fn begin_access(&self) {
        self.inner.access_scope.begin();
    }

    /// Spec §2.2.2.5.2.9 `end_access` — Gegenstueck zu `begin_access`.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` wenn `end_access` ohne
    /// vorhergehendes `begin_access` gerufen wird.
    pub fn end_access(&self) -> Result<()> {
        self.inner.access_scope.end()
    }

    /// `true` wenn aktuell ein Group-Access offen ist.
    #[must_use]
    pub fn is_access_open(&self) -> bool {
        self.inner.access_scope.is_active()
    }

    /// setzt den `SubscriberListener` + StatusMask. `None`
    /// loescht den Slot. Spec §2.2.2.5.6.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcSubscriberListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
    }

    /// aktueller Listener-Klon.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcSubscriberListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Setzt den schwachen Back-Pointer auf den Participant.
    #[cfg(feature = "std")]
    pub(crate) fn attach_participant(
        &self,
        participant: alloc::sync::Weak<crate::participant::ParticipantInner>,
    ) {
        if let Ok(mut slot) = self.inner.participant.lock() {
            *slot = Some(participant);
        }
    }

    /// Snapshot der Reader-Bubble-Up-Kette: gegebenes
    /// `reader_listener`-Tupel + Subscriber-Stage + Participant-Stage.
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

    /// Erzeugt einen typed `DataReader<T>`.
    ///
    /// # Errors
    /// `BadParameter` bei Type-Name-Mismatch.
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
            let (eid, rx) = rt.register_user_reader(crate::runtime::UserReaderConfig {
                topic_name: topic.name().into(),
                type_name: T::TYPE_NAME.into(),
                reliable,
                durability: qos.durability.kind,
                deadline: qos.deadline,
                liveliness: qos.liveliness,
                ownership: qos.ownership.kind,
                partition: qos.partition.names.clone(),
                user_data: qos.user_data.value.clone(),
                topic_data: qos.topic_data.value.clone(),
                group_data: qos.group_data.value.clone(),
                // F-TYPES-3: Topic-Type-Identifier + TCE-QoS weitergeben.
                type_identifier: T::TYPE_IDENTIFIER.clone(),
                type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
                // D.5g — Per-Reader-Override TBD (DataReaderQos::
                // representation noch nicht modelliert). Default
                // `None` = Runtime-Default.
                data_representation_offer: None,
            })?;
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
        // SubscriberQos: Partition / GroupData / Presentation sind alle
        // Changeable=YES per Spec §2.2.3 — kein Immutable-Check nötig.
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

/// Typed DataReader — entnimmt Samples, die der RTPS-Reader fuer
/// das Topic empfangen hat.
///
/// Live-Mode: `rx: Some` liefert Samples aus der Runtime-mpsc.
/// Offline-Mode: in-memory `inbox` fuer Unit-Tests.
pub struct DataReader<T: DdsType> {
    topic: Topic<T>,
    qos: Mutex<DataReaderQos>,
    /// Entity-Lifecycle (DCPS §2.2.2.1).
    entity_state: Arc<crate::entity::EntityState>,
    /// Parent-Subscriber — fuer Bubble-Up zum Subscriber- und
    /// Participant-Listener.
    subscriber: Arc<SubscriberInner>,
    /// optionaler `DataReaderListener` + StatusMask.
    #[cfg(feature = "std")]
    listener: Mutex<Option<(ArcDataReaderListener, StatusMask)>>,
    /// zuletzt gesehene Anzahl matched Writer (fuer
    /// Delta-Detection im poll_subscription_matched).
    #[cfg(feature = "std")]
    last_match_count: std::sync::atomic::AtomicI64,
    /// zuletzt gesehener requested_deadline_missed-Counter.
    #[cfg(feature = "std")]
    last_requested_deadline_missed: std::sync::atomic::AtomicU64,
    /// zuletzt gesehener (alive_count, not_alive_count).
    #[cfg(feature = "std")]
    last_liveliness_alive: std::sync::atomic::AtomicI64,
    /// zuletzt gesehener not_alive-Counter.
    #[cfg(feature = "std")]
    last_liveliness_not_alive: std::sync::atomic::AtomicI64,
    /// zuletzt gesehener requested_incompatible_qos.total_count.
    #[cfg(feature = "std")]
    last_requested_incompatible_qos: std::sync::atomic::AtomicI64,
    /// zuletzt gesehener sample_lost-Counter.
    #[cfg(feature = "std")]
    last_sample_lost: std::sync::atomic::AtomicU64,
    /// zuletzt gesehener sample_rejected.total_count.
    #[cfg(feature = "std")]
    last_sample_rejected: std::sync::atomic::AtomicI64,
    /// Offline-Fallback-Inbox. Speichert volle [`UserSample`]-Werte
    /// (inkl. writer_guid + writer_strength bei Alive), damit
    /// `take()`/`read()` den Exclusive-Ownership-Filter spec-konform
    /// anwenden koennen.
    inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    runtime: Option<Arc<DcpsRuntime>>,
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    entity_id: Option<EntityId>,
    /// Runtime-Channel fuer ankommende Samples (Live-Mode).
    #[cfg(feature = "std")]
    rx: Option<Mutex<mpsc::Receiver<crate::runtime::UserSample>>>,
    /// Optional Content-Filter-Closure. Wird in `take()`
    /// nach dem Decode auf jedes Sample angewendet; liefert `true` →
    /// Sample wird ausgeliefert, `false` → verworfen.
    ///
    /// Spec-Bezug: OMG DDS 1.4 §2.2.2.5.4 `ContentFilteredTopic`.
    /// Diese Rust-Closure-Variante ist idiomatischer als die SQL-
    /// Expression-Syntax der Spec und reicht fuer alle In-Process
    /// Use-Cases. SQL-Parser + Cross-Vendor-SEDP-Propagation kommen
    /// mit .
    #[allow(clippy::type_complexity)]
    filter: Option<Arc<dyn Fn(&T) -> bool + Send + Sync>>,
    ///  Instanz-Buchhaltung (Spec §2.2.2.5.1).
    #[cfg(feature = "std")]
    instances: InstanceTracker,
    ///  Sample-Cache mit aufgeloester [`SampleInfo`]. Der Cache
    /// wird beim Eingang via `ingest_bytes` befuellt; `take`/`read`/
    /// `take_with_info`/`read_with_info` lesen daraus.
    #[cfg(feature = "std")]
    cache: Arc<Mutex<Vec<CachedSample>>>,
    /// Optional konfigurierter Flatdata-SlotBackend fuer den Same-Host-
    /// Zero-Copy-Lese-Pfad (`zerodds-flatdata-1.0` §4.1 + §9.1). Wird
    /// via `set_flat_backend` gesetzt; `read_flat()` faellt auf
    /// klassisches `take()` zurueck wenn `None`.
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

/// Intern: ein dekodierter Sample im Reader-Cache.
///
/// Wir tragen die Bytes (statt `T`), damit der Reader-Cache nicht an
/// `T: Clone` gebunden ist und damit `T::decode` lazy passieren kann.
/// Lifecycle-Marker (Dispose/Unregister) haben `bytes == None`.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct CachedSample {
    pub bytes: Option<Vec<u8>>,
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

    /// Konstruktor fuer Builtin-Topic-Reader.
    ///
    /// Anders als `new_offline` teilt sich dieser Reader die Inbox mit
    /// dem `DcpsRuntime`-Discovery-Hook: SPDP-/SEDP-Receive pusht
    /// ueber denselben `Arc<Mutex<Vec<crate::runtime::UserSample>>>` ein encoded Sample,
    /// das hier per `take()`/`read()` ausgelesen wird.
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

    /// Setzt einen Content-Filter, der auf jedem Sample im `take()`-
    /// Pfad evaluiert wird. Rueckgabe `false` verwirft das Sample.
    ///
    /// Builder-Stil: `reader.with_filter(|s| s.value > 0)`.
    ///
    /// .7a — SQL-Expression-Syntax via `set_filter_expression`
    /// folgt in .
    #[must_use]
    pub fn with_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        self.filter = Some(Arc::new(filter));
        self
    }

    /// Topic, von dem gelesen wird.
    #[must_use]
    pub fn topic(&self) -> &Topic<T> {
        &self.topic
    }

    /// Spec §2.2.2.5.3.6 / §2.2.2.1.1 — `InstanceHandle` dieses
    /// DataReaders. Stabile Identitaet fuer
    /// `DomainParticipant::contains_entity`.
    #[must_use]
    pub fn subscription_handle(&self) -> crate::instance_handle::InstanceHandle {
        self.entity_state.instance_handle()
    }

    /// setzt den `DataReaderListener` + StatusMask. `None`
    /// loescht den Slot. Spec §2.2.2.5.7.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcDataReaderListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.entity_state.set_listener_mask(mask);
    }

    /// aktueller Listener-Klon, sofern vorhanden.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDataReaderListener> {
        self.listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Snapshot der Bubble-Up-Kette (Reader → Subscriber → Participant)
    /// fuer Hot-Path-Listener-Dispatch.
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

    /// Aktuelle QoS (cloned, .1).
    #[must_use]
    pub fn qos(&self) -> DataReaderQos {
        self.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Nimmt alle zwischengespeicherten Samples und entfernt sie aus
    /// der Inbox. Liefert leeren Vec wenn nichts da ist.
    ///
    /// # Errors
    /// - `WireError` wenn ein gespeicherter Payload sich nicht mehr
    ///   decoden laesst (type-eval mismatch).
    pub fn take(&self) -> Result<Vec<T>> {
        // Spec §2.2.3.22 ReaderDataLifecycle.autopurge — bei jedem read/take
        // pruefen, ob abgelaufene Instanzen aus dem Tracker zu entfernen sind.
        #[cfg(feature = "std")]
        {
            let now = get_current_time();
            let mut empty: Vec<CachedSample> = Vec::new();
            self.run_reader_autopurge(now, &mut empty);
        }
        // Live-Mode: zuerst Staging-Inbox (gefuellt von wait_for_data)
        // drainen, dann alle noch unpollten Samples aus mpsc ziehen.
        #[cfg(feature = "std")]
        if let Some(rx_mu) = self.rx.as_ref() {
            let mut out = Vec::new();
            // TimeBasedFilter (Spec §2.2.3.13) min_separation aus QoS lesen,
            // damit Live-Mode dieselbe Filterung wie ingest_into_cache anwendet.
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
                    } => {
                        let sample = T::decode(&bytes).map_err(|e| DdsError::WireError {
                            message: e.to_string(),
                        })?;
                        if !self.sample_passes_filter(&sample) {
                            continue;
                        }
                        if !self.live_mode_time_based_filter_pass(&sample, min_sep_nanos) {
                            continue;
                        }
                        // §2.2.3.23 Exclusive-Ownership-Filter.
                        if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                            continue;
                        }
                        out.push(sample);
                    }
                    crate::runtime::UserSample::Lifecycle { .. } => {
                        // Lifecycle in der Staging-Inbox: in der
                        // Live-Mode-take()-Schleife wird sie sofort
                        // unten via __push_lifecycle behandelt — hier
                        // einfach uebergehen; sie kommt naechste Runde.
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
                    } => {
                        let sample = T::decode(&bytes).map_err(|e| DdsError::WireError {
                            message: e.to_string(),
                        })?;
                        if !self.sample_passes_filter(&sample) {
                            continue;
                        }
                        if !self.live_mode_time_based_filter_pass(&sample, min_sep_nanos) {
                            continue;
                        }
                        // §2.2.3.23 Exclusive-Ownership-Filter.
                        if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                            continue;
                        }
                        out.push(sample);
                    }
                    crate::runtime::UserSample::Lifecycle { key_hash, kind } => {
                        // Lifecycle-Marker via __push_lifecycle in den
                        // Tracker fuettern (Spec §8.2.1.2).
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
        // Offline-Fallback.
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
            } = staged_item
            else {
                continue;
            };
            let sample = T::decode(&bytes).map_err(|e| DdsError::WireError {
                message: e.to_string(),
            })?;
            if !self.sample_passes_filter(&sample) {
                continue;
            }
            // §2.2.3.23 Exclusive-Ownership-Filter (auch im Offline-
            // Fallback). Builtin-Inject-Pfad nutzt writer_guid=[0;16]
            // mit Shared-Ownership-Default; passes_exclusive_ownership
            // returnt dann immer `true`.
            if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                continue;
            }
            out.push(sample);
        }
        Ok(out)
    }

    /// Hilfsfunktion — evaluiert den Content-Filter wenn gesetzt.
    fn sample_passes_filter(&self, sample: &T) -> bool {
        match &self.filter {
            Some(f) => f(sample),
            None => true,
        }
    }

    /// Spec §2.2.3.23 / §2.2.2.5.5 — Exclusive-Ownership-Filter.
    ///
    /// Gibt `true` zurueck wenn das Sample geliefert werden darf:
    /// - Reader-Ownership-QoS = Shared → immer `true` (kein Filter).
    /// - Keyless Topic → immer `true` (keine Per-Instance-Owner-State).
    /// - Sonst: berechnet KeyHash und konsultiert
    ///   [`instance_tracker::InstanceTracker::should_accept_sample_under_exclusive_ownership`]
    ///   das pro Instanz den (writer_guid, writer_strength) der bisher
    ///   gewinnenden Source haelt und Samples schwaecherer Writer rejectet.
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
        // Spec §2.2.3.23: Ownership-Resolution greift per-Instanz; bei
        // keyless Topics behandeln wir das Topic als einzige Instanz mit
        // synthetischem all-zero KeyHash.
        let (kh, key_bytes) = if T::HAS_KEY {
            let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
            sample.encode_key_holder_be(&mut holder);
            let kb = holder.as_bytes().to_vec();
            let max = T::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
            (crate::dds_type::compute_key_hash(&kb, max), kb)
        } else {
            ([0u8; 16], Vec::new())
        };
        // Instance muss registriert sein, damit der Owner-Tracker den
        // Slot anlegen kann (`should_accept` returnt sonst `true` bei
        // unbekannter Instance, was die Filterung umgeht).
        let _ = self.instances.observe_sample(kh, key_bytes, None);
        self.instances
            .should_accept_sample_under_exclusive_ownership(&kh, writer_guid, writer_strength)
    }

    /// Spec §2.2.3.13 TIME_BASED_FILTER fuer den Live-Mode-Pfad.
    /// Gibt `true` zurueck, wenn das Sample geliefert werden darf.
    /// Bei keyless Types oder min_separation=0 immer `true`.
    /// Bei keyed Types: keyhash via `encode_key_holder_be` berechnen,
    /// gegen instance_tracker pruefen, und bei `true` direkt
    /// `record_delivery` aufrufen, damit nachfolgende Samples derselben
    /// Instanz richtig gefiltert werden.
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

    /// Liest alle Samples ohne sie zu entfernen. aktuell identisch
    /// zu `take` minus entfernen. Sample-State (`ReadCondition`
    /// §2.2.2.5.8) folgt im Wire-Up.
    ///
    /// # Errors
    /// Wie `take`.
    pub fn read(&self) -> Result<Vec<T>> {
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
            } = staged_item
            else {
                continue;
            };
            let sample = T::decode(&bytes).map_err(|e| DdsError::WireError {
                message: e.to_string(),
            })?;
            if !self.sample_passes_filter(&sample) {
                continue;
            }
            // §2.2.3.23 Exclusive-Ownership-Filter (auch im Offline-
            // Fallback). Builtin-Inject-Pfad nutzt writer_guid=[0;16]
            // mit Shared-Ownership-Default; passes_exclusive_ownership
            // returnt dann immer `true`.
            if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                continue;
            }
            out.push(sample);
        }
        Ok(out)
    }

    /// Anzahl matched Remote-Writer. Im Offline-Mode immer 0.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.5.3.15 `get_matched_publications`.
    ///
    /// Seiteneffekt — bei einer Aenderung des Matched-Count
    /// gegenueber dem letzten Aufruf wird `on_subscription_matched`
    /// via Bubble-Up-Kette gefeuert (Spec §2.2.4.2.6.7).
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

    /// Delta-Detect-Helper fuer `on_subscription_matched`.
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

    /// Delta-Detect fuer `on_requested_deadline_missed`.
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

    /// Delta-Detect fuer `on_liveliness_changed`. Spec
    /// §2.2.4.2.6.6. Beachtet beide Counter (alive + not_alive); jeder
    /// Wechsel triggert genau einmal.
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
        // Erste Beobachtung (prev == -1) zaehlt nur wenn der Counter
        // ungleich 0 ist; sonst kein triggern.
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

    /// Delta-Detect fuer `on_requested_incompatible_qos`.
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

    /// Delta-Detect fuer `on_sample_lost`. Spec §2.2.4.2.6.2.
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

    /// Delta-Detect fuer `on_sample_rejected`. Spec §2.2.4.2.6.3.
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

    /// Blockiert, bis mindestens `min_count` Remote-Writer matched
    /// sind oder `timeout` verstreicht. Event-driven via Runtime-Condvar
    /// (D.5e Phase-1) — wakup direkt wenn SEDP einen Match propagiert,
    /// kein 20-ms-Polling mehr.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] wenn `min_count` im Zeitfenster nicht
    /// erreicht wird.
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
            // Live-Mode: park auf Runtime-match-event. Spurious wake-ups
            // sind fine — wir checken den count auf naechster iteration.
            if let Some(rt) = self.runtime.as_ref() {
                let _ = rt.wait_match_event(deadline - now);
            } else {
                // Offline-Mode: keine Match-Events, sleep-fallback.
                std::thread::sleep(core::time::Duration::from_millis(20));
            }
        }
    }

    /// Counter fuer requested-Deadline-Verletzungen (Spec
    /// §2.2.4.2.11 `REQUESTED_DEADLINE_MISSED_STATUS`). Monoton steigend;
    /// steigt um 1 pro abgelaufenem Deadline-Fenster ohne empfangenes
    /// Sample. Offline / INFINITE → 0.
    ///
    /// feuert ggf. `on_requested_deadline_missed`.
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

    /// aktueller `RequestedIncompatibleQosStatus`. Spec
    /// §2.2.4.2.6.5. Triggert ggf. `on_requested_incompatible_qos`.
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

    /// SampleLost-Counter. Spec §2.2.4.2.6.2.
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

    /// SampleRejected-Status. Spec §2.2.4.2.6.3.
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

    /// pollt alle Reader-Statuses einmal und feuert pending
    /// Listener. Convenience-Helper fuer Tests + periodische Tick-Aufrufer.
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

    /// Liveliness-Status des matched Writers (Spec §2.2.4.2.14
    /// `LIVELINESS_CHANGED_STATUS`): `(alive, alive_count, not_alive_count)`.
    ///
    /// * `alive`: aktueller Zustand (true = Writer hat Sample innerhalb
    ///   seiner Lease-Duration geliefert).
    /// * `alive_count`: Zaehler der "not_alive → alive"-Transitions.
    /// * `not_alive_count`: Zaehler der "alive → not_alive"-Transitions.
    ///
    /// Offline / INFINITE-Lease → `(false, 0, 0)` / `(true, 0, 0)` je
    /// nach Init. Fuer v1.3 wird nur `LivelinessKind::Automatic` ueberwacht.
    #[must_use]
    pub fn liveliness_changed_status(&self) -> (bool, u64, u64) {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let triple = rt.user_reader_liveliness_status(eid);
            // Listener-Trigger via Delta-Detection.
            self.poll_liveliness_changed(triple.1, triple.2);
            return triple;
        }
        (false, 0, 0)
    }

    /// Blockiert, bis mindestens ein Sample verfuegbar ist oder der
    /// Timeout abgelaufen ist. Das Sample wird dabei nicht entnommen —
    /// es wird in einen Staging-Buffer gelegt, den der naechste `take()`
    /// ausliest. Damit bleibt `wait_for_data` + `take()` der kanonische
    /// Subscriber-Loop, statt busy-polling im Application-Code.
    ///
    /// Spec-Analog: OMG DDS 1.4 §2.2.2.5.8 `ReadCondition` + `WaitSet`.
    /// Diese API liefert die wichtigste Semantik (wake-on-data) ohne die
    /// komplette WaitSet/Condition-Infrastruktur.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] wenn im Zeitfenster nichts ankommt.
    #[cfg(feature = "std")]
    pub fn wait_for_data(&self, timeout: core::time::Duration) -> Result<()> {
        let Some(rx_mu) = self.rx.as_ref() else {
            // Offline-Mode: wenn inbox schon was hat, OK, sonst Timeout.
            let inbox_has = self.inbox.lock().map(|i| !i.is_empty()).unwrap_or(false);
            if inbox_has {
                return Ok(());
            }
            return Err(DdsError::Timeout);
        };

        // Schon was in der Staging-Inbox?
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
        // Lock zuerst freigeben, dann Listener feuern (
        // Lock-Discipline).
        drop(rx);
        if result.is_ok() {
            self.notify_data_arrived();
        }
        result
    }

    /// Builtin-Topic-Helper: gibt den Arc auf die geteilte Inbox
    /// zurueck (Reader-Klone teilen sich denselben Buffer).
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn __inbox_handle(&self) -> Arc<Mutex<Vec<crate::runtime::UserSample>>> {
        Arc::clone(&self.inbox)
    }

    /// Test-Helper: fuegt einen encoded Payload in die Inbox ein.
    /// In Runtime wird das durch den ReliableReader-Delivery-Pfad
    /// ersetzt.
    ///
    /// triggert die Listener-Bubble-Up-Kette
    /// `on_data_on_readers` (Subscriber-Stage) und `on_data_available`
    /// (Reader-Stage). Spec §2.2.4.2.7.1 / §2.2.4.2.6.1.
    #[doc(hidden)]
    pub fn __push_raw(&self, bytes: Vec<u8>) -> Result<()> {
        self.__push_raw_with_writer(bytes, [0u8; 16], 0)
    }

    /// Test-Hook: pusht ein Sample mit explizitem Writer-GUID und
    /// `ownership_strength` in die Inbox. Wird vom Cyclone-Interop-
    /// Harness und den Exclusive-Ownership-Tests benutzt.
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
                payload: bytes,
                writer_guid,
                writer_strength,
            });
        }
        // Listener-Notify ausserhalb des Inbox-Locks, um Re-Entrancy
        // zu vermeiden.
        self.notify_data_arrived();
        Ok(())
    }

    /// ruft die `on_data_on_readers`- und
    /// `on_data_available`-Bubble-Up-Pfade. Spec §2.2.4.1: pro
    /// neuem Sample wird `data_on_readers` (Subscriber-Level) und
    /// `data_available` (Reader-Level) als unabhaengige Statuses
    /// gesetzt; wenn der Subscriber `data_on_readers` konsumiert
    /// hat, soll `data_available` *nicht* unterdrueckt werden — die
    /// beiden Status sind getrennte Bits in der Mask.
    #[cfg(feature = "std")]
    pub(crate) fn notify_data_arrived(&self) {
        let chain = self.listener_chain();
        let reader_handle = self.entity_state.instance_handle();
        crate::listener_dispatch::dispatch_data_on_readers(&chain, reader_handle);
        crate::listener_dispatch::dispatch_data_available(&chain, reader_handle);
    }

    // ========================================================================
    // SampleInfo-Statechart + Instance-Lifecycle.
    // Spec §2.2.2.5.1, §2.2.2.5.3.{5,27,28}.
    // ========================================================================

    /// Liefert den aktuellen [`InstanceTracker`] (geteilt mit der
    /// internen Buchhaltung). Hauptsaechlich fuer Tests / Inspection.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn instance_tracker(&self) -> InstanceTracker {
        self.instances.clone()
    }

    /// Liefert (Runtime, EntityId), wenn der Reader im Live-Mode laeuft.
    /// Cross-Crate-Hook fuer Async-Layer (dcps-async), der den Waker-
    /// Slot direkt registrieren muss.
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

    /// Spec §2.2.3.23 — Hook fuer "Writer X hat Liveliness verloren".
    /// Macht zwei Dinge:
    ///   1. clear OWNERSHIP=EXCLUSIVE-Owner fuer alle Instanzen, deren
    ///      Owner dieser Writer war (so dass der naechste Sample eines
    ///      anderen Writers via `should_accept_sample_under_exclusive_ownership`
    ///      neu gewinnen kann);
    ///   2. liefert die Anzahl betroffener Instanzen zurueck.
    ///
    /// Wird aus dem WLP-Pfad gerufen, sobald ein Writer-Lease abgelaufen
    /// ist (siehe `wlp::WlpEndpoint::lost_peers`).
    #[must_use]
    pub fn notify_writer_liveliness_lost(&self, writer_guid: [u8; 16]) -> usize {
        self.instances.clear_owner_for_writer(writer_guid)
    }

    /// Wie [`Self::notify_writer_liveliness_lost`], aber Match nur ueber
    /// die ersten 12 Bytes (GuidPrefix). Erlaubt Failover, wenn nur die
    /// Participant-Identitaet (z.B. bei SPDP-Lease-Expiry) bekannt ist.
    #[must_use]
    pub fn notify_participant_liveliness_lost(&self, prefix: [u8; 12]) -> usize {
        self.instances.clear_owner_for_writer_prefix(prefix)
    }

    /// Macht aus einem Sample-Wert den dazugehoerigen lokalen
    /// [`InstanceHandle`], oder [`HANDLE_NIL`] wenn unbekannt /
    /// non-keyed. Spec §2.2.2.5.3.26 `lookup_instance` (Reader-Variante).
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

    /// Spec §2.2.2.5.3.25 `get_key_value`. Liefert den Sample-Wert mit
    /// nur den `@key`-Feldern befuellt (rekonstruiert aus dem
    /// gespeicherten Key-Holder via `T::decode`).
    ///
    /// # Errors
    /// `BadParameter` wenn `handle` unbekannt; `WireError` wenn
    /// `T::decode` den Key-Stream nicht rekonstruieren kann.
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

    /// Drainiert alle pending Bytes aus rx + inbox in den internen
    /// Sample-Cache. Dabei wird pro Sample der KeyHash berechnet, die
    /// Instanz registriert (falls neu) und ein passendes [`SampleInfo`]
    /// erzeugt.
    ///
    /// Wird automatisch von den `*_with_info`/`*_instance`-APIs
    /// aufgerufen.
    #[cfg(feature = "std")]
    fn ingest_into_cache(&self) -> Result<()> {
        // Schritt 1: alle eingehenden Samples einsammeln. `raw` traegt
        // (bytes, writer_guid, writer_strength) damit der Exclusive-
        // Ownership-Filter (DDS 1.4 §2.2.3.23) anwendbar ist.
        let mut raw: Vec<(Vec<u8>, [u8; 16], i32)> = Vec::new();
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
                } = item
                {
                    raw.push((payload, writer_guid, writer_strength));
                }
            }
        }
        // Live-Mode-Channel: Alive-Samples in `raw` einreihen,
        // Lifecycle-Marker direkt via __push_lifecycle behandeln.
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
                    } => raw.push((bytes, writer_guid, writer_strength)),
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
        // Lifecycle-Marker erst NACH Drain anwenden, damit der Lock-Pfad
        // sauber bleibt (__push_lifecycle nimmt eigene Locks).
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
            // Auch ohne neue Bytes muss autopurge laufen, sonst verfallen
            // disposed/nowriter-Instanzen nie ausserhalb von Sample-Zufluss.
            self.run_reader_autopurge(now, &mut cache);
            return Ok(());
        }
        for (bytes, writer_guid, writer_strength) in raw {
            // Decode T um (a) den Filter zu evaluieren und (b) den
            // KeyHash zu berechnen.
            let sample = T::decode(&bytes).map_err(|e| DdsError::WireError {
                message: alloc::string::ToString::to_string(&e),
            })?;
            if !self.sample_passes_filter(&sample) {
                continue;
            }
            // §2.2.3.23 Exclusive-Ownership-Filter: rejecte Samples
            // schwaecherer Writer bevor sie in den Cache wandern.
            if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                continue;
            }
            let info = if T::HAS_KEY {
                let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
                sample.encode_key_holder_be(&mut holder);
                let key_bytes = holder.as_bytes().to_vec();
                let max = T::KEY_HOLDER_MAX_SIZE.unwrap_or(usize::MAX);
                let kh = crate::dds_type::compute_key_hash(&key_bytes, max);
                // QoS-Filter VOR observe_sample, damit verworfene Samples
                // den Sample-Zustand nicht beeinflussen.
                let (min_sep_nanos, by_source_ts) = {
                    let qos = self.qos.lock().unwrap_or_else(|e| e.into_inner());
                    (
                        qos.time_based_filter.minimum_separation.to_nanos(),
                        qos.destination_order.kind
                            == zerodds_qos::DestinationOrderKind::BySourceTimestamp,
                    )
                };
                // Spec §2.2.3.13 TIME_BASED_FILTER: drop, wenn weniger als
                // minimum_separation seit dem letzten gelieferten Sample
                // dieser Instanz vergangen ist.
                if !self
                    .instances
                    .should_deliver_under_time_based_filter(&kh, now, min_sep_nanos)
                {
                    continue;
                }
                // Spec §2.2.3.18 DESTINATION_ORDER: bei BY_SOURCE_TIMESTAMP
                // nur Samples mit strikt groesserem source_ts liefern,
                // sonst out-of-order Resolution greift.
                if !self
                    .instances
                    .should_deliver_under_destination_order(&kh, now, by_source_ts)
                {
                    continue;
                }
                let (handle, _) = self.instances.observe_sample(kh, key_bytes, Some(now));
                self.instances.record_delivery(&kh, now);
                let state = match self.instances.get_by_handle(handle) {
                    Some(s) => s,
                    None => continue, // sollte nie passieren — defensiv
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
                    source_timestamp: now,
                    instance_handle: handle,
                    valid_data: true,
                    ..SampleInfo::default()
                }
            } else {
                // Non-keyed Topics: ein "Pseudo-Handle" pro Sample
                // waere overkill — wir lassen es bei HANDLE_NIL (Spec
                // §2.2.2.5.1.10 erlaubt das, weil die Instance-Sicht
                // fuer non-keyed Topics formal "alles eine Instanz" ist).
                SampleInfo {
                    sample_state: SampleStateKind::NotRead,
                    view_state: ViewStateKind::NotNew,
                    instance_handle: HANDLE_NIL,
                    source_timestamp: now,
                    valid_data: true,
                    ..SampleInfo::default()
                }
            };
            cache.push(CachedSample {
                bytes: Some(bytes),
                info,
            });
        }
        // Spec §2.2.3.22 ReaderDataLifecycle: Instanzen, die laenger als
        // autopurge_*_samples_delay in NotAlive-Disposed bzw. NotAlive-
        // NoWriters sind, aus dem Tracker und Cache entfernen.
        self.run_reader_autopurge(now, &mut cache);
        Ok(())
    }

    /// Wendet `ReaderDataLifecycle.autopurge_*` an: entfernt abgelaufene
    /// Instanzen aus Tracker + Cache. Aufgerufen von `ingest_into_cache`
    /// und beim Einlesen ohne neue Bytes.
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

    /// Push eines reinen Lifecycle-Markers (Dispose / Unregister)
    /// in den Cache. Wird von der Runtime aufgerufen, sobald ein Writer
    /// `dispose`/`unregister_instance` schickt.
    #[cfg(feature = "std")]
    #[doc(hidden)]
    pub fn __push_lifecycle(
        &self,
        keyhash: crate::instance_tracker::KeyHash,
        key_holder: Vec<u8>,
        kind: InstanceStateKind,
    ) -> Result<()> {
        let now = get_current_time();
        // Erst die Instanz im Tracker im richtigen Zustand bringen.
        // observe_sample registriert sie ggf. neu und macht sie alive.
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
            return Ok(()); // sollte nie passieren — defensiv
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

    /// `take` mit voller [`SampleInfo`]. Spec §2.2.2.5.3.5
    /// `take`. Konsumiert die Samples aus dem Cache (`NOT_READ → READ`-
    /// Transition entfaellt, weil sie weg sind).
    ///
    /// # Errors
    /// Wie [`Self::take`].
    #[cfg(feature = "std")]
    pub fn take_with_info(&self) -> Result<Vec<Sample<T>>> {
        self.take_filtered(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
    }

    /// `read` mit voller [`SampleInfo`]. Konsumiert nicht — markiert
    /// die Samples nur als `READ` (Spec §2.2.2.5.3.4).
    ///
    /// # Errors
    /// Wie [`Self::read`].
    #[cfg(feature = "std")]
    pub fn read_with_info(&self) -> Result<Vec<Sample<T>>> {
        self.read_filtered(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
    }

    /// `take` mit State-Masken (Spec §2.2.2.5.3.6 `take_w_condition`).
    ///
    /// # Errors
    /// Wie [`Self::take`].
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

    /// `read` mit State-Masken (Spec §2.2.2.5.3.3 `read_w_condition`).
    ///
    /// # Errors
    /// Wie [`Self::read`].
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
            // Snapshot bauen (mit aktueller Sample-State-Sicht).
            let snapshot = Sample::new(
                self.decode_or_keyholder(s.bytes.as_deref(), s.info.instance_handle)?,
                s.info,
            );
            // Sample-State Transition NOT_READ → READ (Spec §2.2.2.5.3.4).
            s.info.sample_state = SampleStateKind::Read;
            self.instances.mark_view_seen(s.info.instance_handle);
            out.push(snapshot);
        }
        Ok(out)
    }

    /// `read_w_condition` (Spec §2.2.2.5.3.7) — wendet zusaetzlich zur
    /// State-Mask den SQL-Filter der QueryCondition pro Sample an.
    /// Samples bleiben im Cache (Sample-State NOT_READ → READ).
    ///
    /// # Errors
    /// `PreconditionNotMet` bei Lock-Poisoning oder SQL-Eval-Fehler.
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
            // Filter-Eval-Fehler -> Sample wird abgelehnt (Spec: "filter
            // expression false" Semantik), aber wir propagieren keinen
            // harten Error nach oben, ausser Lock-Poisoning.
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

    /// `take_w_condition` (Spec §2.2.2.5.3.8) — wie `read_w_condition`,
    /// aber konsumiert die Samples (entfernt aus dem Cache).
    ///
    /// # Errors
    /// `PreconditionNotMet` bei Lock-Poisoning oder SQL-Eval-Fehler.
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

    /// `read_instance` (Spec §2.2.2.5.3.27). Liefert nur Samples der
    /// angegebenen Instanz.
    ///
    /// # Errors
    /// `BadParameter` wenn `handle == HANDLE_NIL`.
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

    /// `take_instance` (Spec §2.2.2.5.3.27, Take-Variante). Konsumiert.
    ///
    /// # Errors
    /// `BadParameter` wenn `handle == HANDLE_NIL`.
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

    /// `read_next_instance` (Spec §2.2.2.5.3.28). Liefert die Samples
    /// der **naechsten** Instanz (nach Sortier-Ordnung) hinter
    /// `previous`.
    ///
    /// `previous == HANDLE_NIL` startet beim ersten Handle.
    ///
    /// # Errors
    /// Wie `read`.
    #[cfg(feature = "std")]
    pub fn read_next_instance(&self, previous: InstanceHandle) -> Result<Vec<Sample<T>>> {
        let Some(next) = self.instances.next_handle_after(previous) else {
            return Ok(Vec::new());
        };
        self.read_instance(next)
    }

    /// `take_next_instance` (Spec §2.2.2.5.3.28). Take-Variante.
    ///
    /// # Errors
    /// Wie `take`.
    #[cfg(feature = "std")]
    pub fn take_next_instance(&self, previous: InstanceHandle) -> Result<Vec<Sample<T>>> {
        let Some(next) = self.instances.next_handle_after(previous) else {
            return Ok(Vec::new());
        };
        self.take_instance(next)
    }

    /// Hilfsfunktion: aus einem CachedSample ein `Sample<T>` machen.
    /// Bei Lifecycle-Markern (`bytes == None`) wird `T` aus dem
    /// gespeicherten Key-Holder rekonstruiert (Spec §2.2.2.5.1.13:
    /// `data` enthaelt dann nur den Key-Anteil).
    #[cfg(feature = "std")]
    fn materialize(&self, s: CachedSample) -> Result<Sample<T>> {
        let data = self.decode_or_keyholder(s.bytes.as_deref(), s.info.instance_handle)?;
        #[cfg(feature = "metrics")]
        crate::metrics::add_samples_read(self.topic.name(), 1);
        Ok(Sample::new(data, s.info))
    }

    /// Decode-Helper: bei `Some(bytes)` via `T::decode`, bei `None`
    /// (Lifecycle-Marker) ueber den Key-Holder der Instanz; falls
    /// auch der nicht verfuegbar, faellt zurueck auf `T::decode(&[])`.
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
    /// RESOURCE_LIMITS, OWNERSHIP sind Changeable=NO post-enable.
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

// ---- Boxed-typemapped variant fuer heterogene Reader-Listen ----
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
        // Inbox ist jetzt leer.
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

    // poll_subscription_matched + Listener-Slot-API.

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
        // sub_matched-Counter unveraendert (anderer Status-Bit).
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
        // Spec §2.2.2.5.2.9 — end ohne begin ist Spec-Verletzung.
        let s = Subscriber::new(SubscriberQos::default(), None);
        let res = s.end_access();
        assert!(matches!(
            res,
            Err(crate::error::DdsError::PreconditionNotMet { .. })
        ));
    }

    #[test]
    fn subscriber_begin_access_is_nestable() {
        // Spec §2.2.2.5.2.8 — Verschachtelung erlaubt; jedes
        // begin braucht ein eigenes end.
        let s = Subscriber::new(SubscriberQos::default(), None);
        s.begin_access();
        s.begin_access();
        assert!(s.is_access_open());
        s.end_access().unwrap();
        // Nach erstem end noch offen (rekursive Verschachtelung).
        assert!(s.is_access_open());
        s.end_access().unwrap();
        // Erst nach zweitem end ist der Scope wieder zu.
        assert!(!s.is_access_open());
    }

    #[test]
    fn subscriber_too_many_ends_after_balanced_returns_error() {
        // Negativ: nach balanciertem begin/end ist der naechste end
        // ein Underflow → PreconditionNotMet.
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
